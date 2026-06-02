#!/bin/sh
set -e
u=https://github.com/9sx77ssl/xlzt/releases/latest/download/lzt
d=/usr/local/bin/lzt
t=$(mktemp)
[ "$(id -u)" = 0 ] && s='' || s=sudo
curl -fsSL "$u" -o "$t" &
pid=$!
while kill -0 "$pid" 2>/dev/null; do
  for e in '^.^' '-.-' '^.^' 'o.o'; do
    printf '\r\033[35m(%s)\033[0m fetching lzt…' "$e"
    sleep 0.12
  done
done
wait "$pid"
chmod +x "$t"
[ -e "$d" ] && m=updated || m=installed
rm -f "$HOME/.local/bin/lzt"
$s install -m755 "$t" "$d"
rm -f "$t"
printf '\r\033[2K\033[32m✓ lzt %s ^.^\033[0m\n' "$m"
