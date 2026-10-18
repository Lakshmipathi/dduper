# -*- coding: utf-8 -*-
#
# csum_backend.py - fetch BTRFS checksums for a file.
#
# Part of dduper. Licensed under the GNU General Public License v2 (see
# COPYING in the dduper source tree).
#
# Two backends are provided:
#
#   ioctl   - reads the csum tree of a *mounted* filesystem directly via
#             the kernel ioctls BTRFS_IOC_INO_LOOKUP, BTRFS_IOC_FS_INFO
#             and BTRFS_IOC_TREE_SEARCH_V2. Needs root (CAP_SYS_ADMIN),
#             which dduper requires anyway. No patched btrfs-progs needed.
#   binary  - runs `btrfs inspect-internal dump-csum <file> <device>`,
#             which needs btrfs-progs built with the dduper dump-csum
#             patch (see patch/ directory). This is the original backend.
#
# The ioctl backend produces byte-identical output to the dump-csum
# binary (including its quirks: the 8-csums-per-line counter runs across
# extents, reflinked extents are read from the start of the on-disk
# extent, and the dump is truncated on the first unresolvable extent),
# so sha256 hashes of the output - and therefore existing dduper.db
# caches - are the same whichever backend produced them.
#
# Credits:
#  - ioctl numbers and struct layouts follow the kernel userspace ABI in
#    include/uapi/linux/btrfs.h (GPL-2.0 WITH Linux-syscall-note). That
#    ABI is stable: INO_LOOKUP exists since 2009, TREE_SEARCH_V2 since
#    kernel 3.14 (2014).
#  - The tree-search approach was informed by the python-btrfs project
#    (LGPL-3.0+) by Hans van Kranenburg,
#    https://github.com/knorrie/python-btrfs - in particular its
#    examples/show_file_csum.py. No code was copied from it.
#  - Output format matches the btrfs-progs dump-csum patch by
#    Lakshmipathi.G shipped in the dduper repository.

import fcntl
import logging
import os
import struct
import subprocess

# --- kernel ABI ------------------------------------------------------------

BTRFS_IOC_TREE_SEARCH_V2 = 0xC0709411  # _IOWR(0x94, 17, 112)
BTRFS_IOC_INO_LOOKUP = 0xD0009412      # _IOWR(0x94, 18, 4096)
BTRFS_IOC_FS_INFO = 0x8400941F         # _IOR(0x94, 31, 1024)

CSUM_TREE_OBJECTID = 7
EXTENT_CSUM_OBJECTID = 0xFFFFFFFFFFFFFFF6  # (u64)-10
FIRST_FREE_OBJECTID = 256
EXTENT_DATA_KEY = 108
EXTENT_CSUM_KEY = 128
FILE_EXTENT_INLINE = 0

ULLONG_MAX = 0xFFFFFFFFFFFFFFFF
CSUM_SIZE = 4  # crc32c; other csum types fall back to the binary backend

# struct btrfs_ioctl_search_key: tree_id, min/max objectid, min/max offset,
# min/max transid (7 x u64), min/max type, nr_items, unused (4 x u32),
# unused1..4 (4 x u64) = 104 bytes.
_SEARCH_KEY = struct.Struct('=QQQQQQQLLLL4Q')
_SEARCH_KEY_NR_ITEMS_OFF = 64
# struct btrfs_ioctl_search_header: transid, objectid, offset (u64),
# type, len (u32) = 32 bytes, immediately followed by `len` bytes of item.
_SEARCH_HEADER = struct.Struct('=QQQLL')
_SEARCH_BUF_SIZE = 1024 * 1024
# Items start after the search key (104) + buf_size field (8).
_SEARCH_ITEMS_OFF = 112

# struct btrfs_file_extent_item: generation, ram_bytes (u64), compression,
# encryption (u8), other_encoding (u16), type (u8) = 21 bytes, then
# disk_bytenr, disk_num_bytes, offset, num_bytes (u64) for non-inline.
# Unlike the ioctl argument structs above (native-endian, filled by the
# kernel in memory), item payloads are raw leaf bytes: little-endian on
# disk on every architecture.
_FILE_EXTENT_HEAD = struct.Struct('<QQBBHB')
_FILE_EXTENT_DISK = struct.Struct('<QQQQ')


