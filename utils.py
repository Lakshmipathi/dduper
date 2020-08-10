import os
import subprocess

from fcntl import ioctl
from itertools import izip_longest
from stat import *

FICLONERANGE = 0x4020940d
FIDEDUPERANGE = 0xc0189436


def ioctl_ficlonerange(dst_fd, s):

    try:
        ioctl(dst_fd, FICLONERANGE, s)
    except Exception as e:
        print "error({0})".format(e)


def ioctl_fideduperange(src_fd, s):

    try:
        ioctl(src_fd, FIDEDUPERANGE, s)
    except Exception as e:
        print "error({0})".format(e)


def cmp_files(file1, file2):

    md1 = subprocess.Popen(['sha256sum', file1],
                           stdout=subprocess.PIPE,
                           close_fds=True).stdout.read().split(" ")[0]
    md2 = subprocess.Popen(['sha256sum', file2],
                           stdout=subprocess.PIPE,
                           close_fds=True).stdout.read().split(" ")[0]
    if md1 == md2:
        return 0
    else:
        return 1


def btrfs_dump_csum(filename, device_name):

    btrfs_bin = "/usr/sbin/btrfs.static"
    if os.path.exists(btrfs_bin) is False:
        btrfs_bin = "btrfs"

    out = subprocess.Popen(
        [btrfs_bin, 'inspect-internal', 'dump-csum', filename, device_name],
        stdout=subprocess.PIPE,
        close_fds=True).stdout.readlines()
    return out


def creat_reflink(dst_file, bkup_file):

    return subprocess.Popen(['cp', '--reflink=always', dst_file, bkup_file],
                            stdout=subprocess.PIPE)


def validate_results(src_file, dst_file, bkup_file):

    ret = cmp_files(dst_file, bkup_file)
    if ret == 0:
        print "Dedupe validation successful " + src_file + ":" + dst_file
        # Removing temporary backup file path
        os.unlink(bkup_file)
    else:
        msg = "\nFAILURE: Deduplication for " + dst_file + " resulted in corruption." + \
              "You can restore original file from " + bkup_file
        print msg
        with open("/var/log/dduper_backupfile_info.log", "a") as wfd:
            wfd.write(msg)
        # TODO: Remove this file from further op


def validate_file(filename, run_len):
    if os.path.exists(filename) is False:
        return False
    file_stat = os.stat(filename)
    # Verify its a unique regular file
    if (S_ISREG(file_stat.st_mode) and (file_stat.st_size >= 4096)):
        # and (file_stat.st_size >= run_len)):
        return True
    else:
        print "Skipped", filename, "not unique regular files or \
            file size < 4kb "

    return False


# From https://stackoverflow.com/questions/434287
def grouper(iterable, n, fillvalue=None):
    args = [iter(iterable)] * n
    return izip_longest(*args, fillvalue=fillvalue)
