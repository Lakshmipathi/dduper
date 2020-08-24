#!/usr/bin/env bash
#
# Use debian image and setup repos.
set -x

ci_branch=$1

apt-get update
apt-get -y install python3-pip git

# Setup rootfs
IMG="/repo/qemu-image.img"
DIR="/target"
mkdir -p $DIR
for i in {0..7};do
mknod -m 0660 "/dev/loop$i" b 7 "$i"
done

# mount the image file
mount -o loop $IMG $DIR

# Pull latest code
rm -rf $DIR/dduper
rm -rf $DIR/btrfs-progs

git clone -b $ci_branch https://github.com/Lakshmipathi/dduper.git $DIR/dduper
git clone https://github.com/kdave/btrfs-progs.git $DIR/btrfs-progs 

pip3 install --target=$DIR/usr/lib/python3/dist-packages/ -r $DIR/dduper/requirements.txt

cd /
umount $DIR
rmdir $DIR

