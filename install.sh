#!/bin/sh
set -e
u=https://github.com/9sx77ssl/xlzt/releases/latest/download/lzt
d=/usr/local/bin/lzt
t=$(mktemp)
trap 'rm -f "$t" "$t.sha"' EXIT
[ "$(id -u)" = 0 ] && s='' || s=sudo
curl -fsSL "$u" -o "$t" &
pid=$!
while kill -0 "$pid" 2>/dev/null; do
  for e in '^.^' '-.-' '^.^' 'o.o'; do
    printf '\r\033[35m(%s)\033[0m fetching lzt…' "$e"
    sleep 0.12
  done
done
if ! wait "$pid"; then
  printf '\r\033[2K\033[31mdownload failed\033[0m\n' >&2
  exit 1
fi
c=$(curl -fsSL "$u.sha256" 2>/dev/null || true)
c=${c%% *}
a=$(sha256sum "$t")
a=${a%% *}
if [ -n "$c" ] && [ "$c" != "$a" ]; then
  printf '\r\033[2K\033[31mchecksum mismatch\033[0m\n' >&2
  exit 1
fi
chmod +x "$t"
[ -e "$d" ] && m=updated || m=installed
rm -f "$HOME/.local/bin/lzt"
$s install -m755 "$t" "$d"
printf '\r\033[2K\033[32m✓ lzt %s ^.^\033[0m\n' "$m"
if command -v fc-list >/dev/null 2>&1 && ! fc-list 2>/dev/null | grep -qi nerd; then
  printf '\033[33m(=^.^=) image blocks need a nerd font →\033[0m sudo pacman -S ttf-nerd-fonts-symbols\n'
fi
