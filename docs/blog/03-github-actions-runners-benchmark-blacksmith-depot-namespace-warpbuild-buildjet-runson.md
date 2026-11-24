# GitHub Actions Alternative Runners Benchmarked (`Blacksmith`, `Depot`, `Namespace`, `WarpBuild`, `BuildJet` & `RunsOn`) — Verified `vCPU + RAM` Pricing & Live OSS Case Studies with MiniCI

> **SEO & AI Search Metadata (GEO / AIO Ready)**
> - **Suggested URL Slug:** `/blog/github-actions-runners-benchmark-blacksmith-depot-namespace-warpbuild-buildjet-runson`
> - **Meta Title:** `Blacksmith vs Depot vs Namespace vs WarpBuild vs GitHub Runners`
> - **Meta Description:** `Hardware-matched (vCPU + RAM) GitHub Actions runner pricing benchmark across Blacksmith, Depot, Namespace, WarpBuild, BuildJet & RunsOn on real OSS repos with MiniCI.`
> - **Target Keywords:** `GitHub Actions alternative runners`, `Blacksmith vs Depot vs Namespace`, `WarpBuild vs BuildJet pricing`, `RunsOn GitHub Actions cost`, `GitHub Larger Runners pricing per minute`, `reduce GitHub Actions runner cost`, `MiniCI`

---

## Why Runner Comparisons Fail Without `vCPU + RAM` Parity and Billing-Unit Math

Comparing GitHub Actions runner providers by headline percentage claims is often misleading unless two variables are held strictly constant:

1. **Matched `vCPU + RAM` Hardware Tiers:** Comparing a `2 vCPU` runner against an `8 vCPU` runner—or ignoring RAM differences (`7 GB` vs. `16 GB` vs. `32 GB`)—distorts unit economics.
2. **Billing Granularity (`Whole-Minute Ceiling` vs. `Fractional/Per-Second Billing`):** Standard and Larger GitHub-hosted runners round every expanded matrix job up to the next whole minute (`ceil(duration_minutes)`). High-speed third-party MicroVM providers (**Blacksmith**, **Depot**, **Namespace**, **WarpBuild**, **BuildJet**) and BYO-cloud orchestrators (**RunsOn**) bill fractional minutes—meaning a `2.1-minute` shard bills `2.1m`, not `3.0m`.

