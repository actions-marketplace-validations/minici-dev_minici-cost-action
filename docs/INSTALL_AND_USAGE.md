# MiniCI (`minici-cost-action`) — Installation & Usage Guide

This guide covers installing and running `minici` (`v0.9.2`) as a **Local CLI**, **GitHub Action**, **Interactive `@minici` PR Bot**, **Shift-Left Git / IDE Hook**, and **`8`-Tool MCP Server**.

* **Terminal Demo (`.mp4`):** [`../web/how-to-install-and-use.mp4`](../web/how-to-install-and-use.mp4)
* **Asciinema Recording (`.cast`):** [`../web/how-to-install-and-use.cast`](../web/how-to-install-and-use.cast)

---

## 1. Local CLI Installation

`crates/minici` uses **100% Rust standard library (`std`) with zero external crate dependencies**. Prebuilt binaries are published for Linux (`x86_64`, `aarch64`), macOS (`aarch64`, `x86_64`), and Windows (`x86_64`), verified against `SHA256SUMS.txt`.

### Option A: Install a pinned release with Cargo

```bash
cargo install --locked --git https://github.com/minici-dev/minici-cost-action.git --rev 0fceb7a0ba81403e8ec3262970ff6f4ccd8c8c18 minici
minici --version
# -> minici 0.9.2
```

### Option B: Build Offline from Source (`~1.5s`)

```bash
git clone https://github.com/minici-dev/minici-cost-action.git
cd minici-cost-action
cargo build --release --manifest-path crates/minici/Cargo.toml

mkdir -p ~/.local/bin
cp crates/minici/target/release/minici ~/.local/bin/minici
```

### Option C: Install a verified release binary (Linux and macOS)

```bash
MINICI_VERSION=v0.9.2 bash scripts/install.sh
```

---

## 2. Everyday CLI Workflows

### A. Repository Cost & Quota Receipt (`minici breakdown`)

Run inside any repository containing `.github/workflows/*.yml`, `.gitlab-ci.yml`, `.circleci/config.yml`, `bitbucket-pipelines.yml`, `buildspec.yml`, `cloudbuild.yaml`, or `azure-pipelines.yml`:

```bash
minici breakdown --path .
```

Customize team activity assumptions to match your engineering organization:

```bash
minici breakdown \
  --path . \
  --tier paid \
  --prs-per-week 40 \
  --pushes-per-pr 3 \
  --dev-hourly-rate 85
```

* `--tier <auto|free|pro|team|enterprise|paid>`: `auto` checks whether monthly OS-weighted minutes exceed your provider's free-tier quota (`2,000 mins/mo` on GitHub Free, `3,000 mins/mo` on Pro/Team, `50,000 mins/mo` on Enterprise) and switches to paid rates automatically when exceeded.
* `--prs-per-week <NUM>` & `--pushes-per-pr <NUM>`: Controls monthly CI run projection (`prs_per_week * pushes_per_pr * 4.33`).
* `--dev-hourly-rate <USD>`: Engineering hourly rate for critical-path wait-time & context-switch payroll calculations (`0` disables wait-time math).

### B. Pull Request / Branch Cost Delta (`minici diff`)

Before pushing a workflow change, compare your working tree against `main`, `origin/main`, `HEAD~1`, or another directory:

```bash
# Compare uncommitted/committed branch changes against `main`
minici diff --path . --compare-to main

# Compare against the previous commit
minici diff --path . --compare-to HEAD~1

# Emit a sticky-ready GitHub Pull Request Markdown diff table
minici diff --path . --compare-to main --format markdown --out-file pr-cost-diff.md
```

### C. Zero-Clone Public GitHub Repository Audit (`--repo`)

Inspect any public GitHub repository's CI spend without cloning its source tree:

```bash
minici breakdown --repo open-webui/open-webui
minici breakdown --repo https://github.com/denoland/deno

# Force refresh cached workflows (~/.minici/cache/repos/)
minici breakdown --repo open-webui/open-webui --refresh
```

> **Tip:** Export `GITHUB_TOKEN=ghp_...` to raise GitHub REST API rate limits from `60 req/hr` to `5,000 req/hr` and access private repositories you have permission to read.

### D. Multi-Format Reports (`html`, `json`, `markdown`, `csv`)

```bash
minici breakdown --path . --report ci-executive-report.html
minici breakdown --path . --report ci-breakdown.json --quiet
minici breakdown --path . --report ci-jobs.csv
minici breakdown --path . --report ci-summary.md
```

### E. Pricing SKU Catalog & Live Override Sync (`catalog` & `self-update`)

```bash
# Print all compiled SaaS & Cloud runner SKUs as JSON
minici catalog

# Verify 0.00% catalog drift and refresh local override cache (~/.minici/pricing_overrides.json)
minici self-update
```

---

Set `MINICI_SNAPSHOT_DIR` to opt into offline workflow snapshots for remote repository audits. The benchmark verifier also accepts `--fresh-dir` for its world-scale fixtures.

