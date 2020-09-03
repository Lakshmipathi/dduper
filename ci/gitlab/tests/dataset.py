import argparse
import sys

mb = 1024 * 1024

'''
Same file:
dataset.py aaaa_1  aaaa_1 => 4mb dup

100% dup:
dataset.py a_1 aaaaaaaa_1 => 8mb 

50% dup:
dataset.py abcd_1 abxy_1 => 2mb

others:
dataset.py abcd_1 bbxy_1 => 2mb
dataset.py abcd_1 cdba_1 => 4mb

chunk_size 1m:
dataset.py abcdwxyz_1 wxyzabcd_1 => 8mb

backup:
dataset.py abcd_1 abcdefg_1 => 4mb 

'''

def file_layout(filename, layout, seg_size):
    filename = filename +"_" + str(len(layout) * seg_size) + "mb"
    with open(filename, "w") as fd:
        for c in layout:
            content = c * (seg_size * mb)
            fd.write(content)
    print(filename)


def validate_lfile(lfile,dir_path):
    for lf in lfile:
        s1 = lf.split("_")
        if len(s1) != 3:
           print("Error: fn_<datalayout>_<segment_size> required")
           sys.exit(0)
        (lout,lseg_sz)=s1[1],int(s1[2])
        file_layout(dir_path +"/" + lf, lout, lseg_sz)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()

    parser.add_argument('-d',
                        '--dir_path',
                        action='store',
                        dest='dir_path',
                        type=str,
                        help='BTRFS dir (ex: /mnt/playground) ',
                        required=True)
    parser.add_argument('-l',
                        '--layout',
                        action='store',
                        dest='lfile',
                        nargs='+',
                        help='Layout of file fn_<datalayout>_<segment_size>',
                        type=str,
                        required=True)

    results = parser.parse_args()
    print("fn_<file_datalayout>_<segment_size>_<total_file_size>")
    validate_lfile(results.lfile,results.dir_path)

