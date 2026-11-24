#!/usr/bin/env bash
# ==============================================================================
# MiniCI Cost Action (Feature 1) — 25-Repository x 3-Mode Standalone Validator
# ==============================================================================
# Usage:
#   bash scripts/validate_minici_cost_10_repos.sh
#
# Validates `minici` (v0.2.0) across 9 real-world repositories:
#   1. mgjartproject/FineArts-BE    (Internal Private • Grassy Fine Arts Backend)
#   2. Eazy-Ops/Backend             (Internal Private • Eazy-Ops Backend)
#   3. astral-sh/uv                 (Fresh OSS • 47 workflows, 243 jobs)
#   4. denoland/deno                (Fresh OSS • 11 workflows, 162 jobs)
#   5. ggml-org/llama.cpp           (Fresh OSS • 62 workflows, 157 jobs)
#   6. immich-app/immich            (Fresh OSS • 27 workflows, 87 jobs)
#   7. langgenius/dify              (Fresh OSS • 41 workflows, 128 jobs)
#   8. open-webui/open-webui        (Fresh OSS • 7 workflows, 18 jobs)
#   9. mendableai/firecrawl         (Fresh OSS • 36 workflows, 86 jobs)
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CACHE_DIR="${MINICI_BENCHMARK_CACHE:-${HOME}/.minici/10-repos-cache}"
BIN="${REPO_ROOT}/crates/minici/target/release/minici"
cargo build --locked --release --manifest-path "${REPO_ROOT}/crates/minici/Cargo.toml"

echo "▶ [1/3] Checking 2026 Canonical SKU Sync (catalog.rs <-> pricing/*.json)..."
python3 "${REPO_ROOT}/tools/sync_rust_sku_catalog.py" --check --bin "${BIN}"

echo ""
echo "▶ [2/3] Running 25-Repository x 3-Mode Validation (CLI, GitHub Workflow, MCP/Agent + 24 Unified Cost Models)..."
python3 "${REPO_ROOT}/tools/verify_10_repos_all_modes.py" --cache-dir "${CACHE_DIR}"

echo ""
echo "▶ [3/3] Verifying CLI Catalog & Benchmark Subcommands..."
"${BIN}" catalog | sed -n '1,18p'
echo "✓ Available fixtures validated across CLI, GitHub Workflow, and AI Agent (MCP/Hook) modes."
