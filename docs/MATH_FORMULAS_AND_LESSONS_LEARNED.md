# MiniCI Official CI/CD Mathematical Formulas, Invariants & Lessons Learned

This document is the permanent canonical reference for every mathematical formula, billing ceiling rule, allowance deduction invariant, statistical estimator, and invoice reconciliation rule implemented in `minici` (`crates/minici/src/`). Every change to `catalog.rs`, `providers.rs`, `telemetry.rs`, `engine.rs`, `optimizer.rs`, `renderer.rs`, `jev.rs`, or `worker.js` **must** preserve all invariants below (`0.00` cent drift).

---

## 1. Per-Job Duration Rounding & Jensen's Inequality (`A1–A6`)

1. **Vendor Billing Rounding Rules (`providers.rs`, `catalog.rs`):**
   - **GitHub Actions** (`standard`, `larger`, `gpu`): whole-minute ceiling per job ($\max(1, \lceil d \rceil)$), **only** for executed jobs with $d > 0$ and `conclusion != "startup_failure"`. `0s` / `skipped` / `startup_failure` jobs bill `$0.00`.
   - **Azure Pipelines** (`azure-pipelines-ms-hosted`): whole-minute ceiling per job ($\max(1, \lceil d \rceil)$).
   - **Bitbucket Pipelines**: whole-minute ceiling **per step** ($\max(1, \lceil d_{\text{step}} \rceil)$), multiplied by step `size` (`1x`, `2x`, `4x`, `8x`, `16x`).
   - **GitLab CI, CircleCI, GCP Cloud Build, AWS CodeBuild Lambda, Blacksmith / WarpBuild / Depot / BuildJet / Cloud Spot**: exact per-second billing ($d_{\text{sec}} / 60.0$) with **no artificial `0.1m` (`6s`) floor**.
   - **AWS CodeBuild EC2**: `1.0m` minimum, per-second thereafter ($\max(1.0, d_{\text{sec}} / 60.0)$); **Dedicated Mac Host**: `1440.0m` (`24h` minimum allocation).

2. **Jensen's Inequality in Live Telemetry (`telemetry.rs`, `engine.rs`):**
   - Because $f(x) = \max(1, \lceil x \rceil)$ is convex-stepwise, $\mathbb{E}[\lceil D \rceil] \ge \lceil \text{median}(D) \rceil$.
   - Example: runs of `[1.05m, 1.05m, 2.05m]` bill `2 + 2 + 3 = 7` minutes (`2.33m/run`), whereas `ceil(median = 1.05m) = 2.00m/run` underbills by `14.3%`.
   - `telemetry.rs` computes and persists `job_billed_mins = mean_i(ceil_vendor(d_i))` alongside `job_durations_mins`, and `engine.rs` uses `job_billed_mins` directly.

3. **Unceiled Cloud Spot / MicroVM Benchmarks & Per-Job Rounding Tax (`engine.rs`):**
   - Cloud Spot and per-second MicroVM alternatives multiply **unceiled** `est_raw_duration_mins_per_job * matrix_combination_count * equiv_rate`.
   - Monthly sub-minute rounding overhead weights each job's wasted sub-minute tail by **that job's own runner rate and workflow cadence**:
     $$\text{monthly\_rounding\_tax\_usd} = \text{round}\!\left(\sum_j \max\!\left(0, B_j - d_j M_j\right) \cdot \text{rate}_j \cdot r_j,\; 2\right)$$

---

## 2. Live Telemetry Window Extrapolation & EOM Velocity (`B1–B7`)

1. **Completed-Only & Unconditional Window Cutoff (`telemetry.rs`):**
   - Filter `status == "completed"` before duration/cadence aggregation so `in_progress`/`queued` runs never count as `0s` or `skipped`.
   - Enforce `created_at >= cutoff_window` unconditionally (never bypass `cutoff_window` on `< 20` runs).
   - Never fabricate `provenance_mode = "live_telemetry"` if `/actions/runs` failed or was rate-limited (`403`/`401`); retry unauthenticated if an ambient/keychain token is expired (`401`) and return `Err(...)` when runs cannot be fetched.

2. **Untruncated vs. Truncated Window Extrapolation (`telemetry.rs`, `worker.js`):**
   - **Untruncated** (pagination reached `cutoff_window` or end of history):
     $$r_{30} = N_{\text{obs}} \times \frac{30.0}{\text{window\_days}}$$
     (`1` run in a `30d` window = `1.0` run/month, never `30.0` runs/month).
   - **Truncated** (API pagination cap hit before `cutoff_window`):
     $$r_{30} = N_{\text{obs}} \times \frac{30.0}{\max(1/1440,\; (t_{\text{now}} - t_{\text{oldest}}) / 86400)}$$

3. **Algebraically Exact End-of-Month (EOM) Burn Velocity (`engine.rs`):**
   - Historical 30-day gross spend decomposes into prior 23 days at daily rate $s_0$ and last 7 days at daily rate $s_1 = v \cdot s_0$:
     $$G_{30} = 23 s_0 + 7 (v s_0) = s_0 (7v + 23)$$
   - Projecting a full 30-day month at current 7-day pace $30 s_1 = 30 v s_0$ is therefore:
     $$\text{projected\_eom\_compute\_usd} = \text{round}\!\left(G_{30} \times \frac{30 v}{7v + 23},\; 2\right)$$
   - Require `eff_sample_runs >= 5.0` before activating `spend_anomaly_alert_active` (`v >= 1.50`).

