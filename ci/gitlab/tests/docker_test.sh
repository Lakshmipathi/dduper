#!/bin/bash
set -x

echo "This will create 512mb under /tmp and validates dduper behaviour."

echo "-------setup image-----------------------"
echo "creating 512mb btrfs img"
IMG="/img"
MNT_DIR="/btrfs_mnt"

loop_dev=$(losetup -f)
mknod -m640 $loop_dev b 7 0
ls -l /dev/loop*

mkdir -p $MNT_DIR
truncate -s512m $IMG
mkfs.btrfs -f $IMG

echo "-------mount image-----------------------"
losetup $loop_dev $IMG

echo "mounting it under $MNT_DIR"
mount  $loop_dev $MNT_DIR


echo "-------setup files-----------------------"
echo "Creating 50mb test file"
dd if=/dev/urandom of=/tmp/f1 bs=1M count=50

echo "Coping to mount point"
cp -v /tmp/f1 $MNT_DIR/f1
cp -v /tmp/f1 $MNT_DIR/f2
#loop_dev=$(/sbin/losetup --find --show $IMG)
sync

used_space2=$(df --output=used -h -m $MNT_DIR | tail -1 | tr -d ' ')

echo "-------dduper verification-----------------------"
echo "Running simple dduper --dry-run"
dduper --device ${loop_dev} --dir $MNT_DIR --dry-run

echo "Running simple dduper in default mode"
dduper --device ${loop_dev} --dir $MNT_DIR

sync
sleep 5
used_space3=$(df --output=used -h -m $MNT_DIR | tail -1 | tr -d ' ')

echo "-------results summary-----------------------"
echo "disk usage before de-dupe: $used_space2 MB"
echo "disk usage after de-dupe: $used_space3 MB"

deduped=$(expr $used_space2 - $used_space3)

if [ $deduped -eq 50 ];then
echo "dduper verification passed"
else
echo "dduper verification failed"
fi

umount $MNT_DIR
