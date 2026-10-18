How to install dduper?
---------------------

You can install dduper in 3 different ways.

1. Using pre-built binary which requires couple of python packages.
2. Using Docker image.
3. Using Source code.

All three approach is described below.

Install pre-built binaries:
---------------------------

To Install `dduper` binaries, execute following commands:

```
        git clone https://github.com/Lakshmipathi/dduper.git && cd dduper
        pip3 install -r requirements.txt
        cp -v bin/btrfs.static /usr/sbin/     # this copies required btrfs binary.
        cp -v dduper /usr/sbin/               # copy dduper script.
```

That's all. Now type `dduper --help` to list options and continue with README.md for usage.

Note: If you want to perform basic check you can use this [script](https://github.com/Lakshmipathi/dduper/blob/master/tests/verify.sh)

Install using Docker :
----------------------

If you are already using docker and don't want to install any dependencies. Then simply pull the `laks/dduper` image and
pass your device and mount dir like:

```
$ docker run -it --device /dev/sda1 -v /btrfs_mnt:/mnt laks/dduper dduper --device /dev/sda1 --dir /mnt --analyze
```

Make sure to replace `/dev/sda1` with your btrfs device and `/btrfs_mnt` with btrfs mount point.


Install from Source:
--------------------
`dduper` relies on BTRFS checksums. To expose these checksums to userspace you need to apply additional patch on btrfs-progs first.
This introduces a new command to dump csum using `btrfs inspect-internal dump-csum`.

If you are using latest btrfs-progs use `patch/btrfs-progs-v6.11/0001-Print-csum-for-a-given-file-on-stdout-with-subvolume.patch`.
It supports files inside subvolumes and applies cleanly on btrfs-progs v6.11 through v6.15.

Note: the older `0001-Print-csum-for-a-given-file-on-stdout.patch` files (including the one in the same
`patch/btrfs-progs-v6.11/` directory) do not support subvolumes: for a file inside a subvolume, `dump-csum`
returns empty output or another file's checksums. Use them only for older btrfs-progs releases on
filesystems without subvolumes.

Steps should be similar to:

1. git clone https://github.com/Lakshmipathi/dduper.git && cd dduper
2. git clone https://github.com/kdave/btrfs-progs.git && cd btrfs-progs
3. Apply the patch like `patch -p1 < ../patch/btrfs-progs-v6.11/0001-Print-csum-for-a-given-file-on-stdout-with-subvolume.patch`
4. Now compile and install btrfs-progs.
5. After successful compilation, you should see following `dump-csum` option.

```
	./btrfs inspect-internal dump-csum --help
	usage: btrfs inspect-internal dump-csum <path/to/file> <device>

	    Get csums for the given file.
```
6. Now we have required patch. Go install dduper.
```
	cd ~/dduper
	pip install -r requirements.txt
	cp -v dduper /usr/sbin/
```

7. Type `dduper --help` to list options and continue with README.md for usage.

Misc:
----
If you interested in dumping csum data, please check this demo: https://asciinema.org/a/34565

Original mailing-list announcement: https://www.mail-archive.com/linux-btrfs@vger.kernel.org/msg79853.html
Older patch: https://patchwork.kernel.org/patch/10540229
