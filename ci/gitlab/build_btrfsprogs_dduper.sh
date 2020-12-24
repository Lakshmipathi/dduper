#!/bin/bash
#
# Build dduper with btrfs-progs patch and install it.
# If btrfs already present, then execute even test script via test_cmd.
#
##############################################################################
# NOTE:  If you make changes to this script, make sure to rebuild debian image.
###############################################################################
set -x

BTRFS_BIN="btrfs"
MNT_DIR="/mnt"
BUILD_DIR="/btrfs-progs"
test_cmd=$(cat ${MNT_DIR}/cmd)

if [ ${test_cmd} == "build_with_patch" ]
then
	echo "=========================== Build btrfs-progs ================"
	cd $BUILD_DIR/
        ls -l /dduper $BUILD_DIR
        cat $BUILD_DIR/cmds/inspect-dump-csum.c
	patch -p1 < /dduper/patch/btrfs-progs-v5.9/0001-Print-csum-for-a-given-file-on-stdout.patch
	./autogen.sh && ./configure --disable-documentation --disable-backtrace && make -j`nproc` && make install && touch "${MNT_DIR}/build_pass.txt"
	echo "=================  Install dduper =========================="
	cp -v /dduper/dduper /usr/sbin/
	/usr/sbin/dduper --help
	poweroff
else
    echo "================= Running dduper Tests  ================================="
    cd /mnt && ${test_cmd}
    poweroff
fi
