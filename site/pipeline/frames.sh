#!/usr/bin/env bash
# Masters to scroll frames: numbered AVIF per shot, desktop 1920w and mobile 960w.
# 15 fps, not the source 24: a scrubbed frame is held while the reader scrolls, so
# density past ~15 per second of footage buys smoothness nobody sees and costs bytes.
# usage: frames.sh [shot ...]   (default: every master in site/gen/masters)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
src=$root/gen/masters; out=$root/public/frames; tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
shots=("$@"); [ ${#shots[@]} -gt 0 ] || shots=($(cd "$src" && ls *.mp4 | sed 's/\.mp4$//'))
for s in "${shots[@]}"; do
  for v in d:1920 m:960; do
    name=${v%%:*} w=${v##*:}
    mkdir -p "$tmp/$s$name" "$out/$s/$name"; rm -f "$out/$s/$name"/*.avif
    ffmpeg -v error -i "$src/$s.mp4" -vf "fps=15,scale=$w:-2:flags=lanczos" "$tmp/$s$name/%04d.png"
    q=60; [ "$name" = m ] && q=55
    ls "$tmp/$s$name"/*.png | xargs -P "$(nproc)" -I{} sh -c \
      'avifenc -q '"$q"' -s 7 -j 1 "{}" "'"$out/$s/$name"'/$(basename "{}" .png).avif" >/dev/null'
    echo "$s/$name $(ls "$out/$s/$name" | wc -l) frames $(du -sh "$out/$s/$name" | cut -f1)"
  done
done
