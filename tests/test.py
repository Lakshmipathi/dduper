f1k = 1024
f10k = 1024 * 10
f1m = 1024 * 1024
f10m = 1024 * 1024 * 10
f100m = 1024 * 1024 * 100


def file_layout(filename, layout, seg_size):
    print("filename:%s layout:%s seg_size:%s file_size:%s" %
          (filename, layout, seg_size, len(layout) * seg_size))
    with open(filename, "w") as fd:
        for c in layout:
            content = c * seg_size
            fd.write(content)


layout = ["abcd", "aaaa", "abac", "abcdabcd"]
seg_size = [f1k, f10k, f1m, f10m]
print('*' * 100)
print(
    "\t\t\t *** Files format: fn_<file_datalayout>_<segement_size>_<total_file_size> ***"
)
print('*' * 100)
for sz in seg_size:
    for lt in layout:
        file_layout("fn_" + str(lt) + "_" + str(sz) + "_" + str(len(lt) * sz),
                    lt, sz)
