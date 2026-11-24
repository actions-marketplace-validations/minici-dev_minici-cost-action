<p align="center">
  <img src="https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/web/minici-icon-128.png" width="60" height="60" alt="MiniCI" />
</p>

<h1 align="center">MiniCI — See What Every Pull Request Costs Before You Merge</h1>

<p align="center">
  <strong>AI coding and rapid shipping have created a 10&times; Pull Request storm — yet CI/CD billing stays invisible until the month-end invoice lands.</strong><br />
  Meanwhile, developers lose hours in slow queues and reruns with zero cost or time visibility per workflow, build phase, or step.<br />
  <strong>MiniCI</strong> posts the estimated compute delta, developer wait-time cost, and <strong>1-click YAML fixes</strong> right on every Pull Request in under 2 ms — with zero API keys and zero external code uploads.
</p>

<p align="center">
  <a href="https://github.com/marketplace/actions/minici-ci-cd-cost-breakdown-pr-diff-quota-receipt"><img src="https://img.shields.io/badge/Marketplace-MiniCI_Cost_Action-0969da?logo=github" alt="GitHub Marketplace" /></a>
  <a href="https://github.com/minici-dev/minici-cost-action/actions/workflows/ci.yml"><img src="https://github.com/minici-dev/minici-cost-action/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/minici-dev/minici-cost-action/releases/latest"><img src="https://img.shields.io/github/v/release/minici-dev/minici-cost-action?color=0ea5e9&amp;label=release" alt="Release" /></a>
  <a href="https://github.com/minici-dev/minici-cost-action/blob/main/crates/minici/Cargo.toml"><img src="https://img.shields.io/badge/rust-std_only_(0_crates)-f97316.svg" alt="Pure Rust" /></a>
  <a href="https://buy.polar.sh/polar_cl_SCcWdjeIJvtwoa3JKBo5ouVXBL0yBv40bzuZr36iPMm"><img src="https://img.shields.io/badge/Plans_%26_Checkout-Polar.sh-10b981" alt="MiniCI Plans on Polar" /></a>
</p>

<p align="center">
  <a href="#what-your-team-sees-on-every-pull-request">PR Preview</a> &middot;
  <a href="#15-second-github-action-quickstart">Action Quickstart</a> &middot;
  <a href="#local-cli-quickstart-zero-clone-audit">CLI Quickstart</a> &middot;
  <a href="#ai-agent-skill--plugin-all-agents">AI Agent Skill</a> &middot;
  <a href="#permissions--zero-egress-security">Permissions &amp; Security</a> &middot;
  <a href="#action-inputs--outputs">Inputs &amp; Outputs</a> &middot;
  <a href="#troubleshooting">Troubleshooting</a> &middot;
  <a href="#frequently-asked-questions-faq">FAQ</a>
</p>

---

## What Your Team Sees on Every Pull Request

MiniCI runs non-blocking (`exit 0`), updates **one sticky comment** per PR, and attaches committable **Files changed** suggestions when workflow waste is detected:

> ### 💰 MiniCI Pull Request CI/CD Cost Diff (`433 µs` • Non-Blocking)
>
> | Metric | Base Branch | This PR | Impact (`Before → After`) |
> | :--- | ---: | ---: | :--- |
> | **Expanded Matrix Jobs** | 1 job | 8 jobs | **+7 matrix jobs** |
> | **Billed Minutes / Run** | 4.0 min | 44.0 min | **+40.0 min / run** (incl. 4.0m rounding overhead) |
> | **Monthly CI Compute** | `$10.39/mo` | `$413.08/mo` | **`+$402.69/mo`** |
> | **Developer Wait-Time Impact** | `$723.11/mo` | `$2,701.92/mo` | **`+$1,978.81/mo`** (14.4m critical path) |
>
> **Changed Cost Drivers & Phase Bottlenecks:**
> - **Added `release.yml • build-matrix [macos-latest]`** (`2x macos-latest` at `$0.0620/m`) → **`+$322.15/mo`** (`10×` free-tier multiplier)
> - **Added `release.yml • verify-gates & linux/windows matrix`** (`5` jobs) → **`+$80.54/mo`**
> - **Top Build Phase Bottleneck (`dependency_install`):** Uncached Cargo/step setup across 7/8 jobs wastes **`~$224.34/mo`**; missing `cancel-in-progress` wastes **`~$61.96/mo`** on superseded PR pushes.
>
> **✂️ Verified 1-Click YAML Fixes Ready:** Click **Commit suggestion** on *Files changed* (or comment `@minici shrink`):
> ```diff
> # .github/workflows/ci.yml
> + concurrency:
> +   group: ${{ github.workflow }}-${{ github.ref }}
> +   cancel-in-progress: ${{ github.event_name == 'pull_request' }}
>   jobs:
>     verify-action:
> -     runs-on: ubuntu-latest
> +     runs-on: ubuntu-24.04-arm
> +     timeout-minutes: 30
> ```

