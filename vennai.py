from matplotlib_venn import venn2, venn2_circles
import matplotlib.pyplot as plt
import os, pdb, time

def visualize(src_dict, dst_dict, src_file, dst_file, perfect_match, chunk_size, avail_dedupe, unmatched_chunks, src_file_sz, dst_file_sz):
	src_dict_keys = [v for v in src_dict.keys()]
	dst_dict_keys = [v for v in dst_dict.keys()]

	total = len(set(dst_dict_keys).union(set(src_dict_keys)))

	f1_keys = set(src_dict_keys)
	f2_keys = set(dst_dict_keys)
	f1 = os.path.basename(src_file)
	f2 = os.path.basename(dst_file)

	plt.figure()
	ax = plt.gca()
	v = venn2([f1_keys, f2_keys],
	      set_labels = ("", ""), ax = ax)
#	      set_colors=('orange', 'darkgrey'), alpha = 0.8, ax = ax)
#	      set_labels = (f1 +" ", f2),
#	      subset_label_formatter=lambda x: f"{(x/total):1.0%}")

	v.get_label_by_id('01').set_text('')
	v.get_label_by_id('10').set_text('')

        # setup color
	v.get_patch_by_id('01').set_color('red')
	v.get_patch_by_id('10').set_color('green')

        ##############
	src_fz_kb = src_file_sz >> 10
	dst_fz_kb = dst_file_sz >> 10
	h, l = [],[]
	h.append(v.get_patch_by_id('01'))
	l.append("target:" + f2 + "(" + str(dst_fz_kb) + "KB)")
	h.append(v.get_patch_by_id('10'))
	l.append("source: " + f1 + "("+ str(src_fz_kb) + " KB)")

        ## Add only if dup avail
	if v.get_label_by_id('11') is not None:
		h.append(v.get_patch_by_id('11'))
		v.get_patch_by_id('11').set_color('blue')
		l.append("dup: (" + str(avail_dedupe) + "KB)")

	#create legend from handles and labels    
	title = "chunk size: " + str(chunk_size)+"KB"
	ax.legend(handles=h, labels=l, title=title)
        ##############
        # print shared if available
	if perfect_match != 1 and v.get_label_by_id('11') is not None:
		v.get_label_by_id('11').set_text(str(avail_dedupe)+"KB")
		v.get_patch_by_id('11').set_color('blue')
		v.get_patch_by_id('11').set_edgecolor('none')
		v.get_patch_by_id('11').set_alpha(0.4)
	elif perfect_match == 1:
		v.get_label_by_id('11').set_text("matched: "+str(avail_dedupe)+"KB")
		v.get_patch_by_id('11').set_color('blue')
                # always show target in background
		v.get_patch_by_id('10').set_color('red')

	# plt.show()
	print("Saving images..")
	plt.title("dduper visual analyzer") 
	plt.savefig(f1+"_"+f2+str(chunk_size))
	plt.clf()
	plt.cla()
	plt.close()
	# Merge files $ convert +append *.png out.png 