**[MiniCI](https://github.com/minici-dev/minici-cost-action)** (`minici v0.9.0`) compiles all `7` runner ecosystems—including exact `vCPU`, `RAM (GB)`, `OS/Arch`, per-minute rates, and `ceil_to_whole_minute` billing rules—into a `1.6 MiB` zero-dependency Rust binary. Below is the hardware-matched verification matrix and live results from **6 major open-source repositories already running these providers in production**.

---

## Hardware-Matched (`vCPU + RAM`) Pricing & Billing Matrix (`2026` Verified in MiniCI)

Every `runs-on` label below was verified directly through `minici breakdown` and `minici catalog`:

| Hardware Tier (`OS • Arch`) | Provider & `runs-on` Label | `vCPU` | `RAM` | Billing Unit | Rate / Min (`USD`) | vs. Matched GitHub Hosted |
| :--- | :--- | :---: | :---: | :---: | :---: | :--- |
| **Linux Small** (`x86_64`) | **GitHub Hosted** (`ubuntu-latest`) | `2` | `7 GB` | `1-min ceiling` | `$0.0060/m` | Baseline |
| **Linux Small** (`arm64`) | **GitHub Hosted ARM** (`ubuntu-24.04-arm`) | `2` | `7 GB` | `1-min ceiling` | `$0.0050/m` | `-16.7%` (still `1-min` ceil) |
| **Linux Small** (`x86_64`) | **Blacksmith / Depot / WarpBuild / BuildJet** (`*-2vcpu-*` / `depot-ubuntu-24.04`) | `2` | `8 GB` | **Fractional** | **`$0.0040/m`** | **`-33.3%` rate + `0` rounding** |
| **Linux Small** (`x86_64`) | **RunsOn BYO AWS EC2** (`runs-on-2vcpu-linux-x64`) | `2` | `8 GB` | **Fractional** | **`$0.0012/m`** | **`-80.0%` (in your AWS acct)** |
| **Linux Medium** (`x86_64`) | **GitHub Larger Runner** (`ubuntu-4core`) | `4` | `16 GB` | `1-min ceiling` | `$0.0120/m` | Baseline (`0` free mins) |
| **Linux Medium** (`x86_64`) | **Blacksmith / Depot / Namespace / WarpBuild / BuildJet** (`4vcpu` tier) | `4` | `16 GB` | **Fractional** | **`$0.0080/m`** | **`-33.3%` rate + `0` rounding** |
| **Linux Medium** (`x86_64`) | **RunsOn BYO AWS EC2** (`runs-on-4vcpu-linux-x64` / `aws-ec2-runs-on`) | `4` | `16 GB` | **Fractional** | **`$0.0024/m`** | **`-80.0%` (in your AWS acct)** |
| **Linux Large** (`x86_64`) | **GitHub Larger Runner** (`ubuntu-8core`) | `8` | `32 GB` | `1-min ceiling` | `$0.0220/m` | Baseline (`0` free mins) |
| **Linux Large** (`x86_64`) | **Blacksmith / Depot / Namespace / WarpBuild / BuildJet** (`8vcpu` tier) | `8` | `32 GB` | **Fractional** | **`$0.0160/m`** | **`-27.3%` rate + `0` rounding** |
| **Linux Large** (`x86_64`) | **RunsOn BYO AWS EC2** (`runs-on-8vcpu-linux-x64`) | `8` | `32 GB` | **Fractional** | **`$0.0048/m`** | **`-78.2%` (in your AWS acct)** |
| **Linux XLarge** (`x86_64`) | **GitHub Larger Runner** (`ubuntu-16core`) | `16` | `64 GB` | `1-min ceiling` | `$0.0420/m` | Baseline (`0` free mins) |
| **Linux XLarge** (`x86_64`) | **Blacksmith / Depot / WarpBuild / BuildJet** (`16vcpu` tier) | `16` | `64 GB` | **Fractional** | **`$0.0320/m`** | **`-23.8%` rate + `0` rounding** |
| **Linux XLarge** (`x86_64`) | **RunsOn BYO AWS EC2** (`runs-on-16vcpu-linux-x64`) | `16` | `64 GB` | **Fractional** | **`$0.0096/m`** | **`-77.1%` (in your AWS acct)** |
| **macOS Silicon** (`arm64`) | **GitHub Hosted** (`macos-latest` M1/M2) | `3` | `7 GB` | `1-min ceiling` | `$0.0620/m` | Baseline (`10x` quota burn) |
| **macOS Silicon** (`arm64`) | **Namespace / Blacksmith / WarpBuild** (`namespace-profile-macos-15` / `*-macos-*`) | `4` | `14 GB` | **Fractional** | **`$0.0320/m`** | **`-48.4%` + `2x` RAM (`14 GB`)** |

> **Tip:** Have an enterprise committed-spend discount with Blacksmith, Depot, Namespace, WarpBuild, BuildJet, or AWS? Pin your exact contracted rate in `< 1 ms` via `minici catalog set <runner_label> <usd_per_min>` (stored locally in `~/.minici/pricing_overrides.json`).

---

## How MiniCI Renders All 6 Providers Side-by-Side (`2.51 ms` Live Run)

Running `minici breakdown` on a multi-provider verification workflow (`test.yml`, where every job executes an identical `3.35m` raw `checkout + pytest` workload) shows exact SKU resolution and fractional billing across all `6` ecosystems:

```text
$ minici breakdown --path ./verify_labels

Project: ./verify_labels (1 workflow(s), 26 expanded jobs • 2.51 ms)

 WORKFLOW / JOB                              MATRIX × RUNNER           BILLED MINS    RATE/MIN    COST/RUN   MONTHLY COST
 ──────────────────────────────────────────  ────────────────────────  ───────────  ──────────  ──────────  ─────────────
 test.yml • bs4                              1x blacksmith-4vcpu-u...         3.4m   $0.0080/m      $0.027      $11.60/mo
 test.yml • bs8                              1x blacksmith-8vcpu-u...         3.4m   $0.0160/m      $0.054      $23.21/mo
 test.yml • dp4                              1x depot-ubuntu-24.04-4          3.4m   $0.0080/m      $0.027      $11.60/mo
 test.yml • dp8                              1x depot-ubuntu-24.04-8          3.4m   $0.0160/m      $0.054      $23.21/mo
 test.yml • ns4                              1x namespace-profile-...         3.4m   $0.0080/m      $0.027      $11.60/mo
 test.yml • nsmac                            1x namespace-profile-...         3.4m   $0.0320/m      $0.107      $46.42/mo
 test.yml • wb4                              1x warpbuild-ubuntu-2...         3.4m   $0.0080/m      $0.027      $11.60/mo
 test.yml • bj4                              1x buildjet-4vcpu-ubu...         3.4m   $0.0080/m      $0.027      $11.60/mo
 test.yml • ro4                              1x runs-on-4vcpu-linu...         3.4m   $0.0024/m      $0.008       $3.46/mo
 test.yml • ro8                              1x runs-on-8vcpu-linu...         3.4m   $0.0048/m      $0.016       $6.97/mo
```

Notice that an identical `3.35m` job on GitHub's `ubuntu-4core` (`$0.0120/m`) rounds up to `4.0m` (`$0.048/run` = **`$20.78/mo`**), whereas `blacksmith-4vcpu`, `depot-ubuntu-24.04-4`, `namespace-profile-linux-x86-64-4`, `warpbuild-ubuntu-2404-4vcpu`, and `buildjet-4vcpu` bill exact fractional runtime (`3.35m × $0.0080/m` = **`$11.60/mo`, `-44.2%` combined savings**), and `runs-on-4vcpu` bills **`$3.46/mo`** in BYO AWS EC2 compute.

---

## Live Production Case Studies: 6 Real Open-Source Repos Audited with MiniCI

Several of the world's fastest-growing open-source teams already mix GitHub-hosted runners with Blacksmith, Depot, Namespace, and RunsOn. Running `minici breakdown` (normalized at `433 runs/mo` on Apple M4 Pro, `< 7 MiB` RSS RAM) quantifies the exact savings those specific jobs realize vs. hardware-matched (`same vCPU, OS & Arch`) GitHub-hosted runners:

| Repository | Active Runner Provider(s) | Jobs on Custom Runners | Detected Production `runs-on` SKUs | Billed Mins (`Fractional` vs. `GH Ceil`) | Current Provider Cost | Same-Spec GitHub Hosted | Verified Monthly Savings |
| :--- | :--- | :---: | :--- | :---: | :---: | :---: | :---: |
| **`astral-sh/uv`** | **Depot + Namespace** | `30 / 243` | `depot-ubuntu-24.04-{4,8,16}`, `namespace-profile-macos-15`, `namespace-profile-windows-2022-x86-64-{4,16x32}` | `143.00m` vs. `160.00m` | **`$1,273.02/mo`** | `$2,776.40/mo` | **`$1,503.38/mo` (`-54.1%`)** |
| **`mendableai/firecrawl`** | **Blacksmith** | `50 / 82` | `blacksmith-{2,4}vcpu-ubuntu-2404`, `blacksmith-4vcpu-ubuntu-2404-arm` | `602.15m` vs. `626.00m` | **`$1,955.60/mo`** | `$3,005.89/mo` | **`$1,050.28/mo` (`-34.9%`)** |
| **`langgenius/dify`** | **Depot** | `79 / 117` | `depot-ubuntu-24.04` (`2 vCPU`), `depot-ubuntu-24.04-4` (`4 vCPU`) | `286.00m` vs. `322.00m` | **`$719.65/mo`** | `$1,208.07/mo` | **`$488.42/mo` (`-40.4%`)** |
| **`n8n-io/n8n`** | **Blacksmith** | `21 / 44` | `blacksmith-{2,4,8}vcpu-ubuntu-2204` | `214.35m` vs. `218.00m` | **`$757.49/mo`** | `$1,144.85/mo` | **`$387.36/mo` (`-33.8%`)** |
| **`supabase/supabase`** | **Blacksmith** | `19 / 25` | `blacksmith-{4,8}vcpu-ubuntu-2404` | `62.35m` vs. `68.00m` | **`$255.47/mo`** | `$405.29/mo` | **`$149.82/mo` (`-37.0%`)** |
| **`pydantic/pydantic`** | **RunsOn (AWS EC2)** | `9 / 175` | `aws-ec2-runs-on` (`4 vCPU`) | `25.70m` vs. `29.00m` | **`$26.67/mo`** | `$150.68/mo` | **`$124.01/mo` (`-82.3%`)** |

### Contextual Intelligence: MiniCI Never Double-Counts Already-Optimized Fleets
When you run `minici breakdown` on **`supabase/supabase`** (`19/25` jobs already on Blacksmith) or **`mendableai/firecrawl`** (`50/82` jobs already on Blacksmith), `minici` recognizes the custom runners, excludes them from GitHub's `20`-job concurrency cap (`Concurrency: 25 peak jobs (6 hosted / 19 custom)`), and skips generic runner-migration advice in favor of the actual remaining bottleneck:

```text
$ minici breakdown --repo supabase/supabase
...
 OVERALL TOTAL (433 runs/mo • paid tier)     25 expanded job(s)              73.3m           —      $0.656     $284.05/mo

 • Free-Tier Quota  : 31761 / 2000 mins (238.2% of monthly quota) • Net Billed: $272.05/mo • Rounding Overhead: 1.6m/run ($6.39/mo)
 • Dev Wait-Time    : 7.7m critical path ($3.34/run • $1446.22/mo at 40% DORA context-switch loss) • Concurrency: 25 peak jobs (6 hosted / 19 custom)
 • Artifact Storage : 2 upload job(s) • 90d retention ($162.38/mo, 649.50 GB steady-state) (14d retention reduces storage to $25.26/mo)
 • Recommendation   : Pipeline compute already optimized (19/25 jobs on custom runners); set artifact `retention-days: 14` to cut storage from $162.38/mo to $25.26/mo
```

---

## Verifying a Runner Migration in a Pull Request (`minici diff` in `364 µs`)

When evaluating a move from GitHub Larger/macOS runners (`4x ubuntu-4core`, `1x ubuntu-8core`, `1x macos-latest`) to hardware-matched third-party runners (`4x blacksmith-4vcpu-ubuntu-2404`, `1x depot-ubuntu-24.04-8`, `1x namespace-profile-macos-15`), `minici diff` and `minici comment github` prove the exact Pull Request savings before merge:

```text
$ minici diff --path . --compare-to main

+====================================================================================+
| MINICI — PULL REQUEST COST DIFF (364 µs • Exit Code 0 Informational)                |
+====================================================================================+
Comparing            : `git-ref:main` (Base) -> `./runner-migration-demo` (PR Head)
1. Matrix Jobs       : 6 jobs -> 6 jobs (+0 jobs)
2. Billed Mins / Run : 27.0m -> 23.6m (-3.4m / run)
3. Monthly CI Cost   : $257.20/mo -> $140.29/mo (-$116.91/mo • -45.5%)
4. Dev Wait Cost ($) : $1286.01/mo -> $1286.01/mo (+$0.00/mo) [Assumed Rate Configurable]
5. Key Change        : Modified `api-tests` (`.github/workflows/ci.yml`): `4x ubuntu-4core` (16.0m • $0.192/run) -> `4x blacksmith-4vcpu-ubuntu-2404` (13.4m • $0.107/run) (total changes: 3)
+====================================================================================+
```

---

## When to Pick Managed MicroVMs vs. BYO-Cloud (`RunsOn`) vs. GitHub Hosted

1. **Managed High-Speed Runners (`Blacksmith`, `Depot`, `Namespace`, `WarpBuild`, `BuildJet`):** Best when you want zero AWS/IAM maintenance, `1`-line `runs-on:` adoption, co-located NVMe build caches, fractional-minute billing, and `33%–48%` lower per-minute rates at matched `vCPU + RAM`. (`Depot` and `Namespace` also shine on native container build acceleration and Windows/macOS profiles, as seen in `astral-sh/uv` and `langgenius/dify`.)
2. **BYO AWS EC2 Orchestration (`RunsOn`):** Best when you already manage an AWS organization, want `75%–82%` raw compute savings (`$0.0024/m` `4-vCPU` On-Demand or Spot), and keep S3/ECR Gateway VPC Endpoints configured so private-subnet NAT Gateway charges stay near `$0`.
3. **GitHub Hosted (`ubuntu-slim`, `ubuntu-24.04-arm`, `ubuntu-latest`):** Optimal for lightweight `1–2` minute lint, PR-title, and governance checks (`$0.0020/m` on `ubuntu-slim` or covered by included plan minutes)—exactly why `n8n-io/n8n`, `supabase/supabase`, and `mendableai/firecrawl` keep lightweight checks on GitHub Hosted while routing heavy test/build jobs to custom runners.

---

## Verify on Your Own Repository in `30 Seconds`

```bash
# Install MiniCI (zero-dependency Rust CLI)
cargo install --locked --git https://github.com/minici-dev/minici-cost-action.git --rev 45f57abdaf9e2987451d8767fb2e164adf22c864 minici

# Audit live OSS repos using Blacksmith, Depot, Namespace & RunsOn:
minici breakdown --repo supabase/supabase
minici breakdown --repo n8n-io/n8n
minici breakdown --repo langgenius/dify
minici breakdown --repo astral-sh/uv
minici breakdown --repo pydantic/pydantic

# Inspect all compiled runner & cloud VM rates:
minici catalog
```

---

## Frequently Asked Questions (FAQ)

### Why do Blacksmith, Depot, Namespace, WarpBuild, and BuildJet bill fewer minutes than GitHub Actions for the exact same runtime?
GitHub Actions rounds every job's duration up to the next whole minute (`ceil(minutes)`), so ten `2.1-minute` matrix shards bill `30.0` minutes. Managed MicroVM runner providers bill per second / fractional minute, so those same ten `2.1-minute` shards bill `21.0` minutes—saving `30%` in billed time before per-minute rate discounts are even applied.

### How do I choose between Managed MicroVM runners and RunsOn (self-hosted AWS EC2)?
Managed MicroVM providers (`Blacksmith`, `Depot`, `Namespace`, `WarpBuild`, `BuildJet`) require zero cloud infrastructure management and save `33%–54%` vs. matched GitHub Larger/macOS runners. `RunsOn` runs ephemeral VMs inside your own AWS account, saving `77%–82%` on compute, provided you manage AWS quotas and configure S3/ECR VPC Endpoints to avoid `$0.045/GB` NAT Gateway fees.

### Does MiniCI work with custom enterprise runner labels?
Yes. MiniCI automatically detects `blacksmith-*`, `depot-*`, `namespace-profile-*`, `warpbuild-*`, `buildjet-*`, `runs-on-*`, and `self-hosted*` labels out of the box, and supports custom per-label rate overrides via `minici catalog set <label> <usd_per_min>`.
