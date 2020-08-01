import argparse

f1m = 1
f10m = 10
f100m = 100
f512m = 512
mb = 1024 * 1024
layout = ["abcd", "aaaa", "abac", "abcdabcd",'cdcdcd']
seg_size = [f1m, f10m, f100m, f512m]


def file_layout(filename, layout, seg_size):
    print("filename:%s layout:%s seg_size:%s file_size:%s" %
          (filename, layout, seg_size, len(layout) * seg_size))

    with open(filename, "w") as fd:
        for c in layout:
            content = c * (seg_size * mb)
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
                    lt, sz)
