#!/bin/sh
# Surveys which bytes of a game's files Barrel's parsers never use (`barrel-tool unread`), over
# a sample of the files in its PAK archives (Sophie 2, the Ryza games), by kind of file.
#   tools/unread-survey.sh DATA_DIR WORK_DIR [containers, default 60] [loose files a kind, default 8]
# DATA_DIR holds the .PAK files; WORK_DIR (emptied first; keep it off a RAM disk) gets the
# extracted sample and `report.txt`, one line a file. Prints a table by extension: files,
# bytes, unused bytes and share, and the file with the most unused. Containers
# (.elixir.gz) are taken evenly from those under 300 KB and their entries surveyed one by one.
tool=${BARREL_TOOL:-target/release/barrel-tool}
data=$1 work=$2 containers=${3:-60} loose=${4:-8}
[ -d "$data" ] && [ -n "$work" ] || { sed -n '2,9p' "$0"; exit 1; }
rm -rf "${work:?}"; mkdir -p "$work/files"
for p in "$data"/*.PAK; do "$tool" pak-ls "$p" 2>/dev/null | sed "s|^|$p\t|"; done > "$work/entries.txt"
# evenly spaced picks of each kind: `pick PATTERN COUNT`
pick() {
  grep -i "$1" "$work/entries.txt" | awk -F'\t' '{n=split($2,a," "); if (a[1]+0 < 300000) print}' > "$work/kind.txt"
  total=$(wc -l < "$work/kind.txt")
  [ "$total" -gt 0 ] || return
  awk -v t="$total" -v c="$2" 'BEGIN{s=t/c; if (s<1) s=1} int((NR-1)/s)!=int((NR-2)/s) || NR==1' "$work/kind.txt" | head -n "$2"
}
{ pick '\.elixir\.gz$' "$containers"; for k in '\.g1t$' '\.xml$' '\.ebd$' '\.xml\.e$'; do pick "$k" "$loose"; done; } > "$work/sample.txt"
n=0
while IFS="$(printf '\t')" read -r pak rest; do
  name=$(echo "$rest" | sed 's/^ *[0-9]* *//')
  n=$((n + 1))
  base=$(printf '%03d_%s' "$n" "$(echo "$name" | sed 's/.*\\//')")
  "$tool" pak-get "$pak" "$name" "$work/files/$base" > /dev/null 2>&1 || continue
  case $base in *.elixir.gz) "$tool" unpack "$work/files/$base" "$work/files/$base.d" > /dev/null 2>&1; rm -f "${work:?}/files/${base:?}" ;; esac
done < "$work/sample.txt"
find "$work/files" -type f | sort > "$work/list.txt"
# small files to 4 bytes, larger ones coarser, each within 15 seconds
xargs -P 8 -I{} sh -c 's=$(wc -c < "$1"); g=4; [ "$s" -gt 65536 ] && g=64; "$0" unread "$1" --gran $g --budget 15 2>&1 | tail -1' "$tool" {} < "$work/list.txt" > "$work/report.txt"
awk '
  / bytes, / {
    file=$1; sub(/:$/,"",file); ext=tolower(file); sub(/.*\./,"",ext)
    for (i=2;i<=NF;i++) { if ($i=="bytes,") size=$(i-1); if ($i=="unused") un=$(i-1) }
    files[ext]++; bytes[ext]+=size; unused[ext]+=un
    if (un > worst[ext]) { worst[ext]=un; wfile[ext]=file; wsize[ext]=size }
  }
  END {
    printf "%-8s %6s %12s %12s %7s  %s\n", "kind", "files", "bytes", "unused", "share", "most unused"
    for (e in files) printf "%-8s %6d %12d %12d %6.1f%%  %s (%d of %d)\n", e, files[e], bytes[e], unused[e], bytes[e] ? 100*unused[e]/bytes[e] : 0, wfile[e], worst[e], wsize[e]
  }' "$work/report.txt" | sed "s|$work/files/||" | sort -k5 -t' ' | (read -r h; echo "$h"; sort -k4,4nr)
echo "$(grep -vc ' bytes, ' "$work/report.txt") files with no parser or an error (see $work/report.txt)"
