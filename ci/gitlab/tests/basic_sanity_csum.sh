#!/usr/bin/env bash
# Verify different csum types
#
set -e

csum_type=$1

echo "creating 512mb btrfs img"
IMG="/img"
MNT_DIR="/btrfs_mnt"
HOST_DIR="/mnt/"
PASS_FILE="$HOST_DIR/${csum_type}_pass.txt"
deduped=0
rm -rf $PASS_FILE

function setup_fs {
        echo "-----------------------------------------------------------setup image-----------------------"
	mkdir -p $MNT_DIR
        rm -rf $IMG
	truncate -s512m $IMG

	if [ $csum_type == "crc32" ];
	then
	mkfs.btrfs -f $IMG
	else
	mkfs.btrfs -f $IMG --csum $csum_type
	fi

	echo "-------mount image-----------------------"
	echo "mounting it under $MNT_DIR"
	mount $IMG $MNT_DIR
}

function setup_data {
	echo "----------------------------------------------------------setup files-----------------------"
        if [ $1 == "random" ]; then
		echo "Creating 50mb test file"
		dd if=/dev/urandom of=/tmp/f1 bs=1M count=50

		echo "Coping to mount point"
		cp -v /tmp/f1 $MNT_DIR/f1
		cp -v /tmp/f1 $MNT_DIR/random
 
        else
        	python  /mnt/ci/gitlab/tests/dataset.py -d $MNT_DIR -l $1 $2
        fi
        sleep 2
        ls -l $MNT_DIR
        sync
}


function start_dedupe {
	loop_dev=$(/sbin/losetup --find --show $IMG)
	sync

	echo "--------------------------------------------------------dduper run-----------------------"
	echo "Running dduper --dry-run"
	dduper --device ${loop_dev} --dir $MNT_DIR --dry-run

	echo "Running dduper in default mode"
	dduper --device ${loop_dev} --dir $MNT_DIR

	sync
	sleep 5
}


function verify_results {
        echo "------------------------------------------------Verifying results-----------------------"
        f1=$1
        f2=$2
        v=$3
        btrfs fi du ${MNT_DIR}/$f2* | tee /tmp/du.log
        cat /tmp/du.log
        content=$(tail -n1 /tmp/du.log)
        echo $content | awk '{print $(NF-1)}'
        deduped=$(echo $content | awk '{print $(NF-1)}' )
        echo "deduped: $deduped"
	if [ "${deduped}" == "${v}.00MiB" ];then
		echo "dduper verification passed"
                echo "f1:$f1 f2:$f2 v:$v"
		echo "dduper verification passed" > $PASS_FILE
	else
		echo "dduper verification failed"
                echo "f1:$f1 f2:$f2 v:$v"
                rm -rf $PASS_FILE
                abort_test
	fi

}

function cleanup {
	umount $MNT_DIR
}

function abort_test {
        echo "Abort further tests"
        sleep 10
	poweroff
}

function test_dduper {
        f1=$1
        f2=$2
        v=$3
	setup_fs
	setup_data  $f1 $f2
	start_dedupe 
	verify_results $f1 $2 $v
	cleanup
}

test_dduper "random" "random" "50" 
test_dduper "fn_a_1" "fn_aaaa_1" "4"
test_dduper "fn_a_1" "fn_aaaaaaaa_1" "8"
test_dduper "fn_abacad_1" "fn_xbyczd_2" "6"
test_dduper "fn_abcdef_1" "fn_xyzijkdef_2" "6"
test_dduper "fn_abcdab_2" "fn_ijxyabc_6" "18"
echo "All tests completed."
shutdown
