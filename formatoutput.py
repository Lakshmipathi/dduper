from beautifultable import BeautifulTable


def prettyprint(analyze_dict):
    for k, v in analyze_dict.iteritems():
        total_sz = 0
        table = BeautifulTable()
        table.column_headers = ["Chunk Size(KB)", "Files", "Duplicate(KB)"]
        table.set_style(BeautifulTable.STYLE_NONE)
        table.column_separator_char = ':'
        table.top_border_char = '-'
        table.header_separator_char = '-'
        table.bottom_border_char = '='
        for v1 in v:
            table_row = []
            table_row.append(k)
            f, z = v1
            total_sz += z
            table_row.append(str(f))
            table_row.append(z)
            table.append_row(table_row)
        print(table)
        print("dduper:%sKB of duplicate data found with chunk size:%dKB \n\n" %
              (total_sz, k))
