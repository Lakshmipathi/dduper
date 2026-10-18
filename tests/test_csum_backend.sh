#!/bin/bash
# Test the ioctl csum backend (csum_backend.py).
#
# Part 1 - parity: the ioctl backend must produce byte-identical output to
#          `btrfs inspect-internal dump-csum` (the patched binary), across
#          top-level files, subvolume files, odd sizes, multi-extent files
#          and reflinked slice-clones. Skipped when no dump-csum capable
#          binary is available (set DUMP_CSUM_BIN to point at one). The
#          binary must be built with the subvolume-aware patch
#          (patch/btrfs-progs-v6.11/...-with-subvolume.patch); older
#          dump-csum builds return wrong csums for subvolume files and
#          will fail parity - that is the binary's bug, not the backend's.
# Part 2 - end-to-end: dduper (default ioctl backend, no patched
#          btrfs-progs needed) dedupes duplicate files inside a subvolume.
#
# Needs root, mkfs.btrfs, and python3 with dduper deps (numpy, prettytable).
# Creates a 1GB sparse image under /tmp, cleans up after itself.
#
# Usage: sudo [PYTHON=/path/to/python3] [DUMP_CSUM_BIN=/path/to/btrfs] \
#            bash tests/test_csum_backend.sh

set -u

TOPDIR="$(cd "$(dirname "$0")/.." && pwd)"
PYTHON="${PYTHON:-python3}"
DUMP_CSUM_BIN="${DUMP_CSUM_BIN:-}"
IMG=""
MNT=""
WORKDIR=""
LOOP=""
PASS=0
FAIL=0

ok()   { echo "PASS: $1"; PASS=$((PASS + 1)); }
bad()  { echo "FAIL: $1"; FAIL=$((FAIL + 1)); }

cleanup() {
    cd /
    [ -n "$MNT" ] && umount "$MNT" 2>/dev/null
    [ -n "$LOOP" ] && losetup -d "$LOOP" 2>/dev/null
    rm -f "$IMG"
    rm -rf "$MNT" "$WORKDIR"
}
trap cleanup EXIT

if [ "$(id -u)" != "0" ]; then
    echo "This test needs root (loop mounts + csum tree ioctls)." >&2
    exit 1
fi

echo "[1/5] Creating 1GB btrfs image with a subvolume..."
IMG=$(mktemp /tmp/dduper_csum_XXXX.img)
MNT=$(mktemp -d /tmp/dduper_csum_mnt_XXXX)
WORKDIR=$(mktemp -d /tmp/dduper_csum_wk_XXXX)
truncate -s 1G "$IMG"
mkfs.btrfs -q -f "$IMG"
LOOP=$(losetup -f --show "$IMG")
mount "$LOOP" "$MNT"
btrfs -q subvolume create "$MNT/sub1"

echo "[2/5] Creating test files..."
dd if=/dev/urandom of="$MNT/rootfile" bs=1M count=8 status=none
dd if=/dev/urandom of="$MNT/big" bs=1M count=200 status=none
dd if=/dev/urandom of="$MNT/sub1/f1" bs=1M count=8 status=none
# --reflink=never: plain cp reflinks on btrfs, which would leave
# nothing to deduplicate (and nothing to prove).
cp --reflink=never "$MNT/sub1/f1" "$MNT/sub1/f1copy"
# 1036KB: csum count not a multiple of 8, exercises the partial last line.
dd if=/dev/urandom of="$MNT/sub1/odd" bs=1K count=1036 status=none
# Slice-clone: reflink then rewrite the first 1MB, so the tail references
# the middle of another file's on-disk extent (extent offset != 0).
cp --reflink=always "$MNT/sub1/f1" "$MNT/sub1/sliceclone"
dd if=/dev/urandom of="$MNT/sub1/sliceclone" bs=1M count=1 conv=notrunc status=none
sync && sleep 2 && sync

echo "[3/5] Parity: ioctl backend vs dump-csum binary..."
if [ -z "$DUMP_CSUM_BIN" ]; then
    for cand in /usr/sbin/btrfs.static btrfs; do
        if "$cand" inspect-internal dump-csum --help >/dev/null 2>&1; then
            DUMP_CSUM_BIN="$cand"
            break
        fi
    done
fi
if [ -z "$DUMP_CSUM_BIN" ]; then
    echo "SKIP: no dump-csum capable btrfs found (set DUMP_CSUM_BIN)."
else
    if "$PYTHON" - "$TOPDIR" "$DUMP_CSUM_BIN" "$LOOP" \
        "$MNT/rootfile" "$MNT/big" "$MNT/sub1/f1" "$MNT/sub1/f1copy" \
        "$MNT/sub1/odd" "$MNT/sub1/sliceclone" <<'PYEOF'
import subprocess
import sys

topdir, binary, device = sys.argv[1:4]
sys.path.insert(0, topdir)
import csum_backend

csum_backend.configure('ioctl')
rc = 0
for f in sys.argv[4:]:
    want = subprocess.Popen(
        [binary, 'inspect-internal', 'dump-csum', f, device],
        stdout=subprocess.PIPE, close_fds=True).stdout.readlines()
    got = csum_backend.dump_csum(f, device)
    if got == want and len(got) > 0:
        print("  parity ok: %s (%d lines)" % (f, len(got)))
    else:
        print("  parity MISMATCH: %s (binary %d lines, ioctl %d lines)"
              % (f, len(want), len(got)))
        rc = 1
sys.exit(rc)
PYEOF
    then
        ok "ioctl output byte-identical to dump-csum binary"
    else
        bad "ioctl output differs from dump-csum binary"
    fi
fi

echo "[4/5] End-to-end: dduper ioctl backend dedupes a subvolume..."
# "Exclusive" bytes drop when a file starts sharing extents. ("Set shared"
# would not grow here: f1copy joins an extent sliceclone already shares.)
exclusive_bytes() {
    btrfs filesystem du -s --raw "$1" 2>/dev/null | awk 'NR==2 {print $2}'
}
before=$(exclusive_bytes "$MNT/sub1")
cd "$WORKDIR"  # dduper writes dduper.db / dduper.log into cwd
"$PYTHON" "$TOPDIR/dduper" --fast-mode --device "$LOOP" --recurse \
    --dir "$MNT/sub1" > dduper.out 2>&1
status=$?
sync  # commit clone metadata before measuring extent sharing
after=$(exclusive_bytes "$MNT/sub1")

if [ $status -eq 0 ] && ! grep -q "Traceback" dduper.out; then
    ok "dduper completed without errors"
else
    bad "dduper failed (exit $status), see $WORKDIR/dduper.out"
    tail -5 dduper.out
fi
if grep -q "Dedupe validation successful.*f1copy" dduper.out; then
    ok "duplicate pair deduped and validated"
else
    bad "no successful dedupe reported for f1:f1copy"
fi
if cmp -s "$MNT/sub1/f1" "$MNT/sub1/f1copy"; then
    ok "file contents intact after dedupe"
else
    bad "file contents CHANGED after dedupe"
fi
if [ -n "$before" ] && [ -n "$after" ] && [ "$after" -le $((before - 8388608)) ]; then
    ok "exclusive bytes dropped by >= 8MB (before=$before after=$after)"
else
    bad "exclusive bytes did not drop as expected (before=$before after=$after)"
fi

echo "[5/5] Summary: $PASS passed, $FAIL failed."
[ $FAIL -eq 0 ]
