---
name: minici-cost
description: >-
  Deterministic CI/CD FinOps cost breakdown, Pull Request cost diff, runner right-sizing
  (ARM64, ubuntu-slim, spot), and comment-preserving workflow YAML optimization across
  GitHub Actions, GitLab CI, CircleCI, Bitbucket Pipelines, AWS CodeBuild, Google Cloud Build,
  and Azure Pipelines. Use this skill whenever the user asks to audit CI/CD costs or minutes,
  optimize slow or expensive workflows (.github/workflows/*.yml, .gitlab-ci.yml, .circleci/config.yml,
  bitbucket-pipelines.yml, buildspec.yml, cloudbuild.yaml, azure-pipelines.yml), preview PR matrix
  fan-out or billing deltas before pushing, migrate runners to ARM64 or across CI vendors,
  reconcile CI billing CSVs, or check any public GitHub repository's CI spend without cloning.
---

# MiniCI (`minici-cost`) — Deterministic CI/CD Cost & Workflow Optimizer

Use `minici` to compute exact CI/CD vendor compute spend, 1-minute ceiling rounding waste, matrix expansion, critical-path developer wait-time impact, and verified YAML optimizations in under 2 ms — locally and with zero source-code egress.

Never estimate CI runner rates, OS multipliers (`10x` macOS, `2x` Windows), or `ceil(minutes)` rounding mentally when `minici` can calculate the deterministic receipt directly.

---

## Step 0: Ensure `minici` Is Available

Check whether `minici` is on `PATH`. If needed, run `bash scripts/ensure_minici.sh` from the installed skill directory. For a pinned Cargo install, run:

```bash
cargo install --locked --git https://github.com/minici-dev/minici-cost-action.git --rev 45f57abdaf9e2987451d8767fb2e164adf22c864 minici
export PATH="$HOME/.cargo/bin:$PATH"
```

If the `minici` MCP server is connected in this session (`minici_breakdown`, `minici_diff`, `minici_shrink`, `minici_migrate`, `minici_anomalies`, `minici_sync_live`, `minici_permissions`, `minici_ask`), you may call either the MCP tools or the `minici` CLI — both execute the identical deterministic Rust engine.

---

## Core Workflows

### 1. Audit Local Repository CI/CD Spend & Bottlenecks

Run inside any repository containing `.github/workflows/*.yml`, `.gitlab-ci.yml`, `.circleci/config.yml`, `bitbucket-pipelines.yml`, `buildspec.yml`, `cloudbuild.yaml`, or `azure-pipelines.yml`:

```bash
# Human-readable unified summary + Top-15 cost drivers (pass `--all` for full monorepo table, `--ascii` for non-UTF8 terminals)
minici breakdown --path .

# Token-efficient agent digest (<2KB JSON with top jobs, `line_start`/`line_end`, and `next_actions`)
minici breakdown --path . --format json --compact --quiet

# Full 24-model FinOps ledger + per-job `line_start`/`line_end` spans
minici breakdown --path . --format json --quiet
```

Adjust org parameters only when the user specifies custom volume, plan tier, or pure compute mode:
- `--tier <auto|free|pro|team|enterprise|paid>` (default: `auto`)
- `--prs-per-week <N>` (default: `25`) and `--pushes-per-pr <N>` (default: `4`)
- `--dev-hourly-rate <USD>` (default: `65`; pass `0` when the user wants pure CI vendor compute without developer wait-time payroll)

### 2. Audit Any Public GitHub Repository Without Cloning

When the user names a GitHub `owner/repo` or URL not cloned locally:

```bash
minici breakdown --repo <owner/repo>
```

Export `GITHUB_TOKEN` if hitting unauthenticated GitHub REST rate limits or auditing a private repo the user has read access to.

### 3. Preview Pull Request / Branch Cost Delta Before Merging

Always run `minici diff` after editing any CI workflow file or when reviewing a branch/PR:

```bash
# Compare working tree against main / origin/main / HEAD
minici diff --path . --compare-to HEAD
minici diff --path . --compare-to origin/main

# Generate sticky-comment Markdown diff table (or platform-native PR/MR/Slack payloads)
minici diff --path . --compare-to origin/main --format markdown
minici comment github --path . --compare-to origin/main
minici comment gitlab --path . --compare-to origin/main
minici comment bitbucket --path . --compare-to origin/main
minici comment slack --path . --compare-to origin/main
```

### 4. Apply Surgical, Comment-Preserving YAML Fixes (`shrink`)

When asked to reduce CI bills, fix missing `concurrency.cancel-in-progress`, add `timeout-minutes`, shallow-clone `actions/checkout`, right-size `upload-artifact` retention, or add dependency caching:

1. **Preview verified patches and shadow-diff savings first:**
   ```bash
   minici shrink --path .
   ```
2. **Apply in place (preserves 100% of `#` YAML comments and formatting):**
   ```bash
   minici shrink --path . --apply
   # Optional: also swap legacy Python/Node/Rust CI steps to Rust (uv/ruff/biome/prek/nextest)
   # and convert bespoke standalone CI scripts (scripts/*.py, tools/*.js, ci/*.sh) via rust-everywhere:
   minici accelerate --path . --apply
   ```
3. **Verify the exact post-fix dollar and minute delta:**
   ```bash
   minici diff --path . --compare-to HEAD
   ```

Prefer `minici shrink --apply` over rewriting entire YAML files manually so inline `#` comments and indentation stay intact.

### 5. Right-Size Runners (`ARM64` / `ubuntu-slim` / `aws-spot`) or Migrate CI Vendors

`minici migrate` runs a 6-gate safety check (skipping jobs with x86-only Docker images, Apple Silicon incompatibilities, or custom self-hosted labels where ineligible):

```bash
# Check verified ARM64 runner savings on GitHub Actions
minici migrate --path . --to github-arm64

# Other targets: ubuntu-slim, aws-spot, gitlab-ci, circleci,
# aws-codebuild, gcp-cloud-build, azure-pipelines, bitbucket-pipelines
minici migrate --path . --to <target>
```

### 6. Wire Shift-Left Guardrails Into a Project (`init-hooks`)

When the user wants automatic CI cost guardrails on every PR, commit, or AI edit:

```bash
minici init-hooks --path .
```

This non-destructively scaffolds `.github/workflows/minici-cost.yml` (sticky PR cost diff + `@minici` bot), `.mcp.json`, `.vscode/mcp.json`, `.agents/hooks.json`, `.claude/settings.json`, `.git/hooks/pre-commit`, and workspace agent skills.

---

## Reporting & Output Guardrails

1. **Single Unified Summary:** Present CI compute, rounding overhead, critical-path wait-time impact, phase bottlenecks, and verified savings in one cohesive report backed by `minici` numbers.
2. **Strict Applicability (Never Hallucinate Irrelevant Line Items):**
   - **Cloud Egress / NAT:** Bandwidth is included (`$0`) on standard hosted SaaS runners (`ubuntu-latest`, `macos-latest`, `blacksmith-*`). Only mention cloud NAT/VPC egress when `minici` detects self-hosted cloud VPC runners (`self-hosted`, `aws-*`, `gcp-*`, `azure-*`, `codebuild`).
   - **Log Retention:** Native CI vendor log retention is `$0/mo`; never invent external SIEM bills unless log-shipping steps are actually present.
   - **Artifact Storage:** Mention artifact storage only when `jobs_uploading_artifacts > 0`, and only recommend lowering retention if `configured_retention_days > 14`.
   - **Runner Recommendations:** Recommend `ARM64` or runner changes only for eligible hosted jobs where `minici` reports positive verified savings (`arm64_migration_monthly_savings_usd > 0`). If a repo already runs heavy jobs on custom/high-speed runners, do not recommend redundant runner migrations.
3. **Calibrating Static Estimates:** If a workflow wraps a long script (`./scripts/ci.sh`) whose runtime differs from static step-signal heuristics, annotate the YAML job with `# est-minutes: <N>` or sync real run durations via `minici sync --path .` / `--live-telemetry`.

---

## Further Reference

Read [references/cli-and-mcp-reference.md](references/cli-and-mcp-reference.md) when you need:
- Exact flags for all 15 `minici` CLI subcommands (`catalog`, `cloud-price`, `telemetry`, `permissions`, `comment`, `launch-pack`)
- Input schemas for the 8 `minici` MCP tools
- GitHub Action (`minici-dev/minici-cost-action@v1`) inputs, 34 outputs, and FOCUS v1.1 CSV invoice reconciliation
