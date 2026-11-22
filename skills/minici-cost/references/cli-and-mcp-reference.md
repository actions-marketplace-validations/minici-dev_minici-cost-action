# MiniCI CLI, MCP & GitHub Action Reference

## Table of Contents
1. [Supported CI/CD Platforms](#1-supported-cicd-platforms)
2. [All 15 CLI Subcommands](#2-all-15-cli-subcommands)
3. [8 MCP Server Tools (`minici mcp`)](#3-8-mcp-server-tools-minici-mcp)
4. [Invoice Calibration & Custom Runner Pricing](#4-invoice-calibration--custom-runner-pricing)
5. [GitHub Action (`minici-dev/minici-cost-action@v1`)](#5-github-action-minici-devminici-cost-actionv1)

---

## 1. Supported CI/CD Platforms

`minici` auto-detects and parses all 7 major CI workflow formats in `--path <DIR>`:

| CI Platform | Auto-Detected Workflow Paths | Minute Billing Rule |
| :--- | :--- | :--- |
| **GitHub Actions** | `.github/workflows/*.yml`, `*.yaml` | `ceil(mins)` per job + OS multipliers (`1x` Linux, `2x` Windows, `10x` macOS) |
| **GitLab CI** | `.gitlab-ci.yml` | Fractional compute minutes × runner cost factor |
| **CircleCI** | `.circleci/config.yml` | Fractional credit burn per resource class |
| **Bitbucket Pipelines** | `bitbucket-pipelines.yml` | `ceil(mins)` × step size (`1x`, `2x`, `4x`, `8x`) |
| **AWS CodeBuild** | `buildspec.yml` | Per-minute/per-second compute SKU by region & arch |
| **Google Cloud Build** | `cloudbuild.yaml`, `cloudbuild.yml` | Per-second machine-type billing (`e2-*`, `n1-*`) |
| **Azure Pipelines** | `azure-pipelines.yml` | Parallel job concurrency tier (`$40/mo` MS-hosted, `$15/mo` self-hosted) + minutes |

---

## 2. All 15 CLI Subcommands

| Subcommand | Example Invocation | Purpose |
| :--- | :--- | :--- |
| `breakdown` | `minici breakdown --path . [--format table\|markdown\|json\|html\|csv] [--report out.html]` | Full 24-model CI/CD cost, phase, wait-time, and quota breakdown |
| `diff` | `minici diff --path . --compare-to origin/main [--format markdown]` | Compare base vs. head Git refs or directories for PR cost delta |
| `shrink` | `minici shrink --path . [--apply] [--security] [--suggest-pr]` | Preview or apply comment-preserving YAML optimizations |
| `migrate` | `minici migrate --path . --to github-arm64` | 6-gate runner right-sizing (`github-arm64`, `ubuntu-slim`, `aws-spot`) or 7×7 vendor translation |
| `ask` | `minici ask "Why did CI spend jump?" --path . [--format json]` | Deterministic FinOps Q&A backed by exact Rust engine math |
| `comment` | `minici comment --path . --compare-to origin/main --post` | Render or post sticky GitHub PR comments (`<!-- minici-cost-comment -->`) & Slack alerts |
| `permissions` | `minici permissions check --path . [--offline]` | Audit read-only token scopes (`source_code_access_required: false`) |
| `sync` | `minici sync --path . [--billing-csv focus.csv]` | Sync live CI run history (`ETag 304`) or billing CSV into `.minici/telemetry.lock.json` |
| `telemetry` | `minici telemetry --path .` | Pull historical run telemetry from GitHub, GitLab, CircleCI, or Bitbucket |
| `catalog` | `minici catalog [set <runner_label> <usd_per_min>]` | Inspect 227+ compiled runner/VM SKUs or set enterprise pricing overrides |
| `cloud-price` | `minici cloud-price c7g.xlarge --region us-east-1 [--format json]` | Query AWS EC2, GCP, and Azure On-Demand & Spot VM rates across 26 regions |
| `mcp` | `minici mcp --path .` | Start the JSON-RPC 2.0 stdio MCP server (NDJSON & LSP `Content-Length` framing) |
| `init-hooks` | `minici init-hooks --path .` | Scaffold GitHub Action, `.mcp.json`, `.vscode/mcp.json`, IDE hooks, Git hook, and workspace skills |
| `self-update` | `minici self-update` | Verify 0.00% SKU catalog drift and update local `minici` binary |
| `launch-pack` | `minici launch-pack --path .` | Export structured repository FinOps summary profile |

---

## 3. 8 MCP Server Tools (`minici mcp`)

Configure in `.mcp.json` (Claude Code, Antigravity, Cursor, Windsurf) or `.vscode/mcp.json` (VS Code / Copilot):

```json
{
  "mcpServers": {
    "minici": {
      "command": "minici",
      "args": ["mcp", "--path", "."]
    }
  }
}
```

| MCP Tool Name | Description |
| :--- | :--- |
| `minici_breakdown` | Full JSON FinOps breakdown (`gross_monthly_compute_cost_usd`, `net_billed_monthly_compute_cost_usd`, jobs, phases, savings) |
| `minici_diff` | Branch/ref cost delta (`compare_to` argument, default `HEAD`) |
| `minici_shrink` | Preview or apply surgical comment-preserving YAML fixes (`apply: bool`, `mode: "hygiene" \| "security"`) |
| `minici_migrate` | Runner right-sizing & cross-vendor pipeline migration (`to` target argument) |
| `minici_anomalies` | Detect spend spikes, missing `timeout-minutes` risk, and runaway matrix jobs |
| `minici_sync_live` | Sync live workflow run telemetry into `.minici/telemetry.lock.json` |
| `minici_permissions` | Verify token scopes and progressive telemetry readiness |
| `minici_ask` | Answer natural-language CI cost questions (`question` string argument) |

---

## 4. Invoice Calibration & Custom Runner Pricing

- **Override opaque script duration:** Add `# est-minutes: 18` above or inside any YAML job definition.
- **Enterprise negotiated runner rates:** Run `minici catalog set <runner_label> <usd_per_min>` (persisted in `~/.minici/pricing_overrides.json`).
- **Exact 1:1 monthly invoice lock:** Run `minici sync --path . --billing-csv ./billing-export.csv` (supports `FOCUS v1.1`, GitHub Actions, GitLab, CircleCI, and Bitbucket billing CSVs; merges cleanly with `.minici/telemetry.lock.json`).

---

## 5. GitHub Action (`minici-dev/minici-cost-action@v1`)

Minimal non-blocking (`exit 0`) Pull Request workflow:

```yaml
name: CI Cost Diff
on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

jobs:
  ci-cost:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: MiniCI PR Cost Diff & 1-Click Fixes
        id: cost
        uses: minici-dev/minici-cost-action@v1
        with:
          compare-to: "origin/${{ github.base_ref }}"
          post-pr-comment: "true"
          suggest-fixes: "true"
```

**Important:** Always include `fetch-depth: 0` on `actions/checkout@v4` when `compare-to` is set so Git can resolve `origin/${{ github.base_ref }}` without treating existing workflows as newly added.
