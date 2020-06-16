dduper
------

dduper is a offline block-level dedupe tool for BTRFS. This works by fetching
in-built csum from BTRFS csum-tree, Instead of reading whole file blocks and
computing checksum. This hugely improves the performance.

Dedupe Files:
-------------

To dedupe two files f1 and f2 on partition sda1:

`python dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2`

Dedupe Files Faster:
--------------------

By default dduper uses `fideduperange` call and asks kernel to verify
given regions are same or not then perform dedupe.

dduper has `--fast-mode` option, which tells kernel to skip verifying
stage and invoke clone directly.

To dedupe two files f1 and f2 on partition sda1 in faster/unsafe mode:

`python dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --fast-mode`

Dedupe multiple files:
----------------------

To dedupe more than two files on a partition (sda1), you simply pass
those filenames like:

`python dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 /mnt/f3 /mnt/f4`

Dedupe Directory:
-----------------

To dedupe entire directory on sda1:

`python dduper --device /dev/sda1 --dir /mnt/dir`

Dedupe Directory recursively:
-----------------------------

To dedupe entire directory also parse its sub-directories on sda1:

`python dduper --device /dev/sda1 --dir /mnt/dir --recurse `

Changing dedupe chunk size:
---------------------------

By default, we use 32KB chunk size. This can be modified using chunk-size
option. Below usage shows chunk size with 1MB

`python dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --chunk-size 1024`

Display information:
--------------------

To perform dry-run to display details without performing dedupe:

`python dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --dry-run `

Skip validation:
----------------

To skip file validation after dedupe (file contents never read):

`python dduper --device /dev/sda1 --files /mnt/f1 /mnt/f2 --skip `