---

## 3. Workflow AST, Matrices, Cron Schedules & Per-Workflow Cadence (`C1–C7, D1–D8`)

1. **Universal Per-Workflow Cadence (`ExpandedJobCost.monthly_runs_for_job`):**
   - Every job carries `monthly_runs_for_job`:
     - **Static PR/push workflows**: `round(prs_per_week * pushes_per_pr * 4.33)` (`433.0` default).
     - **Static `schedule`-only workflows**: exact 5-field cron monthly runs (e.g., `0 2 1 * *` = `1.0/mo`; `0 9-17 * * 1-5` = `9 * (5/7) * 30.4375 = 195.7/mo`).
     - **Static `workflow_dispatch`-only workflows**: `4.0/mo` (manual release cadence, never `433/mo`).
     - **Live Telemetry**: exact per-workflow observed 30d run count (preserving periodic cron schedules when a high-velocity sample window is truncated).
   - Schedule-only and manual-only workflows contribute `0.0m` to `human_wait_mins_per_run` (no developer sits blocked on a 2 AM cron).

2. **Exact Cartesian Hyperplane `exclude:` & Multi-OS Matrix Bucketing (`engine.rs`):**
   - Enumerate the full Cartesian product over matrix axes (`<= 4,096` cells) so a partial `exclude: [{os: macos-latest}]` removes the entire matching hyperplane across all other axes, then bucket surviving cells by OS (`linux`, `windows`, `macos`) so each OS receives its exact SKU rate and quota multiplier while `total_defined_jobs` counts distinct `(workflow_file, base_job_id)`.

3. **Thread-Safe & Telemetry-Symmetric `minici diff` (`main.rs`):**
   - Git ref extraction uses an atomic sequence counter (`minici_gitref_<pid>_<seq>_<ref>`) and copies `.minici/telemetry.lock.json` from the repo root into the extracted ref tree so `diff(HEAD, HEAD)` is **identically `$0.00`** in both Static and Live modes.

---

## 4. Allowance Pool Commutativity, Row-Sum Apportionment & CSV Reconciliation (`E1–E7, F1–F7, H1–H7`)

1. **Order-Invariant Proportional Pool Deduction (`providers.rs`):**
   - Allowance-eligible jobs convert into pool units $P_j = B_j r_j m_j$ (`GitHub`: `1x/2x/10x`; `GitLab`: `rate / $0.010`; `CircleCI`: `rate / $0.006` + `200`-credit `$0.12` DLC flat fee; `Bitbucket`: `rate / $0.010`).
   - Uncovered fraction $\alpha = \max(0, \sum P_j - Q) / \sum P_j$ scales eligible gross compute invariant to YAML job ordering:
     $$\text{Net}_{\text{eligible}} = \alpha \sum_{j \in \text{eligible}} G_j$$
   - Larger Runners (`>2` vCPU on GitHub/GCP/CodeBuild Lambda, macOS Large/XLarge, GPU) and 3rd-party/self-hosted runners bypass free quota (`uses_allowance = false`), including on public OSS repositories.

2. **Hamilton Largest-Remainder Row-Sum Apportionment (`engine.rs`, `renderer.rs`):**
   - Invoice gross is rounded once from unrounded job totals: $G = \text{round}(\sum_j c_j r_j, 2)$, and $\text{Discount} = \text{round}(\max(0, G - \text{Net}), 2)$ so $G - \text{Discount} \equiv \text{Net}$.
   - Per-job monthly amounts in CLI, Markdown, HTML, CSV, and JSON (`monthly_compute_cost_usd`) use Hamilton (Largest-Remainder) cent apportionment over integer cents $\text{round}(100 G)$, guaranteeing $\sum_j \text{row}_j \equiv G$ with **`$0.00` (`0` cents) rounding error**.

3. **Multiplicative Optimization Compounding (`optimizer.rs`, `engine.rs`):**
   - Overlapping compute reductions ($r_{\text{arm}}, r_{\text{cancel}}, r_{\text{cache}}, r_{\text{flake}}$) compound multiplicatively:
     $$\Delta G_{\text{compute}} = G \cdot \left(1 - \prod_k (1 - r_k)\right)$$
     with artifact storage savings added separately without being clipped by compute-only caps.

4. **Hot-Reload Override Spot/vCPU Guard & String-Aware JSON Lockfile Parsing (`catalog.rs`, `telemetry.rs`, `worker.js`):**
   - `apply_live_override` honors exact `raw_label` overrides first, never overwrites `-spot`/`-preemptible` queries with bare On-Demand `sku_label` rates, and never overwrites dynamically vCPU-rescaled templates (`runs-on=runner=8cpu-...`) with 2/4-vCPU base template prices.
   - `TelemetrySnapshot::from_json_str` parses arrays via string-escape-aware bracket matching (`iter_json_array_objects`) so OS-bucketed job keys (`ci.yml::build [os:macos]`) never truncate on `]` inside string literals.
   - Edge Worker (`worker.js`) `/v1/pricing/rates` and live/static repo calculators maintain `0.000000` SKU rate parity with `pricing/*.json` and per-second billing on 3rd-party microVM & self-hosted runners.
