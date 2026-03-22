# dduper Benchmark: BTRFS csum-tree vs SHA256 file reading

## Why dduper is fast

Traditional dedup tools read every byte of both files and compute checksums (e.g., SHA256).
dduper instead fetches pre-computed checksums directly from BTRFS's internal csum-tree — a
compact B-tree — avoiding file I/O entirely.

## Results

Test setup: Two identical files on BTRFS (copied with `--reflink=never` to ensure separate
extents). Caches dropped between runs. Dry-run mode (detection only, no actual dedup).

```
File Size  | SHA256 (naive) | dduper Python  | dduper Rust    | Speedup
-----------+----------------+----------------+----------------+-----------
1GB        |         8.68s |         0.54s  |         0.26s  |    33x
5GB        |        41.62s |         1.27s  |         1.03s  |    40x
10GB       |        83.05s |         2.14s  |         2.09s  |    40x
20GB       |       168.62s |         4.02s  |         4.18s  |    40x
50GB       |       422.88s |         9.36s  |        10.23s  |    41x
100GB      |       850.20s |        18.48s  |        20.29s  |    42x
```

Speedup = SHA256 time / dduper Rust time

- **SHA256 (naive)**: `sha256sum` on both files (reads all data from disk)
- **dduper Python**: fetches checksums from BTRFS csum-tree
- **dduper Rust**: same approach, compiled binary

## Key takeaway

For a 100GB file pair, SHA256 takes **14 minutes** while dduper takes **20 seconds** — a
**42x speedup** by avoiding file I/O entirely.

## How to reproduce

```bash
bash tests/benchmark.sh
```

Requires: patched btrfs-progs (with dump-csum), root access for mount/umount.
