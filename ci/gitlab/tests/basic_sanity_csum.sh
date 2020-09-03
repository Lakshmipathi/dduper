#!/usr/bin/env bash
# Verify different csum types
#
set -xe

csum_type=$1

echo "-------setup image-----------------------"
echo "creating 512mb btrfs img"
IMG="/img"
MNT_DIR="/btrfs_mnt"
HOST_DIR="/mnt/"
PASS_FILE="$HOST_DIR/${csum_type}_pass.txt"
deduped=0
rm -rf $PASS_FILE

function setup_fs {
	mkdir -p $MNT_DIR
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
	echo "-------setup files-----------------------"
        if [ $1 == "random" ]; then
		echo "Creating 50mb test file"
		dd if=/dev/urandom of=/tmp/f1 bs=1M count=50

		echo "Coping to mount point"
		cp -v /tmp/f1 $MNT_DIR/f1
		cp -v /tmp/f1 $MNT_DIR/f2
 
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
        echo -n "$deduped" > /tmp/deduped
}


function verify_results {
        deduped=$(cat /tmp/deduped)
        f1=$1
        f2=$2
        v=$3
        
	if [ $deduped -eq $v ];then
		echo "dduper verification passed"
                echo "f1:$f1 f2:$f2 v:$v"
		echo "dduper verification passed" > $PASS_FILE
	else
		echo "dduper verification failed"
                echo "f1:$f1 f2:$f2 v:$v"
                rm -rf $PASS_FILE
                shutdown
	fi

}

function cleanup {
	umount $MNT_DIR
}

function shutdown {
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
test_dduper "fn_abacad_1" "fn_xbyczd_1" "3"
test_dduper "fn_abcdef_1" "fn_xyzijkdef_1" "3"
test_dduper "fn_abcdab_2" "fn_ijxyabc_6" "18"
shutdown
