# xlzt

Minimal CLI to post threads on [lolz](https://lolz.live) via the API.

## Install

```
curl -fsSL https://raw.githubusercontent.com/9sx77ssl/xlzt/main/install.sh | sh
```

## Use

```
lzt -s   # set token + forum_id (token obfuscated at rest, machine-bound, file 0600)
lzt -t   # create a thread — type title, then body, Ctrl+D to post
```

`Ctrl+V` in the body pastes a clipboard image as `[Image]`; on send each one is uploaded to `y7v.lol` and the placeholder becomes a link in place. Needs `xclip` (X11) or `wl-clipboard` (Wayland).

Falls back across the lolz/zelenka API mirrors on 5xx or network errors.

## Stack

Rust · tokio · reqwest (rustls) · rustyline · sha2

## Build

```
cargo build --release
upx --best --lzma target/release/lzt
install -Dm755 target/release/lzt ~/.local/bin/lzt
```
