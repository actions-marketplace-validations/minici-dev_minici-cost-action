# Changelog

## 0.9.3

- Linux release binaries are now static musl builds (`minici-linux-x86_64`, `minici-linux-aarch64`). They have no glibc floor, so they run on Alpine, Ubuntu 20.04 and Amazon Linux 2023. The v0.9.2 aarch64 asset needed GLIBC_2.39, and x86_64 failed on glibc 2.31.
- Added a `minici-windows-arm64.exe` release asset.
- Added `scripts/install.ps1` for Windows: downloads the asset, verifies SHA-256, installs under `%LOCALAPPDATA%\minici\bin`, and adds it to the user PATH without admin rights.
- `scripts/install.sh` points MSYS/MINGW/CYGWIN shells to `install.ps1`. It also verifies a prebuilt binary runs before installing it, and falls back to a source build if it cannot.
- Runner matrix: removed the `macos-14` leg, which GitHub rejects as unsupported.
