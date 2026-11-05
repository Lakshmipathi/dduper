This file documents how to use test.py and how performance numbers calculated.

csum backend test:
------------------
`test_csum_backend.sh` verifies the ioctl csum backend (csum_backend.py):
byte-parity against the patched dump-csum binary and an end-to-end dedupe
on a subvolume without patched btrfs-progs. It creates a 1GB loopback
btrfs under /tmp and cleans up after itself.

```
sudo [PYTHON=/path/to/python3] [DUMP_CSUM_BIN=/path/to/patched/btrfs] \
    bash tests/test_csum_backend.sh
```



```
mkfs.btrfs /dev/xvdc
mount /dev/xvdc /mnt
python test.py -d /mnt
```

Above should create test data on /mnt. This will create files in specific layout format
`fn_<file_datalayout>_<segment_size>_<total_file_size>`. For example, "fn_abcd_1m_4m"
refers to 4mb file with 1mb of a,b,c and d. i.e first 1mb is `a`, second 1mb is `b`,
third 1mb is `c` and fourth `1mb` is d.

Test run results:
----------------

All three mode saved same amount of data. Original data:

```
/dev/xvdc      104857600 16626016  87204352  17% /mnt
```


After dedupe:

```
/dev/xvdc      104857600   26880 103787776   1% /mnt
```


1. Running in default mode:

```
dduper --device /dev/xvdc --dir /mnt --chunk-size 1024

dduper took 2245.63775706 seconds
```

2. Running in fast mode:

```
dduper --fast-mode --device /dev/xvdc --dir /mnt --chunk-size 1024

dduper took 265.656284094 seconds
```

3. Running insane mode.

```
dduper --fast-mode --skip --device /dev/xvdc --dir /mnt --chunk-size 1024 --recurse

dduper took 3.16962099075 seconds
```

