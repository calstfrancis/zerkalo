import sys, os
out=["= Test Document\n"]
for i in range(60):
    extra = " Teh writter will recieve it. " if i % 3 == 0 else " "
    out.append(f"Paragraph {i}."+extra+"The quick brown fox jumps over the lazy dog and keeps running through the field. "*3+"\n")
    if i % 7 == 2:
        out.append("\n".join(f"- list item {i}.{k} that is long enough to wrap onto a second line in the editor view for sure" for k in range(4))+"\n")
    if i % 9 == 4:
        out.append("// A comment block about this section\n// spanning two lines\n")
    if i % 10 == 5:
        out.append(f"== Section {i}\n")
open(sys.argv[1],"w").write("\n".join(out))
