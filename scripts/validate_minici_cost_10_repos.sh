#!/usr/bin/env bash
# ==============================================================================
# MiniCI Cost Action (Feature 1) — 17-Repository x 3-Mode Standalone Validator
# ==============================================================================
# Usage:
#   bash scripts/validate_minici_cost_10_repos.sh
#
# Validates `minici` (v0.9.2) across 7 public repositories:
#   1. astral-sh/uv                 (Fresh OSS • 47 workflows, 243 jobs)
#   2. denoland/deno                (Fresh OSS • 11 workflows, 162 jobs)
#   3. ggml-org/llama.cpp           (Fresh OSS • 62 workflows, 157 jobs)
#   4. immich-app/immich            (Fresh OSS • 27 workflows, 87 jobs)
#   5. langgenius/dify              (Fresh OSS • 41 workflows, 128 jobs)
#   6. open-webui/open-webui        (Fresh OSS • 7 workflows, 18 jobs)
#   7. mendableai/firecrawl         (Fresh OSS • 36 workflows, 86 jobs)
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
echo "▶ [2/3] Running 17-Repository x 3-Mode Validation (CLI, GitHub Workflow, MCP/Agent + 24 Unified Cost Models)..."
python3 "${REPO_ROOT}/tools/verify_10_repos_all_modes.py" --cache-dir "${CACHE_DIR}"

echo ""
echo "▶ [3/3] Verifying CLI Catalog & Benchmark Subcommands..."
"${BIN}" catalog | sed -n '1,18p'
echo "✓ Available fixtures validated across CLI, GitHub Workflow, and AI Agent (MCP/Hook) modes."