---

## Why Engineering Teams Adopt MiniCI

| Without MiniCI | With MiniCI |
| :--- | :--- |
| **Month-end invoice shock** after 10× PR storms and silent `OS × version × shard` matrix fan-out | **Pre-merge dollar delta on every PR** (`exit 0` sticky comment + GitHub Step Summary) |
| **Zero visibility per build phase or step** (1-min ceiling rounding, 10× macOS burn, artifact retention bloat) | **6-phase & step-signal breakdown** across 7 CI platforms and 227+ runner & cloud VM rates |
| **Wasted developer hours** waiting on uncached installs, slow critical paths, and obsolete PR pushes | **Critical-path wait-time pricing + 1-click YAML fixes** (`minici shrink` & verified `ARM64` right-sizing) |
| **Heavy external SaaS tools** requiring sign-ups, API keys, and cloud access to proprietary repos | **Standalone 1.6 MB Rust binary** (0 third-party crates, 0 API keys, 0 external data egress) |

---

## 15-Second GitHub Action Quickstart

Create `.github/workflows/minici-cost.yml` (or scaffold automatically via `minici init-hooks --path .`):

```yaml
name: CI Cost Diff
on:
  pull_request:

permissions:
  contents: read        # Reads workflow YAML on your runner — zero external egress
  pull-requests: write  # Posts & updates one sticky PR cost comment

jobs:
  ci-cost:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0 # Needed to compare PR workflows against the base branch

      - name: MiniCI PR Cost Diff & 1-Click Fixes
        id: cost
        uses: minici-dev/minici-cost-action@v1
        with:
          compare-to: "origin/${{ github.base_ref }}"
          post-pr-comment: "true"
          suggest-fixes: "true"
```

<details>
<summary><strong>Enable Interactive <code>@minici</code> PR Bot + Slack Alerts</strong></summary>

Reply `@minici shrink`, `@minici accelerate`, `@minici migrate arm64`, `@minici permissions`, or `@minici ask "<question>"` directly on any PR (or wire org-wide across GitHub App `/webhooks/github`, Slack `/slack/interactions`, Linear `/webhooks/linear`, Jira `/webhooks/jira`, and Google Chat `/webhooks/gchat`):

```yaml
name: CI Cost Diff & @minici Bot
on:
  pull_request:
    types: [opened, synchronize, reopened]
  issue_comment:
    types: [created]

concurrency:
  group: minici-${{ github.event.pull_request.number || github.event.issue.number || github.ref }}
  cancel-in-progress: true

permissions:
  contents: read
  pull-requests: write
  issues: write

jobs:
  ci-cost:
    if: >-
      github.event_name != 'issue_comment' ||
      (github.event.issue.pull_request &&
       github.event.comment.user.type != 'Bot' &&
       !contains(github.event.comment.body, '<!-- minici-') &&
       contains(github.event.comment.body, '@minici') &&
       contains(fromJSON('["OWNER","MEMBER","COLLABORATOR"]'), github.event.comment.author_association))
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: minici-dev/minici-cost-action@v1
        with:
          compare-to: "origin/${{ github.base_ref }}"
          post-pr-comment: "true"
          suggest-fixes: "true"
          slack-webhook-url: ${{ secrets.SLACK_WEBHOOK_URL }} # Optional
```

</details>

<details>
<summary><strong>Optional: Enforce a FinOps Budget Gate Using Action Outputs</strong></summary>

MiniCI never blocks PRs by default (`exit 0`), but platform teams can gate extreme spikes using any of its 34 outputs:

