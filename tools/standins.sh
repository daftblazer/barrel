#!/bin/sh
# Lists every stand-in tag under crates/: a comment line with "STAND-IN:" and one sentence on
# what is stood in and what the real thing would be. Prints `file:line: text`, sorted, then a
# last line `N stand-ins`. Run from anywhere; pass directories to search others.
root=$(cd "$(dirname "$0")/.." && pwd)
[ $# -eq 0 ] && set -- "$root/crates"
tags=$(grep -rn --include='*.rs' 'STAND-IN:' "$@" | sed "s|^$root/||" | sed 's|^\([^:]*:[0-9]*:\)[[:space:]]*\(//[/!]*\)[[:space:]]*|\1 |' | sort -t: -k1,1 -k2,2n)
[ -n "$tags" ] && printf '%s\n' "$tags"
printf '%s stand-ins\n' "$(printf '%s' "$tags" | grep -c .)"
