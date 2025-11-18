# dduper Architecture Diagram

## Table of Contents
1. [System Overview](#system-overview)
2. [Component Architecture](#component-architecture)
3. [Data Flow Diagrams](#data-flow-diagrams)
4. [Process Workflows](#process-workflows)
5. [Database Schema](#database-schema)
6. [Deployment Architecture](#deployment-architecture)

---

## 1. System Overview

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           dduper System                                  │
│                                                                           │
│  ┌─────────────┐      ┌──────────────┐      ┌────────────────────┐     │
│  │   User CLI  │─────▶│  Main Controller  │─▶│  Dedupe Engine     │     │
│  │  Interface  │      │   (dduper)        │  │                    │     │
│  └─────────────┘      └──────────────────┘  └────────────────────┘     │
│         │                      │                       │                 │
│         │                      ▼                       ▼                 │
│         │             ┌──────────────────┐   ┌─────────────────┐        │
│         │             │  Argument Parser │   │ Hash Processor  │        │
│         │             │  & Validator     │   │  (SHA256+numpy) │        │
│         │             └──────────────────┘   └─────────────────┘        │
│         │                      │                       │                 │
│         │                      ▼                       ▼                 │
│         │             ┌──────────────────────────────────────┐          │
│         └────────────▶│     SQLite Database Manager          │          │
│                       │  - filehash table                    │          │
│                       │  - btrfscsum table                   │          │
│                       │  - Cache management                  │          │
│                       └──────────────────────────────────────┘          │
│                                    │                                     │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     │
                    ┌────────────────┼────────────────┐
                    │                │                │
                    ▼                ▼                ▼
        ┌─────────────────┐  ┌─────────────┐  ┌────────────────┐
        │ BTRFS Csum Tree │  │  Kernel API │  │ Validation Sys │
        │   (via patch)   │  │   (ioctl)   │  │   (SHA256)     │
        └─────────────────┘  └─────────────┘  └────────────────┘
                │                    │                  │
                ▼                    ▼                  ▼
        ┌─────────────────────────────────────────────────────┐
        │             Linux Kernel + BTRFS                     │
        │   ┌───────────────────────────────────────┐         │
        │   │  FIDEDUPERANGE | FICLONERANGE ioctls  │         │
        │   └───────────────────────────────────────┘         │
        └─────────────────────────────────────────────────────┘
                                 │
                                 ▼
                    ┌──────────────────────────┐
                    │   BTRFS Filesystem       │
                    │   on Block Device        │
                    └──────────────────────────┘
```

---

## 2. Component Architecture

### 2.1 Main Components Breakdown

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Main Controller (dduper)                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────┐     ┌───────────────────────┐            │
│  │  Argument Parser     │     │   Mode Controller     │            │
│  │  (argparse)          │────▶│                       │            │
│  │  - Device path       │     │  - Safe Mode          │            │
│  │  - Files/Directories │     │  - Fast Mode          │            │
│  │  - Options           │     │  - Insane Mode        │            │
│  │  - Flags             │     │  - Analysis Mode      │            │
│  └──────────────────────┘     └───────────────────────┘            │
│           │                              │                          │
│           └──────────────┬───────────────┘                          │
│                          ▼                                          │
│                 ┌─────────────────┐                                 │
│                 │  File Validator │                                 │
│                 │  - Check exists │                                 │
│                 │  - Check size   │                                 │
│                 │  - Check type   │                                 │
│                 │  - Check inode  │                                 │
│                 └─────────────────┘                                 │
│                          │                                          │
│                          ▼                                          │
│         ┌────────────────────────────────────┐                      │
│         │    Orchestration Layer             │                      │
│         │  - dedupe_files()                  │                      │
│         │  - dedupe_dir()                    │                      │
│         │  - main()                          │                      │
│         └────────────────────────────────────┘                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.2 BTRFS Checksum Interface

```
┌───────────────────────────────────────────────────────────────┐
│              BTRFS Checksum Retrieval System                  │
├───────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────────────────────┐                                 │
│  │  btrfs_dump_csum()       │                                 │
│  │                          │                                 │
│  │  1. Check SQLite cache   │                                 │
│  │  2. If miss, call btrfs  │                                 │
│  │  3. Store result         │                                 │
│  └────────┬─────────────────┘                                 │
│           │                                                    │
│           ├─[Cache Hit]────▶ Return cached data               │
│           │                                                    │
│           └─[Cache Miss]───┐                                  │
│                            ▼                                   │
│              ┌──────────────────────────────┐                 │
│              │  do_btrfs_dump_csum()        │                 │
│              │                              │                 │
│              │  Execute subprocess:         │                 │
│              │  btrfs inspect-internal \    │                 │
│              │    dump-csum <file> <dev>    │                 │
│              └──────────────────────────────┘                 │
│                            │                                   │
│                            ▼                                   │
│              ┌──────────────────────────────┐                 │
│              │  Patched btrfs.static        │                 │
│              │  /usr/sbin/btrfs.static      │                 │
│              │                              │                 │
│              │  Reads BTRFS csum-tree       │                 │
│              │  Returns checksum list       │                 │
│              └──────────────────────────────┘                 │
│                            │                                   │
│                            ▼                                   │
│              ┌──────────────────────────────┐                 │
│              │  Parse & Hash Output         │                 │
│              │  - SHA256(output)            │                 │
│              │  - Store in SQLite           │                 │
│              │  - Return checksum list      │                 │
│              └──────────────────────────────┘                 │
└───────────────────────────────────────────────────────────────┘
```

### 2.3 Deduplication Engine

```
┌────────────────────────────────────────────────────────────────────┐
│                    Deduplication Engine                            │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────┐      │
│  │  do_dedupe(src_file, dst_file, dry_run)                 │      │
│  └───────────────────────┬─────────────────────────────────┘      │
│                          │                                         │
│                          ▼                                         │
│           ┌──────────────────────────────┐                        │
│           │  Get Checksums               │                        │
│           │  - src_checksums             │                        │
│           │  - dst_checksums             │                        │
│           └──────────┬───────────────────┘                        │
│                      │                                             │
│                      ▼                                             │
│           ┌──────────────────────────────┐                        │
│           │  Compare Checksums           │                        │
│           │                              │                        │
│           │  if src == dst:              │                        │
│           │    Perfect Match             │                        │
│           │    Auto-adjust chunk size    │                        │
│           │  else:                       │                        │
│           │    Partial Match             │                        │
│           └──────────┬───────────────────┘                        │
│                      │                                             │
│                      ▼                                             │
│           ┌──────────────────────────────┐                        │
│           │  Hash Processing             │                        │
│           │  - get_hashes()              │                        │
│           │  - Group by chunks           │                        │
│           │  - SHA256 hash each chunk    │                        │
│           │  - Build OrderedDict         │                        │
│           └──────────┬───────────────────┘                        │
│                      │                                             │
│                      ▼                                             │
│           ┌──────────────────────────────┐                        │
│           │  Find Matching Regions       │                        │
│           │  - np.intersect1d()          │                        │
│           │  - np.setdiff1d()            │                        │
│           │  - Build offset lists        │                        │
│           └──────────┬───────────────────┘                        │
│                      │                                             │
│                      ▼                                             │
│           ┌──────────────────────────────┐                        │
│           │  Execute Deduplication       │                        │
│           │                              │                        │
│           │  For each matching region:   │                        │
│           │    - Calculate offsets       │                        │
│           │    - Call ioctl              │                        │
│           │    - Track bytes deduped     │                        │
│           └──────────┬───────────────────┘                        │
│                      │                                             │
│                      ▼                                             │
│           ┌──────────────────────────────┐                        │
│           │  Update Database             │                        │
│           │  - Mark processed            │                        │
│           │  - Commit changes            │                        │
│           └──────────────────────────────┘                        │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

---

## 3. Data Flow Diagrams

### 3.1 File-Level Deduplication Flow

```
┌─────────┐
│  User   │
└────┬────┘
     │
     │ dduper --device /dev/sda1 --files f1 f2
     │
     ▼
┌──────────────────────────────────┐
│  Parse Arguments                  │
│  - Device: /dev/sda1             │
│  - Files: [f1, f2]               │
│  - Mode: default (safe)          │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Validate Files                  │
│  ✓ Regular file?                 │
│  ✓ Size >= 4KB?                  │
│  ✓ Different inodes?             │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Open SQLite DB (dduper.db)      │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Get Checksums for f1            │
│  ┌───────────────────────────┐   │
│  │ Check SQLite cache        │   │
│  │ ├─[Hit]─▶ Return cached   │   │
│  │ └─[Miss]─┐                │   │
│  │          ▼                │   │
│  │  Call btrfs dump-csum     │   │
│  │  Hash output (SHA256)     │   │
│  │  Store in cache           │   │
│  │  Return checksums         │   │
│  └───────────────────────────┘   │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Get Checksums for f2            │
│  (Same process as f1)            │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Compare Full Checksums          │
│  if f1_csum == f2_csum:          │
│    ├─▶ Perfect Match             │
│    │   Auto-adjust chunk size    │
│  else:                           │
│    └─▶ Partial Match             │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Chunk & Hash Checksums          │
│  - Group into chunks (128KB)     │
│  - SHA256 hash each chunk        │
│  - Build OrderedDict             │
│    {hash: [offset]}              │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Find Matching Chunks            │
│  matched = np.intersect1d(       │
│    f1_hashes, f2_hashes)         │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  For each matched chunk:         │
│  ┌────────────────────────────┐  │
│  │ Calculate src/dst offsets  │  │
│  │ Calculate length           │  │
│  │ Prepare ioctl struct       │  │
│  └────────────┬───────────────┘  │
│               ▼                  │
│  ┌────────────────────────────┐  │
│  │ Execute Dedupe ioctl       │  │
│  │                            │  │
│  │ [Safe Mode]                │  │
│  │   FIDEDUPERANGE            │  │
│  │   - Kernel verifies        │  │
│  │   - Returns bytes deduped  │  │
│  │                            │  │
│  │ [Fast Mode]                │  │
│  │   Create backup (reflink)  │  │
│  │   FICLONERANGE             │  │
│  │   - Direct clone           │  │
│  │   - No verification        │  │
│  └────────────┬───────────────┘  │
└───────────────┼──────────────────┘
                │
                ▼
┌──────────────────────────────────┐
│  [Fast Mode Only]                │
│  Validate Results                │
│  - SHA256 compare backup vs new  │
│  - If match: delete backup       │
│  - If fail: log recovery info    │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Update Database                 │
│  - Mark f2 as processed          │
│  - Commit transaction            │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Display Summary                 │
│  - Chunks matched/unmatched      │
│  - Bytes deduped                 │
│  - Performance stats             │
└────────────┬─────────────────────┘
             │
             ▼
┌──────────────────────────────────┐
│  Close Database                  │
└────────────┬─────────────────────┘
             │
             ▼
       ┌─────────┐
       │  Done   │
       └─────────┘
```

### 3.2 Directory Deduplication Flow

```
User: dduper --device /dev/sda1 --dir /mnt/data --recurse
  │
  ▼
┌──────────────────────────────────────────────────────────┐
│  Phase 1: File Validation & Database Creation           │
├──────────────────────────────────────────────────────────┤
│  Walk directory tree (if --recurse)                      │
│  For each file:                                          │
│    ✓ Is regular file?                                    │
│    ✓ Size >= 4KB?                                        │
│    ✓ Add to file_list                                    │
│                                                           │
│  Result: file_list = [f1, f2, f3, ..., fn]              │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Phase 1.1: Populate Records                             │
├──────────────────────────────────────────────────────────┤
│  For each file in file_list:                             │
│    Get BTRFS checksums                                   │
│    Calculate SHA256 hash                                 │
│    Insert into SQLite (filehash table)                   │
│    Mark as valid=1, processed=0                          │
│                                                           │
│  SQLite now contains:                                    │
│  ┌────────────────────────────────────────────┐          │
│  │ filename  │ short_hash │ proc │ valid │     │          │
│  ├───────────┼────────────┼──────┼───────┤     │          │
│  │ f1        │ abc123...  │  0   │  1    │     │          │
│  │ f2        │ abc123...  │  0   │  1    │  ◀─ Duplicate │
│  │ f3        │ def456...  │  0   │  1    │     │          │
│  │ f4        │ ghi789...  │  0   │  1    │     │          │
│  └────────────────────────────────────────────┘          │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Phase 2: Detect Duplicate Files                         │
├──────────────────────────────────────────────────────────┤
│  SQL: SELECT short_hash FROM filehash                    │
│       GROUP BY short_hash HAVING count(*) > 1;           │
│                                                           │
│  Result: duplicate_groups = [                            │
│    [f1, f2],        # Same short_hash                    │
│    [f7, f8, f9]     # Same short_hash                    │
│  ]                                                        │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Phase 3: Dedupe Duplicate Files                         │
├──────────────────────────────────────────────────────────┤
│  For each group in duplicate_groups:                     │
│    Generate combinations: (f1,f2), (f1,f3), (f2,f3)...  │
│    For each pair:                                        │
│      do_dedupe(src, dst)                                 │
│      if perfect_match:                                   │
│        Mark dst as processed                             │
│        Skip dst in future comparisons                    │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────┐
│  Phase 4: Dedupe Remaining Files                         │
├──────────────────────────────────────────────────────────┤
│  SQL: SELECT filename FROM filehash                      │
│       WHERE valid=1 AND processed=0;                     │
│                                                           │
│  Result: remaining_files = [f3, f4, f5, f6, ...]        │
│                                                           │
│  Generate all combinations of remaining files            │
│  Look for partial matches (common blocks)                │
│  Dedupe matching regions                                 │
└───────────────────────┬──────────────────────────────────┘
                        │
                        ▼
                   ┌─────────┐
                   │  Done   │
                   └─────────┘
```

---

## 4. Process Workflows

### 4.1 Hash Processing Workflow

```
Checksum Data (from BTRFS)
    │
    │ out1 = [csum1, csum2, csum3, ..., csumN]
    │
    ▼
┌──────────────────────────────────┐
│  get_hashes(out1)                │
└────────────┬─────────────────────┘
             │
             │ Determine ele_sz based on chunk_size
             │ ele_sz = (chunk_size / 4KB) / 8
             │
             ▼
  ┌────────────────────────┐
  │ if ele_sz == 1:        │
  │   Process individually │
  └────┬───────────────────┘
       │
       ▼
  ┌────────────────────────────────────┐
  │ For each checksum element:         │
  │   idx = 0, 1, 2, ..., N            │
  │   hash = SHA256(str(element))      │
  │   od[hash] = [idx]                 │
  │                                    │
  │   if hash already exists:          │
  │     od[hash].append(idx)           │
  │     (collision - same block)       │
  └────────────────────────────────────┘

  ┌────────────────────────┐
  │ else (ele_sz > 1):     │
  │   Process in groups    │
  └────┬───────────────────┘
       │
       ▼
  ┌────────────────────────────────────┐
  │ Group checksums using grouper()    │
  │   chunk0 = [csum1, ..., csum8]     │
  │   chunk1 = [csum9, ..., csum16]    │
  │   ...                              │
  │                                    │
  │ For each chunk group:              │
  │   idx = 0, 1, 2, ..., M            │
  │   hash = SHA256(str(chunk))        │
  │   od[hash] = [idx]                 │
  │                                    │
  │   if hash already exists:          │
  │     od[hash].append(idx)           │
  │     (collision - same chunk)       │
  └────────────────────────────────────┘
             │
             ▼
Return: OrderedDict {hash: [offset_list]}, collision_count

Example Output:
  od = {
    '9f86d081...': [0],        # Chunk at offset 0
    'a591a6d4...': [1, 5, 9],  # Chunks at offsets 1, 5, 9 (duplicates!)
    '7d865e95...': [2],        # Chunk at offset 2
    ...
  }
```

### 4.2 Mode Selection Workflow

```
                    User Command
                         │
                         ▼
            ┌─────────────────────────┐
            │  Parse Arguments        │
            └────────────┬────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
         ▼               ▼               ▼
    --analyze?      --dry-run?     Check mode flags
         │               │               │
    ┌────┴────┐     ┌────┴────┐     ┌───┴────────┐
    │   YES   │     │   YES   │     │            │
    └────┬────┘     └────┬────┘     │  --fast    │
         │               │           │  --skip    │
         ▼               ▼           └──────┬─────┘
  ┌──────────────┐  ┌──────────┐           │
  │ ANALYSIS     │  │ DRY RUN  │           │
  │ MODE         │  │ MODE     │     ┌─────┴──────┬────────┐
  │              │  │          │     │            │        │
  │ - Test all   │  │ - Skip   │     ▼            ▼        ▼
  │   chunk      │  │   ioctl  │  Neither    --fast    --fast
  │   sizes      │  │ - Report │            only       + --skip
  │ - Report     │  │   only   │     │            │        │
  │   stats      │  └──────────┘     │            │        │
  └──────────────┘                   ▼            ▼        ▼
                              ┌───────────┐ ┌──────────┐ ┌──────────┐
                              │   SAFE    │ │   FAST   │ │ INSANE   │
                              │   MODE    │ │   MODE   │ │  MODE    │
                              └─────┬─────┘ └────┬─────┘ └────┬─────┘
                                    │            │            │
                                    ▼            ▼            ▼
                              ┌────────────────────────────────────┐
                              │  Execute Deduplication             │
                              └────────────────────────────────────┘

Mode Details:

┌───────────────────────────────────────────────────────────────────┐
│  SAFE MODE (Default)                                              │
├───────────────────────────────────────────────────────────────────┤
│  • ioctl: FIDEDUPERANGE                                           │
│  • Verification: Kernel byte-by-byte                              │
│  • Backup: No (kernel ensures safety)                             │
│  • skip flag: Forced to True                                      │
│  • Performance: Slowest (reads file content)                      │
│  • Use case: Production, critical data                            │
└───────────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────────┐
│  FAST MODE (--fast-mode)                                          │
├───────────────────────────────────────────────────────────────────┤
│  • ioctl: FICLONERANGE                                            │
│  • Verification: SHA256 post-dedupe                               │
│  • Backup: Yes (cp --reflink=always file.__dduper)                │
│  • skip flag: False                                               │
│  • Performance: Fast (no file reads)                              │
│  • Use case: Faster performance with validation                   │
└───────────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────────┐
│  INSANE MODE (--fast-mode --skip)                                 │
├───────────────────────────────────────────────────────────────────┤
│  • ioctl: FICLONERANGE                                            │
│  • Verification: None                                             │
│  • Backup: No                                                     │
│  • skip flag: True                                                │
│  • Performance: Fastest                                           │
│  • Use case: Data backed up elsewhere                             │
└───────────────────────────────────────────────────────────────────┘
```

---

## 5. Database Schema

### 5.1 Entity Relationship Diagram

```
┌─────────────────────────────────────────────────────────┐
│                      dduper.db                          │
├─────────────────────────────────────────────────────────┤
│                                                          │
│   ┌──────────────────────────────────────────┐          │
│   │         filehash (Table)                 │          │
│   ├──────────────────────────────────────────┤          │
│   │  filename      TEXT    (PK)              │          │
│   │  short_hash    TEXT    (FK) ────────┐    │          │
│   │  processed     INTEGER               │    │          │
│   │  valid         INTEGER               │    │          │
│   └──────────────────────────────────────┼────┘          │
│                                          │               │
│                                          │ 1:N           │
│                                          │               │
│                                          │               │
│   ┌──────────────────────────────────────▼────┐          │
│   │         btrfscsum (Table)                 │          │
│   ├──────────────────────────────────────────┤          │
│   │  short_hash    TEXT    (PK)              │          │
│   │  long_hash     TEXT                      │          │
│   │                                           │          │
│   │  short_hash = SHA256(btrfs checksums)    │          │
│   │  long_hash  = Full checksum output       │          │
│   └──────────────────────────────────────────┘          │
│                                                          │
└─────────────────────────────────────────────────────────┘

Relationship:
  • One btrfscsum entry can be referenced by multiple filehash entries
  • This represents files with identical content (same checksums)
  • Deduplication candidates are files sharing the same short_hash

Data Flow:
  1. File scanned → Checksums fetched → SHA256 computed (short_hash)
  2. Check if btrfscsum has short_hash
     - If exists: Reuse cached long_hash
     - If not: Insert new btrfscsum record
  3. Insert/Update filehash record with short_hash reference
  4. After dedupe: Update processed flag in filehash
```

### 5.2 Sample Data

```sql
-- filehash table
┌──────────────────────┬──────────────────────────────────────┬───────────┬───────┐
│ filename             │ short_hash                           │ processed │ valid │
├──────────────────────┼──────────────────────────────────────┼───────────┼───────┤
│ /mnt/file1.txt       │ 9f86d081884c7d659a2feaa0c55ad015...  │     0     │   1   │
│ /mnt/file2.txt       │ 9f86d081884c7d659a2feaa0c55ad015...  │     1     │   1   │ ← Same hash
│ /mnt/file3.dat       │ a591a6d40bf420404a011733cfb7b190...  │     0     │   1   │
│ /mnt/backup/file1... │ 9f86d081884c7d659a2feaa0c55ad015...  │     1     │   1   │ ← Same hash
└──────────────────────┴──────────────────────────────────────┴───────────┴───────┘

-- btrfscsum table
┌──────────────────────────────────────┬────────────────────────────────────┐
│ short_hash                           │ long_hash                          │
├──────────────────────────────────────┼────────────────────────────────────┤
│ 9f86d081884c7d659a2feaa0c55ad015...  │ [b'csum: 12345...', b'csum: ...']  │
│ a591a6d40bf420404a011733cfb7b190...  │ [b'csum: 67890...', b'csum: ...']  │
└──────────────────────────────────────┴────────────────────────────────────┘

Query to find duplicates:
  SELECT short_hash, COUNT(*) as count
  FROM filehash
  GROUP BY short_hash
  HAVING count > 1;

Result:
  short_hash                              count
  9f86d081884c7d659a2feaa0c55ad015...     3      ← Duplicate files!
```

---

## 6. Deployment Architecture

### 6.1 Installation Methods

```
┌─────────────────────────────────────────────────────────────────┐
│                   Deployment Options                            │
└─────────────────────────────────────────────────────────────────┘

┌──────────────────────────┐  ┌──────────────────────────┐
│  Method 1: Pre-built     │  │  Method 2: Docker        │
├──────────────────────────┤  ├──────────────────────────┤
│                          │  │                          │
│  ┌────────────────────┐  │  │  ┌────────────────────┐  │
│  │ Git Clone Repo     │  │  │  │ Pull Docker Image  │  │
│  └──────┬─────────────┘  │  │  │ laks/dduper        │  │
│         │                │  │  └──────┬─────────────┘  │
│         ▼                │  │         │                │
│  ┌────────────────────┐  │  │         ▼                │
│  │ pip3 install       │  │  │  ┌────────────────────┐  │
│  │ requirements.txt   │  │  │  │ Container includes:│  │
│  │ - numpy            │  │  │  │ - dduper script    │  │
│  │ - PTable           │  │  │  │ - btrfs.static     │  │
│  └──────┬─────────────┘  │  │  │ - Dependencies     │  │
│         │                │  │  └──────┬─────────────┘  │
│         ▼                │  │         │                │
│  ┌────────────────────┐  │  │         ▼                │
│  │ Copy binaries:     │  │  │  ┌────────────────────┐  │
│  │ - btrfs.static →   │  │  │  │ Run with device    │  │
│  │   /usr/sbin/       │  │  │  │ and volume mounts: │  │
│  │ - dduper →         │  │  │  │                    │  │
│  │   /usr/sbin/       │  │  │  │ docker run -it \   │  │
│  └──────┬─────────────┘  │  │  │  --device /dev/... │  │
│         │                │  │  │  -v /mnt:/mnt \    │  │
│         ▼                │  │  │  laks/dduper ...   │  │
│  ┌────────────────────┐  │  │  └────────────────────┘  │
│  │ Ready to use!      │  │  │                          │
│  └────────────────────┘  │  └──────────────────────────┘
└──────────────────────────┘

┌──────────────────────────────────────────────────┐
│  Method 3: From Source                           │
├──────────────────────────────────────────────────┤
│                                                   │
│  ┌────────────────────┐                          │
│  │ 1. Clone dduper    │                          │
│  └──────┬─────────────┘                          │
│         │                                         │
│         ▼                                         │
│  ┌────────────────────┐                          │
│  │ 2. Clone           │                          │
│  │    btrfs-progs     │                          │
│  └──────┬─────────────┘                          │
│         │                                         │
│         ▼                                         │
│  ┌────────────────────────────────────┐          │
│  │ 3. Apply patch:                    │          │
│  │    patch/btrfs-progs-vX.X/         │          │
│  │    0001-Print-csum...patch         │          │
│  │                                    │          │
│  │    Adds: btrfs inspect-internal \  │          │
│  │          dump-csum command         │          │
│  └──────┬─────────────────────────────┘          │
│         │                                         │
│         ▼                                         │
│  ┌────────────────────┐                          │
│  │ 4. Compile         │                          │
│  │    btrfs-progs:    │                          │
│  │    ./autogen.sh    │                          │
│  │    ./configure     │                          │
│  │    make && install │                          │
│  └──────┬─────────────┘                          │
│         │                                         │
│         ▼                                         │
│  ┌────────────────────┐                          │
│  │ 5. Install dduper  │                          │
│  │    pip install -r  │                          │
│  │    requirements    │                          │
│  │    cp dduper ...   │                          │
│  └──────┬─────────────┘                          │
│         │                                         │
│         ▼                                         │
│  ┌────────────────────┐                          │
│  │ Ready to use!      │                          │
│  └────────────────────┘                          │
└──────────────────────────────────────────────────┘
```

### 6.2 Runtime Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Runtime Environment                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  User Space                                                  │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                                                         │ │
│  │  ┌──────────────┐         ┌─────────────────┐          │ │
│  │  │ dduper       │◀───────▶│ dduper.db       │          │ │
│  │  │ (Python 3)   │         │ (SQLite)        │          │ │
│  │  │              │         │ - filehash      │          │ │
│  │  │ - Main logic │         │ - btrfscsum     │          │ │
│  │  │ - ioctl calls│         └─────────────────┘          │ │
│  │  │ - Hash calc  │                                      │ │
│  │  └───┬──────────┘                                      │ │
│  │      │                                                  │ │
│  │      │ subprocess.Popen()                              │ │
│  │      │                                                  │ │
│  │      ▼                                                  │ │
│  │  ┌──────────────────────────┐                          │ │
│  │  │ btrfs.static             │                          │ │
│  │  │ (Patched btrfs-progs)    │                          │ │
│  │  │                          │                          │ │
│  │  │ inspect-internal \       │                          │ │
│  │  │   dump-csum <file> <dev> │                          │ │
│  │  └───┬──────────────────────┘                          │ │
│  │      │                                                  │ │
│  └──────┼──────────────────────────────────────────────────┘ │
│         │                                                    │
│ ────────┼────────────────────────────────────────────────── │
│         │ ioctl() system calls                              │
│         │ - FIDEDUPERANGE                                   │
│         │ - FICLONERANGE                                    │
│         │                                                    │
│  Kernel Space                                                │
│  ┌──────▼──────────────────────────────────────────────────┐ │
│  │                                                         │ │
│  │  ┌────────────────────────────────────────┐            │ │
│  │  │  BTRFS Kernel Module                   │            │ │
│  │  │                                         │            │ │
│  │  │  ┌────────────────┐  ┌───────────────┐ │            │ │
│  │  │  │ Csum Tree      │  │ Extent Tree   │ │            │ │
│  │  │  │ (Checksums)    │  │ (Block refs)  │ │            │ │
│  │  │  └────────────────┘  └───────────────┘ │            │ │
│  │  │                                         │            │ │
│  │  │  ┌────────────────────────────────────┐ │            │ │
│  │  │  │  Dedupe Operations                 │ │            │ │
│  │  │  │  - Verify (FIDEDUPERANGE)          │ │            │ │
│  │  │  │  - Clone (FICLONERANGE)            │ │            │ │
│  │  │  └────────────────────────────────────┘ │            │ │
│  │  └────────────────────────────────────────┘            │ │
│  │                     │                                   │ │
│  └─────────────────────┼───────────────────────────────────┘ │
│                        │                                     │
│ ───────────────────────┼───────────────────────────────────  │
│                        │                                     │
│  Hardware                                                    │
│  ┌─────────────────────▼─────────────────────────────────┐  │
│  │                                                        │  │
│  │  ┌──────────────────────────────────────────────────┐ │  │
│  │  │  Block Device (e.g., /dev/sda1)                  │ │  │
│  │  │                                                   │ │  │
│  │  │  ┌──────────────────────────────────────────┐    │ │  │
│  │  │  │  BTRFS Filesystem                        │    │ │  │
│  │  │  │  - Data blocks                           │    │ │  │
│  │  │  │  - Metadata (checksums, extent refs)     │    │ │  │
│  │  │  │  - Shared extents (after dedupe)         │    │ │  │
│  │  │  └──────────────────────────────────────────┘    │ │  │
│  │  └──────────────────────────────────────────────────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘

File handles during operation:
  - Source file: Opened O_RDONLY
  - Destination file: Opened O_WRONLY
  - Backup file (fast mode): Created via reflink
  - Database: SQLite connection (read/write)
  - Logs: Append-only file handles
```

---

## 7. Security and Permissions

```
┌─────────────────────────────────────────────────────────────┐
│                  Permission Requirements                    │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  dduper (typically requires root)                           │
│    │                                                         │
│    ├─▶ Read access to BTRFS device (/dev/sdX)              │
│    │   - Required for: btrfs inspect-internal dump-csum    │
│    │   - Capability: CAP_SYS_ADMIN or root                 │
│    │                                                         │
│    ├─▶ Write access to target files                        │
│    │   - Required for: FICLONERANGE / FIDEDUPERANGE        │
│    │   - Permission: File ownership or root                │
│    │                                                         │
│    ├─▶ Create/write dduper.db (current directory)          │
│    │   - Required for: SQLite database                     │
│    │   - Permission: Write access to pwd                   │
│    │                                                         │
│    ├─▶ Write to /var/log/ (optional)                       │
│    │   - Required for: Backup failure logging              │
│    │   - Permission: Write access to /var/log              │
│    │                                                         │
│    └─▶ Create backup files (fast mode)                     │
│        - Required for: cp --reflink=always                  │
│        - Permission: Write access to file directory         │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Summary

This architecture document provides a comprehensive view of the dduper system:

1. **Component Architecture**: Modular design with clear separation between checksum retrieval, deduplication logic, and database management

2. **Data Flow**: Well-defined flows for file and directory deduplication, with caching and optimization strategies

3. **Multiple Modes**: Flexible operation modes (safe, fast, insane, analysis) to balance safety and performance

4. **Database Design**: Efficient SQLite schema for caching checksums and tracking processed files

5. **Deployment**: Multiple installation methods (pre-built, Docker, source) for different use cases

6. **Performance**: Optimization through checksum caching, numpy operations, and perfect match detection

7. **Safety**: Built-in validation mechanisms and backup systems to protect data integrity

The architecture leverages BTRFS native capabilities while providing a user-friendly Python interface for efficient block-level deduplication.
