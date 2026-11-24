#!/usr/bin/env bash
# ==============================================================================
# MiniCI Portable Installer (`scripts/install.sh`)
# Usage:
#   bash scripts/install.sh
# ==============================================================================
set -euo pipefail

REPO="minici-dev/minici-cost-action"
VERSION="${MINICI_VERSION:-latest}"
INSTALL_DIR="${MINICI_INSTALL_DIR:-$HOME/.local/bin}"

OS_RAW="$(uname -s)"
ARCH_RAW="$(uname -m)"

case "$OS_RAW" in
  Linux*)  OS="linux" ;;
  Darwin*) OS="macos" ;;
  *) echo "Unsupported OS: $OS_RAW (use cargo install --git https://github.com/$REPO.git minici)"; exit 1 ;;
esac

case "$ARCH_RAW" in
  x86_64|amd64)  ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $ARCH_RAW"; exit 1 ;;
esac

ASSET="minici-${OS}-${ARCH}"
if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/${REPO}/releases/latest/download"
  SOURCE_REF="5c2152f912efcfaf89f82d6cbee3b0dd455a2bba"
else
  BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"
  SOURCE_REF="refs/tags/${VERSION}"
fi

mkdir -p "$INSTALL_DIR"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "⏳ Downloading ${ASSET} from ${REPO} (${VERSION})..."
INSTALLED_PREBUILT=0
if curl -fsSL "${BASE_URL}/${ASSET}" -o "${TMP_DIR}/minici" 2>/dev/null \
  && curl -fsSL "${BASE_URL}/SHA256SUMS.txt" -o "${TMP_DIR}/SHA256SUMS.txt" 2>/dev/null; then
  EXPECTED="$(grep " ${ASSET}\$" "${TMP_DIR}/SHA256SUMS.txt" | awk '{print $1}' || true)"
  if [ -z "$EXPECTED" ]; then
    echo "❌ Release asset ${ASSET} missing from SHA256SUMS.txt! Refusing unverified binary."
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL="$(sha256sum "${TMP_DIR}/minici" | awk '{print $1}')"
  else
    ACTUAL="$(shasum -a 256 "${TMP_DIR}/minici" | awk '{print $1}')"
  fi
  if [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "❌ Checksum mismatch for ${ASSET}! Expected ${EXPECTED}, got ${ACTUAL}"
    exit 1
  fi
  echo "✅ Verified SHA-256 checksum (${ACTUAL:0:16}...)"
  chmod +x "${TMP_DIR}/minici"
  # A checksum-verified asset can still be unrunnable (e.g. built against a newer glibc than this host has).
  if "${TMP_DIR}/minici" --version >/dev/null; then
    rm -f "${INSTALL_DIR}/minici"
    mv "${TMP_DIR}/minici" "${INSTALL_DIR}/minici"
    INSTALLED_PREBUILT=1
  else
    echo "⚠️ Verified ${ASSET} does not run on this system (libc mismatch?); falling back to source build..."
  fi
fi

if [ "$INSTALLED_PREBUILT" = "0" ]; then
  echo "⚙️ Verified release asset + SHA256SUMS.txt not yet cached; compiling std-only Rust binary from source..."
  git init -q "${TMP_DIR}/src"
  git -C "${TMP_DIR}/src" fetch --depth 1 "https://github.com/${REPO}.git" "${SOURCE_REF}"
  git -C "${TMP_DIR}/src" checkout --detach FETCH_HEAD
  cargo build --locked --release --manifest-path "${TMP_DIR}/src/crates/minici/Cargo.toml"
  rm -f "${INSTALL_DIR}/minici"
  cp "${TMP_DIR}/src/crates/minici/target/release/minici" "${INSTALL_DIR}/minici"
  chmod +x "${INSTALL_DIR}/minici"
fi

# Assign separately: inside echo's $(...) a failing --version would not stop the script.
INSTALLED_VERSION="$("${INSTALL_DIR}/minici" --version)"
echo "✅ Installed ${INSTALLED_VERSION} -> ${INSTALL_DIR}/minici"