```yaml
      - name: Enforce Monthly CI Budget Threshold
        if: ${{ fromJSON(steps.cost.outputs.net_billed_monthly_compute_cost_usd) > 1000 }}
        run: |
          echo "::error::Projected monthly CI compute ($${{ steps.cost.outputs.net_billed_monthly_compute_cost_usd }}) exceeds $1,000 budget."
          exit 1
```

</details>

---

## Local CLI Quickstart (Zero-Clone Audit)

Build the pinned release with Cargo, or download release binaries and checksums from [GitHub Releases](https://github.com/minici-dev/minici-cost-action/releases/latest):

```bash
# Option A: Install from a pinned source revision
cargo install --locked --git https://github.com/minici-dev/minici-cost-action.git --rev 5c2152f912efcfaf89f82d6cbee3b0dd455a2bba minici

# Option B: Build from a local checkout
cargo install --locked --path crates/minici
```

Try it immediately — locally or against any public GitHub repo without cloning:

```bash
minici breakdown --repo astral-sh/uv             # Audit any public repo remotely (no clone, no token)
minici breakdown --path .                        # Full repo cost, wait-time & phase breakdown (< 2 ms)
minici diff --path . --compare-to main           # Preview branch cost delta before opening a PR
minici shrink --path . --apply                   # Apply comment-preserving YAML fixes in place
minici accelerate --path . --apply               # Swap CI tools to Rust (uv/ruff/biome/nextest) + rust-everywhere CI converter
minici migrate --path . --to github-arm64        # Check 6-gate ARM64 savings or cross-vendor CI migration
minici init-hooks --path .                       # Wire Antigravity, Claude Code, VS Code, Git & PR hooks
```

[![MiniCI Terminal Demo](https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/web/how-to-install-and-use.png)](https://minici-cost-action.raza-aamir01.workers.dev/how-to-install-and-use.mp4)

### Verified Speed on World-Scale Open-Source Pipelines

Reproduce any row below in seconds with `minici breakdown --repo <owner/repo>`:

| Repository | Workflows / Matrix Jobs | Engine Time | Monthly Gross Compute | Primary Optimization Uncovered |
| :--- | :---: | :---: | ---: | :--- |
| `denoland/deno` | 11 wf / 162 jobs | ~20 ms | `$10,109.35/mo` | Heavy multi-OS build matrix & ARM64 runner savings |
| `microsoft/vscode` | 5 wf / 36 jobs | ~4 ms | `$4,287.57/mo` | Critical-path wait reduction & concurrency guards |
| `pydantic/pydantic` | 12 wf / 173 jobs | ~7 ms | `$3,306.73/mo` | Matrix fan-out ceiling rounding & selective path filters |
| `ggml-org/llama.cpp` | 62 wf / 148 jobs | ~30 ms | `$3,226.86/mo` | Multi-arch build caching & Spot/ARM64 right-sizing |

<details>
<summary><strong>Full CLI Subcommand Reference (15 Subcommands)</strong></summary>

| Subcommand | Purpose |
| :--- | :--- |
| `breakdown` | Compute CI/CD cost, quota burn, phase breakdown, and unit economics (`table`, `markdown`, `json`, `html`, `csv`) |
| `diff` | Compare base vs. head Git refs or directories to quantify Pull Request matrix & dollar deltas |
| `shrink` | Generate or apply comment-preserving YAML optimizations (`--apply`, `--rust-tools`, `--rust-everywhere`, `--security`, `--suggest-pr`) |
| `accelerate` | Two-tier Rust CI acceleration: drop-in Rust toolchains (`uv`, `ruff`, `@biomejs/biome`, `prek`, `cargo nextest`) + scoped `rust-everywhere` bespoke CI-script converter (`.minici/rust-everywhere/<tool>/`) |
| `migrate` | Run 6-gate runner right-sizing (`github-arm64`, `ubuntu-slim`, `codebuild-github-runner`, `aws-spot`) and 7×7 cross-vendor translation |
| `ask` | Answer natural-language CI FinOps questions backed by deterministic Rust math |
| `comment` | Render or post sticky GitHub PR comments, inline suggestions, and Slack Block Kit notifications |
| `permissions` | Audit read-only token scopes and progressive telemetry coverage |
| `sync` | Sync cached live run history or billing CSVs into `.minici/telemetry.lock.json` |
| `telemetry` | Pull historical run CSVs from GitHub Actions, GitLab CI, CircleCI, or Bitbucket Pipelines |
| `catalog` | Inspect, filter, or override 227+ runner and cloud VM rates (`~/.minici/pricing_overrides.json`) |
| `cloud-price` | Query On-Demand and Spot VM pricing across AWS EC2, GCP, and Azure across 26 regions |
| `mcp` | Launch the 8-tool stdio MCP server for Claude Code, Antigravity, Cursor, Windsurf, and VS Code |
| `init-hooks` | Scaffold `.github/workflows/minici-cost.yml`, `.mcp.json`, `.agents/hooks.json`, `.claude/settings.json`, `.vscode/mcp.json`, `.git/hooks/pre-commit`, and multi-agent `SKILL.md` bundles |
| `self-update` | Verify zero SKU catalog drift and update the local `minici` binary |
| `launch-pack` | Export structured repository FinOps summary profiles |

</details>

---

## AI Agent Skill & Plugin (All Agents)

Give any AI coding assistant deterministic sub-millisecond CI/CD FinOps math, PR cost diffs, `ARM64` runner right-sizing, and comment-preserving YAML fixes via the bundled [`skills/minici-cost/SKILL.md`](https://github.com/minici-dev/minici-cost-action/blob/main/skills/minici-cost/SKILL.md) skill and native plugin manifests:

```bash
# Universal Open Agent Skills CLI (Claude Code, Cursor, Codex, Copilot, Antigravity, Windsurf, Cline, Goose, Amp)
npx skills add minici-dev/minici-cost-action

# Claude Code Plugin Marketplace (1-step install)
/plugin install minici-cost --marketplace minici-dev/minici-cost-action

# Or wire workspace Skills + MCP + Shift-Left Hooks + GitHub Action in one command
minici init-hooks --path .
```

| AI Agent / Ecosystem | Included Manifest / Path | What It Enables |
| :--- | :--- | :--- |
| **35+ Agents (`skills.sh`)** | [`skills/minici-cost/SKILL.md`](https://github.com/minici-dev/minici-cost-action/blob/main/skills/minici-cost/SKILL.md) | Progressive-disclosure CI FinOps runbook + auto-bootstrap installer |
| **Google Antigravity** | [`plugin.json`](https://github.com/minici-dev/minici-cost-action/blob/main/plugin.json) + `.agents/skills/minici-cost/` | Native plugin card, starter prompts, `SKILL.md`, and `PreToolUse`/`PostToolUse` hooks |
| **Claude Code** | [`.claude-plugin/plugin.json`](https://github.com/minici-dev/minici-cost-action/blob/main/.claude-plugin/plugin.json) + [`.mcp.json`](https://github.com/minici-dev/minici-cost-action/blob/main/.mcp.json) | 1-step `/plugin install`, `/minici-cost` skill, and 8-tool stdio MCP server |
| **Cursor** | [`.cursor-plugin/plugin.json`](https://github.com/minici-dev/minici-cost-action/blob/main/.cursor-plugin/plugin.json) + `.cursor/skills/` | Native Cursor plugin bundle (`skills/` + `.mcp.json`) |
| **OpenAI Codex & ChatGPT** | [`skills/minici-cost/agents/openai.yaml`](https://github.com/minici-dev/minici-cost-action/blob/main/skills/minici-cost/agents/openai.yaml) + `.agents/skills/` | `$minici-cost` UI card, implicit workflow invocation, and starter prompt |
| **GitHub Copilot & VS Code** | `.github/skills/minici-cost/SKILL.md` + `.vscode/mcp.json` | Workspace Copilot skill + stdio MCP server |

---

## Permissions & Zero-Egress Security

![MiniCI Architecture Overview](https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/web/customer-diagram.svg)

MiniCI compiles strictly against the Rust standard library (`0` third-party Cargo crates) and **never uploads or transmits application source code**:

| Context / Scope | Why It Is Used | External Data Egress |
| :--- | :--- | :---: |
| **Local CLI & Public Repo Audit** | No token scopes required (`Contents: None`). Reads workflow YAML locally or via public API. | **None** |
| **`contents: read` (GitHub Action)** | Allows `actions/checkout@v4` on **your own runner** so MiniCI can diff `.github/workflows/*.yml` against the base branch. Application code is never read or sent externally. | **None** |
| **`pull-requests: write`** | Posts and updates one sticky PR comment (`post-pr-comment: "true"`). | GitHub PR API only |
| **`actions: read` *(Optional)*** | Used only when `live-telemetry: "true"` is enabled to fetch historical run durations. | GitHub API (`GET` only) |
| **Fork & Dependabot PRs** | GitHub automatically provides a read-only token on untrusted forks; MiniCI skips PR commenting cleanly (`exit 0`) and writes the receipt to `$GITHUB_STEP_SUMMARY`. Never use `pull_request_target` with untrusted checkouts. | **None** |

See [SECURITY.md](https://github.com/minici-dev/minici-cost-action/blob/main/SECURITY.md) for the full threat model, SHA-256 verification, and 40-char commit-SHA pinning guide.

---

## Matching Your Exact Invoice & AI Agent (MCP) Setup

Out of the box, MiniCI estimates job duration from step signals, expands matrices, traverses `needs:` DAG critical paths, and applies each provider's exact minute rounding (`ceil(mins)` on GitHub Actions and Bitbucket; fractional minutes on GitLab, CircleCI, and Cloud Build). **Override any assumption without recompiling:**

| To Customize... | How to Configure |
| :--- | :--- |
| **Wrapper Script / Job Duration** | Add `# est-minutes: 18` to any YAML job, or set `live-telemetry: "true"` to use real historical run durations. |
| **Negotiated Enterprise Runner Rates** | Run `minici catalog set <runner_label> <usd_per_min>` (persisted in `~/.minici/pricing_overrides.json`). |
| **Pure Vendor Compute (Hide Dev Wait)** | Set `dev-hourly-rate: "0"` (Action) or `--dev-hourly-rate 0` (CLI) to show billed CI vendor spend only. |
| **1:1 Monthly Invoice Reconciliation** | Pass `billing-csv: "./billing-export.csv"` (`FOCUS v1.1`, GitHub, GitLab, CircleCI, or Bitbucket export — merges cleanly onto live telemetry). |

<details>
<summary><strong>Connect MiniCI to AI Coding Agents via MCP (<code>.mcp.json</code> / <code>.vscode/mcp.json</code>)</strong></summary>

Expose all 8 deterministic CI FinOps tools (`minici_breakdown`, `minici_diff`, `minici_shrink`, `minici_migrate`, `minici_anomalies`, `minici_sync_live`, `minici_permissions`, `minici_ask`) to Claude Code, Antigravity, Cursor, Windsurf, and VS Code (or scaffold automatically with `minici init-hooks --path .`):

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

</details>

---

## Action Inputs & Outputs

| Input | Default | Description |
| :--- | :---: | :--- |
| `path` | `.` | Repository root or workflow directory |
| `compare-to` | `""` | Base Git ref (recommended: `origin/${{ github.base_ref }}`) for Pull Request cost diff |
| `post-pr-comment` | `false` | Post or update a single sticky PR comment (`<!-- minici-cost-comment -->`) |
| `suggest-fixes` | `true` | Attach verified YAML optimization patches (`shrink` + `ARM64`) to PR comments |
| `tier` | `auto` | Plan quota tier (`auto`, `free`, `pro`, `team`, `enterprise`, or `paid`) |
| `prs-per-week` | `25` | Weekly PR volume for monthly run-rate projection |
| `pushes-per-pr` | `4` | Average pushes per Pull Request |
| `dev-hourly-rate` | `65` | Engineering hourly rate in USD for wait-time math (`0` disables wait-time cost) |
| `live-telemetry` | `false` | Sync historical CI run durations prior to calculation |
| `billing-csv` | `""` | Optional `FOCUS v1.1` or CI vendor billing CSV path for exact invoice lock |
| `slack-webhook-url` | `""` | Optional Slack Incoming Webhook URL for Block Kit cost & diff alerts |

**Key Outputs (`steps.<id>.outputs.*`):** `gross_monthly_compute_cost_usd`, `net_billed_monthly_compute_cost_usd`, `monthly_rounding_tax_usd`, `monthly_productive_payroll_loss_usd`, `arm64_migration_monthly_savings_usd`, `recoverable_ci_and_wait_monthly_usd`, `cost_per_merged_pr_usd`, `spend_anomaly_alert_active`, `provenance_mode` (+ 25 more in [action.yml](https://github.com/minici-dev/minici-cost-action/blob/main/action.yml)).

---

## Troubleshooting

| Symptom | Cause & 1-Line Fix |
| :--- | :--- |
| **No sticky comment appeared on the PR** | Ensure `post-pr-comment: "true"`, `compare-to: "origin/${{ github.base_ref }}"`, and `permissions: pull-requests: write` are set. |
| **PR diff reports all existing workflows as newly added** | `actions/checkout@v4` fetches only 1 commit by default. Add `with: fetch-depth: 0` so the base branch ref exists locally. |
| **External fork or Dependabot PR has no comment** | Expected: GitHub restricts fork tokens to read-only. View the full receipt in the Actions **Job Summary** tab. |
| **Minimal Docker/self-hosted runner missing `python3`** | `action.yml` uses `python3` to export the 34 `GITHUB_OUTPUT` fields; run the cost job on `ubuntu-latest` or ensure `python3` is installed. |
| **Static estimate differs from a long wrapper script** | Add `# est-minutes: 30` to the job in YAML, or enable `live-telemetry: "true"` to pull real historical durations. |

---

## Frequently Asked Questions (FAQ)

<details>
<summary><strong>1. Is MiniCI completely free? Do I need an API key or external account?</strong></summary>

Yes. MiniCI is free and open-source under `Apache-2.0`, with zero API keys, zero sign-ups, and all 227+ hosted CI runner and cloud VM rates compiled directly into the binary.

</details>

<details>
<summary><strong>2. Will MiniCI ever fail or block my developers' Pull Requests?</strong></summary>

Never by default. MiniCI exits `0` and updates one sticky PR comment rather than spamming new comments on every push. Teams that want hard budget enforcement can optionally check `steps.<id>.outputs.*` in a subsequent step.

</details>

<details>
<summary><strong>3. Does MiniCI clutter standard repos with irrelevant cloud egress or storage rows?</strong></summary>

No. MiniCI applies strict applicability filtering: artifact storage appears only when workflows upload artifacts, cloud NAT/VPC egress appears only on self-hosted cloud runners, and runner right-sizing tips trigger only where verified savings exist.

</details>

<details>
<summary><strong>4. Does MiniCI work with GitLab CI, CircleCI, Bitbucket, CodeBuild, Cloud Build, or Azure Pipelines?</strong></summary>

Yes. While packaged on GitHub Marketplace as a turnkey GitHub Action, the `minici` CLI natively parses all 7 CI platforms (`.github/workflows/*.yml`, `.gitlab-ci.yml`, `.circleci/config.yml`, `bitbucket-pipelines.yml`, `buildspec.yml`, `cloudbuild.yaml`, `azure-pipelines.yml`) and translates pipelines across them via `minici migrate`.

</details>

<details>
<summary><strong>5. How does <code>minici shrink</code> edit YAML without stripping comments?</strong></summary>

Instead of re-serializing YAML ASTs (which destroys comments and formatting), `minici shrink` applies surgical line-level patches that preserve 100% of `#` comments and verifies dollar savings via a shadow diff first.

</details>

<details>
<summary><strong>6. What are the known static-analysis boundaries in offline mode?</strong></summary>

Local reusable workflows (`uses: ./.github/workflows/...`) resolve automatically without double-counting orchestrator jobs. Dynamic runtime matrices (`${{ fromJSON(...) }}`), YAML merge anchors (`<<: *base`), and cross-repository reusable workflows default to `1x` cardinality offline unless paired with `live-telemetry: "true"`. Full details in [CAPABILITIES_AND_LIMITATIONS.md](https://github.com/minici-dev/minici-cost-action/blob/main/docs/CAPABILITIES_AND_LIMITATIONS.md) and [BENCHMARK_217_CASES_3ARM_RESULTS.md](https://github.com/minici-dev/minici-cost-action/blob/main/docs/BENCHMARK_217_CASES_3ARM_RESULTS.md).

</details>

---

<p align="center">
  Licensed under <a href="https://github.com/minici-dev/minici-cost-action/blob/main/LICENSE">Apache-2.0</a> &middot;
  <a href="https://buy.polar.sh/polar_cl_SCcWdjeIJvtwoa3JKBo5ouVXBL0yBv40bzuZr36iPMm">Plans (Polar.sh)</a> &middot;
  <a href="https://polar.sh/minici/portal">Customer Portal</a> &middot;
  Built in pure Rust by <a href="https://github.com/Aamir-raza-1">Aamir Raza</a>.
</p>
