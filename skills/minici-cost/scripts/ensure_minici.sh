#!/usr/bin/env bash
# Ensures the `minici` binary is installed and available on PATH.
# Uses the bundled installer or builds a pinned revision with Cargo.
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
  cargo install --locked --git https://github.com/minici-dev/minici-cost-action.git --rev 0fceb7a0ba81403e8ec3262970ff6f4ccd8c8c18 minici
fi

minici --version
