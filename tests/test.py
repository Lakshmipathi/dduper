import argparse

f1k = 1
f4k = 4
f8k = 8
f16k = 16
f32k = 32
f64k = 64
f128k = 128
f256k = 256
f512k = 512

f1m = 1
f10m = 10
f100m = 100
f512m = 512
kb = 1024
mb = 1024 * 1024
layout = ["abcd", "aaaa", "abac", "abcdabcd", "cdcdcd", "xyz123xy", "zzz", "123","abcdx"]
# seg_size = [f512k, f1m, f10m, f100m, f512m]
seg_size = [f1m]
seg_size_kb = [f1k, f4k, f16k, f32k, f64k, f128k, f256k, f512k]


def file_layout(filename, layout, seg_size, kb_mb):
    print("filename:%s layout:%s seg_size:%s file_size:%s" %
          (filename, layout, seg_size, len(layout) * seg_size))

    with open(filename, "w") as fd:
        for c in layout:
            content = c * (seg_size * kb_mb)
            fd.write(content)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()

    parser.add_argument('-d',
                        '--dir_path',
                        action='store',
                        dest='dir_path',
                        type=str,
                        help='BTRFS dir (ex: /mnt/playground) ',
                        required=True)

    results = parser.parse_args()
    print(results.dir_path)
    print('*' * 100)
    print(
    "\t\t\t *** Files format: fn_<file_datalayout>_<segment_size>_<total_file_size> ***"
    )
    print('*' * 100)
    for sz in seg_size:
        for lt in layout:
             file_layout(results.dir_path+"/fn_" + str(lt) + "_" + str(sz) + "m_" + str(len(lt) * sz) +"m",
                    lt, sz, mb)

    for sz in seg_size_kb:
        for lt in layout:
             file_layout(results.dir_path+"/fn_" + str(lt) + "_" + str(sz) + "kb_" + str(len(lt) * sz) +"kb",
                    lt, sz, kb)