def _ino_lookup_tree(fd):
    """Return the id of the subvolume tree containing fd's inode."""
    args = bytearray(struct.pack('=QQ', 0, FIRST_FREE_OBJECTID) + bytes(4080))
    fcntl.ioctl(fd, BTRFS_IOC_INO_LOOKUP, args)
    return struct.unpack_from('=Q', args, 0)[0]


def _fs_info(fd):
    """Return (sectorsize, nodesize, csum_type) of fd's filesystem."""
    args = bytearray(1024)
    # flags = BTRFS_FS_INFO_FLAG_CSUM_INFO: kernels >= 5.5 fill in
    # csum_type/csum_size; older kernels ignore it and leave the zeroes,
    # and zero happens to mean crc32c - their only supported type.
    struct.pack_into('=Q', args, 48, 1)
    fcntl.ioctl(fd, BTRFS_IOC_FS_INFO, args)
    nodesize = struct.unpack_from('=L', args, 32)[0]
    sectorsize = struct.unpack_from('=L', args, 36)[0]
    csum_type = struct.unpack_from('=H', args, 44)[0]
    return sectorsize or 4096, nodesize or 16384, csum_type


def _search(fd, tree_id, objectid, key_type, min_offset, max_offset):
    """Yield (offset, item_data) for all matching keys, in key order."""
    while True:
        key = bytearray(_SEARCH_KEY.pack(
            tree_id, objectid, objectid, min_offset, max_offset,
            0, ULLONG_MAX, key_type, key_type, 65536, 0, 0, 0, 0, 0))
        buf = bytearray(bytes(key) + struct.pack('=Q', _SEARCH_BUF_SIZE) +
                        bytes(_SEARCH_BUF_SIZE))
        fcntl.ioctl(fd, BTRFS_IOC_TREE_SEARCH_V2, buf)
        nr_items = struct.unpack_from('=L', buf, _SEARCH_KEY_NR_ITEMS_OFF)[0]
        if nr_items == 0:
            return
        pos = _SEARCH_ITEMS_OFF
        offset = min_offset
        for _ in range(nr_items):
            _, objid, offset, typ, length = _SEARCH_HEADER.unpack_from(buf, pos)
            pos += _SEARCH_HEADER.size
            yield offset, bytes(buf[pos:pos + length])
            pos += length
        if offset >= max_offset:
            return
        min_offset = offset + 1


def _file_extents(fd, tree, ino):
    """Return [(disk_bytenr, num_bytes), ...] in file order, mirroring
    what the dump-csum patch reads for each EXTENT_DATA item. A hole
    (disk_bytenr 0) is included: like the dump-csum binary, the caller
    stops dumping when it hits one."""
    extents = []
    for _, data in _search(fd, tree, ino, EXTENT_DATA_KEY, 0, ULLONG_MAX):
        ftype = _FILE_EXTENT_HEAD.unpack_from(data, 0)[5]
        if ftype == FILE_EXTENT_INLINE:
            # Inline data is csummed inside its metadata block, not in the
            # csum tree. dduper never asks about files this small.
            continue
        disk_bytenr, _, _, num_bytes = _FILE_EXTENT_DISK.unpack_from(data, 21)
        extents.append((disk_bytenr, num_bytes))
    return extents


def _emit(csums, lines, state):
    """Append csums in dump-csum text format: '%x ' each, newline after
    every 8th csum counted globally across the whole file."""
    for csum in csums:
        state[1] += b'%x ' % csum
        state[0] += 1
        if state[0] % 8 == 0:
            state[1] += b'\n'
            lines.append(bytes(state[1]))
            state[1] = bytearray()


