# dduper Design Document

## 1. Overview

### 1.1 Project Summary
dduper is a block-level out-of-band BTRFS deduplication tool that significantly improves performance by fetching built-in checksums from the BTRFS csum-tree instead of reading file blocks and computing checksums itself. This design document outlines the architecture, components, and operational modes of dduper.

### 1.2 Key Features
- **Performance**: Leverages BTRFS native checksums for faster deduplication
- **Multiple Modes**: Safe, fast, and insane modes for different use cases
- **Flexible Operation**: Supports file-level and directory-level deduplication
- **Analysis**: Provides chunk size analysis to optimize deduplication
- **Safety**: Built-in validation and backup mechanisms

### 1.3 Target Environment
- **Filesystem**: BTRFS only
- **Platform**: Linux
- **Language**: Python 3
- **Dependencies**: Modified btrfs-progs, numpy, PTable

## 2. System Architecture

### 2.1 High-Level Components

#### 2.1.1 Core Components
1. **Main Controller (`dduper` script)**
   - Entry point and orchestration
   - Command-line argument parsing
   - Mode selection and execution flow

2. **BTRFS Checksum Interface**
   - Interacts with patched btrfs-progs
   - Executes `btrfs inspect-internal dump-csum` command
   - Caches checksum data in SQLite database

3. **SQLite Database Manager**
   - Stores file checksums and processing status
   - Tracks processed files to avoid redundant operations
   - Two tables: `filehash` and `btrfscsum`

4. **Deduplication Engine**
   - Compares checksums between files
   - Identifies duplicate blocks
   - Executes dedupe operations via ioctl calls

5. **Validation System**
   - Creates backups before deduplication (in safe modes)
   - Performs SHA256 validation after deduplication
   - Restores files on corruption detection

#### 2.1.2 External Dependencies
1. **Patched btrfs-progs**
   - Custom patch adds `dump-csum` functionality
   - Exposes BTRFS internal checksums to userspace
   - Available for multiple btrfs-progs versions (v5.6.1 to v6.11)

2. **Linux Kernel APIs**
   - FICLONERANGE ioctl (0x4020940d) - Fast mode dedupe
   - FIDEDUPERANGE ioctl (0xc0189436) - Safe mode dedupe

### 2.2 Data Flow

#### 2.2.1 Checksum Retrieval Flow
```
User Input (file/dir)
    → Validation
    → SQLite Cache Check
    → [Cache Miss] → btrfs inspect-internal dump-csum
    → Parse Checksum Output
    → Store in SQLite
    → Return Checksums
```

#### 2.2.2 Deduplication Flow
```
File Pair (src, dst)
    → Validate Files
    → Retrieve Checksums
    → Compute SHA256 Hash of Checksums
    → Compare Checksums
    → [Perfect Match] → Auto-adjust chunk size
    → [Partial Match] → Identify matching regions
    → [No Match] → Skip
    → Execute Dedupe (ioctl)
    → [Safe Mode] → Validate Results
    → Update Database
    → Report Results
```

## 3. Operational Modes

### 3.1 Default Mode (Safe Mode)
- **Method**: FIDEDUPERANGE ioctl
- **Verification**: Kernel verifies byte-by-byte before dedupe
- **Backup**: Not created (kernel verification ensures safety)
- **Performance**: Slowest (reads file contents)
- **Safety**: 100% safe
- **Use Case**: Production environments, critical data

### 3.2 Fast Mode
- **Method**: FICLONERANGE ioctl
- **Verification**: SHA256 comparison after dedupe
- **Backup**: Created before dedupe (`file.__dduper`)
- **Performance**: Fast (no file content reads)
- **Safety**: High (SHA256 validation)
- **Use Case**: When faster performance needed with safety checks
- **Flag**: `--fast-mode`

### 3.3 Insane Mode
- **Method**: FICLONERANGE ioctl
- **Verification**: None
- **Backup**: Not created
- **Performance**: Fastest
- **Safety**: Relies entirely on BTRFS checksums
- **Use Case**: When data is backed up elsewhere
- **Flag**: `--fast-mode --skip`

### 3.4 Analysis Mode
- **Purpose**: Determine optimal chunk size
- **Process**: Tests multiple chunk sizes (128KB to 16MB)
- **Output**: Reports duplicate data found per chunk size
- **Flag**: `--analyze`

## 4. Database Schema

### 4.1 Tables

