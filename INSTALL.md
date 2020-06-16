How to install dduper?
---------------------

`dduper` relies on BTRFS checksums. To expose these checksums to userspace you need to apply this patch https://patchwork.kernel.org/patch/10540229/ on btrfs-progs first.
This introduces a new command to dump csum using `btrfs inspect-internal dump-csum`.

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


