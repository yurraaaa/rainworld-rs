import re, struct, sys, zlib
src, out = sys.argv[1], sys.argv[2]
keep = set(int(x, 16) for x in sys.argv[3].split(',')) if len(sys.argv) > 3 else None
text = open(src).read()
arrays = {}
for m in re.finditer(r'static const uint8_t vcb_([0-9a-f]{8})\[\] = \{(.*?)\};', text, re.S):
    arrays[m.group(1)] = bytes(int(b, 16) for b in re.findall(r'0x([0-9a-f]{2})', m.group(2)))
lst = re.findall(r'\{0x([0-9a-f]{8}),0x([0-9a-f]+),vcb_([0-9a-f]{8})\}', text)
entries = []
for crc, size, name in lst:
    data = arrays[name]
    assert len(data) == int(size, 16), name
    c = int(crc, 16)
    if keep is None or c in keep:
        entries.append((c, data))
print(len(arrays), len(lst), len(entries), file=sys.stderr)
bad = [hex(c) for c, d in entries if zlib.crc32(d) != c]
print("crc mismatches:", len(bad), bad[:5], file=sys.stderr)
entries.sort()
with open(out, 'wb') as f:
    f.write(struct.pack('<I', len(entries)))
    for c, d in entries:
        f.write(struct.pack('<II', c, len(d))); f.write(d)