#### 4.1.1 filehash Table
```sql
CREATE TABLE filehash (
    filename TEXT,      -- Full path to file
    short_hash TEXT,    -- SHA256 hash of checksum data
    processed INTEGER,  -- 0 = not processed, 1 = processed
    valid INTEGER       -- 0 = not validated, 1 = validated
)
```

#### 4.1.2 btrfscsum Table
```sql
CREATE TABLE btrfscsum (
    short_hash TEXT,    -- SHA256 hash of checksum data
    long_hash TEXT      -- Full BTRFS checksum output
)
```

### 4.2 Database Operations
- **Caching**: Avoids re-computing checksums for unchanged files
- **Deduplication**: Identifies duplicate files by `short_hash`
- **Tracking**: Marks processed files to avoid redundant operations
- **Storage**: Database file: `dduper.db` in working directory

## 5. Algorithm Details

### 5.1 Chunk-Based Deduplication

#### 5.1.1 Chunk Size Configuration
- **Default**: 128 KB
- **Configurable**: Multiple of 128 KB (128, 256, 512, 1024, 2048, 4096, 8192, 16384 KB)
- **Block Size**: 4 KB (BTRFS default)
- **Calculation**: `no_of_chunks = chunk_size / block_size`

#### 5.1.2 Hash Computation
1. Retrieve BTRFS checksums for file
2. Group checksums into chunks (based on `ele_sz`)
3. Compute SHA256 hash for each chunk
4. Store in OrderedDict with offset as key
5. Compare dictionaries between files using numpy intersection

#### 5.1.3 Perfect Match Optimization
When files have identical checksums:
1. Detect perfect match early
2. Auto-adjust chunk size based on file size:
   - File ≥ 16MB → 16MB chunk
   - File ≥ 8MB → 8MB chunk
   - File ≥ 4MB → 4MB chunk
   - File ≥ 2MB → 2MB chunk
   - File ≥ 1MB → 1MB chunk
   - File ≥ 512KB → 512KB chunk
   - Otherwise → 128KB chunk
3. Reduces number of ioctl calls for whole-file deduplication

### 5.2 Directory Deduplication Process

#### Phase 1: File Validation and Database Creation
- Scan directory (with optional recursion)
- Validate each file (regular file, size ≥ 4KB)
- Build file list

#### Phase 2: Populate Checksum Records
- For each valid file, retrieve BTRFS checksums
- Store in database with validation flag

#### Phase 3: Detect Duplicate Files
- Query database for files with identical `short_hash`
- Group duplicate files together

#### Phase 4: Dedupe Duplicate Files
- Process groups of duplicate files
- Perform pairwise deduplication

#### Phase 5: Dedupe Remaining Files
- Process non-duplicate files for partial matches
- Find common blocks across different files

## 6. Safety Mechanisms

### 6.1 Validation Checks
1. **File Validation**
   - Regular file check
   - Minimum size (4KB)
   - Unique inode check
   - Same device check

2. **Deduplication Validation**
   - Non-empty checksum verification
   - Chunk size limits (≤ 16MB)
   - Offset and length boundary checks

### 6.2 Backup and Recovery
1. **Backup Creation** (Fast mode only)
   - Uses `cp --reflink=always` for CoW copy
   - Backup file: `original_file.__dduper`

2. **Post-Dedupe Validation** (Fast mode only)
   - SHA256 comparison between backup and deduped file
   - Automatic logging to `/var/log/dduper_backupfile_info.log` on failure
   - Backup preserved for manual recovery

3. **Error Handling**
   - Logging to `dduper.log`
   - Exception handling for ioctl failures
   - Status tracking in database

## 7. Performance Optimizations

### 7.1 Checksum Caching
- SQLite database caches all checksum computations
- Avoids redundant `btrfs inspect-internal dump-csum` calls
- Significant speedup for repeated operations

### 7.2 Numpy-Based Comparison
- Uses numpy arrays for fast set operations
- `np.intersect1d()` for finding matching chunks
- `np.setdiff1d()` for finding unique chunks

### 7.3 Processed Files Tracking
- Maintains list of processed files
- Avoids redundant comparisons
- Marks perfectly matching files to skip in subsequent comparisons

### 7.4 Combination Optimization
- Uses `itertools.combinations()` for efficient pairwise generation
- Early termination for perfect matches

## 8. Supported Checksum Types

