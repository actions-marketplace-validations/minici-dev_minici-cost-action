# Benchmarking GitHub Actions Costs Across 10 of the World's Largest Repositories with MiniCI

> **SEO & AI Search Metadata (GEO / AIO Ready)**
> - **Suggested URL Slug:** `/blog/github-actions-cost-benchmark-10-largest-open-source-repos`
> - **Meta Title:** `GitHub Actions Cost Benchmark: 10 World-Scale Repos Audited`
> - **Meta Description:** `Live CI/CD cost benchmark of VS Code, Pydantic, Next.js, Ollama, Supabase & 5 more huge GitHub repos in 41 ms (<7 MiB RAM) using MiniCI, an open-source Rust CLI.`
> - **Target Keywords:** `GitHub Actions cost calculator`, `GitHub Actions pricing per minute 2026`, `CI/CD FinOps`, `GitHub Actions matrix cost`, `reduce GitHub Actions bill`, `GitHub Larger Runners pricing`, `MiniCI`

---

## Why CI/CD Cost Remains Invisible Until the End-of-Month Invoice

Most CI/CD platforms—including GitHub Actions, GitLab CI, and CircleCI—report runner charges in monthly billing dashboards rather than inside Pull Requests. When an engineer modifies `.github/workflows/*.yml` to expand a test `strategy.matrix`, switch a build job to an `8-core` or `macOS` runner, or upload build artifacts without a retention policy, code reviewers see the YAML diff—but **zero dollar or quota impact** before clicking *Merge*.

