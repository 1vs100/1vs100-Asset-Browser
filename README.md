# 1vs100 Asset Browser

Browse and inspect assets for the Xbox 360 game 1vs100.

## Features

- Asset browsing
  - CAFF
  - LZX
  - STFS
  - STRB
  - AVATARS
  - XDBF
  - XEX
  - XSB
  - XUI
  - XUR
  - XWB
  - YTGR
- CLI and GUI interfaces
- Cross-platform

## Quickstart

1. Download the archive for your OS and architecture from "Releases" (https://github.com/1vs100/1vs100-Asset-Browser/releases).
2. Extract the archive.
3. Run the CLI or GUI executable.

## Build

Requirements:
- "Rust toolchain" (https://www.rust-lang.org/tools/install).

```
git clone https://github.com/1vs100/1vs100-Asset-Browser.git
cd 1vs100-Asset-Browser
cargo build --release --all
```

## Credits

- [acceleration](https://github.com/landaire/acceleration) - Xbox 360 rust crate by @landaire
