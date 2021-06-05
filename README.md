dduper 
------

---
Please be aware that dduper is largely un-tesed tool. Validate it, before running it on critical data.
---

dduper is a block-level [out-of-band](https://btrfs.wiki.kernel.org/index.php/Deduplication#Out_of_band_.2F_batch_deduplication) BTRFS dedupe tool. This works by
fetching built-in checksum from BTRFS csum-tree, instead of reading file blocks
and computing checksum itself. This *hugely* improves the performance.

Dedupe Files (default mode):
----------------------------

To dedupe two files f1 and f2 on partition sda1:

`dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2`

This mode is 100% safe, as it uses the `fideduperange` call, which asks the kernel 
to verify given regions byte-by-byte, and only perform dedupe when they match.

Dedupe Files Faster (fast mode):
--------------------------------

dduper also has `--fast-mode` option, which tells kernel to skip verifying
stage and invoke clone directly. This mode is faster since file contents
are never read. dduper relies on file csum maintained by btrfs csum-tree.

To dedupe two files f1 and f2 on partition sda1 in faster mode:

`dduper --fast-mode --device /dev/sda1 --files /mnt/f1 /mnt/f2`

This works by fetching csums and invokes `ficlonerange` on matching regions.
For this mode, dduper adds safety check by performing sha256 comparison.
If validation fails, files can be restored using `/var/log/dduper_backupfile_info.log`.
This file will contain data like:

`
FAILURE: Deduplication for /mnt/foo resulted in corruption.You can restore original file from /mnt/foo.__dduper
`

*Caution: Don't run this, if you don't know what you are doing.*

Dedupe Files blazing fast (insane mode):
----------------------------------------

If you already have backup data in another partition or systems. You can
tell dduper to skip file sha256 validation after dedupe (file contents never read).
This is insanely fast :-)

`dduper --fast-mode --skip --device /dev/sda1 --files /mnt/f1 /mnt/f2`

*Caution: Never run this, if you don't know what you are doing.*

Dedupe multiple files:
----------------------

To dedupe more than two files on a partition (sda1), you simply pass
those filenames like:

`dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 /mnt/f3 /mnt/f4`

Dedupe Directory:
-----------------

To dedupe entire directory on sda1:

`dduper --device /dev/sda1 --dir /mnt/dir`

Dedupe Directory recursively:
-----------------------------

To dedupe entire directory also parse its sub-directories on sda1:

`dduper --device /dev/sda1 --dir /mnt/dir --recurse `

Dedupe multiple directories:
---------------------------

To dedupe multiple directories on sda1:

`dduper --device /dev/sda1 --dir /mnt/dir1 /mnt/dir2`

Analyze with different chunk size:
----------------------------------
You can analyze which chunk size provides better deduplication.

`dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --analyze`

It will perform analysis and report dedupe data for different chunk values.

Sample output: f1 and f2 are 4MB files.

```
--------------------------------------------------
 Chunk Size(KB) :      Files      : Duplicate(KB) 
--------------------------------------------------
      256       : /mnt/f1:/mnt/f2 :     4096      
==================================================
dduper:4096KB of duplicate data found with chunk size:256KB 


--------------------------------------------------
 Chunk Size(KB) :      Files      : Duplicate(KB) 
--------------------------------------------------
      512       : /mnt/f1:/mnt/f2 :     4096      
==================================================
dduper:4096KB of duplicate data found with chunk size:512KB 


--------------------------------------------------
 Chunk Size(KB) :      Files      : Duplicate(KB) 
--------------------------------------------------
      1024      : /mnt/f1:/mnt/f2 :     4096      
==================================================
dduper:4096KB of duplicate data found with chunk size:1024KB 


--------------------------------------------------
 Chunk Size(KB) :      Files      : Duplicate(KB) 
--------------------------------------------------
      2048      : /mnt/f1:/mnt/f2 :       0       
==================================================
dduper:0KB of duplicate data found with chunk size:2048KB 


--------------------------------------------------
 Chunk Size(KB) :      Files      : Duplicate(KB) 
--------------------------------------------------
      4096      : /mnt/f1:/mnt/f2 :       0       
==================================================
dduper:0KB of duplicate data found with chunk size:4096KB 


--------------------------------------------------
 Chunk Size(KB) :      Files      : Duplicate(KB) 
--------------------------------------------------
      8192      : /mnt/f1:/mnt/f2 :       0       
==================================================
dduper:0KB of duplicate data found with chunk size:8192KB 

dduper took 0.149248838425 seconds
```

Above output shows, whole 4MB file (f2) can be deduped with chunk size 256KB, 512KB or 1MB.
With larger chunk size 2MB, 4MB and 8MB, dduper unable to detect deduplicate data. In this
case, its wise to use 1MB as chunk size while performing dedupe, because it invoke less
dedupe calls compared to 256KB/512KB chunk size.

You can analyze more than two files like,

`dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 /mnt/file3 --analyze`

or directory and its sub-directories using

`dduper --device /dev/sda1 --dir /mnt --recurse --analyze`

Changing dedupe chunk size:
---------------------------

By default, dduper uses 128KB chunk size. This can be modified using chunk-size
option. Below usage shows chunk size with 1MB

`dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --chunk-size 1024`

Display stats:
-------------

To perform dry-run to display details without performing dedupe:

`dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --dry-run`

Also check `--analyze` option for detailed data.

List duplicate files:
---------------------

To list duplicate files from a directory:

`dduper --device /dev/sda1 --dir /mnt --recurse --perfect-match-only`


Known Issues:
------------

- dduper supports ~~only~~ crc32. ~~Doesn't work with csum types like xxhash,blake2, sha256.~~
  Now Initial support available for xxhash64, blake2 and sha256.

- subvolume won't work with dduper.

- Cannot yet de-duplicate identical content blocks within a single file

- Please be aware that dduper is largely un-tesed tool. Validate it, before running it on critical data.

Reporting bugs:
--------------

To report issues please use

- [github issue track](https://github.com/lakshmipathi/dduper/issues)