## 3. GitHub Action Integration (`action.yml`)

Add `.github/workflows/minici-cost.yml` to post job summaries and expose structured FinOps outputs to downstream workflow steps:

```yaml
name: MiniCI Cost Receipt
on:
  pull_request:
  push:
    branches: [main]

permissions:
  contents: read
  pull-requests: write

jobs:
  cost-receipt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Compute CI Cost Breakdown
        id: cost
        uses: minici-dev/minici-cost-action@v1
        with:
          path: "."
          tier: "auto"
          prs-per-week: "25"
          pushes-per-pr: "4"
          dev-hourly-rate: "65"

      - name: Print Key FinOps Outputs
        run: |
          echo "Gross Monthly Compute ($): ${{ steps.cost.outputs.gross_monthly_compute_cost_usd }}"
          echo "Net Billed Monthly ($):    ${{ steps.cost.outputs.net_billed_monthly_compute_cost_usd }}"
          echo "ARM64 Savings ($/mo):      ${{ steps.cost.outputs.arm64_migration_monthly_savings_usd }}"
          echo "Detected Stack:            ${{ steps.cost.outputs.detected_stack_archetype }}"
```

### Action Inputs

| Input | Default | Description |
| :--- | :---: | :--- |
| `path` | `.` | Repository root or workflow directory |
| `compare-to` | `""` | Optional base Git ref (e.g., `origin/main`) to also append a PR cost delta table |
| `tier` | `auto` | Pricing tier mode (`auto`, `free`, `pro`, `team`, `enterprise`, or `paid`) |
| `prs-per-week` | `25` | Pull requests per week for monthly run projection |
| `pushes-per-pr` | `4` | Average pushes per pull request |
| `dev-hourly-rate` | `65` | Developer hourly rate in USD for wait-time calculations |
| `live-telemetry` | `false` | Sync live CI run history (`ETag 304` cached) before computing receipt |
| `billing-csv` | `""` | Optional `FOCUS v1.1` or vendor billing CSV/TSV path for exact invoice reconciliation |
| `post-pr-comment` | `false` | Post or update a single idempotent sticky PR comment (`<!-- minici-cost-comment -->`) |

---

## 4. AI Agent Skill, Shift-Left Hooks & 8-Tool MCP Server

Install the standalone [`skills/minici-cost/SKILL.md`](../skills/minici-cost/SKILL.md) skill across any AI coding agent, or wire everything into a workspace via `minici init-hooks --path .`:

```bash
# Install across Claude Code, Cursor, Codex, Copilot, Antigravity, Windsurf, Cline, Goose & Amp
npx skills add minici-dev/minici-cost-action

# Or install directly in Claude Code via Marketplace
/plugin install minici-cost --marketplace minici-dev/minici-cost-action
```

Run `minici init-hooks --path .` inside any repository to scaffold all shift-left, AI agent skill, and CI integration files:
1. `.minici/hooks.json`, `.agents/hooks.json`, and `.claude/settings.json` — Configures `PreToolUse` (`minici breakdown --path . --hook pre-tool-use`) and `PostToolUse` (`minici diff --path . --compare-to HEAD`) shift-left workflow cost-delta hooks.
2. `.mcp.json` and `.vscode/mcp.json` — Registers the stdio JSON-RPC 2.0 MCP server (`minici mcp`, supporting both NDJSON and LSP `Content-Length` framing) so AI coding agents can call all `8` live tools (`minici_breakdown`, `minici_diff`, `minici_sync_live`, `minici_permissions`, `minici_anomalies`, `minici_shrink`, `minici_migrate`, and `minici_ask`) directly during workflow edits.
3. `.agents/skills/minici-cost/SKILL.md` (+ `.claude/skills/`, `.cursor/skills/`, `.github/skills/`) — Installs the progressive-disclosure `minici-cost` skill across Antigravity, Claude Code, Cursor, OpenAI Codex, Windsurf, and GitHub Copilot.
4. `.github/workflows/minici-cost.yml` — Installs the non-blocking GitHub Action Pull Request & push cost receipt workflow (`exit 0` informational).

Test the MCP server directly from your shell:

```bash
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"minici_breakdown"}}\n' | minici mcp --path .
```

---

## 5. Surgical YAML Optimization & Safe Runner Migration (`shrink` & `migrate`)

Once `minici breakdown` highlights missing `cancel-in-progress`, uncached dependencies, full-history `actions/checkout` clones, long `upload-artifact` retention, or eligible `x86_64 -> ARM64` runner savings, use `minici`'s built-in comment-preserving optimizer directly:

```bash
# Preview surgical YAML optimizations and verified dollar/minute savings
minici shrink --path .

# Apply optimizations in-place while preserving 100% of # YAML comments
minici shrink --path . --apply

# Evaluate 6-gate verified runner or 7x7 cross-platform migration
minici migrate --path . --to github-arm64

# Verify the exact before/after cost delta against Git HEAD
minici diff --path . --compare-to HEAD
```
