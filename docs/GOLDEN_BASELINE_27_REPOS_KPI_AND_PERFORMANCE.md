# Locked 25-Repository Golden Baseline: KPIs, 24 Unified CI/CD Cost Models & 3-Mode Latency (`v0.9.0`)

**Locked Version:** `minici 0.9.0` (`tag: v0.9.0`)  
**Total Repositories Locked:** `25` (`9` Primary + `6` Internal Edge-Case + `10` Fresh World-Scale OSS)
**Execution Modes Verified on Every Repo:** `Mode 1: CLI` • `Mode 2: GitHub Action Workflow` • `Mode 3: MCP JSON-RPC 2.0 (7 Tools) + Shift-Left Hook`  
**Machine-Readable Companion Lock:** [`tests/golden_baseline_10_repos.json`](../tests/golden_baseline_10_repos.json)

## How to Re-Verify Zero Deviation in Future Sessions

```bash
# Run the 25-repository parallel verification gate (< 3s):
bash scripts/validate_minici_cost_10_repos.sh

# Or invoke the verifier directly:
python3 tools/verify_10_repos_all_modes.py
```

If any commit alters job expansion, 1-minute ceiling rounding, SKU pricing, or any of the 24 unified CI/CD cost models (`6` invoice components + `5` hidden payroll/pipeline models + `5` team-stage & pipeline-shape models + `4` multi-vendor calculator & stack intelligence models + `4` unit economics, anomaly/timeout, container registry & fleet SLA models) on any of the 25 repositories, `verify_10_repos_all_modes.py` exits non-zero and prints the exact field delta.

---

## Part 1: 9 Primary Repositories (`2 Internal + 7 Modern OSS`) — Core KPIs & 3-Mode Performance

| Repository | Category | WF / Def / Exp Jobs | Billed Mins/Run | Gross Monthly Compute | Free-Tier Burn % | Monthly Dev Wait (`$65/hr`) | Engine (`ms`) | CLI (`ms`) | Workflow (`ms`) | MCP 4-RPC (`ms`) |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **`mgjartproject/FineArts-BE`** | Internal (Grassy Fine Arts • Private) | `2` / `6` / `6` | `18.00m` | **`$46.76/mo`** | `389.7%` | `$1,191.47/mo` (`6.35m`) | `2.13` | `10.83` | `7.32` | `53.38` |
| **`Eazy-Ops/Backend`** | Internal (Eazy-Ops • Private) | `5` / `5` / `5` | `25.00m` | **`$64.95/mo`** | `541.3%` | `$2,775.10/mo` (`14.79m`) | `2.42` | `10.87` | `8.76` | `56.13` |
| **`astral-sh/uv`** | Fresh OSS (Rust/Python Packaging) | `47` / `220` / `243` | `724.74m` | **`$323.71/mo`** | `2718.0%` | `$2,157.78/mo` (`24.01m`) | `40.78` | `50.09` | `37.64` | `159.18` |
| **`denoland/deno`** | Fresh OSS (JS/TS Runtime) | `11` / `51` / `162` | `1551.00m` | **`$10,109.35/mo`** | `85391.7%` | `$6,606.57/mo` (`35.21m`) | `20.92` | `26.73` | `24.98` | `111.38` |
| **`ggml-org/llama.cpp`** | Fresh OSS (C++ LLM Inference) | `62` / `121` / `148` | `839.40m` | **`$3,226.86/mo`** | `22890.2%` | `$3,020.90/mo` (`18.3m`) | `32.54` | `40.25` | `38.48` | `135.02` |
| **`immich-app/immich`** | Fresh OSS (Photo/Video Cloud) | `27` / `74` / `85` | `250.00m` | **`$983.13/mo`** | `6716.9%` | `$2,007.68/mo` (`10.7m`) | `12.40` | `17.38` | `12.99` | `84.21` |
| **`langgenius/dify`** | Fresh OSS (Agentic AI Platform) | `41` / `82` / `117` | `341.51m` | **`$306.41/mo`** | `954.0%` | `$2,523.67/mo` (`13.45m`) | `22.58` | `27.52` | `25.86` | `116.32` |
| **`open-webui/open-webui`** | Fresh OSS (AI Web Interface) | `7` / `13` / `18` | `74.00m` | **`$187.10/mo`** | `1559.2%` | `$3,489.98/mo` (`18.6m`) | `3.97` | `11.20` | `7.88` | `61.28` |
| **`mendableai/firecrawl`** | Fresh OSS (AI Web Scraper) | `36` / `71` / `82` | `570.72m` | **`$1,763.61/mo`** | `1234.1%` | `$13,560.26/mo` (`72.27m`) | `14.90` | `20.59` | `19.01` | `95.70` |

### Part 1B: 9 Primary Repositories — 6 Core CI/CD Invoice Components (`$/mo`)

