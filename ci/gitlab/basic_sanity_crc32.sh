#!/usr/bin/env bash
#
#
set -x

dduper --help
truncate -s512m /img
mkfs.btrfs -f /img
mount /img /mnt
df 
dd if=/dev/urandom of=/f1 bs=1M count=50
cp /f1 /mnt/f1
cp /f1 /mnt/f2
loop_dev=`/sbin/losetup --find --show /img`
sync
dduper --device ${loop_dev} --dir /mnt --analyze
umount /mnt
poweroff

