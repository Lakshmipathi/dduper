How to install dduper?
---------------------

Install from Source:
--------------------
`dduper` relies on BTRFS checksums. To expose these checksums to userspace you need to apply additional patch on btrfs-progs first.
This introduces a new command to dump csum using `btrfs inspect-internal dump-csum`.

If you are using latest btrfs-progs you can get it from this repo `patch/btrfs-progs-v5.6.1/`. Optionally, you can also download 
the patch from here https://patchwork.kernel.org/patch/10540229

Steps should be similar to:

1. git clone https://github.com/Lakshmipathi/dduper.git && cd dduper
2. git clone https://github.com/kdave/btrfs-progs.git && cd btrfs-progs
3. Apply the patch like `patch -p1 < ../patch/btrfs-progs-v5.6.1/0001-Print-csum-for-a-given-file-on-stdout.patch`
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

Install using Docker :
----------------------

If you are already using docker and don't want to install via source. Then simply pull the `laks/dduper` image and
pass your device and mount dir like:

```
$ docker run -it --device /dev/sda1 -v /btrfs_mnt:/mnt laks/dduper dduper --device /dev/sda1 --dir /mnt --analyze
```

Make sure to replace `/dev/sda1` with your btrfs device and `/btrfs_mnt` with btrfs mount point.

Misc:
----
If you interested in dumping csum data, please check this demo: https://asciinema.org/a/34565

Original mailing-list announcement: https://www.mail-archive.com/linux-btrfs@vger.kernel.org/msg79853.html