def _ioctl_dump_csum(filename):
    """Dump csums for filename via tree-search ioctls. Returns the same
    list-of-bytes-lines that .readlines() on dump-csum output gives."""
    st = os.stat(filename)
    fd = os.open(filename, os.O_RDONLY)
    try:
        # Per-file: a run may touch more than one mounted filesystem.
        sectorsize, nodesize, csum_type = _fs_info(fd)
        if csum_type != 0:
            raise SystemExit("Error: %s uses csum type %d, but the ioctl "
                             "backend only supports crc32c. Use "
                             "--csum-backend dump-csum." % (filename,
                                                            csum_type))
        # One csum item covers at most ~(nodesize / CSUM_SIZE) * sectorsize
        # bytes of data (16MiB with 16K nodes, 4K sectors). The item holding
        # an extent's first csum can start at most that far before it.
        lookback = (nodesize // CSUM_SIZE) * sectorsize
        tree = _ino_lookup_tree(fd)
        extents = _file_extents(fd, tree, st.st_ino)
        lines = []
        state = [0, bytearray()]  # [csums emitted, current partial line]
        for bytenr, num_bytes in extents:
            pending = num_bytes // sectorsize
            emitting = False
            start = bytenr - lookback if bytenr > lookback else 0
            if bytenr != 0:
                for offset, data in _search(fd, CSUM_TREE_OBJECTID,
                                            EXTENT_CSUM_OBJECTID,
                                            EXTENT_CSUM_KEY,
                                            start, ULLONG_MAX):
                    ncsums = len(data) // CSUM_SIZE
                    if not emitting:
                        if offset + ncsums * sectorsize <= bytenr:
                            continue  # item ends before our extent starts
                        if offset > bytenr:
                            break  # nothing covers this extent's start
                        first = (bytenr - offset) // sectorsize
                        emitting = True
                    else:
                        first = 0
                    take = min(ncsums - first, pending)
                    csums = struct.unpack_from('<%dL' % take, data,
                                               first * CSUM_SIZE)
                    _emit(csums, lines, state)
                    pending -= take
                    if pending == 0:
                        break
            if pending > 0:
                # Hole, preallocated or nodatasum extent: no csums to read.
                # The dump-csum binary aborts the whole dump at this point,
                # leaving a truncated listing - mirror that exactly so both
                # backends hash identically.
                logging.warning("csum_backend: csum tree does not fully "
                                "cover %s, truncating dump", filename)
                break
        if state[1]:
            lines.append(bytes(state[1]))  # final partial line, no newline
        return lines
    finally:
        os.close(fd)


# --- binary (dump-csum) backend --------------------------------------------

def _binary_dump_csum(filename, device):
    """Original backend: patched `btrfs inspect-internal dump-csum`."""
    btrfs_bin = "/usr/sbin/btrfs.static"
    if os.path.exists(btrfs_bin) is False:
        btrfs_bin = "btrfs"
    return subprocess.Popen(
        [btrfs_bin, 'inspect-internal', 'dump-csum', filename, device],
        stdout=subprocess.PIPE,
        close_fds=True).stdout.readlines()


# --- backend selection ------------------------------------------------------

_requested = 'ioctl'
_probed = False


def configure(requested):
    """Set the backend: 'ioctl' (default) or 'dump-csum'."""
    global _requested, _probed
    _requested = requested
    _probed = False


def _probe(filename):
    """Fail fast with a clear message if the ioctl backend can't work
    here, instead of erroring midway through a run."""
    try:
        fd = os.open(filename, os.O_RDONLY)
        try:
            _, _, csum_type = _fs_info(fd)
            if csum_type != 0:
                raise OSError("csum type %d is not crc32c" % csum_type)
            _ino_lookup_tree(fd)
            # One real search proves we have CAP_SYS_ADMIN and a working
            # TREE_SEARCH_V2 (kernel >= 3.14).
            for _ in _search(fd, CSUM_TREE_OBJECTID, EXTENT_CSUM_OBJECTID,
                             EXTENT_CSUM_KEY, 0, ULLONG_MAX):
                break
        finally:
            os.close(fd)
    except OSError as e:
        raise SystemExit("Error: ioctl csum backend unusable (%s). "
                         "Run as root on a mounted btrfs, or use "
                         "--csum-backend dump-csum." % e)


def dump_csum(filename, device):
    """Fetch csums for filename, list of bytes lines (readlines format)."""
    global _probed
    if _requested == 'dump-csum':
        return _binary_dump_csum(filename, device)
    if not _probed:
        _probe(filename)
        _probed = True
        logging.debug("csum_backend: using ioctl backend")
    return _ioctl_dump_csum(filename)
