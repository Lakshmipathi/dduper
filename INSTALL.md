How to install dduper?
---------------------

`dduper` relies on BTRFS checksums. To expose these checksums to userspace you need to apply additional patch on btrfs-progs first.
This introduces a new command to dump csum using `btrfs inspect-internal dump-csum`.

You can either download the patch from here https://patchwork.kernel.org/patch/10540229 or if you are using latest btrfs-progs you
can get it from patch/btrfs-progs-v5.6.1/ on this repo.

Steps should be similar to:

1. git clone https://github.com/kdave/btrfs-progs.git
2. Download the patch 
3. Apply the patch like `patch -p1 < btrfs-inspect-internal-dump-csum.patch`
4. Now compile the brtfs-progs. 
5. After successful compilation, you should see following `dump-csum` option.

```
	./btrfs inspect-internal dump-csum --help
	usage: btrfs inspect-internal dump-csum <path/to/file> <device>

	    Get csums for the given file.
```

Misc:
----
If you interested in dumping csum data, please check this demo: https://asciinema.org/a/34565

Original mailing-list annoucement: https://www.mail-archive.com/linux-btrfs@vger.kernel.org/msg79853.html