### 8.1 Current Support
- **CRC32**: Fully supported (default BTRFS)
- **XXHASH64**: Initial support
- **BLAKE2**: Initial support
- **SHA256**: Initial support

### 8.2 Limitations
- Subvolumes not supported
- Cannot deduplicate identical blocks within single file
- Requires matching checksum type across compared files

## 9. Installation and Deployment

### 9.1 Installation Methods

#### 9.1.1 Pre-built Binary
- Clone repository
- Install Python dependencies (numpy, PTable)
- Copy `btrfs.static` and `dduper` to `/usr/sbin/`

#### 9.1.2 Docker
- Use `laks/dduper` image
- Mount device and directory
- No dependency installation required

#### 9.1.3 From Source
- Clone repository
- Clone btrfs-progs
- Apply patch for target version
- Compile and install btrfs-progs
- Install dduper script

### 9.2 Patch Maintenance
- Patches maintained for multiple btrfs-progs versions
- Version-specific patches in `patch/` directory
- Patches add `dump-csum` command to `btrfs inspect-internal`

## 10. Usage Patterns

### 10.1 File Deduplication
```bash
# Safe mode (default)
dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2

# Fast mode with validation
dduper --fast-mode --device /dev/sda1 --files /mnt/f1 /mnt/f2

# Insane mode (no validation)
dduper --fast-mode --skip --device /dev/sda1 --files /mnt/f1 /mnt/f2
```

### 10.2 Directory Deduplication
```bash
# Single directory
dduper --device /dev/sda1 --dir /mnt/dir

# Recursive
dduper --device /dev/sda1 --dir /mnt/dir --recurse

# Multiple directories
dduper --device /dev/sda1 --dir /mnt/dir1 /mnt/dir2
```

### 10.3 Analysis
```bash
# Analyze optimal chunk size
dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --analyze

# Dry run (no actual deduplication)
dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --dry-run
```

### 10.4 Finding Duplicates
```bash
# List perfect match files only
dduper --device /dev/sda1 --dir /mnt --recurse --perfect-match-only
```

## 11. Future Enhancements

### 11.1 Potential Improvements
1. **Multi-threading**: Parallel checksum retrieval
2. **Incremental Mode**: Track filesystem changes
3. **Extent-based Dedup**: Handle inline extents
4. **Subvolume Support**: Cross-subvolume deduplication
5. **Progress Reporting**: Real-time progress for large operations
6. **Configurable Hash**: User-selectable hash algorithm
7. **Single-file Dedup**: Deduplicate repeated blocks within one file

### 11.2 Known Limitations
1. Subvolume incompatibility
2. Single-file block deduplication not supported
3. No automatic scheduling/daemon mode
4. Manual chunk size selection for optimal results

## 12. Testing Strategy

### 12.1 Test Coverage
- Basic functionality tests in `tests/test.py`
- GitLab CI integration (`.gitlab-ci.yml`)
- Docker-based testing environment
- Dataset generation for consistent testing

### 12.2 Verification Script
- Basic check script available: `tests/verify.sh`
- Validates deduplication correctness
- Ensures data integrity

## 13. Logging and Monitoring

### 13.1 Log Files
1. **dduper.log**
   - Debug logging
   - Operation tracking
   - Error messages

2. **/var/log/dduper_backupfile_info.log**
   - Corruption detection alerts
   - Backup file locations
   - Recovery instructions

### 13.2 Output Verbosity
- **Default**: Summary information
- **Verbose mode** (`--verbose`): Detailed logging
- **Analysis mode**: Comprehensive chunk size reports

## 14. Security Considerations

### 14.1 Permissions
- Requires read access to BTRFS device
- Requires write access to target files
- Typically needs root privileges

### 14.2 Data Safety
- Default mode uses kernel verification
- Fast mode includes SHA256 validation
- Backup mechanism prevents data loss
- Checksum integrity ensures accuracy

### 14.3 Resource Usage
- SQLite database grows with file count
- Minimal memory footprint
- I/O limited by checksum retrieval
- CPU usage dominated by hash computation

## 15. Conclusion

dduper provides an efficient, safe, and flexible deduplication solution for BTRFS filesystems. By leveraging native BTRFS checksums, it achieves significant performance improvements over traditional block-reading approaches. The multi-mode operation allows users to balance safety and performance based on their specific requirements, while the built-in validation and backup mechanisms ensure data integrity even in the fastest modes.
