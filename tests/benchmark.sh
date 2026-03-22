#!/bin/bash
# Benchmark: dduper (Python vs Rust) vs naive SHA256 approach
# Shows why fetching BTRFS csum-tree checksums is faster than reading file data
set -e

BTRFS_BIN="/usr/sbin/btrfs.static"
PYTHON="/home/laks/foss/dduper/.venv/bin/python3"
DDUPER_RUST="/home/laks/foss/dduper/target/release/dduper"
DDUPER_PY="/home/laks/foss/dduper/dduper"
IMG="/home/laks/foss/btrfs_bench.img"
MNT="/tmp/dduper_bench_mnt"

# File sizes to test (in MB)
SIZES="1024 5120 10240 20480 51200 102400"

echo "================================================================"
echo "  dduper Benchmark: BTRFS csum-tree vs SHA256 file reading"
echo "================================================================"
echo ""
echo "This benchmark compares three approaches to find duplicate data:"
echo "  1. SHA256 (naive): read entire file contents, compute hash"
echo "  2. dduper Python:  fetch checksums from BTRFS csum-tree"
echo "  3. dduper Rust:    same approach, compiled Rust binary"
echo ""

# Cleanup
sudo umount "$MNT" 2>/dev/null || true
rm -f "$IMG"
mkdir -p "$MNT"

# Create BTRFS image (large enough for 100GB file + copy)
echo "[Setup] Creating 250GB BTRFS image..."
truncate -s 250G "$IMG"
mkfs.btrfs -f "$IMG" > /dev/null 2>&1
sudo mount -o loop "$IMG" "$MNT"
sudo chmod 777 "$MNT"
LOOP=$(findmnt -n -o SOURCE "$MNT")
echo "[Setup] Mounted on $LOOP"
echo ""

# Print header
printf "%-10s | %-14s | %-14s | %-14s | %-10s\n" \
    "File Size" "SHA256 (naive)" "dduper Python" "dduper Rust" "Speedup"
printf "%-10s-+-%-14s-+-%-14s-+-%-14s-+-%-10s\n" \
    "----------" "--------------" "--------------" "--------------" "----------"

results=""

for SIZE_MB in $SIZES; do
    SIZE_LABEL="${SIZE_MB}MB"
    if [ "$SIZE_MB" -ge 1024 ]; then
        SIZE_LABEL="$((SIZE_MB / 1024))GB"
    fi

    # Create test files
    dd if=/dev/urandom of="$MNT/bench_src" bs=1M count=$SIZE_MB status=none 2>/dev/null
    cp --reflink=never "$MNT/bench_src" "$MNT/bench_dst"
    sync
    # Drop caches to make SHA256 test fair
    echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null

    # Benchmark 1: Naive SHA256 (read full file, compute hash, compare)
    SHA_START=$(date +%s%N)
    sha256sum "$MNT/bench_src" > /dev/null
    sha256sum "$MNT/bench_dst" > /dev/null
    SHA_END=$(date +%s%N)
    SHA_MS=$(( (SHA_END - SHA_START) / 1000000 ))

    # Drop caches again
    echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null

    # Benchmark 2: dduper Python (dry-run)
    cd /tmp
    sudo rm -f dduper.db dduper.log
    PY_START=$(date +%s%N)
    sudo "$PYTHON" "$DDUPER_PY" --device "$LOOP" --files "$MNT/bench_src" "$MNT/bench_dst" --dry-run > /dev/null 2>&1
    PY_END=$(date +%s%N)
    PY_MS=$(( (PY_END - PY_START) / 1000000 ))

    # Drop caches again
    echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null

    # Benchmark 3: dduper Rust (dry-run)
    sudo rm -f dduper.db dduper.log
    RS_START=$(date +%s%N)
    sudo "$DDUPER_RUST" --device "$LOOP" --files "$MNT/bench_src" "$MNT/bench_dst" --dry-run > /dev/null 2>&1
    RS_END=$(date +%s%N)
    RS_MS=$(( (RS_END - RS_START) / 1000000 ))

    # Convert to seconds
    SHA_SEC=$(echo "scale=2; $SHA_MS / 1000" | bc 2>/dev/null || echo "N/A")
    PY_SEC=$(echo "scale=2; $PY_MS / 1000" | bc 2>/dev/null || echo "N/A")
    RS_SEC=$(echo "scale=2; $RS_MS / 1000" | bc 2>/dev/null || echo "N/A")

    # Compute speedup
    if [ "$RS_MS" -gt 0 ]; then
        SHA_VS_RUST=$(echo "scale=1; $SHA_MS / $RS_MS" | bc 2>/dev/null || echo "N/A")
    else
        SHA_VS_RUST="N/A"
    fi

    printf "%-10s | %12ss | %12ss | %12ss | %7sx\n" \
        "$SIZE_LABEL" "$SHA_SEC" "$PY_SEC" "$RS_SEC" "$SHA_VS_RUST"

    # Cleanup test files for next iteration
    rm -f "$MNT/bench_src" "$MNT/bench_dst"
    sync
done

echo ""
echo "Speedup = SHA256 time / Rust dduper time"
echo ""
echo "Why dduper is faster:"
echo "  SHA256 must read every byte of both files from disk."
echo "  dduper fetches pre-computed checksums from BTRFS csum-tree"
echo "  (a compact B-tree), avoiding file I/O entirely."
echo ""

# Cleanup
sudo umount "$MNT"
rm -f "$IMG"
sudo rm -f /tmp/dduper.db /tmp/dduper.log
echo "Done."
