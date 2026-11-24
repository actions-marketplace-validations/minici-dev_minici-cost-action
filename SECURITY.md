# 🔒 Security Policy & Read-Only Least-Privilege Architecture (`minici`)

## 1. Zero Proprietary Source-Code Access (`Contents: None` by Default)

`minici` is architected from the ground up for security-conscious engineering organizations and CISOs:

1. **Zero Application Source-Code Access:**
   - For **local checkouts** (`minici breakdown --path .`) and **public repositories** (`minici sync --repo owner/repo`), `minici` requires **`Contents: None`** (`0` source-code permissions).
   - Only when fetching workflow YAML remotely via REST API on a **private repository without a local checkout** (`minici breakdown --repo private-org/repo`) is `Contents: Read` needed strictly to list `.github/workflows/*.yml` — `minici` never clones, reads, or transmits application source files.
2. **Zero External Runtime Crates:** `crates/minici/Cargo.toml` has **`0` external crate dependencies** (`100%` Rust `std` library only), eliminating supply-chain `build.rs` and transitive crate risks.
3. **Zero Write Permissions Ever Requested for Analysis:** `minici` actively inspects OAuth, Fine-Grained PAT (`X-Accepted-GitHub-Permissions` + `/repos` `permissions.push`), GitLab, CircleCI, and Bitbucket scopes via `minici permissions check` and **prints an explicit Security Advisory warning** if a caller passes an over-privileged `repo`, `workflow`, `write:*`, or `push:true` token.
4. **Fail-Closed Verification:** Prebuilt binary installers (`action.yml` and `scripts/install.sh`) enforce fail-closed `SHA256SUMS.txt` verification (falling back to local `std`-only `cargo build --release` if checksums are unavailable), SHA-pinned and branch or major-version Action refs build from the checked-out Action source. MCP mutating tools require explicit `"confirm_write": true`, and Slack webhooks enforce fail-closed HMAC-SHA256 `X-Slack-Signature` validation.

---

## 2. Five Progressive Read-Only Telemetry Tiers (`108` FinOps Attributes)

Run `minici permissions check --repo owner/repo` (or `--path . --offline`) at any time to verify your exact token posture, live endpoint statuses, and unlocked FinOps telemetry coverage:

| Tier | Mode | Required Token Permissions | Source Code Access | Attributes Unlocked |
| :--- | :--- | :--- | :---: | :---: |
| **Tier 0** | Local Offline Static YAML (`minici breakdown --path .`) | **None (`0` network calls)** | Local workflow YAML only | **`33 / 108` (`30.6%`)** |
| **Tier 1** | Public Repo Live API (`minici sync --repo owner/repo`) | **None (`Zero-Token` Public API + `ETag 304`)** | **`None`** | **`68 / 108` (`63.0%`)** |
| **Tier 2** | Least-Privilege Read Token (`GitHub` / `GitLab` / `CircleCI` / `Bitbucket`) | `Actions: Read`, `Metadata: Read` *(+ `Contents: Read` only for remote private YAML)* | **`None`** | **`89 / 108` (`82.4%`)** |
| **Tier 3** | Read Token + Billing CSV (`--billing-csv focus.csv`) | Same Read-Only token + `FOCUS v1.1` / Vendor CSV (`100%` local) | **`None`** | **`99 / 108` (`91.7%`)** *(`104 / 108` = `96.3%` w/ CircleCI CPU/RAM CSV)* |
| **Tier 4** | Tier 3 + In-Workflow Hardware Sampler (`telemetry-step`) | `uses: minici-dev/minici-cost-action/telemetry-step@v1` (`0` network egress) | **`None`** | **`108 / 108` (`100.0%`)** |

> **Note on Sticky PR Comments (`post-pr-comment: "true"` or `minici comment github --post`):** Only when you explicitly opt into posting sticky PR comments inside GitHub Actions does the workflow job use GitHub's ephemeral `GITHUB_TOKEN` with `pull-requests: write` scoped strictly to that pull request.

---

## 3. Reporting a Vulnerability

Please report security issues privately to **Aamir Raza** at **`raza.aamir01@gmail.com`**. All reports receive an acknowledgment within 24 hours.
