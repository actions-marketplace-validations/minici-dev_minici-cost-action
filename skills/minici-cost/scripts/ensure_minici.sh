#!/usr/bin/env bash
# Ensures the `minici` binary is installed and available on PATH.
# Uses only SHA-256 verified GitHub Release binaries (with `cargo install` fallback).
set -euo pipefail

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

if command -v minici >/dev/null 2>&1; then
  minici --version
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_INSTALLER="${SCRIPT_DIR}/../../../scripts/install.sh"

if [ -f "$REPO_INSTALLER" ]; then
  bash "$REPO_INSTALLER"
else
  curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | bash
fi

minici --version
