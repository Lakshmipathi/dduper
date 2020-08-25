#!/bin/bash
#
# Build dduper with btrfs-progs patch and install it.
# If btrfs already present, then execute even test script via test_cmd.
#
set -x

BTRFS_BIN="btrfs"
MNT_DIR="/mnt/"
BUILD_DIR="/btrfs/"
test_cmd=$(cat ${MNT_DIR}/cmd)

${BTRFS_BIN} --version

if [ $? -ne 0 ]
then
	echo "=========================== Build btrfs-progs ================"
	cd /btrfs-progs
	patch -p1 < /dduper/patch/btrfs-progs-v5.6.1/0001-Print-csum-for-a-given-file-on-stdout.patch
	./autogen.sh && ./configure --disable-documentation --disable-backtrace && make -j`nproc` && make install
	echo "=================  Install dduper =========================="
	cp -v /dduper/dduper /usr/sbin/
	/usr/sbin/dduper --help
	poweroff
else
    echo "================= Running dduper Tests  ================================="
    cd /mnt && ${test_cmd}
    poweroff
fi