| Repository | C1: Rounding Tax | C2: Self-Hosted EC2 (`Spot $/mo`) | C3: Artifact (`90d` -> `S3 14d`) | C4: Parallelism Tax | C5: Log Profile (`External 90d`) | C6: Egress (`NAT Worst` -> `VPC+Cache`) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **`mgjartproject/FineArts-BE`** | `$11.69` | `t3.medium (2 vCPU / 4 GB)` (`$36.97`) | `$0.00` -> `$0.00` | `$0.00` | Python / pytest (0.3-1.2 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`Eazy-Ops/Backend`** | `$6.52` | `t3.medium (2 vCPU / 4 GB)` (`$41.65`) | `$0.00` -> `$0.00` | `$0.00` | Node / npm / Vitest (0.2-0.8 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`astral-sh/uv`** | `$46.47` | `c5.4xlarge (16 vCPU / 32 GB)` (`$89.82`) | `$2.25` -> `$0.03` | `$510.00` | Rust / cargo test (0.6-2.0 MB/build) (`$0.00`) | `$181.22` -> `$8.63` |
| **`denoland/deno`** | `$399.62` | `c5.4xlarge (16 vCPU / 32 GB)` (`$3,128.65`) | `$237.68` -> `$2.66` | `$510.00` | Rust / cargo test (0.6-2.0 MB/build) (`$0.00`) | `$1,813.04` -> `$86.34` |
| **`ggml-org/llama.cpp`** | `$248.64` | `c5.4xlarge (16 vCPU / 32 GB)` (`$881.58`) | `$213.38` -> `$2.39` | `$510.00` | iOS / xcodebuild (2.0-8.0 MB/build) (`$0.00`) | `$941.35` -> `$44.83` |
| **`immich-app/immich`** | `$88.59` | `m6g.xlarge (Arm 4 vCPU / 16 GB)` (`$226.38`) | `$243.56` -> `$2.73` | `$100.00` | iOS / xcodebuild (2.0-8.0 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`langgenius/dify`** | `$13.35` | `m5.xlarge (4 vCPU / 16 GB)` (`$107.96`) | `$73.22` -> `$0.82` | `$0.00` | Python / pytest (0.3-1.2 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`open-webui/open-webui`** | `$15.47` | `t3.medium (2 vCPU / 4 GB)` (`$64.26`) | `$162.38` -> `$1.82` | `$0.00` | Python / pytest (0.3-1.2 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`mendableai/firecrawl`** | `$35.12` | `m6g.xlarge (Arm 4 vCPU / 16 GB)` (`$535.71`) | `$243.56` -> `$2.73` | `$100.00` | Rust / cargo test (0.6-2.0 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |

### Part 1C: 9 Primary Repositories — 5 Hidden Payroll & Pipeline Hygiene Cost Models (`$/mo`)

| Repository | H1: 40% Context-Switch Payroll Loss | H2: Rerun Flake Cost (`12%`) | H3: Cache Coverage (`Unrealized Savings`) | H4: Combined Flake Tax (`+ Triage`) | H5: Missing `cancel-in-progress` Waste | H5: Missing Path-Filter Savings |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **`mgjartproject/FineArts-BE`** | `$1,191.47/mo` | `$190.31/mo` | `0/6` (`$5.20/mo`) | `$570.93/mo` | `2` wf (`$7.01/mo`) | `2` wf (`$18.71/mo`) |
| **`Eazy-Ops/Backend`** | `$2,775.10/mo` | `$191.27/mo` | `1/5` (`$3.51/mo`) | `$573.81/mo` | `5` wf (`$9.74/mo`) | `5` wf (`$25.98/mo`) |
| **`astral-sh/uv`** | `$2,157.78/mo` | `$411.02/mo` | `63/243` (`$128.91/mo`) | `$1,233.06/mo` | `4` wf (`$3.51/mo`) | `4` wf (`$9.35/mo`) |
| **`denoland/deno`** | `$6,606.57/mo` | `$1,498.31/mo` | `130/162` (`$14.71/mo`) | `$4,494.93/mo` | `1` wf (`$0.39/mo`) | `2` wf (`$4,032.96/mo`) |
| **`ggml-org/llama.cpp`** | `$3,020.90/mo` | `$733.59/mo` | `77/148` (`$430.37/mo`) | `$2,200.77/mo` | `8` wf (`$203.75/mo`) | `5` wf (`$4.16/mo`) |
| **`immich-app/immich`** | `$2,007.68/mo` | `$484.29/mo` | `13/85` (`$107.57/mo`) | `$1,452.87/mo` | `10` wf (`$11.30/mo`) | `17` wf (`$222.74/mo`) |
| **`langgenius/dify`** | `$2,523.67/mo` | `$409.10/mo` | `53/117` (`$47.18/mo`) | `$1,227.30/mo` | `9` wf (`$6.14/mo`) | `6` wf (`$63.86/mo`) |
| **`open-webui/open-webui`** | `$3,489.98/mo` | `$197.70/mo` | `1/18` (`$31.70/mo`) | `$593.10/mo` | `2` wf (`$2.34/mo`) | `4` wf (`$62.35/mo`) |
| **`mendableai/firecrawl`** | `$13,560.26/mo` | `$571.01/mo` | `28/82` (`$109.06/mo`) | `$1,713.03/mo` | `30` wf (`$71.47/mo`) | `2` wf (`$508.48/mo`) |

### Part 1D: 9 Primary Repositories — 5 Dynamic Team-Stage & Pipeline-Shape Cost Models

| Repository | S1: Inferred Team Stage (`Devs` • `Mac mini Payback`) | S2: Recoverable Waste (`Platform Eng ROI %`) | S3: Compliance Controls (`Residency` • `1yr SIEM`) | S4: Pipeline Shape (`Units` • `Tool` • `Affected Savings`) | S5: Setup Share (`Shared Base Savings` • `Unpinned`) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **`mgjartproject/FineArts-BE`** | `startup-seed-series-a (6-15 devs)` (`5` devs • `0.00m`) | `$534.19/mo` (`3.24%`) | `3` (`1.00x` • `$1.01/mo`) | `single-service` (`0` • `none` • `$0.00/mo`) | `45.0%` (`$12.62/mo` • `0`) |
| **`Eazy-Ops/Backend`** | `startup-seed-series-a (6-15 devs)` (`7` devs • `0.00m`) | `$1,017.76/mo` (`6.17%`) | `2` (`1.00x` • `$0.56/mo`) | `single-service` (`0` • `none` • `$0.00/mo`) | `44.5%` (`$17.33/mo` • `1`) |
| **`astral-sh/uv`** | `scaleup (25-100 devs)` (`34` devs • `0.00m`) | `$1,140.37/mo` (`6.91%`) | `3` (`1.00x` • `$59.79/mo`) | `monorepo` (`5` • `path-filters-only` • `$113.30/mo`) | `45.0%` (`$87.40/mo` • `0`) |
| **`denoland/deno`** | `enterprise (250+ devs)` (`396` devs • `0.12m`) | `$6,974.41/mo` (`42.27%`) | `2` (`1.00x` • `$39.86/mo`) | `single-service` (`0` • `none` • `$0.00/mo`) | `13.7%` (`$831.59/mo` • `0`) |
| **`ggml-org/llama.cpp`** | `scaleup (25-100 devs)` (`194` devs • `0.79m`) | `$2,118.43/mo` (`12.84%`) | `1` (`1.00x` • `$148.98/mo`) | `multi-service-microservices` (`2` • `path-filters-only` • `$0.00/mo`) | `23.8%` (`$460.41/mo` • `0`) |
| **`immich-app/immich`** | `scaleup (25-100 devs)` (`61` devs • `2.96m`) | `$1,321.19/mo` (`8.01%`) | `3` (`1.00x` • `$85.56/mo`) | `monorepo` (`20` • `path-filters-only` • `$344.10/mo`) | `45.0%` (`$265.45/mo` • `2`) |
| **`langgenius/dify`** | `scaleup (25-100 devs)` (`37` devs • `0.00m`) | `$1,220.28/mo` (`7.40%`) | `1` (`1.00x` • `$19.63/mo`) | `monorepo` (`67` • `path-filters-only` • `$107.24/mo`) | `44.6%` (`$81.96/mo` • `0`) |
| **`open-webui/open-webui`** | `scaleup (25-100 devs)` (`20` devs • `0.00m`) | `$1,286.41/mo` (`7.80%`) | `3` (`1.00x` • `$3.02/mo`) | `multi-service-microservices` (`0` • `none` • `$0.00/mo`) | `45.0%` (`$50.52/mo` • `1`) |
| **`mendableai/firecrawl`** | `scaleup (25-100 devs)` (`150` devs • `0.00m`) | `$5,153.56/mo` (`31.23%`) | `2` (`1.00x` • `$20.18/mo`) | `monorepo` (`16` • `path-filters-only` • `$617.26/mo`) | `18.4%` (`$194.70/mo` • `0`) |

### Part 1E: 9 Primary Repositories — 4 Calculator & Stack Intelligence Pillars (`P17–P20`)

| Repository | P17: Cheapest Vendor TCO (`2x Migration Rule`) | P18: Best First Phase (`Reducible $/mo`) | P19: Compute Profile (`Larger` • `ARM64 Savings` • `Best Tier`) | P20: Stack Archetype (`Stack Savings $/mo`) |
| :--- | :---: | :---: | :---: | :---: |
| **`mgjartproject/FineArts-BE`** | `buildkite-pro-byo` (`$30.00/mo` • 2x:`False`) | `dependency_install` (`$24.88/mo`) | `balanced-web-backend` (`0` • `$7.79/mo` • `larger-16-core`) | `python-uv-pytest` (`$11.69/mo`) |
| **`Eazy-Ops/Backend`** | `gcp-cloud-build` (`$49.95/mo` • 2x:`False`) | `dependency_install` (`$27.05/mo`) | `balanced-web-backend` (`0` • `$10.83/mo` • `larger-16-core`) | `node-typescript-vitest` (`$19.49/mo`) |
| **`astral-sh/uv`** | `aws-codebuild` (`$288.12/mo` • 2x:`False`) | `dependency_install` (`$117.96/mo`) | `compile-heavy` (`10` • `$53.02/mo` • `larger-8-core`) | `rust-cargo` (`$48.56/mo`) |
| **`denoland/deno`** | `azure-pipelines` (`$3,400.00/mo` • 2x:`False`) | `clone_checkout` (`$4,264.33/mo`) | `compile-heavy` (`1` • `$369.13/mo` • `larger-4-core`) | `rust-cargo` (`$4,549.21/mo`) |
| **`ggml-org/llama.cpp`** | `azure-pipelines` (`$1,680.00/mo` • 2x:`False`) | `dependency_install` (`$1,051.50/mo`) | `compile-heavy` (`12` • `$252.90/mo` • `larger-4-core`) | `cpp-cmake-ninja` (`$580.83/mo`) |
| **`immich-app/immich`** | `azure-pipelines` (`$520.00/mo` • 2x:`False`) | `dependency_install` (`$400.18/mo`) | `compile-heavy` (`4` • `$79.35/mo` • `larger-8-core`) | `flutter-dart-mobile` (`$344.10/mo`) |
| **`langgenius/dify`** | `github-actions-team` (`$244.47/mo` • 2x:`False`) | `dependency_install` (`$129.99/mo`) | `io-bound` (`0` • `$18.84/mo` • `larger-4-core`) | `python-uv-pytest` (`$76.60/mo`) |
| **`open-webui/open-webui`** | `aws-codebuild` (`$155.42/mo` • 2x:`False`) | `dependency_install` (`$87.13/mo`) | `test-heavy` (`0` • `$31.18/mo` • `larger-16-core`) | `python-uv-pytest` (`$46.78/mo`) |
| **`mendableai/firecrawl`** | `github-actions-team` (`$730.09/mo` • 2x:`False`) | `dependency_install` (`$681.01/mo`) | `compile-heavy` (`0` • `$24.68/mo` • `larger-8-core`) | `rust-cargo` (`$793.62/mo`) |

### Part 1F: 9 Primary Repositories — 4 FinOps Unit Economics, Anomaly/Timeout, Registry & Fleet SLA Models (`M21–M24`)

| Repository | M21: Cost / Merged PR (`$/Dev/Mo`) | M22: Projected EOM (`Missing Timeouts`) | M23: Container Images (`Registry $/mo`) | M24: Est. P95 Queue (`Idle Warm Pool $/mo`) |
| :--- | :---: | :---: | :---: | :---: |
| **`mgjartproject/FineArts-BE`** | `$0.43` (`$9.35/dev`) | `$46.76/mo` (`6` jobs) | `1` (`$6.72/mo`) | `0.0s` (`$0.00/mo`) |
| **`Eazy-Ops/Backend`** | `$0.60` (`$9.28/dev`) | `$64.95/mo` (`5` jobs) | `6` (`$40.82/mo`) | `0.0s` (`$0.00/mo`) |
| **`astral-sh/uv`** | `$2.99` (`$9.52/dev`) | `$323.71/mo` (`79` jobs) | `7` (`$47.64/mo`) | `115.0s` (`$0.00/mo`) |
| **`denoland/deno`** | `$93.39` (`$25.53/dev`) | `$10,109.35/mo` (`10` jobs) | `0` (`$0.00/mo`) | `589.0s` (`$0.00/mo`) |
| **`ggml-org/llama.cpp`** | `$29.81` (`$16.63/dev`) | `$3,226.86/mo` (`118` jobs) | `9` (`$61.28/mo`) | `523.0s` (`$415.52/mo`) |
| **`immich-app/immich`** | `$9.08` (`$16.12/dev`) | `$983.13/mo` (`74` jobs) | `10` (`$68.10/mo`) | `235.0s` (`$0.00/mo`) |
| **`langgenius/dify`** | `$2.83` (`$8.28/dev`) | `$306.41/mo` (`61` jobs) | `9` (`$61.28/mo`) | `0.0s` (`$0.00/mo`) |
| **`open-webui/open-webui`** | `$1.73` (`$9.36/dev`) | `$187.10/mo` (`8` jobs) | `11` (`$74.92/mo`) | `0.0s` (`$0.00/mo`) |
| **`mendableai/firecrawl`** | `$16.29` (`$11.76/dev`) | `$1,763.61/mo` (`69` jobs) | `6` (`$40.82/mo`) | `37.0s` (`$0.00/mo`) |

---

## Part 2: 10 Fresh World-Scale GitHub Repositories — Core KPIs & 3-Mode Performance

| Repository | Category | WF / Def / Exp Jobs | Billed Mins/Run | Gross Monthly Compute | Free-Tier Burn % | Monthly Dev Wait (`$65/hr`) | Engine (`ms`) | CLI (`ms`) | Workflow (`ms`) | MCP 4-RPC (`ms`) |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **`microsoft/vscode`** | World-Scale OSS (VS Code IDE) | `5` / `26` / `36` | `239.00m` | **`$4,287.57/mo`** | `129.9%` | `$1,660.56/mo` (`8.85m`) | `4.48` | `10.17` | `10.49` | `43.73` |
| **`flutter/flutter`** | World-Scale OSS (Flutter SDK) | `6` / `11` / `11` | `27.00m` | **`$94.39/mo`** | `779.4%` | `$1,848.19/mo` (`9.85m`) | `2.91` | `8.39` | `6.45` | `39.36` |
| **`ollama/ollama`** | World-Scale OSS (Local LLM Runner) | `4` / `18` / `47` | `407.00m` | **`$2,445.58/mo`** | `17255.1%` | `$3,180.38/mo` (`16.95m`) | `2.98` | `8.78` | `9.39` | `42.76` |
| **`huggingface/transformers`** | World-Scale OSS (ML Transformers) | `7` / `37` / `57` | `221.00m` | **`$574.16/mo`** | `4784.7%` | `$1,426.01/mo` (`7.6m`) | `3.58` | `9.74` | `8.26` | `42.07` |
| **`vercel/next.js`** | World-Scale OSS (Next.js Framework) | `5` / `38` / `45` | `144.00m` | **`$1,137.92/mo`** | `2663.0%` | `$1,191.47/mo` (`6.35m`) | `3.75` | `11.18` | `7.35` | `43.67` |
| **`langchain-ai/langchain`** | World-Scale OSS (Agentic Framework) | `27` / `41` / `43` | `111.00m` | **`$89.27/mo`** | `743.9%` | `$1,238.38/mo` (`16.35m`) | `17.00` | `23.09` | `14.86` | `48.78` |
| **`n8n-io/n8n`** | World-Scale OSS (Workflow Automation) | `12` / `38` / `44` | `258.35m` | **`$866.61/mo`** | `952.6%` | `$2,598.72/mo` (`13.85m`) | `5.45` | `12.06` | `7.57` | `34.95` |
| **`supabase/supabase`** | World-Scale OSS (Postgres Cloud) | `12` / `17` / `25` | `68.85m` | **`$268.46/mo`** | `238.2%` | `$1,022.60/mo` (`5.45m`) | `3.40` | `8.39` | `7.00` | `33.87` |
| **`home-assistant/core`** | World-Scale OSS (Home Automation) | `19` / `66` / `83` | `352.00m` | **`$380.13/mo`** | `3172.3%` | `$5,606.48/mo` (`32.36m`) | `23.49` | `28.41` | `23.58` | `69.32` |
| **`pydantic/pydantic`** | World-Scale OSS (Python Validation) | `12` / `60` / `173` | `727.47m` | **`$3,306.78/mo`** | `27311.5%` | `$2,486.14/mo` (`16.32m`) | `8.61` | `14.02` | `10.80` | `43.90` |

### Part 2B: 10 Fresh World-Scale Repositories — 6 Core CI/CD Invoice Components (`$/mo`)

| Repository | C1: Rounding Tax | C2: Self-Hosted EC2 (`Spot $/mo`) | C3: Artifact (`90d` -> `S3 14d`) | C4: Parallelism Tax | C5: Log Profile (`External 90d`) | C6: Egress (`NAT Worst` -> `VPC+Cache`) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **`microsoft/vscode`** | `$348.35` | `c5.4xlarge (16 vCPU / 32 GB)` (`$923.41`) | `$0.00` -> `$0.00` | `$510.00` | Node / npm / Vitest (0.2-0.8 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`flutter/flutter`** | `$13.12` | `m5.xlarge (4 vCPU / 16 GB)` (`$49.70`) | `$0.00` -> `$0.00` | `$0.00` | Node / npm / Vitest (0.2-0.8 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`ollama/ollama`** | `$87.94` | `c5.4xlarge (16 vCPU / 32 GB)` (`$725.74`) | `$243.56` -> `$2.73` | `$510.00` | Go / go test (0.1-0.4 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`huggingface/transformers`** | `$64.17` | `t3.medium (2 vCPU / 4 GB)` (`$131.71`) | `$81.19` -> `$0.91` | `$100.00` | Python / pytest (0.3-1.2 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`vercel/next.js`** | `$102.02` | `c5.4xlarge (16 vCPU / 32 GB)` (`$290.93`) | `$0.00` -> `$0.00` | `$100.00` | Node / npm / Vitest (0.2-0.8 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`langchain-ai/langchain`** | `$15.43` | `t3.medium (2 vCPU / 4 GB)` (`$44.71`) | `$0.75` -> `$0.01` | `$0.00` | Python / pytest (0.3-1.2 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`n8n-io/n8n`** | `$18.45` | `c5.2xlarge (8 vCPU / 16 GB)` (`$275.47`) | `$0.00` -> `$0.00` | `$100.00` | Node / npm / Vitest (0.2-0.8 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`supabase/supabase`** | `$4.29` | `c5.2xlarge (8 vCPU / 16 GB)` (`$106.64`) | `$162.38` -> `$1.82` | `$0.00` | Node / npm / Vitest (0.2-0.8 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`home-assistant/core`** | `$48.33` | `m6g.xlarge (Arm 4 vCPU / 16 GB)` (`$96.21`) | `$64.50` -> `$0.72` | `$0.00` | Python / pytest (0.3-1.2 MB/build) (`$0.00`) | `$0.00` -> `$0.00` |
| **`pydantic/pydantic`** | `$294.16` | `m5.xlarge (4 vCPU / 16 GB)` (`$908.29`) | `$219.83` -> `$2.46` | `$510.00` | Rust / cargo test (0.6-2.0 MB/build) (`$0.00`) | `$1,275.32` -> `$60.73` |

### Part 2C: 10 Fresh World-Scale Repositories — 5 Hidden Payroll & Pipeline Hygiene Cost Models (`$/mo`)

| Repository | H1: 40% Context-Switch Payroll Loss | H2: Rerun Flake Cost (`12%`) | H3: Cache Coverage (`Unrealized Savings`) | H4: Combined Flake Tax (`+ Triage`) | H5: Missing `cancel-in-progress` Waste | H5: Missing Path-Filter Savings |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **`microsoft/vscode`** | `$1,660.56/mo` | `$851.45/mo` | `0/36` (`$964.70/mo`) | `$2,554.35/mo` | `5` wf (`$643.13/mo`) | `4` wf (`$109.12/mo`) |
| **`flutter/flutter`** | `$1,848.19/mo` | `$192.82/mo` | `0/11` (`$9.35/mo`) | `$578.46/mo` | `6` wf (`$14.16/mo`) | `4` wf (`$16.63/mo`) |
| **`ollama/ollama`** | `$3,180.38/mo` | `$316.56/mo` | `5/47` (`$356.71/mo`) | `$949.68/mo` | `4` wf (`$366.84/mo`) | `3` wf (`$608.28/mo`) |
| **`huggingface/transformers`** | `$1,426.01/mo` | `$218.07/mo` | `0/57` (`$100.80/mo`) | `$654.21/mo` | `7` wf (`$86.12/mo`) | `7` wf (`$229.66/mo`) |
| **`vercel/next.js`** | `$1,191.47/mo` | `$247.74/mo` | `1/45` (`$155.49/mo`) | `$743.22/mo` | `5` wf (`$170.69/mo`) | `4` wf (`$242.13/mo`) |
| **`langchain-ai/langchain`** | `$1,238.38/mo` | `$192.55/mo` | `7/43` (`$13.25/mo`) | `$577.65/mo` | `6` wf (`$3.51/mo`) | `7` wf (`$28.06/mo`) |
| **`n8n-io/n8n`** | `$2,598.72/mo` | `$471.34/mo` | `0/44` (`$183.30/mo`) | `$1,414.02/mo` | `12` wf (`$129.99/mo`) | `11` wf (`$334.17/mo`) |
| **`supabase/supabase`** | `$1,022.60/mo` | `$404.88/mo` | `0/25` (`$36.71/mo`) | `$1,214.64/mo` | `12` wf (`$40.27/mo`) | `11` wf (`$85.56/mo`) |
| **`home-assistant/core`** | `$5,606.48/mo` | `$417.29/mo` | `37/83` (`$13.63/mo`) | `$1,251.87/mo` | `0` wf (`$0.00/mo`) | `0` wf (`$0.00/mo`) |
| **`pydantic/pydantic`** | `$2,486.14/mo` | `$361.89/mo` | `69/173` (`$613.00/mo`) | `$1,085.67/mo` | `3` wf (`$365.66/mo`) | `5` wf (`$1,321.15/mo`) |

### Part 2D: 10 Fresh World-Scale Repositories — 5 Dynamic Team-Stage & Pipeline-Shape Cost Models

| Repository | S1: Inferred Team Stage (`Devs` • `Mac mini Payback`) | S2: Recoverable Waste (`Platform Eng ROI %`) | S3: Compliance Controls (`Residency` • `1yr SIEM`) | S4: Pipeline Shape (`Units` • `Tool` • `Affected Savings`) | S5: Setup Share (`Shared Base Savings` • `Unpinned`) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **`microsoft/vscode`** | `scaleup (25-100 devs)` (`65` devs • `0.42m`) | `$2,648.76/mo` (`16.05%`) | `0` (`1.00x` • `$0.00/mo`) | `single-service` (`0` • `none` • `$0.00/mo`) | `32.7%` (`$840.71/mo` • `0`) |
| **`flutter/flutter`** | `startup-seed-series-a (6-15 devs)` (`8` devs • `36.53m`) | `$742.11/mo` (`4.50%`) | `0` (`1.00x` • `$0.00/mo`) | `multi-service-microservices` (`0` • `none` • `$0.00/mo`) | `45.0%` (`$25.49/mo` • `0`) |
| **`ollama/ollama`** | `scaleup (25-100 devs)` (`111` devs • `0.58m`) | `$2,272.79/mo` (`13.77%`) | `1` (`1.00x` • `$2.63/mo`) | `single-service` (`0` • `none` • `$0.00/mo`) | `24.1%` (`$353.05/mo` • `0`) |
| **`huggingface/transformers`** | `scaleup (25-100 devs)` (`60` devs • `0.00m`) | `$921.09/mo` (`5.58%`) | `0` (`1.00x` • `$0.00/mo`) | `single-service` (`0` • `none` • `$0.00/mo`) | `45.0%` (`$155.02/mo` • `0`) |
| **`vercel/next.js`** | `scaleup (25-100 devs)` (`39` devs • `0.00m`) | `$1,016.22/mo` (`6.16%`) | `0` (`1.00x` • `$0.00/mo`) | `monorepo` (`5` • `turborepo` • `$0.00/mo`) | `45.0%` (`$307.24/mo` • `0`) |
| **`langchain-ai/langchain`** | `startup-seed-series-a (6-15 devs)` (`10` devs • `0.00m`) | `$563.12/mo` (`3.41%`) | `2` (`1.00x` • `$7.21/mo`) | `monorepo` (`5` • `custom-affected-graph` • `$0.00/mo`) | `45.0%` (`$24.10/mo` • `0`) |
| **`n8n-io/n8n`** | `scaleup (25-100 devs)` (`70` devs • `0.00m`) | `$1,662.97/mo` (`10.08%`) | `0` (`1.00x` • `$0.00/mo`) | `monorepo` (`4` • `custom-affected-graph` • `$0.00/mo`) | `35.0%` (`$182.20/mo` • `0`) |
| **`supabase/supabase`** | `scaleup (25-100 devs)` (`19` devs • `0.00m`) | `$792.14/mo` (`4.80%`) | `0` (`1.00x` • `$0.00/mo`) | `monorepo` (`5` • `path-filters-only` • `$93.96/mo`) | `45.0%` (`$72.49/mo` • `0`) |
| **`home-assistant/core`** | `scaleup (25-100 devs)` (`40` devs • `0.00m`) | `$2,070.14/mo` (`12.55%`) | `5` (`1.00x` • `$13.92/mo`) | `multi-service-microservices` (`0` • `none` • `$0.00/mo`) | `34.0%` (`$77.45/mo` • `5`) |
| **`pydantic/pydantic`** | `scaleup (25-100 devs)` (`182` devs • `0.47m`) | `$2,825.41/mo` (`17.12%`) | `2` (`1.00x` • `$42.57/mo`) | `single-service` (`1` • `none` • `$0.00/mo`) | `45.0%` (`$892.83/mo` • `0`) |

### Part 2E: 10 Fresh World-Scale Repositories — 4 Calculator & Stack Intelligence Pillars (`P17–P20`)

| Repository | P17: Cheapest Vendor TCO (`2x Migration Rule`) | P18: Best First Phase (`Reducible $/mo`) | P19: Compute Profile (`Larger` • `ARM64 Savings` • `Best Tier`) | P20: Stack Archetype (`Stack Savings $/mo`) |
| :--- | :---: | :---: | :---: | :---: |
| **`microsoft/vscode`** | `azure-pipelines` (`$560.00/mo` • 2x:`False`) | `dependency_install` (`$2,448.20/mo`) | `io-bound` (`34` • `$420.98/mo` • `standard-2-core`) | `node-typescript-vitest` (`$1,286.27/mo`) |
| **`flutter/flutter`** | `gcp-cloud-build` (`$79.39/mo` • 2x:`False`) | `dependency_install` (`$46.47/mo`) | `compile-heavy` (`0` • `$11.26/mo` • `larger-16-core`) | `flutter-dart-mobile` (`$33.04/mo`) |
| **`ollama/ollama`** | `azure-pipelines` (`$960.00/mo` • 2x:`False`) | `dependency_install` (`$1,104.57/mo`) | `compile-heavy` (`1` • `$101.76/mo` • `larger-8-core`) | `go-mod-build-cache` (`$733.67/mo`) |
| **`huggingface/transformers`** | `aws-codebuild` (`$477.97/mo` • 2x:`False`) | `dependency_install` (`$313.04/mo`) | `compile-heavy` (`0` • `$95.69/mo` • `larger-4-core`) | `python-uv-pytest` (`$143.54/mo`) |
| **`vercel/next.js`** | `azure-pipelines` (`$360.00/mo` • 2x:`False`) | `dependency_install` (`$505.49/mo`) | `compile-heavy` (`15` • `$198.85/mo` • `larger-4-core`) | `node-typescript-vitest` (`$341.38/mo`) |
| **`langchain-ai/langchain`** | `aws-codebuild` (`$73.89/mo` • 2x:`False`) | `dependency_install` (`$43.57/mo`) | `test-heavy` (`0` • `$14.88/mo` • `larger-16-core`) | `python-uv-pytest` (`$22.32/mo`) |
| **`n8n-io/n8n`** | `github-actions-team` (`$376.31/mo` • 2x:`False`) | `dependency_install` (`$481.62/mo`) | `io-bound` (`0` • `$19.05/mo` • `standard-2-core`) | `node-typescript-vitest` (`$259.98/mo`) |
| **`supabase/supabase`** | `github-actions-team` (`$86.58/mo` • 2x:`False`) | `dependency_install` (`$153.36/mo`) | `io-bound` (`0` • `$4.76/mo` • `standard-2-core`) | `node-typescript-vitest` (`$80.54/mo`) |
| **`home-assistant/core`** | `aws-codebuild` (`$316.73/mo` • 2x:`False`) | `dependency_install` (`$148.89/mo`) | `io-bound` (`0` • `$63.33/mo` • `larger-4-core`) | `python-uv-pytest` (`$95.03/mo`) |
| **`pydantic/pydantic`** | `azure-pipelines` (`$1,560.00/mo` • 2x:`False`) | `dependency_install` (`$1,239.32/mo`) | `compile-heavy` (`0` • `$230.14/mo` • `larger-4-core`) | `rust-cargo` (`$496.02/mo`) |

### Part 2F: 10 Fresh World-Scale Repositories — 4 FinOps Unit Economics, Anomaly/Timeout, Registry & Fleet SLA Models (`M21–M24`)

| Repository | M21: Cost / Merged PR (`$/Dev/Mo`) | M22: Projected EOM (`Missing Timeouts`) | M23: Container Images (`Registry $/mo`) | M24: Est. P95 Queue (`Idle Warm Pool $/mo`) |
| :--- | :---: | :---: | :---: | :---: |
| **`microsoft/vscode`** | `$39.61` (`$65.96/dev`) | `$4,287.57/mo` (`26` jobs) | `0` (`$0.00/mo`) | `121.0s` (`$0.00/mo`) |
| **`flutter/flutter`** | `$0.87` (`$11.80/dev`) | `$94.39/mo` (`11` jobs) | `0` (`$0.00/mo`) | `0.0s` (`$0.00/mo`) |
| **`ollama/ollama`** | `$22.59` (`$22.03/dev`) | `$2,445.58/mo` (`18` jobs) | `0` (`$0.00/mo`) | `187.0s` (`$0.00/mo`) |
| **`huggingface/transformers`** | `$5.30` (`$9.57/dev`) | `$574.16/mo` (`37` jobs) | `0` (`$0.00/mo`) | `247.0s` (`$0.00/mo`) |
| **`vercel/next.js`** | `$10.51` (`$29.18/dev`) | `$1,137.92/mo` (`38` jobs) | `0` (`$0.00/mo`) | `175.0s` (`$0.00/mo`) |
| **`langchain-ai/langchain`** | `$0.82` (`$8.93/dev`) | `$89.27/mo` (`30` jobs) | `0` (`$0.00/mo`) | `0.0s` (`$0.00/mo`) |
| **`n8n-io/n8n`** | `$8.01` (`$12.38/dev`) | `$866.61/mo` (`38` jobs) | `0` (`$0.00/mo`) | `43.0s` (`$0.00/mo`) |
| **`supabase/supabase`** | `$2.48` (`$14.13/dev`) | `$268.46/mo` (`17` jobs) | `1` (`$6.72/mo`) | `0.0s` (`$0.00/mo`) |
| **`home-assistant/core`** | `$3.51` (`$9.50/dev`) | `$380.13/mo` (`48` jobs) | `5` (`$34.00/mo`) | `0.0s` (`$0.00/mo`) |
| **`pydantic/pydantic`** | `$30.55` (`$18.17/dev`) | `$3,306.78/mo` (`57` jobs) | `0` (`$0.00/mo`) | `697.0s` (`$37.67/mo`) |

---

## Part 3: 6 Internal Edge-Case Repositories — KPIs & Latency

| Repository | Edge-Case Scenario | WF / Exp Jobs | Gross Monthly Compute | Net Billed Monthly | CLI Latency (`ms`) |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **`mgjartproject/gfa-data-pipelines`** | Internal (17x matrix scraper pipeline) | `2` / `23` | `$122.15/mo` | **`$110.15/mo`** | `8.22` |
| **`mgjartproject/frontend`** | Internal (GFA Frontend WIF + Security) | `2` / `6` | `$44.17/mo` | **`$32.17/mo`** | `7.06` |
| **`mgjartproject/FineArt_AI_Pipeline`** | Internal Edge Case (0 workflows) | `0` / `0` | `$0.00/mo` | **`$0.00/mo`** | `5.55` |
| **`Eazy-Ops/frontend`** | Internal (Eazy-Ops Frontend Docker) | `2` / `2` | `$20.78/mo` | **`$8.78/mo`** | `6.91` |
| **`Eazy-Ops/multi-cloud-finops-mcp-server`** | Internal (Within 2,000m Free Tier -> $0 net) | `1` / `1` | `$5.20/mo` | **`$0.00/mo`** | `6.54` |
| **`Eazy-Ops/Automation`** | Internal Edge Case (Nested cron/.github/workflows) | `0` / `0` | `$0.00/mo` | **`$0.00/mo`** | `6.01` |
