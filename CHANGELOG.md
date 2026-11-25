# Changelog

## 0.9.3

- Linux release binaries are now static musl builds (`minici-linux-x86_64`, `minici-linux-aarch64`). They have no glibc floor, so they run on Alpine, Ubuntu 20.04 and Amazon Linux 2023. The v0.9.2 aarch64 asset needed GLIBC_2.39, and x86_64 failed on glibc 2.31.
- Added a `minici-windows-arm64.exe` release asset; the Action uses it on Windows ARM64 runners.
- Added `scripts/install.ps1` for Windows: downloads the asset, verifies SHA-256, installs under `%LOCALAPPDATA%\minici\bin`, and adds it to the user PATH without admin rights.
- `scripts/install.sh` points MSYS/MINGW/CYGWIN shells to `install.ps1`. It also verifies a prebuilt binary runs before installing it, and falls back to a source build if it cannot.
- Fixed: the generated rust-everywhere helper no longer reports a pass without running the original check; it always runs the script and returns its exit code.
- Fixed: `migrate --to macos-arm64` now rewrites Intel macOS runners to `macos-14` (it previously made no edits), and prints why when there is nothing to change.
- Fixed: ARM64 migration savings are computed once and match across breakdown, migrate table and JSON (`arm64_migration_monthly_savings_usd`).
- Runner matrix: removed the `macos-14` leg, which GitHub rejects as unsupported.
