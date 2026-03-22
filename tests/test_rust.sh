#!/bin/bash
# Test script: creates BTRFS image, generates test files, runs dduper (Rust + Python)
set -e

BTRFS_BIN="/usr/sbin/btrfs.patched"
PYTHON="/home/laks/foss/dduper/.venv/bin/python3"
IMG="/tmp/dduper_test.img"
MNT="/tmp/dduper_mnt"
DDUPER_RUST="/home/laks/foss/dduper/target/release/dduper"
DDUPER_PY="/home/laks/foss/dduper/dduper"
SIZE_MB=512

echo "=== dduper test: Python vs Rust ==="

# Cleanup
sudo umount "$MNT" 2>/dev/null || true
rm -f "$IMG"
mkdir -p "$MNT"

# Create BTRFS image
echo "[1/6] Creating ${SIZE_MB}MB BTRFS image..."
truncate -s ${SIZE_MB}M "$IMG"
mkfs.btrfs -f "$IMG" > /dev/null 2>&1
sudo mount -o loop "$IMG" "$MNT"
sudo chmod 777 "$MNT"

# Get the loop device
LOOP_DEV=$(findmnt -n -o SOURCE "$MNT")
echo "       Mounted on $LOOP_DEV"

# Create test files (50MB each, identical content)
echo "[2/6] Creating test files..."
dd if=/dev/urandom of="$MNT/file1" bs=1M count=50 2>/dev/null
cp "$MNT/file1" "$MNT/file2"
cp "$MNT/file1" "$MNT/file3"
# Create a partially different file
cp "$MNT/file1" "$MNT/file4"
dd if=/dev/urandom of="$MNT/file4" bs=1M count=10 conv=notrunc 2>/dev/null

sync
echo "       Created 4 test files (3 identical, 1 partial match)"

# Check disk usage before
BEFORE=$(df --output=used "$MNT" | tail -1 | tr -d ' ')
echo "       Disk used before dedupe: ${BEFORE}KB"

# Test Python version (dry-run)
echo ""
echo "[3/6] Testing Python dduper (dry-run)..."
cd /tmp
sudo rm -f dduper.db dduper.log
PYTHON_START=$(date +%s%N)
sudo "$PYTHON" "$DDUPER_PY" --device "$LOOP_DEV" --dir "$MNT" --dry-run 2>&1 || echo "(Python exited with error)"
PYTHON_END=$(date +%s%N)
PYTHON_MS=$(( (PYTHON_END - PYTHON_START) / 1000000 ))
echo "       Python dry-run took: ${PYTHON_MS}ms"

# Test Rust version (dry-run)
echo ""
echo "[4/6] Testing Rust dduper (dry-run)..."
sudo rm -f dduper.db dduper.log
RUST_START=$(date +%s%N)
sudo "$DDUPER_RUST" --device "$LOOP_DEV" --dir "$MNT" --dry-run 2>&1 || echo "(Rust exited with error)"
RUST_END=$(date +%s%N)
RUST_MS=$(( (RUST_END - RUST_START) / 1000000 ))
echo "       Rust dry-run took: ${RUST_MS}ms"

# Test Rust version (actual dedupe)
echo ""
echo "[5/6] Running Rust dduper (actual dedupe)..."
sudo rm -f dduper.db dduper.log
RUST_DEDUPE_START=$(date +%s%N)
sudo "$DDUPER_RUST" --device "$LOOP_DEV" --dir "$MNT" 2>&1 || echo "(Rust dedupe exited with error)"
RUST_DEDUPE_END=$(date +%s%N)
RUST_DEDUPE_MS=$(( (RUST_DEDUPE_END - RUST_DEDUPE_START) / 1000000 ))
echo "       Rust dedupe took: ${RUST_DEDUPE_MS}ms"

sync

# Check disk usage after
AFTER=$(df --output=used "$MNT" | tail -1 | tr -d ' ')
echo "       Disk used after dedupe: ${AFTER}KB"
SAVED=$(( BEFORE - AFTER ))
echo "       Space saved: ${SAVED}KB"

# Summary
echo ""
echo "[6/6] === SUMMARY ==="
echo "       Python dry-run: ${PYTHON_MS}ms"
echo "       Rust dry-run:   ${RUST_MS}ms"
echo "       Rust dedupe:    ${RUST_DEDUPE_MS}ms"
echo "       Space saved:    ${SAVED}KB"
if [ "$PYTHON_MS" -gt 0 ]; then
    SPEEDUP=$(echo "scale=1; $PYTHON_MS / $RUST_MS" | bc 2>/dev/null || echo "N/A")
    echo "       Speedup:        ${SPEEDUP}x"
fi

# Cleanup
sudo umount "$MNT"
rm -f "$IMG"
sudo rm -f dduper.db dduper.log
echo ""
echo "Done."