**[MiniCI](https://github.com/minici-dev/minici-cost-action)** (`minici v0.9.0`) solves this locally and in CI. Written in `100%` pure Rust (`std` only, `0` external crates, `1.6 MiB` static binary), MiniCI expands workflow matrices, applies vendor billing rules (`1x` Linux, `2x` Windows, `10x` macOS quota multipliers, whole-minute ceiling rounding, and `227+` runner & cloud VM rates across `7` CI platforms), and posts a non-blocking (`exit 0`) cost receipt on every Pull Request—requiring **zero API keys and zero source-code read permissions (`Contents: None` default)**.

To test MiniCI at real engineering scale, we benchmarked it offline against **10 of the world's most active open-source GitHub repositories**.

---

## Benchmark Methodology & Exact Hardware Disclosure

Performance numbers are meaningless without disclosing the exact machine, threading model, and memory footprint used to measure them:

| Benchmark Parameter | Measured Value |
| :--- | :--- |
| **MiniCI Version & Binary** | `minici 0.9.0` (`1.6 MiB` static ARM64 binary, `0` external crates, `Apache-2.0`) |
| **Host CPU & Architecture** | Apple M4 Pro (`14` logical cores: `10` performance + `4` efficiency, `arm64`) — **single-threaded (`1` CPU thread per `minici` process)** |
| **Host Memory & OS** | `48 GiB` unified RAM • macOS `15.1` (`Build 24B2083`) |
| **Peak Process Memory (`maxrss`)** | **`2.59 MiB – 7.00 MiB` Peak RSS** (`1.39 MiB – 5.91 MiB` peak memory footprint via `/usr/bin/time -l`) |
| **CPU Time per Invocation** | `~0.01s user + ~0.01s sys` (`100%` local disk YAML parse, `0` network calls) |
| **Normalized Volume Baseline** | Default `minici` baseline: `25 PRs/week × 4 pushes/PR = 433 runs/month` (`paid` rate comparison; `$65/hr` developer rate at `40%` DORA context-switch weight) |

> **Note:** Open-source public repositories receive generous hosted runner grants from GitHub. The dollar figures below answer the practical FinOps question every private engineering organization faces: *"What does this exact workflow architecture cost per Pull Request and per month on standard metered billing?"*

---

## Live Results: 10 World-Scale Repositories (`109` Workflows, `566` Expanded Jobs)

Across all `10` repositories combined (`109` workflow files expanding to `566` matrix jobs per run), `minici` audited every workflow in **`40.67 ms` total** (`100`-run warm average via `minici breakdown --benchmark`; `54.93 ms` cold single-run sum) while never exceeding **`7.00 MiB` of RAM**:

| Repository | WF / Expanded Jobs | Billed Mins / Run | Gross Monthly Compute | `ceil(60s)` Rounding Overhead | Critical-Path Dev Wait | `100`-Run Avg (`Cold`) | Peak RSS RAM |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **`pydantic/pydantic`** | `12` / `175` | `933.70m` | **`$5,395.87/mo`** | `$423.60/mo` | `$3,217.19/mo` (`21.9m`) | `5.10 ms` (`6.40 ms`) | `3.44 MiB` |
| **`microsoft/vscode`** | `5` / `36` | `242.00m` | **`$4,316.14/mo`** | `$346.00/mo` | `$2,082.73/mo` (`11.1m`) | `1.93 ms` (`3.25 ms`) | `3.11 MiB` |
| **`ollama/ollama`** | `4` / `47` | `413.00m` | **`$2,606.66/mo`** | `$100.98/mo` | `$3,178.22/mo` (`17.0m`) | `0.77 ms` (`2.04 ms`) | `3.03 MiB` |
| **`home-assistant/core`** | `19` / `83` | `558.00m` | **`$1,394.26/mo`** | `$105.82/mo` | `$17,449.90/mo` (`93.0m`) | `17.52 ms` (`19.38 ms`) | `7.00 MiB` |
| **`vercel/next.js`** | `5` / `45` | `144.00m` | **`$1,137.92/mo`** | `$120.51/mo` | `$1,190.75/mo` (`6.4m`) | `1.48 ms` (`2.71 ms`) | `2.94 MiB` |
| **`n8n-io/n8n`** | `12` / `44` | `258.35m` | **`$866.61/mo`** | `$24.15/mo` | `$2,598.00/mo` (`13.8m`) | `1.86 ms` (`3.03 ms`) | `2.67 MiB` |
| **`huggingface/transformers`** | `7` / `57` | `223.00m` | **`$579.35/mo`** | `$63.52/mo` | `$1,424.57/mo` (`7.6m`) | `1.73 ms` (`3.34 ms`) | `3.58 MiB` |
| **`langchain-ai/langchain`** | `27` / `43` | `113.00m` | **`$293.57/mo`** | `$52.87/mo` | `$1,238.38/mo` (`16.4m`) | `8.68 ms` (`10.60 ms`) | `4.48 MiB` |
| **`supabase/supabase`** | `12` / `25` | `73.35m` | **`$284.05/mo`** | `$6.39/mo` | `$1,446.22/mo` (`7.7m`) | `1.19 ms` (`2.43 ms`) | `2.77 MiB` |
| **`flutter/flutter`** | `6` / `11` | `27.00m` | **`$94.39/mo`** | `$12.76/mo` | `$1,848.91/mo` (`9.9m`) | `0.41 ms` (`1.75 ms`) | `2.59 MiB` |
| **TOTAL (10 Repos)** | **`109` / `566`** | **`2,985.40m`** | **`$16,968.82/mo`** | **`$1,256.60/mo`** | **`$35,674.87/mo`** | **`40.67 ms`** (`54.93 ms`) | **`<= 7.00 MiB`** |

---

## How MiniCI Displays This in Your Terminal (`microsoft/vscode` Live Output)

Running `minici breakdown` prints a unified job-by-job invoice, quota burn, pipeline hygiene audit, and contextual recommendation in a single screen:

```text
$ minici breakdown --path ./01-microsoft-vscode

Project: ./01-microsoft-vscode (5 workflow(s), 36 expanded jobs • 3.25 ms)

 WORKFLOW / JOB                              MATRIX × RUNNER           BILLED MINS    RATE/MIN    COST/RUN   MONTHLY COST
 ──────────────────────────────────────────  ────────────────────────  ───────────  ──────────  ──────────  ─────────────
 code-oss.yml • detect-changes               1x vscode-large-runne...         8.0m   $0.0220/m      $0.176      $76.21/mo
 code-oss.yml • linux-electron-smoke         3x vscode-large-runne...        21.0m   $0.0220/m      $0.462     $200.05/mo
 code-oss.yml • windows-electron-smoke       3x vscode-large-runne...        27.0m   $0.0420/m      $1.134     $491.02/mo
 code-oss.yml • macos-electron-smoke         2x macos-26-xlarge              14.0m   $0.1020/m      $1.428     $618.32/mo
 ... (20 additional expanded jobs) ...
 monaco-editor-checks.yml • monaco-edito...  1x ubuntu-latest                 3.0m   $0.0060/m      $0.018       $7.79/mo
 telemetry.yml • check-metadata              1x ubuntu-latest                 3.0m   $0.0060/m      $0.018       $7.79/mo
 ──────────────────────────────────────────  ────────────────────────  ───────────  ──────────  ──────────  ─────────────
 OVERALL TOTAL (433 runs/mo • paid tier)     36 expanded job(s)             242.0m           —      $9.968    $4316.14/mo

 • Free-Tier Quota  : 104786 / 2000 mins (129.9% of monthly quota) • Net Billed: $4304.14/mo • Rounding Overhead: 19.4m/run ($346.00/mo)
 • Dev Wait-Time    : 11.1m critical path ($4.81/run • $2082.73/mo at 40% DORA context-switch loss) • Concurrency: 36 peak jobs (36 hosted > 20 cap, +$510.00/mo tier upgrade)
 • Pipeline Hygiene : Cache 0/36 jobs (caching 36 uncached saves ~$971.13/mo) • Cancel-in-progress missing on 5 wf (~$647.42/mo superseded waste) • Path filters missing on 4 wf (~$120.55/mo selective savings) • 34 larger runner job(s) ($4300.56/mo metered from 1st sec; never draw free mins) • PR matrix (18 unconditional cells; 1-cell PR + full main/nightly saves ~$900.64/mo) • 26 job(s) missing timeout-minutes ($511.20 6h hung-run exposure)
 • Unit Economics   : $39.87/merged PR • $65.40/active dev/mo
 • Recommendation   : Right-size heavy jobs to ARM64 / High-Speed Runners ($1990.89/mo, saves $2325.25/mo) or Cloud Spot ($928.09/mo, saves $3388.05/mo)
```

---

## Four Engineering Insights From the 10-Repo Benchmark

### 1. Heavy Builds Legitimately Need Larger Runners—Which Makes Trigger Hygiene Essential
Repositories like `microsoft/vscode` (`34/36` jobs on `8-core` Linux/Windows and `12-core` `macos-26-xlarge`) and `ollama/ollama` compile C++, Electron, and native cross-OS binaries. They **genuinely require** high-core runners to keep developer feedback loops fast (`11.1m` critical path on `vscode`, `17.0m` on `ollama`).

Because GitHub Larger Runners never draw from included plan minutes (`$4,300.56/mo` of `vscode`'s `$4,316.14/mo` is metered from the first second, or **`$39.87` per merged PR**), the right FinOps lever is **not** starving compilers of CPU/RAM—it is eliminating wasted runs:
- Adding `concurrency: { cancel-in-progress: true }` across `vscode`'s `5` workflows reclaims **`~$647.42/mo`** from obsolete PR pushes without slowing down a single build.
- Adding `paths:` filters and running `1` representative matrix cell on draft PRs (keeping the full `18`-cell matrix on `main`/merge queue) reclaims **`~$900.64/mo`**.

### 2. High-Shard Matrices Amplify GitHub's Whole-Minute Rounding (`$1,256.60/mo` Total)
GitHub Actions bills each expanded job in whole-minute increments (`ceil(raw_mins)`). On a `2`-job workflow, a `25-second` remainder costs fractions of a cent. On **`pydantic/pydantic`** (`175` expanded jobs per run across OS and Python/third-party compatibility matrices), sub-minute remainders add up to **`73.3m/run` (`$423.60/mo`)** of unexecuted billed time. By contrast, `supabase/supabase` and `n8n-io/n8n`—which run most jobs on fractional-minute runners—pay just **`$6.39/mo`** and **`$24.15/mo`** in rounding overhead.

### 3. Low Compute Cost Can Mask Severe Critical-Path Bottlenecks
**`home-assistant/core`** spends a relatively modest **`$1,394.26/mo`** on CI runner compute across `83` Linux jobs (`ubuntu-latest`, `ubuntu-24.04-arm`, and `ubuntu-slim` at `$0.0020/m`). However, `minici`'s `needs:` DAG analysis reveals a **`93.0-minute` sequential critical path**, translating to **`$17,449.90/mo`** in blocked engineering review flow—showing why compute dollars and critical-path duration must be tracked together.

### 4. Artifact Storage & Hung-Job Timeouts Are Zero-Risk Quick Wins
In **`supabase/supabase`**, compute is already lean (`$284.05/mo`), so `minici` automatically shifts its recommendation to storage hygiene: lowering `upload-artifact` retention from GitHub's `90-day` default (`$162.38/mo` for `649.50 GB` steady-state) to `14 days` cuts artifact spend by **`84.4%` (`$25.26/mo`)**—and `minici shrink --path . --apply` applies `retention-days`, `cancel-in-progress`, shallow clones, and `timeout-minutes` automatically while preserving YAML comments.

---

## Reproduce This Benchmark Locally in `< 1 Second`

```bash
# 1. Install MiniCI (pinned Rust source, 0 external crates)
cargo install --locked --git https://github.com/minici-dev/minici-cost-action.git --rev 45f57abdaf9e2987451d8767fb2e164adf22c864 minici

# 2. Audit any public GitHub repository remotely without cloning:
minici breakdown --repo pydantic/pydantic
minici breakdown --repo microsoft/vscode
minici breakdown --repo supabase/supabase

# 3. Run the 100-iteration microsecond/millisecond benchmark on a local checkout:
minici breakdown --path . --benchmark
```

---

## Frequently Asked Questions (FAQ)

### How is GitHub Actions cost per Pull Request calculated?
GitHub Actions bills per expanded job by rounding each job's runtime up to the next whole minute (`ceil(duration_minutes)`), multiplied by the runner's per-minute SKU rate (for example, `$0.006/min` for `ubuntu-latest` `2-core`, `$0.022/min` for `ubuntu-8core`, `$0.062/min` for `macos-latest`, and `$0.102/min` for `macos-xlarge` in 2026 pricing). MiniCI expands `strategy.matrix` combinations statically and multiplies per-run cost by monthly PR push cadence.

### Does MiniCI require GitHub tokens or send source code externally?
No. `minici` is a standalone Rust binary (`0` third-party crates) that parses workflow YAML files locally with `Contents: None` source-code access by default and zero external network calls during `breakdown`, `diff`, `shrink`, or `comment`.

### Can MiniCI run inside GitHub Actions on every Pull Request?
Yes. Adding `uses: minici-dev/minici-cost-action@v1` with `compare-to: "origin/${{ github.base_ref }}"` and `post-pr-comment: "true"` posts a single auto-updating sticky Pull Request cost diff comment and always exits `0` so developer builds are never blocked.
