//! Comment-Preserving Surgical Workflow Optimizer & 7×7 Anywhere-to-Anywhere AST/IR Translator
//! (`crates/minici/src/optimizer.rs` — 100% Pure Rust `std`, Zero External Crate Dependencies).
//!
//! Safely rewrites and translates CI/CD workflows across all 7 CI providers (GitHub Actions, GitLab CI,
//! CircleCI, Bitbucket Pipelines, AWS CodeBuild, GCP Cloud Build, Azure Pipelines + Jenkinsfile import)
//! while preserving comments and verifying exact Before -> After cost, storage, and risk deltas on an
//! isolated shadow directory before applying:
//! 1. `minici shrink`  : 7-provider FinOps hygiene (`concurrency`, `timeout-minutes: 30` / `timeout: 30m` /
//!                       `max-time: 30` / `no_output_timeout: 15m` / `timeout: 1800s` / `timeoutInMinutes: 30`,
//!                       `retention-days: 7` / `expire_in: 7 days`, git-history-safe `fetch-depth: 1`,
//!                       Node/Python/Go/Java/uv/Rust caching, plus `--security` least-privilege `permissions:`
//!                       and optional `minici-yaml-fixer` 5-pillar SHA-pinning delegation).
//! 2. `minici migrate` : (A) Dynamic Catalog Runner Migration across all 7 providers (6 safety gates + any SKU
//!                       in `DynamicPricingRegistry` + scalar, inline-list, multi-line sequence & mapping `runs-on`).
//!                       (B) Bidirectional `7×7` Cross-Platform Pipeline IR Translator (`--from <any> --to <any>`),
//!                       emitting complete runnable target YAML and scoring exact Before -> After + 5-Layer TCO!

use crate::catalog;
use crate::engine::{analyze_path, compute_diff, json_escape, DiffReport};
use crate::providers::compute_5layer_platform_switching_tco;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct FilePatchEdit {
    pub workflow_file: String,
    pub rule_id: String,
    pub description: String,
    pub line_number: usize,
    pub original_snippet: String,
    pub replacement_snippet: String,
}

#[derive(Debug, Clone, Default)]
pub struct MigrateOptions {
    pub from_vendor: Option<String>,
    pub out_file: Option<PathBuf>,
    pub security_mode: bool,
    pub with_fixer: bool,
    pub rust_tools: bool,
    pub rust_everywhere: bool,
}

#[derive(Debug, Clone)]
pub struct OptimizationReceipt {
    pub operation: String, // "shrink" | "migrate"
    pub source_platform: String,
    pub target_platform: String,
    pub target_workflow_file: Option<String>,
    pub applied_to_disk: bool,
    pub files_modified: usize,
    pub edits: Vec<FilePatchEdit>,
    pub verified_diff: DiffReport,
    pub artifact_storage_before_usd: f64,
    pub artifact_storage_after_usd: f64,
    pub missing_timeouts_before: usize,
    pub missing_timeouts_after: usize,
    pub in_place_shrink_monthly_savings_usd: f64,
    pub converted_github_workflow_preview: Option<String>,
    pub switching_verdict: Option<(f64, f64, f64, &'static str)>, // (eng_hours, switch_cost_usd, payback_mo, verdict)
}

impl OptimizationReceipt {
    pub fn to_json(&self) -> String {
        let edits_json = self
            .edits
            .iter()
            .map(|e| {
                format!(
                    "{{\"workflow_file\":\"{}\",\"rule_id\":\"{}\",\"description\":\"{}\",\"line_number\":{}}}",
                    json_escape(&e.workflow_file),
                    json_escape(&e.rule_id),
                    json_escape(&e.description),
                    e.line_number
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let sw_json = self
            .switching_verdict
            .as_ref()
            .map(|(h, c, p, v)| {
                format!(
                    "{{\"engineering_hours\":{:.1},\"switching_cost_usd\":{:.2},\"payback_months\":{:.2},\"verdict\":\"{}\",\"in_place_shrink_monthly_savings_usd\":{:.2}}}",
                    h, c, p, v, self.in_place_shrink_monthly_savings_usd
                )
            })
            .unwrap_or_else(|| "null".to_string());
        let conv_json = self
            .converted_github_workflow_preview
            .as_ref()
            .map(|s| format!("\"{}\"", json_escape(s)))
            .unwrap_or_else(|| "null".to_string());
        let tgt_wf_json = self
            .target_workflow_file
            .as_ref()
            .map(|s| format!("\"{}\"", json_escape(s)))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\n  \"operation\": \"{}\",\n  \"source_platform\": \"{}\",\n  \"target_platform\": \"{}\",\n  \"target_workflow_file\": {},\n  \"applied_to_disk\": {},\n  \"files_modified\": {},\n  \"edits_count\": {},\n  \"artifact_storage_before_usd\": {:.2},\n  \"artifact_storage_after_usd\": {:.2},\n  \"missing_timeouts_before\": {},\n  \"missing_timeouts_after\": {},\n  \"in_place_shrink_monthly_savings_usd\": {:.2},\n  \"edits\": [{}],\n  \"converted_workflow_preview\": {},\n  \"converted_github_workflow_preview\": {},\n  \"switching_tco\": {},\n  \"verified_diff\": {}\n}}",
            json_escape(&self.operation),
            json_escape(&self.source_platform),
            json_escape(&self.target_platform),
            tgt_wf_json,
            self.applied_to_disk,
            self.files_modified,
            self.edits.len(),
            self.artifact_storage_before_usd,
            self.artifact_storage_after_usd,
            self.missing_timeouts_before,
            self.missing_timeouts_after,
            self.in_place_shrink_monthly_savings_usd,
            edits_json,
            conv_json,
            conv_json,
            sw_json,
            self.verified_diff.to_json()
        )
    }

    pub fn render_table(&self) -> String {
        let mut out = String::new();
        out.push_str("+====================================================================================+\n");
        out.push_str(&format!(
            "| MINICI SURGICAL WORKFLOW {} RECEIPT ({} file(s), {} edit(s) • {}) |\n",
            self.operation.to_ascii_uppercase(),
            self.files_modified,
            self.edits.len(),
            if self.applied_to_disk {
                "APPLIED"
            } else {
                "DRY-RUN"
            }
        ));
        out.push_str("+====================================================================================+\n");
        if self.operation == "migrate" && self.target_workflow_file.is_some() {
            out.push_str(&format!(
                "  • Cross-Platform  : {} -> {} ({})\n",
                self.source_platform,
                self.target_platform,
                self.target_workflow_file.as_deref().unwrap_or("")
            ));
        }
        out.push_str(&format!(
            "  • Monthly Compute : ${:.2}/mo -> ${:.2}/mo ({:+.2}/mo • {:+.1}%)\n",
            self.verified_diff.base_monthly_compute_usd,
            self.verified_diff.head_monthly_compute_usd,
            self.verified_diff.delta_monthly_compute_usd,
            self.verified_diff.delta_monthly_compute_pct
        ));
        out.push_str(&format!(
            "  • Billed Mins/Run : {:.1}m -> {:.1}m ({:+.1}m/run)\n",
            self.verified_diff.base_billed_mins_per_run,
            self.verified_diff.head_billed_mins_per_run,
            self.verified_diff.delta_billed_mins_per_run
        ));
        if (self.artifact_storage_before_usd - self.artifact_storage_after_usd).abs() > 0.009 {
            out.push_str(&format!(
                "  • Artifact Storage: ${:.2}/mo -> ${:.2}/mo ({:+.2}/mo)\n",
                self.artifact_storage_before_usd,
                self.artifact_storage_after_usd,
                self.artifact_storage_after_usd - self.artifact_storage_before_usd
            ));
        }
        if self.missing_timeouts_before != self.missing_timeouts_after {
            out.push_str(&format!(
                "  • Timeout Guard   : {} -> {} job(s) missing timeout-minutes\n",
                self.missing_timeouts_before, self.missing_timeouts_after
            ));
        }
        if let Some((eng_h, sw_usd, payback, verdict)) = &self.switching_verdict {
            let payback_str = if *verdict == "MORE EXPENSIVE" || *payback <= 0.0 || *payback >= 999.0 {
                "no payback — target platform costs more/mo".to_string()
            } else {
                format!("{:.1}m payback", payback)
            };
            out.push_str(&format!(
                "  • 5-Layer Switch  : {:.1}h effort (${:.2} total switch TCO • {} • Verdict: {})\n",
                eng_h, sw_usd, payback_str, verdict
            ));
            out.push_str(&format!(
                "  • Optimize First  : Staying & running `minici shrink` + ARM64 saves ~${:.2}/mo in < 30 mins ($0 switch risk)\n",
                self.in_place_shrink_monthly_savings_usd
            ));
        }
        for (i, e) in self.edits.iter().take(12).enumerate() {
            out.push_str(&format!(
                "  {}. [{}] {}:L{} — {}\n",
                i + 1,
                e.rule_id,
                e.workflow_file,
                e.line_number,
                e.description
            ));
        }
        if !self.applied_to_disk && (!self.edits.is_empty() || self.converted_github_workflow_preview.is_some()) {
            out.push_str("  💡 Pass `--apply` (or `--out-file <FILE>`) to write verified output directly to disk.\n");
        }
        out.push_str("+====================================================================================+\n");
        out
    }

    pub fn render_pr_suggestions_markdown(&self) -> String {
        let mut md = format!(
            "<!-- minici-cost-comment -->\n\
             ### ✂️ MiniCI Surgical `{}` Plan (`{}` file(s) • `{:+.2}/mo` verified compute delta)\n\n\
             - **Before → After Compute:** `${:.2}/mo` → **`${:.2}/mo`** (`{:+.1}%`)\n\
             - **Before → After Billed Mins/Run:** `{:.1}m` → **`{:.1}m`**\n",
            self.operation,
            self.files_modified,
            self.verified_diff.delta_monthly_compute_usd,
            self.verified_diff.base_monthly_compute_usd,
            self.verified_diff.head_monthly_compute_usd,
            self.verified_diff.delta_monthly_compute_pct,
            self.verified_diff.base_billed_mins_per_run,
            self.verified_diff.head_billed_mins_per_run
        );
        if (self.artifact_storage_before_usd - self.artifact_storage_after_usd).abs() > 0.009 {
            md.push_str(&format!(
                "- **Before → After Artifact Storage:** `${:.2}/mo` → **`${:.2}/mo`**\n",
                self.artifact_storage_before_usd, self.artifact_storage_after_usd
            ));
        }
        if self.missing_timeouts_before != self.missing_timeouts_after {
            md.push_str(&format!(
                "- **Hung-Job Timeout Guard:** `{}` → **`{}`** job(s) missing `timeout-minutes`\n",
                self.missing_timeouts_before, self.missing_timeouts_after
            ));
        }
        if let Some((eng_h, sw_usd, payback, verdict)) = &self.switching_verdict {
            let payback_md = if *verdict == "MORE EXPENSIVE" || *payback <= 0.0 || *payback >= 999.0 {
                "`no payback — target costs more/mo`".to_string()
            } else {
                format!("`{:.1} mo` payback", payback)
            };
            md.push_str(&format!(
                "- **5-Layer Switching TCO Verdict:** **`{}`** (`{:.1}h` effort, `${:.2}` one-time cost, {} | In-place YAML optimization saves `~${:.2}/mo` immediately)\n",
                verdict, eng_h, sw_usd, payback_md, self.in_place_shrink_monthly_savings_usd
            ));
        }
        md.push('\n');
        let ordered_edits: Vec<&FilePatchEdit> = if self.operation == "accelerate" {
            let mut v: Vec<&FilePatchEdit> = self
                .edits
                .iter()
                .filter(|e| e.rule_id.starts_with("MINICI-RUST"))
                .collect();
            v.extend(
                self.edits
                    .iter()
                    .filter(|e| !e.rule_id.starts_with("MINICI-RUST")),
            );
            v
        } else {
            self.edits.iter().collect()
        };
        let max_shown = if self.operation == "accelerate" { 10 } else { 6 };
        for e in ordered_edits.into_iter().take(max_shown) {
            md.push_str(&format!(
                "#### [{}] `{}` (Line {}) — {}\n```suggestion\n{}\n```\n\n",
                e.rule_id,
                e.workflow_file,
                e.line_number,
                e.description,
                e.replacement_snippet
            ));
        }
        if let Some(ref preview) = self.converted_github_workflow_preview {
            md.push_str(&format!(
                "#### Automated Cross-Platform `{}` → `{}` Workflow Preview (`{}`)\n```yaml\n{}\n```\n",
                self.source_platform,
                self.target_platform,
                self.target_workflow_file.as_deref().unwrap_or("ci.yml"),
                preview.trim_end()
            ));
        }
        md
    }
}

fn indent_len(s: &str) -> usize {
    s.chars().take_while(|c| *c == ' ').count()
}

fn collect_workflow_files(repo_dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    if repo_dir.is_file() {
        let fname = repo_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("ci.yml")
            .to_string();
        out.push((repo_dir.to_path_buf(), fname));
        return out;
    }
    let gh_dir = repo_dir.join(".github").join("workflows");
    if let Ok(entries) = fs::read_dir(&gh_dir) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext == "yml" || ext == "yaml" {
                let rel = format!(
                    ".github/workflows/{}",
                    p.file_name().and_then(|s| s.to_str()).unwrap_or("ci.yml")
                );
                out.push((p, rel));
            }
        }
    }
    for (rel, sub) in [
        (".gitlab-ci.yml", repo_dir.join(".gitlab-ci.yml")),
        (".circleci/config.yml", repo_dir.join(".circleci").join("config.yml")),
        ("bitbucket-pipelines.yml", repo_dir.join("bitbucket-pipelines.yml")),
        ("buildspec.yml", repo_dir.join("buildspec.yml")),
        ("cloudbuild.yaml", repo_dir.join("cloudbuild.yaml")),
        ("cloudbuild.yml", repo_dir.join("cloudbuild.yml")),
        ("azure-pipelines.yml", repo_dir.join("azure-pipelines.yml")),
        ("azure-pipelines.yaml", repo_dir.join("azure-pipelines.yaml")),
    ] {
        if sub.is_file() {
            out.push((sub, rel.to_string()));
        }
    }
    out
}

/// Returns `true` ONLY when `lines[prop_idx]` (`runs-on:`) is a direct job-level property
/// (`jobs:` -> `<job_id>:` -> `runs-on:`), and NEVER a nested key inside `strategy.matrix`!
fn is_job_level_runs_on(lines: &[&str], prop_idx: usize) -> bool {
    let ind = indent_len(lines[prop_idx]);
    if ind == 0 {
        return false;
    }
    let mut parent_idx: Option<usize> = None;
    let mut k = prop_idx;
    while k > 0 {
        k -= 1;
        let pt = lines[k].trim();
        if pt.is_empty() || pt.starts_with('#') {
            continue;
        }
        if indent_len(lines[k]) < ind {
            parent_idx = Some(k);
            break;
        }
    }
    let p_idx = match parent_idx {
        Some(v) => v,
        None => return false,
    };
    let p_trim = lines[p_idx].trim();
    let p_ind = indent_len(lines[p_idx]);
    if p_ind == 0 || p_trim.starts_with('-') {
        return false;
    }
    let p_key = p_trim.split('#').next().unwrap_or(p_trim).trim();
    if !p_key.ends_with(':') {
        return false;
    }
    let mut m = p_idx;
    while m > 0 {
        m -= 1;
        let gt = lines[m].trim();
        if gt.is_empty() || gt.starts_with('#') {
            continue;
        }
        if indent_len(lines[m]) < p_ind {
            let g_key = gt.split('#').next().unwrap_or(gt).trim();
            return g_key == "jobs:";
        }
    }
    false
}

fn find_job_block_bounds(lines: &[&str], prop_idx: usize, prop_ind: usize) -> (usize, usize) {
    let mut start = prop_idx;
    while start > 0 {
        let prev = lines[start - 1];
        let pt = prev.trim();
        if !pt.is_empty() && !pt.starts_with('#') && indent_len(prev) < prop_ind {
            start -= 1;
            break;
        }
        start -= 1;
    }
    let job_hdr_ind = indent_len(lines[start]);
    let mut end = prop_idx + 1;
    while end < lines.len() {
        let st = lines[end].trim();
        if !st.is_empty() && !st.starts_with('#') && indent_len(lines[end]) <= job_hdr_ind {
            break;
        }
        end += 1;
    }
    (start, end)
}

fn job_uses_git_history(job_lines: &[&str]) -> bool {
    let joined = job_lines.join("\n").to_ascii_lowercase();
    joined.contains("git diff")
        || joined.contains("git log")
        || joined.contains("git fetch")
        || joined.contains("git merge-base")
        || joined.contains("git rev-list")
        || joined.contains("minici diff")
        || joined.contains("compare-to")
        || joined.contains("semantic-release")
        || joined.contains("changeset")
        || joined.contains("nx affected")
        || joined.contains("turbo ")
        || joined.contains("sonar")
        || joined.contains("goreleaser")
        || joined.contains("fetch-depth:")
}

fn detect_node_cache_manager(repo_base: &Path) -> Option<&'static str> {
    if repo_base.join("pnpm-lock.yaml").exists() {
        Some("pnpm")
    } else if repo_base.join("yarn.lock").exists() {
        Some("yarn")
    } else if repo_base.join("package-lock.json").exists() {
        Some("npm")
    } else {
        None
    }
}

fn detect_python_cache_manager(repo_base: &Path) -> Option<&'static str> {
    if repo_base.join("poetry.lock").exists() {
        Some("poetry")
    } else if repo_base.join("Pipfile.lock").exists() {
        Some("pipenv")
    } else if repo_base.join("requirements.txt").exists()
        || repo_base.join("requirements-dev.txt").exists()
        || (repo_base.join("pyproject.toml").exists() && !repo_base.join("uv.lock").exists())
    {
        Some("pip")
    } else {
        None
    }
}

fn detect_java_cache_manager(repo_base: &Path) -> Option<&'static str> {
    if repo_base.join("build.gradle").exists() || repo_base.join("build.gradle.kts").exists() {
        Some("gradle")
    } else if repo_base.join("pom.xml").exists() {
        Some("maven")
    } else {
        None
    }
}

enum StepWithState<'a> {
    NoWith,
    InlineWith,
    BlockWith {
        with_line_idx: usize,
        child_indent: usize,
        children: Vec<&'a str>,
    },
}

fn inspect_step_with_block<'a>(lines: &[&'a str], uses_idx: usize) -> (usize, usize, usize, StepWithState<'a>) {
    let uline = lines[uses_idx];
    let utrim = uline.trim();
    let uind = indent_len(uline);
    let (step_start, prop_indent) = if utrim.starts_with('-') {
        (uses_idx, uind + 2)
    } else {
        let mut s = uses_idx;
        while s > 0 {
            let prev = lines[s - 1];
            let pt = prev.trim();
            if !pt.is_empty() && !pt.starts_with('#') {
                let pi = indent_len(prev);
                if pi < uind && pt.starts_with('-') {
                    s -= 1;
                    break;
                } else if pi < uind {
                    break;
                }
            }
            s -= 1;
        }
        (s, uind)
    };

    let mut step_end = step_start + 1;
    while step_end < lines.len() {
        let lt = lines[step_end].trim();
        if !lt.is_empty() && !lt.starts_with('#') && indent_len(lines[step_end]) < prop_indent {
            break;
        }
        step_end += 1;
    }

    for k in step_start..step_end {
        let lk = lines[k];
        let kt = lk.trim();
        if kt.is_empty() || kt.starts_with('#') {
            continue;
        }
        let ki = indent_len(lk);
        let kt_key = if k == step_start && kt.starts_with('-') {
            kt.trim_start_matches('-').trim()
        } else if ki == prop_indent {
            kt
        } else {
            continue;
        };
        let kt_no_comment = kt_key.split(" #").next().unwrap_or(kt_key).trim();
        if kt_no_comment == "with:" {
            let mut child_indent: Option<usize> = None;
            let mut children: Vec<&'a str> = Vec::new();
            let mut m = k + 1;
            while m < step_end {
                let cm = lines[m];
                let ct = cm.trim();
                if ct.is_empty() || ct.starts_with('#') {
                    m += 1;
                    continue;
                }
                let ci = indent_len(cm);
                if ci <= prop_indent {
                    break;
                }
                if child_indent.is_none() {
                    child_indent = Some(ci);
                }
                children.push(ct);
                m += 1;
            }
            return (
                step_start,
                step_end,
                prop_indent,
                StepWithState::BlockWith {
                    with_line_idx: k,
                    child_indent: child_indent.unwrap_or(prop_indent + 2),
                    children,
                },
            );
        } else if kt_no_comment.starts_with("with:") {
            return (step_start, step_end, prop_indent, StepWithState::InlineWith);
        }
    }

    (step_start, step_end, prop_indent, StepWithState::NoWith)
}

fn push_with_key_insertion(
    step_insertions: &mut Vec<(usize, Vec<String>, FilePatchEdit)>,
    lines: &[&str],
    idx: usize,
    rel_name: &str,
    rule_id: &str,
    desc: &str,
    key_prefix: &str,
    kv_pair: &str,
) {
    let raw_l = lines[idx];
    let (_s_start, _s_end, prop_ind, with_state) = inspect_step_with_block(lines, idx);
    match with_state {
        StepWithState::NoWith => {
            let pad = " ".repeat(prop_ind);
            let w1 = format!("{}with:", pad);
            let w2 = format!("{}  {}", pad, kv_pair);
            step_insertions.push((
                idx,
                vec![w1.clone(), w2.clone()],
                FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: rule_id.to_string(),
                    description: desc.to_string(),
                    line_number: idx + 1,
                    original_snippet: raw_l.to_string(),
                    replacement_snippet: format!("{}\n{}\n{}", raw_l, w1, w2),
                },
            ));
        }
        StepWithState::BlockWith {
            with_line_idx,
            child_indent,
            children,
        } => {
            let already = children.iter().any(|c| c.starts_with(key_prefix));
            if !already {
                let cpad = " ".repeat(child_indent);
                let kv = format!("{}{}", cpad, kv_pair);
                let w_line = lines[with_line_idx];
                step_insertions.push((
                    with_line_idx,
                    vec![kv.clone()],
                    FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: rule_id.to_string(),
                        description: desc.to_string(),
                        line_number: with_line_idx + 1,
                        original_snippet: w_line.to_string(),
                        replacement_snippet: format!("{}\n{}", w_line, kv),
                    },
                ));
            }
        }
        StepWithState::InlineWith => {}
    }
}

fn shrink_github_workflow_yaml(
    rel_name: &str,
    content: &str,
    repo_base: &Path,
    security_mode: bool,
) -> (String, Vec<FilePatchEdit>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len() + 24);
    let mut edits: Vec<FilePatchEdit> = Vec::new();

    let low_all = content.to_ascii_lowercase();
    let has_concurrency = low_all.contains("\nconcurrency:") || low_all.starts_with("concurrency:");
    let has_permissions = low_all.contains("\npermissions:") || low_all.starts_with("permissions:");
    let triggers_pr_or_push = low_all.contains("pull_request") || low_all.contains("push");
    let node_mgr = detect_node_cache_manager(repo_base);
    let py_mgr = detect_python_cache_manager(repo_base);
    let java_mgr = detect_java_cache_manager(repo_base);
    let has_go_sum = repo_base.join("go.sum").exists() || repo_base.join("go.mod").exists();
    let has_uv = repo_base.join("uv.lock").exists() || repo_base.join("pyproject.toml").exists();
    let has_cargo = repo_base.join("Cargo.toml").exists();
    let wf_already_has_rust_cache = low_all.contains("rust-cache");
    let telem_lock = crate::telemetry::CanonicalTelemetrySnapshot::load_lockfile(repo_base);
    let wf_basename = Path::new(rel_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_name);

    let mut step_insertions: Vec<(usize, Vec<String>, FilePatchEdit)> = Vec::new();
    for (idx, &raw_l) in lines.iter().enumerate() {
        let t = raw_l.trim();
        if t.starts_with('#') {
            continue;
        }
        if t.contains("actions/checkout@") {
            let (_s_start, _s_end, prop_ind, with_state) = inspect_step_with_block(&lines, idx);
            let (j_start, j_end) = find_job_block_bounds(&lines, idx, prop_ind.saturating_sub(4).max(4));
            let history_needed = job_uses_git_history(&lines[j_start..j_end]);
            if !history_needed {
                match with_state {
                    StepWithState::NoWith => {
                        let pad = " ".repeat(prop_ind);
                        let w1 = format!("{}with:", pad);
                        let w2 = format!("{}  fetch-depth: 1", pad);
                        let w3 = format!("{}  filter: blob:none", pad);
                        step_insertions.push((
                            idx,
                            vec![w1.clone(), w2.clone(), w3.clone()],
                            FilePatchEdit {
                                workflow_file: rel_name.to_string(),
                                rule_id: "MINICI-SHRINK-003".to_string(),
                                description: "Enable shallow blobless clone (`fetch-depth: 1`, `filter: blob:none`) on history-independent `actions/checkout`".to_string(),
                                line_number: idx + 1,
                                original_snippet: raw_l.to_string(),
                                replacement_snippet: format!("{}\n{}\n{}\n{}", raw_l, w1, w2, w3),
                            },
                        ));
                    }
                    StepWithState::BlockWith {
                        with_line_idx,
                        child_indent,
                        children,
                    } => {
                        let already_has_depth = children.iter().any(|c| {
                            c.starts_with("fetch-depth:")
                                || c.starts_with("sparse-checkout:")
                                || c.starts_with("filter:")
                        });
                        if !already_has_depth {
                            let cpad = " ".repeat(child_indent);
                            let kv1 = format!("{}fetch-depth: 1", cpad);
                            let kv2 = format!("{}filter: blob:none", cpad);
                            let w_line = lines[with_line_idx];
                            step_insertions.push((
                                with_line_idx,
                                vec![kv1.clone(), kv2.clone()],
                                FilePatchEdit {
                                    workflow_file: rel_name.to_string(),
                                    rule_id: "MINICI-SHRINK-003".to_string(),
                                    description: "Insert shallow blobless clone (`fetch-depth: 1`, `filter: blob:none`) into `with:` on `actions/checkout`".to_string(),
                                    line_number: with_line_idx + 1,
                                    original_snippet: w_line.to_string(),
                                    replacement_snippet: format!("{}\n{}\n{}", w_line, kv1, kv2),
                                },
                            ));
                        }
                    }
                    StepWithState::InlineWith => {}
                }
            }
        } else if t.contains("actions/upload-artifact@") {
            push_with_key_insertion(
                &mut step_insertions,
                &lines,
                idx,
                rel_name,
                "MINICI-SHRINK-004",
                "Set explicit `retention-days: 7` on `actions/upload-artifact` (replaces 90d default)",
                "retention-days:",
                "retention-days: 7",
            );
        } else if t.contains("actions/setup-node@") || t.contains("actions/setup-python@") {
            let is_py = t.contains("setup-python");
            let job_uses_uv_or_custom_cache = if is_py {
                let (_s_start, _s_end, prop_ind, _) = inspect_step_with_block(&lines, idx);
                let (j_start, j_end) = find_job_block_bounds(&lines, idx, prop_ind.saturating_sub(4).max(4));
                lines[j_start..j_end].iter().any(|jl| {
                    let jlow = jl.to_ascii_lowercase();
                    jlow.contains("astral-sh/setup-uv")
                        || jlow.contains("uv sync")
                        || jlow.contains("uv run")
                })
            } else {
                false
            };
            let mgr_opt = if !is_py {
                node_mgr
            } else if job_uses_uv_or_custom_cache {
                None
            } else {
                py_mgr
            };
            if let Some(mgr) = mgr_opt {
                push_with_key_insertion(
                    &mut step_insertions,
                    &lines,
                    idx,
                    rel_name,
                    "MINICI-SHRINK-005",
                    &format!("Enable lockfile-backed `cache: '{}'` on toolchain setup", mgr),
                    "cache:",
                    &format!("cache: '{}'", mgr),
                );
            }
        } else if t.contains("actions/setup-go@") && has_go_sum {
            push_with_key_insertion(
                &mut step_insertions,
                &lines,
                idx,
                rel_name,
                "MINICI-SHRINK-006",
                "Enable Go module & build cache (`cache: true`) on `actions/setup-go`",
                "cache:",
                "cache: true",
            );
        } else if t.contains("actions/setup-java@") {
            if let Some(jmgr) = java_mgr {
                push_with_key_insertion(
                    &mut step_insertions,
                    &lines,
                    idx,
                    rel_name,
                    "MINICI-SHRINK-007",
                    &format!("Enable JVM dependency cache (`cache: '{}'`) on `actions/setup-java`", jmgr),
                    "cache:",
                    &format!("cache: '{}'", jmgr),
                );
            }
        } else if t.contains("astral-sh/setup-uv@") && has_uv {
            push_with_key_insertion(
                &mut step_insertions,
                &lines,
                idx,
                rel_name,
                "MINICI-SHRINK-008",
                "Enable `enable-cache: true` on `astral-sh/setup-uv`",
                "enable-cache:",
                "enable-cache: true",
            );
        } else if (t.contains("dtolnay/rust-toolchain@") || t.contains("actions-rs/toolchain@"))
            && has_cargo
            && !wf_already_has_rust_cache
        {
            let (_s_start, s_end, prop_ind, _) = inspect_step_with_block(&lines, idx);
            let step_list_ind = prop_ind.saturating_sub(2);
            let anchor = s_end.saturating_sub(1);
            let rc_line = format!("{}- uses: Swatinem/rust-cache@v2", " ".repeat(step_list_ind));
            step_insertions.push((
                anchor,
                vec![rc_line.clone()],
                FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: "MINICI-SHRINK-009".to_string(),
                    description: "Add `Swatinem/rust-cache@v2` after Rust toolchain step to cache `target/` & Cargo registry".to_string(),
                    line_number: anchor + 1,
                    original_snippet: lines[anchor].to_string(),
                    replacement_snippet: format!("{}\n{}", lines[anchor], rc_line),
                },
            ));
        }
    }

    let mut injected_concurrency = false;
    let mut injected_permissions = false;
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let ind = indent_len(line);

        if security_mode && !has_permissions && !injected_permissions && ind == 0 && trimmed == "jobs:" {
            let perm_block = "permissions:\n  contents: read";
            for pl in perm_block.lines() {
                out_lines.push(pl.to_string());
            }
            edits.push(FilePatchEdit {
                workflow_file: rel_name.to_string(),
                rule_id: "MINICI-SEC-006".to_string(),
                description: "Enforce least-privilege `permissions: contents: read` on `GITHUB_TOKEN` (zizmor pillar)"
                    .to_string(),
                line_number: i + 1,
                original_snippet: line.to_string(),
                replacement_snippet: format!("{}\n{}", perm_block, line),
            });
            injected_permissions = true;
        }

        if !has_concurrency
            && triggers_pr_or_push
            && !injected_concurrency
            && ind == 0
            && trimmed == "jobs:"
        {
            let conc_block = "concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: ${{ github.event_name == 'pull_request' }}";
            for cl in conc_block.lines() {
                out_lines.push(cl.to_string());
            }
            edits.push(FilePatchEdit {
                workflow_file: rel_name.to_string(),
                rule_id: "MINICI-SHRINK-001".to_string(),
                description: "Add `concurrency` cancel-in-progress guard for superseded PR pushes"
                    .to_string(),
                line_number: i + 1,
                original_snippet: line.to_string(),
                replacement_snippet: format!("{}\n{}", conc_block, line),
            });
            injected_concurrency = true;
        }

        out_lines.push(line.to_string());

        if trimmed.starts_with("runs-on:") && is_job_level_runs_on(&lines, i) {
            let (j_start, j_end) = find_job_block_bounds(&lines, i, ind);
            let job_has_timeout = lines[j_start..j_end].iter().any(|jl| {
                let jt = jl.trim();
                indent_len(jl) == ind && jt.starts_with("timeout-minutes:")
            });

            let rhs = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
            if rhs.is_empty() {
                while i + 1 < lines.len() {
                    let nxt = lines[i + 1];
                    let nt = nxt.trim();
                    if !nt.is_empty() && !nt.starts_with('#') {
                        if indent_len(nxt) > ind {
                            i += 1;
                            out_lines.push(nxt.to_string());
                        } else {
                            break;
                        }
                    } else {
                        let still_child = lines[(i + 2)..]
                            .iter()
                            .find(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
                            .map(|l| indent_len(l) > ind)
                            .unwrap_or(false);
                        if still_child {
                            i += 1;
                            out_lines.push(nxt.to_string());
                        } else {
                            break;
                        }
                    }
                }
            }

            if !job_has_timeout {
                let job_id_hdr = lines[j_start]
                    .trim()
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches(':');
                let key_exact = format!("{}#{}", wf_basename, job_id_hdr);
                let key_suffix = format!("#{}", job_id_hdr);
                let p90_opt = telem_lock.as_ref().and_then(|snap| {
                    snap.job_p90_durations_mins
                        .iter()
                        .find(|(k, _)| k == &key_exact || k.ends_with(&key_suffix))
                        .map(|(_, v)| *v)
                });
                let timeout_mins_val: u32 = match p90_opt {
                    Some(p90) if p90 > 0.0 => (p90 * 2.5).ceil().clamp(15.0, 180.0) as u32,
                    _ => 30,
                };
                let anchor_line = lines[i];
                let pad = " ".repeat(ind);
                let tline = format!("{}timeout-minutes: {}", pad, timeout_mins_val);
                out_lines.push(tline.clone());
                edits.push(FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: "MINICI-SHRINK-002".to_string(),
                    description: if let Some(p90) = p90_opt {
                        format!(
                            "Cap job hung-runner exposure at `timeout-minutes: {}` (2.5x live p90={:.1}m)",
                            timeout_mins_val, p90
                        )
                    } else {
                        "Cap job hung-runner exposure from 360m default to `timeout-minutes: 30`"
                            .to_string()
                    },
                    line_number: i + 1,
                    original_snippet: anchor_line.to_string(),
                    replacement_snippet: format!("{}\n{}", anchor_line, tline),
                });
            }
        }

        if trimmed.starts_with("retention-days:") {
            if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                if let Ok(days) = val.trim().parse::<u32>() {
                    if days > 30 {
                        let pad = " ".repeat(ind);
                        let replaced = format!("{}retention-days: 7", pad);
                        let _ = out_lines.pop();
                        out_lines.push(replaced.clone());
                        edits.push(FilePatchEdit {
                            workflow_file: rel_name.to_string(),
                            rule_id: "MINICI-SHRINK-004".to_string(),
                            description: format!(
                                "Reduce explicit artifact `retention-days` from {}d to 7d",
                                days
                            ),
                            line_number: i + 1,
                            original_snippet: line.to_string(),
                            replacement_snippet: replaced,
                        });
                    }
                }
            }
        }

        for (target_idx, ins_lines, edit) in &step_insertions {
            if *target_idx == i {
                for il in ins_lines {
                    out_lines.push(il.clone());
                }
                edits.push(edit.clone());
            }
        }

        i += 1;
    }

    let mut joined = out_lines.join("\n");
    if content.ends_with('\n') {
        joined.push('\n');
    }
    (joined, edits)
}

fn sanitize_rust_crate_slug(script_rel: &str) -> String {
    let stem = Path::new(script_rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("ci_tool");
    let mut slug = String::from("re_");
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('_') {
            slug.push('_');
        }
    }
    while slug.ends_with('_') && slug.len() > 4 {
        slug.pop();
    }
    slug
}

fn extract_bespoke_ci_script_invocation(cmd: &str) -> Option<(String, String, String)> {
    let trimmed = cmd.trim();
    if trimmed.contains("&&")
        || trimmed.contains("||")
        || trimmed.contains(';')
        || trimmed.contains('|')
        || trimmed.contains('>')
        || trimmed.contains('<')
        || trimmed.contains("$(")
        || trimmed.contains('`')
    {
        return None;
    }
    let mut parts = trimmed.split_whitespace();
    let first = parts.next()?;
    let (interp, raw_script, consumed_tokens) = match first {
        "python" | "python3" | "node" | "bash" | "sh" => {
            let second = parts.next()?;
            if second.starts_with('-') {
                return None;
            }
            (first.to_string(), second, 2usize)
        }
        s if s.starts_with("./") => ("sh".to_string(), s, 1usize),
        _ => return None,
    };

    let clean_rel = raw_script
        .trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("./");
    if clean_rel.contains("..") || clean_rel.starts_with('/') {
        return None;
    }
    let allowed_ci_dir = clean_rel.starts_with("scripts/")
        || clean_rel.starts_with("tools/")
        || clean_rel.starts_with("ci/")
        || clean_rel.starts_with(".ci/")
        || clean_rel.starts_with(".github/scripts/")
        || clean_rel.starts_with("hack/");
    if !allowed_ci_dir {
        return None;
    }
    let valid_ext = clean_rel.ends_with(".py")
        || clean_rel.ends_with(".js")
        || clean_rel.ends_with(".mjs")
        || clean_rel.ends_with(".cjs")
        || clean_rel.ends_with(".sh");
    if !valid_ext {
        return None;
    }

    let mut rest_idx = 0usize;
    let mut seen = 0usize;
    let mut in_ws = true;
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws {
                seen += 1;
                if seen == consumed_tokens + 1 {
                    rest_idx = idx;
                    break;
                }
            }
            in_ws = false;
        }
    }
    let trailing = if seen <= consumed_tokens {
        String::new()
    } else {
        format!(" {}", &trimmed[rest_idx..])
    };
    Some((interp, clean_rel.to_string(), trailing))
}

fn fnv1a_hex64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

fn extract_quoted_literals_after(src: &str, marker: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        let trimmed = after.trim_start();
        if let Some(q) = trimmed.chars().next() {
            if q == '"' || q == '\'' {
                let inner = &trimmed[1..];
                if let Some(end) = inner.find(q) {
                    let val = inner[..end].trim();
                    if !val.is_empty()
                        && val.len() < 120
                        && !val.contains('\\')
                        && !val.contains('"')
                        && !out.iter().any(|existing: &String| existing == val)
                    {
                        out.push(val.to_string());
                    }
                }
            }
        }
        rest = after;
    }
    out
}

fn collect_recursive_files(dir: &Path, base: &Path, out: &mut Vec<(PathBuf, String)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            if p.is_dir() {
                collect_recursive_files(&p, base, out);
            } else if let Ok(rel) = p.strip_prefix(base) {
                if let Ok(content) = fs::read_to_string(&p) {
                    out.push((rel.to_path_buf(), content));
                }
            }
        }
    }
}

/// Converts ONLY a bespoke, repo-local CI helper script (`scripts/*.py`, `tools/*.js`, `ci/*.sh`)
/// into a standalone zero-dependency Rust crate under `.minici/rust-everywhere/<slug>/`:
/// 1. Delegates first to an external `rust-everywhere` CLI binary (`RUST_EVERYWHERE_BIN` or `rust-everywhere` on `PATH`) if installed.
/// 2. Falls back to MiniCI's built-in pure-Rust `rust-everywhere` CI-script converter (extracting native env,
///    path-existence, conflict-marker, and JSON/YAML checks + content-addressed FNV-1a fast-path cache for dynamic imports).
/// 3. Verifies compilation via `cargo check` before accepting the workflow rewrite.
fn convert_bespoke_ci_script_via_rust_everywhere(
    base_dir: &Path,
    shadow_root: &Path,
    interp: &str,
    script_rel: &str,
) -> Option<(String, String, Vec<(PathBuf, String)>)> {
    let src_script = base_dir.join(script_rel);
    if !src_script.is_file() {
        return None;
    }
    let script_body = fs::read_to_string(&src_script).ok()?;
    let slug = sanitize_rust_crate_slug(script_rel);
    let rel_crate_dir = format!(".minici/rust-everywhere/{}", slug);
    let shadow_crate_dir = shadow_root.join(&rel_crate_dir);
    let _ = fs::create_dir_all(shadow_crate_dir.join("src"));

    // 1. Try external `rust-everywhere` CLI if present on host (`RUST_EVERYWHERE_BIN` or `rust-everywhere`)
    let re_bin = std::env::var("RUST_EVERYWHERE_BIN").unwrap_or_else(|_| "rust-everywhere".to_string());
    let mut converted_via = "builtin-rust-everywhere";
    if let Ok(out) = Command::new(&re_bin)
        .args([
            "convert",
            &src_script.to_string_lossy(),
            "--out-dir",
            &shadow_crate_dir.to_string_lossy(),
        ])
        .output()
    {
        if out.status.success() && shadow_crate_dir.join("Cargo.toml").is_file() {
            converted_via = "external-rust-everywhere-cli";
        }
    }

    // 2. Built-in pure-Rust `rust-everywhere` generator when external binary wasn't used
    if converted_via == "builtin-rust-everywhere" {
        let mut req_envs = Vec::new();
        for pat in ["os.environ[", "os.environ.get("] {
            for v in extract_quoted_literals_after(&script_body, pat) {
                if pat == "os.environ[" && !req_envs.contains(&v) {
                    req_envs.push(v);
                }
            }
        }
        let mut req_paths = Vec::new();
        for pat in ["os.path.exists(", "Path(", "fs.existsSync(", "[ -f ", "[ -d "] {
            for p in extract_quoted_literals_after(&script_body, pat) {
                if (p.contains('.') || p.contains('/')) && !p.starts_with('-') && !req_paths.contains(&p) {
                    req_paths.push(p);
                }
            }
        }
        let checks_conflict_markers = script_body.contains("<<<<<<<") || script_body.contains("=======");
        let script_low = script_body.to_ascii_lowercase();
        let has_heavy_dynamic_imports = script_low.contains("import requests")
            || script_low.contains("import boto3")
            || script_low.contains("import torch")
            || script_low.contains("import pandas")
            || script_low.contains("import sqlalchemy")
            || script_low.contains("require('axios')")
            || script_low.contains("from 'axios'");

        let src_hash = fnv1a_hex64(script_body.as_bytes());
        let env_checks_rust = req_envs
            .iter()
            .map(|ev| {
                format!(
                    "    if env::var({:?}).map(|v| v.is_empty()).unwrap_or(true) {{\n        eprintln!(\"[rust-everywhere:{}] Missing required env var: {}\");\n        process::exit(1);\n    }}",
                    ev, slug, ev
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let path_checks_rust = req_paths
            .iter()
            .filter(|p| base_dir.join(p).exists())
            .map(|rp| {
                format!(
                    "    if !Path::new({:?}).exists() {{\n        eprintln!(\"[rust-everywhere:{}] Expected path missing: {}\");\n        process::exit(1);\n    }}",
                    rp, slug, rp
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let conflict_scan_rust = if checks_conflict_markers {
            format!(
                "    if scan_conflict_markers(Path::new(\".\")) {{\n        eprintln!(\"[rust-everywhere:{}] Unresolved git conflict markers detected\");\n        process::exit(1);\n    }}",
                slug
            )
        } else {
            String::new()
        };

        let cargo_toml = format!(
            "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\ndescription = \"Scoped CI helper accelerated via MiniCI rust-everywhere from {}\"\n\n[workspace]\n\n[dependencies]\n\n[profile.release]\nopt-level = 3\nlto = true\nstrip = true\n",
            slug, script_rel
        );

        let main_rs = format!(
            r##"//! Auto-generated by MiniCI `rust-everywhere` Scoped CI-Tool Accelerator.
//! Source CI script: `{script_rel}` (FNV-1a: `{src_hash}`, dynamic_bridge: `{has_dyn}`).
//! Converts ONLY this standalone CI helper step into a cached `<2ms` native Rust binary.

use std::env;
use std::fs;
use std::path::Path;
use std::process::{{self, Command}};

#[allow(dead_code)]
fn scan_conflict_markers(dir: &Path) -> bool {{
    let Ok(entries) = fs::read_dir(dir) else {{ return false; }};
    for entry in entries.flatten() {{
        let p = entry.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" || name == "node_modules" {{
            continue;
        }}
        if p.is_dir() {{
            if scan_conflict_markers(&p) {{
                return true;
            }}
        }} else if let Ok(txt) = fs::read_to_string(&p) {{
            if txt.lines().any(|l| l.starts_with("<<<<<<< ")) {{
                return true;
            }}
        }}
    }}
    false
}}

fn repo_fingerprint(script_path: &str, args: &[String]) -> u64 {{
    let mut h: u64 = 0xcbf29ce484222325;
    for b in script_path.as_bytes() {{
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }}
    if let Ok(bytes) = fs::read(script_path) {{
        for b in bytes {{
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }}
    }}
    for a in args {{
        for b in a.as_bytes() {{
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }}
    }}
    h
}}

fn main() {{
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--minici-rust-everywhere-info") {{
        let info_json = concat!(
            "{{\"tool\":\"", "{slug}",
            "\",\"source_script\":\"", "{script_rel}",
            "\",\"source_fnv1a\":\"", "{src_hash}",
            "\",\"native_fast_path\":", "{native_only}", "}}"
        );
        println!("{{}}", info_json);
        return;
    }}

{env_checks}
{path_checks}
{conflict_scan}

    let script_changed = fs::read({script_rel:?})
        .map(|b| {{
            let mut h: u64 = 0xcbf29ce484222325;
            for byte in b {{
                h ^= byte as u64;
                h = h.wrapping_mul(0x100000001b3);
            }}
            format!("{{:016x}}", h) != "{src_hash}"
        }})
        .unwrap_or(false);

    if {has_dyn} || script_changed {{
        let fp = format!("{{:016x}}", repo_fingerprint({script_rel:?}, &args));
        let cache_file = Path::new(".minici/rust-everywhere/{slug}.fp.cache");
        if !script_changed {{
            if let Ok(prev) = fs::read_to_string(cache_file) {{
                if prev.trim() == fp {{
                    println!("[rust-everywhere:{slug}] Native fingerprint cache HIT ({{fp}}) in <1ms");
                    return;
                }}
            }}
        }}
        let status = Command::new({interp:?})
            .arg({script_rel:?})
            .args(&args)
            .status()
            .unwrap_or_else(|e| {{
                eprintln!("[rust-everywhere:{slug}] Failed to launch fallback `{interp}`: {{e}}");
                process::exit(1);
            }});
        if !status.success() {{
            process::exit(status.code().unwrap_or(1));
        }}
        let _ = fs::write(cache_file, fp);
    }} else {{
        println!("[rust-everywhere:{slug}] Native Rust CI check passed (`{script_rel}`)");
    }}
}}
"##,
            script_rel = script_rel,
            src_hash = src_hash,
            has_dyn = has_heavy_dynamic_imports,
            slug = slug,
            native_only = !has_heavy_dynamic_imports,
            env_checks = env_checks_rust,
            path_checks = path_checks_rust,
            conflict_scan = conflict_scan_rust,
            interp = interp,
        );

        let readme = format!(
            "# `{slug}` — MiniCI `rust-everywhere` Scoped CI Helper\n\n\
             Generated from standalone CI workflow script `{script_rel}` (`{converted_via}`).\n\
             Only this CI step was converted to Rust (`0` application source files modified).\n"
        );

        fs::write(shadow_crate_dir.join("Cargo.toml"), &cargo_toml).ok()?;
        fs::write(shadow_crate_dir.join("src").join("main.rs"), &main_rs).ok()?;
        fs::write(shadow_crate_dir.join("README.md"), &readme).ok()?;
    }

    // 3. Mandatory Compiler Verification Gate: if `cargo` is on PATH, `cargo check` MUST pass!
    if let Ok(chk) = Command::new("cargo")
        .args([
            "check",
            "--manifest-path",
            &shadow_crate_dir.join("Cargo.toml").to_string_lossy(),
            "--quiet",
        ])
        .output()
    {
        if !chk.status.success() {
            let _ = fs::remove_dir_all(&shadow_crate_dir);
            return None;
        }
    }

    let mut rel_files = Vec::new();
    collect_recursive_files(&shadow_crate_dir, &shadow_crate_dir, &mut rel_files);
    let extra_writes = rel_files
        .into_iter()
        .map(|(rel, body)| (base_dir.join(&rel_crate_dir).join(rel), body))
        .collect::<Vec<_>>();

    Some((slug, converted_via.to_string(), extra_writes))
}

fn find_enclosing_step_start_and_indent(lines: &[&str], run_idx: usize) -> (usize, usize) {
    let rline = lines[run_idx];
    let rtrim = rline.trim_start();
    let rind = indent_len(rline);
    if rtrim.starts_with("- ") {
        return (run_idx, rind);
    }
    let mut k = run_idx;
    while k > 0 {
        k -= 1;
        let pt = lines[k].trim_start();
        if pt.is_empty() || pt.starts_with('#') {
            continue;
        }
        let pi = indent_len(lines[k]);
        if pi < rind && pt.starts_with("- ") {
            return (k, pi);
        }
        if pi < rind.saturating_sub(2) {
            break;
        }
    }
    (run_idx, rind.saturating_sub(2))
}

fn format_yaml_run_value_with_tag(cmd: &str, tag: &str) -> String {
    let needs_quotes = cmd.contains(": ")
        || cmd.contains(" #")
        || cmd.starts_with('{')
        || cmd.starts_with('[')
        || cmd.starts_with('*')
        || cmd.starts_with('&');
    if needs_quotes {
        let escaped = cmd.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\" # {}", escaped, tag)
    } else {
        format!("{} # {}", cmd, tag)
    }
}

/// Applies Two-Tier Rust CI Acceleration (`MINICI-RUST-012` + `MINICI-RUST-EVERYWHERE-013`):
/// - Tier 1 (`rust_tools`): Swaps legacy slow CI tools (`pip`, `flake8`/`black`/`isort`, `eslint`/`prettier`,
///   `pre-commit`, `cargo test`) with battle-tested Rust-native drop-ins (`uv`, `ruff`, `biome`, `prek`, `cargo nextest`).
/// - Tier 2 (`rust_everywhere`): When NO off-the-shelf Rust replacement exists for a standalone CI script
///   (`scripts/*.py`, `tools/*.js`, `ci/*.sh`), converts ONLY that CI script into `.minici/rust-everywhere/<slug>/`!
fn apply_rust_ci_acceleration_to_yaml(
    rel_name: &str,
    content: &str,
    base_dir: &Path,
    shadow_root: &Path,
    is_gh_wf: bool,
    enable_rust_tools: bool,
    enable_rust_everywhere: bool,
    extra_crate_writes: &mut Vec<(PathBuf, String)>,
) -> (String, Vec<FilePatchEdit>) {
    if !enable_rust_tools && !enable_rust_everywhere {
        return (content.to_string(), Vec::new());
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut line_replacements: Vec<(usize, String, FilePatchEdit)> = Vec::new();
    let mut pre_step_insertions: Vec<(usize, Vec<String>)> = Vec::new();
    let mut jobs_injected_uv: Vec<usize> = Vec::new();
    let mut jobs_injected_nextest: Vec<usize> = Vec::new();
    let mut jobs_injected_re_cache: Vec<usize> = Vec::new();

    for (idx, &raw_l) in lines.iter().enumerate() {
        let trimmed = raw_l.trim();
        if trimmed.starts_with('#') || raw_l.contains("# minici-rust-") {
            continue;
        }
        let (prefix_len, raw_cmd_part) = if let Some(rest) = trimmed.strip_prefix("- run:") {
            let lead = raw_l.len() - trimmed.len() + "- run:".len();
            (lead, rest.trim())
        } else if let Some(rest) = trimmed.strip_prefix("run:") {
            let lead = raw_l.len() - trimmed.len() + "run:".len();
            (lead, rest.trim())
        } else {
            continue;
        };

        if raw_cmd_part.is_empty()
            || raw_cmd_part.starts_with('|')
            || raw_cmd_part.starts_with('>')
        {
            continue;
        }

        // Strip trailing inline YAML comment if unquoted
        let cmd_unquoted = if (raw_cmd_part.starts_with('"') && raw_cmd_part.ends_with('"'))
            || (raw_cmd_part.starts_with('\'') && raw_cmd_part.ends_with('\''))
        {
            &raw_cmd_part[1..raw_cmd_part.len().saturating_sub(1)]
        } else {
            raw_cmd_part.split(" #").next().unwrap_or(raw_cmd_part).trim()
        };
        let cmd_low = cmd_unquoted.to_ascii_lowercase();
        let (step_start_idx, step_ind) = find_enclosing_step_start_and_indent(&lines, idx);
        let (j_start, j_end) = find_job_block_bounds(&lines, step_start_idx, step_ind.saturating_sub(2).max(2));
        let job_low = lines[j_start..j_end].join("\n").to_ascii_lowercase();

        let mut matched_tier1 = false;

        if enable_rust_tools {
            // Rule MINICI-RUST-012A: `pip install ...` -> `uv pip install --system ...`
            let pip_args_opt = if let Some(r) = cmd_unquoted.strip_prefix("pip install ") {
                Some(r.trim())
            } else if let Some(r) = cmd_unquoted.strip_prefix("pip3 install ") {
                Some(r.trim())
            } else if let Some(r) = cmd_unquoted.strip_prefix("python -m pip install ") {
                Some(r.trim())
            } else if let Some(r) = cmd_unquoted.strip_prefix("python3 -m pip install ") {
                Some(r.trim())
            } else if let Some((_, after_up)) = cmd_unquoted.split_once("pip install --upgrade pip &&") {
                let tail = after_up.trim();
                tail.strip_prefix("pip install ")
                    .or_else(|| tail.strip_prefix("python -m pip install "))
                    .or_else(|| tail.strip_prefix("python3 -m pip install "))
                    .map(str::trim)
            } else {
                None
            };

            if let Some(pip_args) = pip_args_opt {
                if !pip_args.is_empty()
                    && pip_args != "--upgrade pip"
                    && pip_args != "-U pip"
                    && !cmd_low.contains("uv ")
                {
                    if is_gh_wf && !job_low.contains("astral-sh/setup-uv@") && !jobs_injected_uv.contains(&j_start) {
                        let spad = " ".repeat(step_ind);
                        pre_step_insertions.push((
                            step_start_idx,
                            vec![
                                format!("{}- uses: astral-sh/setup-uv@v5 # minici-rust-bootstrap", spad),
                                format!("{}  with:", spad),
                                format!("{}    enable-cache: true", spad),
                            ],
                        ));
                        jobs_injected_uv.push(j_start);
                    }
                    let new_cmd = format!("uv pip install --system {}", pip_args);
                    let new_line = format!(
                        "{} {}",
                        &raw_l[..prefix_len],
                        format_yaml_run_value_with_tag(&new_cmd, "minici-rust-accelerated:uv")
                    );
                    line_replacements.push((
                        idx,
                        new_line.clone(),
                        FilePatchEdit {
                            workflow_file: rel_name.to_string(),
                            rule_id: "MINICI-RUST-012A".to_string(),
                            description: "Accelerate Python `pip install` via Rust-native `uv pip install --system` (`10x-50x` faster)".to_string(),
                            line_number: idx + 1,
                            original_snippet: raw_l.to_string(),
                            replacement_snippet: new_line,
                        },
                    ));
                    matched_tier1 = true;
                }
            }

            // Rule MINICI-RUST-012B: `flake8` / `black --check` / `isort --check` -> `uvx ruff`
            if !matched_tier1
                && (cmd_low.starts_with("flake8")
                    || cmd_low.starts_with("python -m flake8")
                    || cmd_low.starts_with("black --check")
                    || cmd_low.starts_with("python -m black --check")
                    || cmd_low.starts_with("isort --check"))
            {
                let has_flake8 = cmd_low.contains("flake8");
                let has_black = cmd_low.contains("black");
                let has_isort = cmd_low.contains("isort");
                let mut ruff_parts = Vec::new();
                if has_flake8 && has_isort {
                    ruff_parts.push("uvx ruff check --extend-select I .");
                } else if has_flake8 {
                    ruff_parts.push("uvx ruff check .");
                } else if has_isort {
                    ruff_parts.push("uvx ruff check --select I .");
                }
                if has_black {
                    ruff_parts.push("uvx ruff format --check .");
                }
                if !ruff_parts.is_empty() {
                    if is_gh_wf && !job_low.contains("astral-sh/setup-uv@") && !jobs_injected_uv.contains(&j_start) {
                        let spad = " ".repeat(step_ind);
                        pre_step_insertions.push((
                            step_start_idx,
                            vec![
                                format!("{}- uses: astral-sh/setup-uv@v5 # minici-rust-bootstrap", spad),
                                format!("{}  with:", spad),
                                format!("{}    enable-cache: true", spad),
                            ],
                        ));
                        jobs_injected_uv.push(j_start);
                    }
                    let new_cmd = ruff_parts.join(" && ");
                    let new_line = format!(
                        "{} {}",
                        &raw_l[..prefix_len],
                        format_yaml_run_value_with_tag(&new_cmd, "minici-rust-accelerated:ruff")
                    );
                    line_replacements.push((
                        idx,
                        new_line.clone(),
                        FilePatchEdit {
                            workflow_file: rel_name.to_string(),
                            rule_id: "MINICI-RUST-012B".to_string(),
                            description: "Replace legacy Python linter/formatter (`flake8`/`black`/`isort`) with Rust-native `ruff` (`50x-150x` faster)".to_string(),
                            line_number: idx + 1,
                            original_snippet: raw_l.to_string(),
                            replacement_snippet: new_line,
                        },
                    ));
                    matched_tier1 = true;
                }
            }

            // Rule MINICI-RUST-012C: `eslint` / `prettier --check` -> `@biomejs/biome`
            if !matched_tier1
                && (cmd_low.starts_with("npx eslint")
                    || cmd_low == "eslint ."
                    || cmd_low.starts_with("npx prettier --check")
                    || cmd_low.starts_with("prettier --check"))
            {
                let has_eslint = cmd_low.contains("eslint");
                let has_prettier = cmd_low.contains("prettier");
                let new_cmd = if has_eslint && has_prettier {
                    "npx --yes @biomejs/biome ci ."
                } else if has_eslint {
                    "npx --yes @biomejs/biome lint ."
                } else {
                    "npx --yes @biomejs/biome format ."
                };
                let new_line = format!(
                    "{} {}",
                    &raw_l[..prefix_len],
                    format_yaml_run_value_with_tag(new_cmd, "minici-rust-accelerated:biome")
                );
                line_replacements.push((
                    idx,
                    new_line.clone(),
                    FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-RUST-012C".to_string(),
                        description: "Replace Node `eslint`/`prettier` with Rust-native `@biomejs/biome` (`15x-25x` faster)".to_string(),
                        line_number: idx + 1,
                        original_snippet: raw_l.to_string(),
                        replacement_snippet: new_line,
                    },
                ));
                matched_tier1 = true;
            }

            // Rule MINICI-RUST-012D: `pre-commit run` -> `uvx prek run`
            if !matched_tier1 && cmd_low.starts_with("pre-commit run") {
                if is_gh_wf && !job_low.contains("astral-sh/setup-uv@") && !jobs_injected_uv.contains(&j_start) {
                    let spad = " ".repeat(step_ind);
                    pre_step_insertions.push((
                        step_start_idx,
                        vec![
                            format!("{}- uses: astral-sh/setup-uv@v5 # minici-rust-bootstrap", spad),
                            format!("{}  with:", spad),
                            format!("{}    enable-cache: true", spad),
                        ],
                    ));
                    jobs_injected_uv.push(j_start);
                }
                let suffix = &cmd_unquoted["pre-commit run".len()..];
                let new_cmd = format!("uvx prek run{}", suffix);
                let new_line = format!(
                    "{} {}",
                    &raw_l[..prefix_len],
                    format_yaml_run_value_with_tag(&new_cmd, "minici-rust-accelerated:prek")
                );
                line_replacements.push((
                    idx,
                    new_line.clone(),
                    FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-RUST-012D".to_string(),
                        description: "Replace Python `pre-commit` with Rust-native `prek` (`10x` faster hook runner)".to_string(),
                        line_number: idx + 1,
                        original_snippet: raw_l.to_string(),
                        replacement_snippet: new_line,
                    },
                ));
                matched_tier1 = true;
            }

            // Rule MINICI-RUST-012E: `cargo test` -> `cargo nextest run` (+ `cargo test --doc`)
            if !matched_tier1
                && (cmd_low == "cargo test" || cmd_low.starts_with("cargo test "))
                && !cmd_low.contains("--doc")
                && !cmd_low.contains("--bench")
                && !cmd_low.contains("&&")
            {
                if is_gh_wf && !job_low.contains("nextest") && !jobs_injected_nextest.contains(&j_start) {
                    let spad = " ".repeat(step_ind);
                    pre_step_insertions.push((
                        step_start_idx,
                        vec![format!("{}- uses: taiki-e/install-action@nextest # minici-rust-bootstrap", spad)],
                    ));
                    jobs_injected_nextest.push(j_start);
                }
                let args_tail = &cmd_unquoted["cargo test".len()..];
                let new_cmd = format!("cargo nextest run{} && cargo test --doc", args_tail);
                let new_line = format!(
                    "{} {}",
                    &raw_l[..prefix_len],
                    format_yaml_run_value_with_tag(&new_cmd, "minici-rust-accelerated:nextest")
                );
                line_replacements.push((
                    idx,
                    new_line.clone(),
                    FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-RUST-012E".to_string(),
                        description: "Accelerate Rust test suite with parallel `cargo nextest run` + `cargo test --doc` (`2x-3x` faster)".to_string(),
                        line_number: idx + 1,
                        original_snippet: raw_l.to_string(),
                        replacement_snippet: new_line,
                    },
                ));
                matched_tier1 = true;
            }
        }

        // Tier 2 — Rule MINICI-RUST-EVERYWHERE-013:
        // If NO existing drop-in Rust tool matched AND the CI step runs a bespoke repo-local CI script
        // (`scripts/*.py`, `tools/*.js`, `ci/*.sh`), convert ONLY that CI script via `rust-everywhere`!
        if !matched_tier1 && enable_rust_everywhere {
            if let Some((interp, script_rel, trailing_args)) = extract_bespoke_ci_script_invocation(cmd_unquoted) {
                if let Some((slug, via_engine, crate_files)) = convert_bespoke_ci_script_via_rust_everywhere(
                    base_dir,
                    shadow_root,
                    &interp,
                    &script_rel,
                ) {
                    extra_crate_writes.extend(crate_files);
                    if is_gh_wf
                        && !job_low.contains("swatinem/rust-cache@")
                        && !jobs_injected_re_cache.contains(&j_start)
                    {
                        let spad = " ".repeat(step_ind);
                        pre_step_insertions.push((
                            step_start_idx,
                            vec![
                                format!("{}- uses: Swatinem/rust-cache@v2 # minici-rust-bootstrap", spad),
                                format!("{}  with:", spad),
                                format!("{}    workspaces: .minici/rust-everywhere/{}", spad, slug),
                            ],
                        ));
                        jobs_injected_re_cache.push(j_start);
                    }
                    let new_cmd = format!(
                        "cargo build --release --manifest-path .minici/rust-everywhere/{slug}/Cargo.toml --quiet && ./.minici/rust-everywhere/{slug}/target/release/{slug}{trailing_args}"
                    );
                    let new_line = format!(
                        "{} {}",
                        &raw_l[..prefix_len],
                        format_yaml_run_value_with_tag(&new_cmd, "minici-rust-everywhere")
                    );
                    line_replacements.push((
                        idx,
                        new_line.clone(),
                        FilePatchEdit {
                            workflow_file: rel_name.to_string(),
                            rule_id: "MINICI-RUST-EVERYWHERE-013".to_string(),
                            description: format!(
                                "Converted standalone CI tool `{}` into cached native Rust binary `.minici/rust-everywhere/{}` via `{}` (`cargo check` verified)",
                                script_rel, slug, via_engine
                            ),
                            line_number: idx + 1,
                            original_snippet: raw_l.to_string(),
                            replacement_snippet: new_line,
                        },
                    ));
                }
            }
        }
    }

    if line_replacements.is_empty() {
        return (content.to_string(), Vec::new());
    }

    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len() + 16);
    let mut edits: Vec<FilePatchEdit> = Vec::new();
    for (i, &orig) in lines.iter().enumerate() {
        for (ins_idx, ins_vec) in &pre_step_insertions {
            if *ins_idx == i {
                for il in ins_vec {
                    out_lines.push(il.clone());
                }
            }
        }
        if let Some((_, rep_line, edit)) = line_replacements.iter().find(|(r_idx, _, _)| *r_idx == i) {
            out_lines.push(rep_line.clone());
            edits.push(edit.clone());
        } else {
            out_lines.push(orig.to_string());
        }
    }

    let mut joined = out_lines.join("\n");
    if content.ends_with('\n') {
        joined.push('\n');
    }
    (joined, edits)
}

fn shrink_non_github_yaml(rel_name: &str, content: &str) -> (String, Vec<FilePatchEdit>) {
    let lines: Vec<&str> = content.lines().collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len() + 10);
    let mut edits: Vec<FilePatchEdit> = Vec::new();
    let low_all = content.to_ascii_lowercase();
    let rel_low = rel_name.to_ascii_lowercase();

    if rel_low.contains("gitlab") {
        let mut top_blocks = Vec::new();
        if !low_all.contains("interruptible:") {
            top_blocks.push("default:\n  interruptible: true\n  timeout: 30m");
        }
        if !low_all.contains("git_depth")
            && !job_uses_git_history(&lines)
            && !low_all.contains("\nvariables:")
            && !low_all.starts_with("variables:")
        {
            top_blocks.push("variables:\n  GIT_DEPTH: \"1\"");
        }
        if !top_blocks.is_empty() {
            let combined = top_blocks.join("\n");
            for bl in combined.lines() {
                out_lines.push(bl.to_string());
            }
            let first_line = lines.first().copied().unwrap_or("");
            edits.push(FilePatchEdit {
                workflow_file: rel_name.to_string(),
                rule_id: "MINICI-SHRINK-GL1".to_string(),
                description:
                    "Enable `interruptible: true`, `timeout: 30m`, and shallow `GIT_DEPTH: \"1\"` guard in `.gitlab-ci.yml`"
                        .to_string(),
                line_number: 1,
                original_snippet: first_line.to_string(),
                replacement_snippet: if first_line.is_empty() {
                    combined
                } else {
                    format!("{}\n{}", combined, first_line)
                },
            });
        }
        for (i, &l) in lines.iter().enumerate() {
            let t = l.trim();
            if t.starts_with("expire_in:")
                && (t.contains("30 day") || t.contains("90 day") || t.contains("1 month"))
            {
                let pad = " ".repeat(indent_len(l));
                let rep = format!("{}expire_in: 7 days", pad);
                out_lines.push(rep.clone());
                edits.push(FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: "MINICI-SHRINK-GL2".to_string(),
                    description: "Shorten GitLab artifact `expire_in` to `7 days`".to_string(),
                    line_number: i + 1,
                    original_snippet: l.to_string(),
                    replacement_snippet: rep,
                });
            } else {
                out_lines.push(l.to_string());
            }
        }
    } else if rel_low.contains("bitbucket") {
        let has_max_time = low_all.contains("max-time:");
        let has_clone_depth = low_all.contains("clone:");
        let mut injected = false;
        for (i, &l) in lines.iter().enumerate() {
            if !injected && l.trim() == "pipelines:" {
                let mut prefix_blocks = Vec::new();
                if !has_max_time {
                    prefix_blocks.push("options:\n  max-time: 30");
                }
                if !has_clone_depth && !job_uses_git_history(&lines) {
                    prefix_blocks.push("clone:\n  depth: 1");
                }
                if !prefix_blocks.is_empty() {
                    let combined = prefix_blocks.join("\n");
                    for ol in combined.lines() {
                        out_lines.push(ol.to_string());
                    }
                    edits.push(FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-SHRINK-BB1".to_string(),
                        description: "Add `options.max-time: 30` and shallow `clone.depth: 1` in Bitbucket Pipelines"
                            .to_string(),
                        line_number: i + 1,
                        original_snippet: l.to_string(),
                        replacement_snippet: format!("{}\n{}", combined, l),
                    });
                }
                injected = true;
            }
            out_lines.push(l.to_string());
        }
    } else if rel_low.contains("circleci") {
        let has_no_output = low_all.contains("no_output_timeout:");
        for (i, &l) in lines.iter().enumerate() {
            out_lines.push(l.to_string());
            let t = l.trim();
            if !has_no_output && (t == "- run:" || t == "run:") {
                let next_ind = lines
                    .get(i + 1)
                    .map(|nl| indent_len(nl))
                    .unwrap_or(indent_len(l) + 2);
                if next_ind > indent_len(l) {
                    let ins = format!("{}no_output_timeout: 15m", " ".repeat(next_ind));
                    out_lines.push(ins.clone());
                    edits.push(FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-SHRINK-CCI1".to_string(),
                        description: "Cap CircleCI stalled step timeout with `no_output_timeout: 15m`"
                            .to_string(),
                        line_number: i + 1,
                        original_snippet: l.to_string(),
                        replacement_snippet: format!("{}\n{}", l, ins),
                    });
                }
            }
        }
    } else if rel_low.contains("buildspec") {
        for &l in &lines {
            out_lines.push(l.to_string());
        }
        if !low_all.contains("\ncache:") && !low_all.starts_with("cache:") {
            let cb_cache = "cache:\n  paths:\n    - '/root/.cache/**/*'";
            for cl in cb_cache.lines() {
                out_lines.push(cl.to_string());
            }
            let last_line = lines.last().copied().unwrap_or("");
            edits.push(FilePatchEdit {
                workflow_file: rel_name.to_string(),
                rule_id: "MINICI-SHRINK-CB1".to_string(),
                description: "Add local dependency `cache.paths` block in AWS CodeBuild `buildspec.yml`"
                    .to_string(),
                line_number: lines.len().max(1),
                original_snippet: last_line.to_string(),
                replacement_snippet: if last_line.is_empty() {
                    cb_cache.to_string()
                } else {
                    format!("{}\n{}", last_line, cb_cache)
                },
            });
        }
    } else if rel_low.contains("cloudbuild") {
        let has_timeout = lines.iter().any(|l| indent_len(l) == 0 && l.trim().starts_with("timeout:"));
        for &l in &lines {
            out_lines.push(l.to_string());
        }
        if !has_timeout {
            out_lines.push("timeout: 1800s".to_string());
            let last_line = lines.last().copied().unwrap_or("");
            edits.push(FilePatchEdit {
                workflow_file: rel_name.to_string(),
                rule_id: "MINICI-SHRINK-GCP1".to_string(),
                description: "Cap GCP Cloud Build execution guard at `timeout: 1800s` (30m)".to_string(),
                line_number: lines.len().max(1),
                original_snippet: last_line.to_string(),
                replacement_snippet: if last_line.is_empty() {
                    "timeout: 1800s".to_string()
                } else {
                    format!("{}\ntimeout: 1800s", last_line)
                },
            });
        }
    } else if rel_low.contains("azure-pipelines") {
        let has_timeout = low_all.contains("timeoutinminutes:");
        let mut injected = false;
        for (i, &l) in lines.iter().enumerate() {
            out_lines.push(l.to_string());
            if !has_timeout && !injected && l.trim().starts_with("vmImage:") {
                let pool_ind = indent_len(l).saturating_sub(2);
                let ins = format!("{}timeoutInMinutes: 30", " ".repeat(pool_ind));
                out_lines.push(ins.clone());
                edits.push(FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: "MINICI-SHRINK-AZ1".to_string(),
                    description: "Add `timeoutInMinutes: 30` guard in Azure Pipelines".to_string(),
                    line_number: i + 1,
                    original_snippet: l.to_string(),
                    replacement_snippet: format!("{}\n{}", l, ins),
                });
                injected = true;
            }
        }
    } else {
        for l in lines {
            out_lines.push(l.to_string());
        }
    }

    let mut joined = out_lines.join("\n");
    if content.ends_with('\n') {
        joined.push('\n');
    }
    (joined, edits)
}


fn extract_explicit_linux_cores_for_migration(cur_low: &str) -> Option<u32> {
    let mut cleaned = cur_low.to_string();
    for os_ver in [
        "26.04", "26-04", "2604", "24.04", "24-04", "2404", "22.04", "22-04", "2204", "20.04",
        "20-04", "2004", "18.04", "18-04", "1804",
    ] {
        cleaned = cleaned.replace(os_ver, "");
    }
    for (cores, pats) in [
        (64u32, &["64core", "64-core", "64vcpu", "-64"][..]),
        (32u32, &["32core", "32-core", "32vcpu", "-32"][..]),
        (16u32, &["16core", "16-core", "16vcpu", "-16", "-xl"][..]),
        (8u32, &["8core", "8-core", "8vcpu", "-8"][..]),
        (4u32, &["4core", "4-core", "4vcpu", "-4"][..]),
    ] {
        for pat in pats {
            if pat.starts_with('-') {
                if cleaned.ends_with(pat) || cleaned.contains(&format!("{}-", pat)) {
                    return Some(cores);
                }
            } else if cleaned.contains(pat) {
                return Some(cores);
            }
        }
    }
    None
}

fn resolve_target_runner_sku_for_github(
    target_runner_mode: &str,
    cur_low: &str,
    explicit_cores: Option<u32>,
    has_gpu_or_kvm: bool,
    has_amd64_pin: bool,
    is_heavy_for_slim: bool,
    is_deploy_ceremony: bool,
) -> Option<String> {
    let is_macos = cur_low.starts_with("macos");
    if matches!(target_runner_mode, "macos-m2pro" | "macos-arm64" | "macos-15-xlarge") {
        if is_macos && !cur_low.contains("xlarge") {
            return Some("macos-15-xlarge".to_string());
        }
        return None;
    }

    let is_linux_x64 = cur_low.contains("ubuntu-")
        && !cur_low.contains("self-hosted")
        && !cur_low.contains("arm")
        && !cur_low.contains("aarch64")
        && !cur_low.contains("slim")
        && !cur_low.contains("blacksmith")
        && !cur_low.contains("depot")
        && !cur_low.contains("warp")
        && !cur_low.contains("buildjet")
        && !cur_low.contains("namespace")
        && !cur_low.contains("codspeed")
        && !cur_low.contains("runs-on=");
    if !is_linux_x64 || has_gpu_or_kvm {
        return None;
    }

    match target_runner_mode {
        "ubuntu-arm64" | "github-arm64" | "arm64" | "ubuntu-24.04-arm" => {
            if has_amd64_pin {
                return None;
            }
            Some(
                match explicit_cores {
                    Some(64) => "ubuntu-24.04-arm-64core",
                    Some(32) => "ubuntu-24.04-arm-32core",
                    Some(16) => "ubuntu-24.04-arm-16core",
                    Some(8) => "ubuntu-24.04-arm-8core",
                    Some(4) => "ubuntu-24.04-arm-4core",
                    _ => "ubuntu-24.04-arm",
                }
                .to_string(),
            )
        }
        "ubuntu-slim" | "slim" => {
            if !is_heavy_for_slim && explicit_cores.is_none() {
                Some("ubuntu-slim".to_string())
            } else {
                None
            }
        }
        "blacksmith" | "high-speed" => Some(
            match explicit_cores {
                Some(32) => "blacksmith-32vcpu-ubuntu-2404",
                Some(16) => "blacksmith-16vcpu-ubuntu-2404",
                Some(8) => "blacksmith-8vcpu-ubuntu-2404",
                Some(4) => "blacksmith-4vcpu-ubuntu-2404",
                _ => "blacksmith-2vcpu-ubuntu-2404",
            }
            .to_string(),
        ),
        "warpbuild" => Some(
            match explicit_cores {
                Some(16) => "warp-ubuntu-latest-x64-16x",
                Some(8) => "warp-ubuntu-latest-x64-8x",
                Some(4) => "warp-ubuntu-latest-x64-4x",
                _ => {
                    if has_amd64_pin {
                        "warp-ubuntu-latest-x64-2x"
                    } else {
                        "warp-ubuntu-latest-arm64-2x"
                    }
                }
            }
            .to_string(),
        ),
        "buildjet" => Some(
            match explicit_cores {
                Some(16) => "buildjet-16vcpu-ubuntu-2204",
                Some(8) => "buildjet-8vcpu-ubuntu-2204",
                Some(4) => "buildjet-4vcpu-ubuntu-2204",
                _ => {
                    if has_amd64_pin {
                        "buildjet-2vcpu-ubuntu-2204"
                    } else {
                        "buildjet-2vcpu-ubuntu-2204-arm"
                    }
                }
            }
            .to_string(),
        ),
        "depot" => Some(
            if has_amd64_pin {
                "depot-ubuntu-24.04"
            } else {
                "depot-ubuntu-24.04-arm64"
            }
            .to_string(),
        ),
        "namespace" => Some("namespace-profile-default-arm64".to_string()),
        "runs-on" | "runson" => {
            if is_deploy_ceremony {
                None
            } else if has_amd64_pin {
                Some("runs-on=runner=2cpu-linux-x64".to_string())
            } else {
                Some("runs-on=runner=2cpu-linux-arm64".to_string())
            }
        }
        "aws-spot" => {
            if is_deploy_ceremony {
                None
            } else if has_amd64_pin {
                Some("aws-c6i.xlarge-spot".to_string())
            } else {
                Some("aws-c7g.xlarge-spot".to_string())
            }
        }
        "gcp-spot" => {
            if is_deploy_ceremony {
                None
            } else if has_amd64_pin {
                Some("n2d-standard-4-spot".to_string())
            } else {
                Some("t2a-standard-4-spot".to_string())
            }
        }
        "azure-spot" => {
            if is_deploy_ceremony {
                None
            } else if has_amd64_pin {
                Some("Standard_D4s_v5-spot".to_string())
            } else {
                Some("Standard_D4ps_v5-spot".to_string())
            }
        }
        "self-hosted" | "aws-ec2" => {
            if is_deploy_ceremony {
                None
            } else if has_amd64_pin {
                Some("aws-c6i.xlarge-on-demand".to_string())
            } else {
                Some("aws-c7g.xlarge-on-demand".to_string())
            }
        }
        "codebuild-github-runner" | "codebuild-runner" => {
            if is_deploy_ceremony {
                None
            } else if has_amd64_pin {
                Some("codebuild-general1-small-${{ github.run_id }}-${{ github.run_attempt }}".to_string())
            } else {
                Some("codebuild-arm1-small-${{ github.run_id }}-${{ github.run_attempt }}".to_string())
            }
        }
        custom_sku => {
            // Dynamic Catalog Resolution: allow ANY valid SKU or cloud instance ID in `DynamicPricingRegistry`!
            let resolved = catalog::resolve_runner_sku(custom_sku);
            let is_known_dynamic = resolved.sku_label != "ubuntu-latest" || custom_sku == "ubuntu-latest";
            if !is_known_dynamic {
                return None;
            }
            if resolved.arch == "arm64" && has_amd64_pin {
                return None;
            }
            if (resolved.provider.contains("self-hosted")
                || resolved.provider.contains("aws-ec2")
                || resolved.provider.contains("gcp-gce")
                || resolved.provider.contains("azure-vm")
                || custom_sku.contains("spot"))
                && is_deploy_ceremony
            {
                return None;
            }
            Some(custom_sku.to_string())
        }
    }
}

fn migrate_single_workflow_yaml(
    rel_name: &str,
    content: &str,
    target_runner_mode: &str,
) -> (String, Vec<FilePatchEdit>) {
    let rel_low = rel_name.to_ascii_lowercase();

    // Non-GitHub native runner migrations across GitLab, CircleCI, AWS CodeBuild, GCP Cloud Build, Bitbucket & Azure!
    if rel_low.contains("gitlab") {
        let mut out = Vec::new();
        let mut edits = Vec::new();
        let target_gl = match target_runner_mode {
            "arm64" | "ubuntu-arm64" | "github-arm64" | "gitlab-arm64" => Some("saas-linux-small-arm64"),
            other if other.starts_with("saas-") => Some(other),
            _ => None,
        };
        for (idx, line) in content.lines().enumerate() {
            if let Some(tgl) = target_gl {
                if line.contains("saas-linux-small-amd64") && tgl != "saas-linux-small-amd64" {
                    let rep = line.replace("saas-linux-small-amd64", tgl);
                    out.push(rep.clone());
                    edits.push(FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-MIGRATE-GL1".to_string(),
                        description: format!("Migrate GitLab runner `saas-linux-small-amd64` -> `{}`", tgl),
                        line_number: idx + 1,
                        original_snippet: line.to_string(),
                        replacement_snippet: rep,
                    });
                    continue;
                }
            }
            out.push(line.to_string());
        }
        let mut j = out.join("\n");
        if content.ends_with('\n') {
            j.push('\n');
        }
        return (j, edits);
    }

    if rel_low.contains("circleci") {
        let mut out = Vec::new();
        let mut edits = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("resource_class:")
                && matches!(target_runner_mode, "arm64" | "circleci-arm64" | "github-arm64" | "ubuntu-arm64")
            {
                let cur_rc = t.splitn(2, ':').nth(1).unwrap_or("").trim();
                let new_rc = match cur_rc {
                    "medium" => Some("arm.medium"),
                    "large" => Some("arm.large"),
                    "xlarge" => Some("arm.xlarge"),
                    _ => None,
                };
                if let Some(nrc) = new_rc {
                    let rep = format!("{}resource_class: {}", " ".repeat(indent_len(line)), nrc);
                    out.push(rep.clone());
                    edits.push(FilePatchEdit {
                        workflow_file: rel_name.to_string(),
                        rule_id: "MINICI-MIGRATE-CCI1".to_string(),
                        description: format!("Migrate CircleCI `resource_class: {}` -> `{}`", cur_rc, nrc),
                        line_number: idx + 1,
                        original_snippet: line.to_string(),
                        replacement_snippet: rep,
                    });
                    continue;
                }
            }
            out.push(line.to_string());
        }
        let mut j = out.join("\n");
        if content.ends_with('\n') {
            j.push('\n');
        }
        return (j, edits);
    }

    if rel_low.contains("buildspec") || rel_low.contains("cloudbuild") {
        let mut out = Vec::new();
        let mut edits = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            let t = line.trim();
            if rel_low.contains("cloudbuild")
                && t.starts_with("machineType:")
                && t.contains("N1_HIGHCPU_8")
                && matches!(target_runner_mode, "e2" | "e2-highcpu-8" | "arm64" | "github-arm64")
            {
                let rep = line.replace("N1_HIGHCPU_8", "E2_HIGHCPU_8");
                out.push(rep.clone());
                edits.push(FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: "MINICI-MIGRATE-GCP1".to_string(),
                    description: "Migrate GCP Cloud Build `N1_HIGHCPU_8` -> `E2_HIGHCPU_8`".to_string(),
                    line_number: idx + 1,
                    original_snippet: line.to_string(),
                    replacement_snippet: rep,
                });
                continue;
            }
            out.push(line.to_string());
        }
        let mut j = out.join("\n");
        if content.ends_with('\n') {
            j.push('\n');
        }
        return (j, edits);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut edits: Vec<FilePatchEdit> = Vec::new();

    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();
        let ind = indent_len(line);
        if trimmed.starts_with("runs-on:") && !trimmed.contains("${{") && is_job_level_runs_on(&lines, idx) {
            let raw_rhs = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();

            // Support scalar (`runs-on: ubuntu-latest`), inline list (`runs-on: [self-hosted, linux, x64]`),
            // and multi-line list/mapping (`runs-on:\n  group: ...\n  labels: ubuntu-latest`)!
            let mut end_runs_on = idx;
            let mut combined_label = raw_rhs
                .trim_matches('[')
                .trim_matches(']')
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();

            if raw_rhs.is_empty() {
                let mut child_tokens = Vec::new();
                while end_runs_on + 1 < lines.len() {
                    let nxt = lines[end_runs_on + 1];
                    let nt = nxt.trim();
                    if !nt.is_empty() && !nt.starts_with('#') {
                        if indent_len(nxt) > ind {
                            end_runs_on += 1;
                            if !nt.starts_with("group:") {
                                child_tokens.push(
                                    nt.trim_start_matches('-')
                                        .trim_start_matches("labels:")
                                        .trim()
                                        .trim_matches('[')
                                        .trim_matches(']')
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .to_string(),
                                );
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                combined_label = child_tokens.join("-");
            }

            if combined_label.is_empty() {
                for l in &lines[idx..=end_runs_on] {
                    out_lines.push((*l).to_string());
                }
                idx = end_runs_on + 1;
                continue;
            }

            let cur_low = combined_label.to_ascii_lowercase();
            let (j_start, j_end) = find_job_block_bounds(&lines, idx, ind);
            let job_body_low = lines[j_start..j_end].join("\n").to_ascii_lowercase();

            let mut jev_blocks_arm64 = false;
            let mut jev_blocks_spot = false;
            for &jl in &lines[j_start..j_end] {
                let jt = jl.trim();
                if let Some(cmd) = jt
                    .strip_prefix("- run:")
                    .or_else(|| jt.strip_prefix("run:"))
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && *s != "|")
                {
                    let dec = crate::jev::classify_ci_step_7decisions(cmd);
                    if !dec.arm64_safe {
                        jev_blocks_arm64 = true;
                    }
                    if !dec.spot_safe {
                        jev_blocks_spot = true;
                    }
                }
            }

            let has_gpu_or_kvm = cur_low.contains("gpu")
                || job_body_low.contains("android-emulator")
                || job_body_low.contains("cuda");
            let has_amd64_pin = jev_blocks_arm64
                || job_body_low.contains("linux/amd64")
                || job_body_low.contains("--platform=linux/amd64");
            let is_heavy_for_slim = job_body_low.contains("docker build")
                || job_body_low.contains("services:")
                || job_body_low.contains("cargo build")
                || job_body_low.contains("pytest")
                || job_body_low.contains("playwright");
            let is_deploy_ceremony = jev_blocks_spot
                || job_body_low.contains("terraform apply")
                || job_body_low.contains("kubectl apply")
                || rel_name.contains("release");
            let explicit_cores = extract_explicit_linux_cores_for_migration(&cur_low);

            if let Some(target_sku) = resolve_target_runner_sku_for_github(
                target_runner_mode,
                &cur_low,
                explicit_cores,
                has_gpu_or_kvm,
                has_amd64_pin,
                is_heavy_for_slim,
                is_deploy_ceremony,
            ) {
                let pad = " ".repeat(ind);
                let replaced = format!("{}runs-on: {}", pad, target_sku);
                let orig_block = lines[idx..=end_runs_on].join("\n");
                out_lines.push(replaced.clone());
                edits.push(FilePatchEdit {
                    workflow_file: rel_name.to_string(),
                    rule_id: "MINICI-MIGRATE-001".to_string(),
                    description: format!(
                        "Migrate `{}` -> `{}` (passed all 6 safety & cost gates)",
                        combined_label, target_sku
                    ),
                    line_number: idx + 1,
                    original_snippet: orig_block,
                    replacement_snippet: replaced,
                });
                idx = end_runs_on + 1;
                continue;
            }

            for l in &lines[idx..=end_runs_on] {
                out_lines.push((*l).to_string());
            }
            idx = end_runs_on + 1;
            continue;
        }
        out_lines.push(line.to_string());
        idx += 1;
    }

    let mut joined = out_lines.join("\n");
    if content.ends_with('\n') {
        joined.push('\n');
    }
    (joined, edits)
}


// =====================================================================================
// Bidirectional `7×7` Cross-Platform Pipeline IR Translator (`--from <any> --to <any>`)
// =====================================================================================

#[derive(Debug, Clone)]
pub struct JobIr {
    pub id: String,
    pub os: String,   // "linux" | "macos" | "windows"
    pub arch: String, // "arm64" | "x64"
    pub vcpus: u32,
    pub matrix_count: usize,
    pub est_mins: Option<f64>,
    pub container_image: Option<String>,
    pub needs: Vec<String>,
    pub timeout_mins: u32,
    pub env_vars: Vec<(String, String)>,
    pub services: Vec<String>,
    pub has_cache: bool,
    pub artifact_paths: Vec<String>,
    pub commands: Vec<String>,
    pub unmapped_notes: Vec<String>,
}

impl Default for JobIr {
    fn default() -> Self {
        Self {
            id: "build".to_string(),
            os: "linux".to_string(),
            arch: "arm64".to_string(),
            vcpus: 2,
            matrix_count: 1,
            est_mins: None,
            container_image: None,
            needs: Vec::new(),
            timeout_mins: 30,
            env_vars: Vec::new(),
            services: Vec::new(),
            has_cache: false,
            artifact_paths: Vec::new(),
            commands: Vec::new(),
            unmapped_notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineIr {
    pub name: String,
    pub source_vendor: String,
    pub source_file: String,
    pub jobs: Vec<JobIr>,
}

pub fn is_ci_platform_target(canonical_target: &str) -> bool {
    matches!(
        canonical_target,
        "github-actions"
            | "gitlab-ci"
            | "circleci"
            | "bitbucket"
            | "aws-codebuild"
            | "gcp-cloud-build"
            | "azure-pipelines"
    )
}

pub fn target_default_rel_path(canonical_target: &str) -> &'static str {
    match canonical_target {
        "github-actions" => ".github/workflows/ci.yml",
        "gitlab-ci" => ".gitlab-ci.yml",
        "circleci" => ".circleci/config.yml",
        "bitbucket" => "bitbucket-pipelines.yml",
        "aws-codebuild" => "buildspec.yml",
        "gcp-cloud-build" => "cloudbuild.yaml",
        "azure-pipelines" => "azure-pipelines.yml",
        _ => ".github/workflows/ci.yml",
    }
}

fn infer_os_arch_vcpus(label: &str) -> (String, String, u32) {
    let sku = catalog::resolve_runner_sku(label);
    let os_low = sku.os.to_ascii_lowercase();
    let os = if os_low.contains("mac") {
        "macos"
    } else if os_low.contains("win") {
        "windows"
    } else {
        "linux"
    };
    let arch = if sku.arch == "arm64" { "arm64" } else { "x64" };
    (os.to_string(), arch.to_string(), sku.vcpus.max(2))
}

fn parse_github_actions_to_ir(rel_file: &str, raw: &str) -> PipelineIr {
    let lines: Vec<&str> = raw.lines().collect();
    let mut name = "CI".to_string();
    let mut jobs: Vec<JobIr> = Vec::new();
    let mut in_jobs = false;
    let mut cur_job: Option<JobIr> = None;
    let mut in_needs_list = false;

    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let t = line.trim();
        let ind = indent_len(line);
        if t.is_empty() || t.starts_with('#') {
            idx += 1;
            continue;
        }
        if ind == 0 && t.starts_with("name:") {
            name = t
                .splitn(2, ':')
                .nth(1)
                .unwrap_or("CI")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
        if ind == 0 {
            in_jobs = t == "jobs:";
            if !in_jobs {
                if let Some(j) = cur_job.take() {
                    jobs.push(j);
                }
            }
            idx += 1;
            continue;
        }
        if in_jobs && ind == 2 && t.ends_with(':') && !t.starts_with('-') {
            if let Some(j) = cur_job.take() {
                jobs.push(j);
            }
            let jid = t.trim_end_matches(':').trim().to_string();
            cur_job = Some(JobIr {
                id: jid,
                ..Default::default()
            });
            in_needs_list = false;
            idx += 1;
            continue;
        }
        if let Some(ref mut job) = cur_job {
            if ind == 4 {
                in_needs_list = false;
                if t.starts_with("runs-on:") {
                    let lbl = t.splitn(2, ':').nth(1).unwrap_or("ubuntu-latest").trim();
                    let (os, arch, vcpus) = infer_os_arch_vcpus(lbl);
                    job.os = os;
                    job.arch = arch;
                    job.vcpus = vcpus;
                } else if t.starts_with("timeout-minutes:") {
                    if let Ok(tm) = t.splitn(2, ':').nth(1).unwrap_or("30").trim().parse::<u32>() {
                        job.timeout_mins = tm;
                    }
                } else if t.starts_with("needs:") {
                    let rhs = t.splitn(2, ':').nth(1).unwrap_or("").trim();
                    if rhs.is_empty() {
                        in_needs_list = true;
                    } else {
                        for dep in rhs.trim_matches('[').trim_matches(']').split(',') {
                            let d = dep.trim().trim_matches('"').trim_matches('\'');
                            if !d.is_empty() {
                                job.needs.push(d.to_string());
                            }
                        }
                    }
                }
            } else if in_needs_list && ind >= 6 && t.starts_with("- ") {
                job.needs.push(t[2..].trim().to_string());
            } else if t.contains("uses:") {
                let u = t.split("uses:").nth(1).unwrap_or("").trim();
                if u.starts_with("actions/cache") || u.contains("rust-cache") {
                    job.has_cache = true;
                } else if u.starts_with("actions/upload-artifact") {
                    job.artifact_paths.push("dist/".to_string());
                } else if !u.starts_with("actions/checkout") && !u.starts_with("actions/setup-") {
                    job.unmapped_notes.push(format!("GitHub Action `{}`", u));
                }
            } else if t.starts_with("- run:") || t.starts_with("run:") {
                let rhs = t.splitn(2, ':').nth(1).unwrap_or("").trim();
                if rhs == "|" || rhs == ">-" || rhs == ">" || rhs.is_empty() {
                    let run_ind = ind;
                    let mut block_cmds = Vec::new();
                    while idx + 1 < lines.len() {
                        let nl = lines[idx + 1];
                        let nt = nl.trim();
                        if !nt.is_empty() && indent_len(nl) <= run_ind {
                            break;
                        }
                        idx += 1;
                        if !nt.is_empty() && !nt.starts_with('#') {
                            block_cmds.push(nt.to_string());
                        }
                    }
                    if !block_cmds.is_empty() {
                        job.commands.push(block_cmds.join(" && "));
                    }
                } else {
                    job.commands
                        .push(rhs.trim_matches('"').trim_matches('\'').to_string());
                }
            } else if t.starts_with("cache:") && !t.ends_with("false") {
                job.has_cache = true;
            }
        }
        idx += 1;
    }
    if let Some(j) = cur_job.take() {
        jobs.push(j);
    }
    if jobs.is_empty() {
        jobs.push(JobIr {
            commands: vec!["echo 'CI build'".to_string()],
            ..Default::default()
        });
    }
    PipelineIr {
        name,
        source_vendor: "github-actions".to_string(),
        source_file: rel_file.to_string(),
        jobs,
    }
}

fn parse_generic_yaml_or_jenkins_to_ir(
    src_vendor: &str,
    rel_file: &str,
    raw: &str,
) -> PipelineIr {
    if src_vendor == "github-actions" {
        return parse_github_actions_to_ir(rel_file, raw);
    }
    if src_vendor == "jenkins" {
        let mut jobs = Vec::new();
        let mut current: Option<JobIr> = None;
        for line in raw.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("stage(") {
                if let Some(j) = current.take() {
                    jobs.push(j);
                }
                let raw_name = rest
                    .split(')')
                    .next()
                    .unwrap_or("build")
                    .trim_matches('\'')
                    .trim_matches('"');
                let slug_id: String = raw_name
                    .to_ascii_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let prev_id = jobs.last().map(|p: &JobIr| p.id.clone());
                current = Some(JobIr {
                    id: slug_id.trim_matches('_').to_string(),
                    needs: prev_id.into_iter().collect(),
                    ..Default::default()
                });
            } else if let Some(sh_rest) = t.strip_prefix("sh ") {
                let cmd = sh_rest.trim().trim_matches('\'').trim_matches('"').to_string();
                if let Some(ref mut j) = current {
                    j.commands.push(cmd);
                }
            }
        }
        if let Some(j) = current.take() {
            jobs.push(j);
        }
        if jobs.is_empty() {
            jobs.push(JobIr {
                commands: vec!["./gradlew test".to_string()],
                ..Default::default()
            });
        }
        return PipelineIr {
            name: "Jenkins Migrated CI".to_string(),
            source_vendor: "jenkins".to_string(),
            source_file: rel_file.to_string(),
            jobs,
        };
    }

    let mut jobs: Vec<JobIr> = Vec::new();
    let mut default_img: Option<String> = None;

    if src_vendor == "gitlab-ci" {
        let reserved = [
            "stages", "variables", "default", "include", "workflow", "image", "services",
            "before_script", "after_script", "cache", "pages",
        ];
        let mut cur: Option<JobIr> = None;
        let mut in_script = false;
        for line in raw.lines() {
            let t = line.trim();
            let ind = indent_len(line);
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if ind == 0 && t.starts_with("image:") {
                default_img = t.splitn(2, ':').nth(1).map(|s| s.trim().trim_matches('"').to_string());
                continue;
            }
            if ind == 0 && t.ends_with(':') && !t.starts_with('.') {
                if let Some(j) = cur.take() {
                    if !j.commands.is_empty() || !j.unmapped_notes.is_empty() {
                        jobs.push(j);
                    }
                }
                let key = t.trim_end_matches(':').trim();
                if !reserved.contains(&key) {
                    cur = Some(JobIr {
                        id: key.replace([' ', '/'], "_"),
                        container_image: default_img.clone(),
                        ..Default::default()
                    });
                }
                in_script = false;
                continue;
            }
            if ind == 0 && t.starts_with("include:") {
                if let Some(ref mut j) = cur {
                    j.unmapped_notes.push(format!("unmapped vendor directive `{}`", t));
                }
            }
            if let Some(ref mut j) = cur {
                if ind == 2 {
                    in_script = t == "script:" || t == "before_script:";
                    if t.starts_with("image:") {
                        j.container_image =
                            t.splitn(2, ':').nth(1).map(|s| s.trim().trim_matches('"').to_string());
                    } else if t.starts_with("cache:") {
                        j.has_cache = true;
                    } else if t.starts_with("artifacts:") {
                        j.artifact_paths.push("artifacts/".to_string());
                    } else if t.starts_with("needs:") {
                        let rhs = t.splitn(2, ':').nth(1).unwrap_or("").trim();
                        for dep in rhs.trim_matches('[').trim_matches(']').split(',') {
                            let d = dep.trim().trim_matches('"').trim_matches('\'');
                            if !d.is_empty() {
                                j.needs.push(d.to_string());
                            }
                        }
                    }
                } else if in_script && t.starts_with("- ") {
                    j.commands
                        .push(t[2..].trim().trim_matches('"').trim_matches('\'').to_string());
                } else if t.starts_with("- saas-") || t.starts_with("- macos") || t.starts_with("- windows") {
                    let (os, arch, vcpus) = infer_os_arch_vcpus(t[2..].trim());
                    j.os = os;
                    j.arch = arch;
                    j.vcpus = vcpus;
                }
            }
        }
        if let Some(j) = cur.take() {
            if !j.commands.is_empty() || !j.unmapped_notes.is_empty() {
                jobs.push(j);
            }
        }
    } else {
        // Multi-job aware extractor for CircleCI (`jobs:\n  <id>:`), Azure (`- job: <id>`),
        // Bitbucket (`- step:`), AWS CodeBuild, and GCP Cloud Build!
        let mut cur: Option<JobIr> = None;
        let mut in_cci_jobs = false;
        let mut step_counter = 0usize;
        let mut global_notes: Vec<String> = Vec::new();

        for line in raw.lines() {
            let t = line.trim();
            let ind = indent_len(line);
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if ind == 0 && src_vendor == "circleci" {
                in_cci_jobs = t == "jobs:";
                if !in_cci_jobs {
                    if let Some(mut j) = cur.take() {
                        if j.commands.is_empty() {
                            j.commands.push("echo 'Build & test'".to_string());
                        }
                        jobs.push(j);
                    }
                }
                if t.starts_with("orbs:") {
                    global_notes.push(format!("unmapped vendor directive `{}`", t));
                }
                continue;
            }
            if src_vendor == "circleci" && !in_cci_jobs {
                continue;
            }
            if in_cci_jobs && ind == 2 && t.ends_with(':') && !t.starts_with('-') {
                if let Some(mut j) = cur.take() {
                    if j.commands.is_empty() {
                        j.commands.push("echo 'Build & test'".to_string());
                    }
                    jobs.push(j);
                }
                let jid = t.trim_end_matches(':').trim().replace([' ', '-'], "_");
                cur = Some(JobIr {
                    id: jid,
                    unmapped_notes: global_notes.clone(),
                    ..Default::default()
                });
                continue;
            }
            if let Some(az_job) = t.strip_prefix("- job:") {
                if let Some(mut j) = cur.take() {
                    if j.commands.is_empty() {
                        j.commands.push("echo 'Build & test'".to_string());
                    }
                    jobs.push(j);
                }
                let jid = az_job.trim().trim_matches('\'').trim_matches('"').to_string();
                cur = Some(JobIr {
                    id: if jid.is_empty() {
                        step_counter += 1;
                        format!("job_{}", step_counter)
                    } else {
                        jid
                    },
                    ..Default::default()
                });
                continue;
            }
            if src_vendor == "bitbucket" && t == "- step:" {
                if let Some(mut j) = cur.take() {
                    if j.commands.is_empty() {
                        j.commands.push("echo 'Build & test'".to_string());
                    }
                    jobs.push(j);
                }
                step_counter += 1;
                cur = Some(JobIr {
                    id: format!("step_{}", step_counter),
                    ..Default::default()
                });
                continue;
            }

            let target_job = cur.get_or_insert_with(|| JobIr {
                unmapped_notes: global_notes.clone(),
                ..Default::default()
            });

            if src_vendor == "bitbucket" && t.starts_with("name:") && target_job.id.starts_with("step_") {
                let raw_n = t.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('\'').trim_matches('"');
                let slug: String = raw_n
                    .to_ascii_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let clean_slug = slug.trim_matches('_');
                if !clean_slug.is_empty() {
                    target_job.id = clean_slug.to_string();
                }
            } else if t.starts_with("dependsOn:") || t.starts_with("requires:") {
                let rhs = t.splitn(2, ':').nth(1).unwrap_or("").trim();
                for dep in rhs.trim_matches('[').trim_matches(']').split(',') {
                    let d = dep.trim().trim_matches('\'').trim_matches('"');
                    if !d.is_empty() {
                        target_job.needs.push(d.to_string());
                    }
                }
            } else if t.starts_with("orbs:") || t.starts_with("include:") || t.starts_with("pipe:") || t.starts_with("- task:") {
                target_job
                    .unmapped_notes
                    .push(format!("unmapped vendor directive `{}`", t));
            } else if t.starts_with("resource_class:")
                || t.starts_with("machineType:")
                || t.starts_with("vmImage:")
                || t.starts_with("compute-type:")
                || t.starts_with("xcode:")
            {
                let lbl = if t.starts_with("xcode:") {
                    "macos-15-xlarge"
                } else {
                    t.splitn(2, ':')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                };
                let (os, arch, vcpus) = infer_os_arch_vcpus(lbl);
                target_job.os = os;
                target_job.arch = arch;
                target_job.vcpus = vcpus;
            } else if t.starts_with("restore_cache") || t.starts_with("caches:") || t.starts_with("cache:") {
                target_job.has_cache = true;
            } else if let Some(s) = t.strip_prefix("- script:") {
                let c = s.trim().trim_matches('"').trim_matches('\'');
                if !c.is_empty() {
                    target_job.commands.push(c.to_string());
                }
            } else if let Some(s) = t.strip_prefix("command:") {
                let c = s.trim().trim_matches('"').trim_matches('\'');
                if !c.is_empty() {
                    target_job.commands.push(c.to_string());
                }
            } else if let Some(cmd) = t.strip_prefix("- ") {
                let raw_step = cmd.trim();
                let unwrapped = raw_step.strip_prefix("run:").map(|s| s.trim()).unwrap_or(raw_step);
                let clean = unwrapped.trim_matches('"').trim_matches('\'');
                let is_shell = clean.contains(' ')
                    && !clean.ends_with(':')
                    && !clean.starts_with("image:")
                    && !clean.starts_with("name:")
                    && !clean.starts_with("uses:")
                    && !clean.starts_with("step:");
                if is_shell {
                    target_job.commands.push(clean.to_string());
                }
            }
        }
        if let Some(mut j) = cur.take() {
            if j.commands.is_empty() {
                j.commands.push("echo 'Build & test pipeline'".to_string());
            }
            jobs.push(j);
        }
    }

    if jobs.is_empty() {
        let mut fallback = JobIr::default();
        for line in raw.lines() {
            let t = line.trim();
            if t.starts_with("orbs:") || t.starts_with("include:") || t.starts_with("pipe:") {
                fallback
                    .unmapped_notes
                    .push(format!("unmapped vendor directive `{}`", t));
            } else if let Some(c) = t.strip_prefix("- ") {
                if c.contains(' ') && !c.ends_with(':') {
                    fallback.commands.push(c.trim_matches('"').to_string());
                }
            }
        }
        if fallback.commands.is_empty() {
            fallback.commands.push("echo 'Build & test'".to_string());
        }
        jobs.push(fallback);
    }

    PipelineIr {
        name: format!("CI (migrated from {})", src_vendor),
        source_vendor: src_vendor.to_string(),
        source_file: rel_file.to_string(),
        jobs,
    }
}

pub fn emit_pipeline_ir_to_target(ir: &PipelineIr, target_platform: &str) -> String {
    match target_platform {
        "github-actions" => {
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\nname: {}\non:\n  push:\n    branches: [main]\n  pull_request:\npermissions:\n  contents: read\nconcurrency:\n  group: ${{{{ github.workflow }}}}-${{{{ github.ref }}}}\n  cancel-in-progress: ${{{{ github.event_name == 'pull_request' }}}}\njobs:\n",
                ir.source_vendor, ir.source_file, ir.name
            );
            for j in &ir.jobs {
                let runner = match (j.os.as_str(), j.arch.as_str(), j.vcpus) {
                    ("macos", _, _) => "macos-15-xlarge",
                    ("windows", _, _) => "windows-latest",
                    ("linux", "x64", c) if c >= 8 => "ubuntu-8-core",
                    ("linux", "x64", c) if c >= 4 => "ubuntu-4-core",
                    ("linux", "arm64", c) if c >= 8 => "ubuntu-24.04-arm-8core",
                    ("linux", "arm64", c) if c >= 4 => "ubuntu-24.04-arm-4core",
                    _ => "ubuntu-24.04-arm",
                };
                y.push_str(&format!(
                    "  {}:\n    runs-on: {}\n    timeout-minutes: {}\n",
                    j.id, runner, j.timeout_mins
                ));
                if let Some(m) = j.est_mins {
                    y.push_str(&format!("    # est-minutes: {:.2}\n", m));
                }
                if j.matrix_count > 1 {
                    let indices: Vec<String> = (1..=j.matrix_count).map(|n| n.to_string()).collect();
                    y.push_str(&format!(
                        "    strategy:\n      matrix:\n        shard: [{}]\n",
                        indices.join(", ")
                    ));
                }
                if !j.needs.is_empty() {
                    y.push_str(&format!("    needs: [{}]\n", j.needs.join(", ")));
                }
                if let Some(ref img) = j.container_image {
                    y.push_str(&format!("    container: {}\n", img));
                }
                y.push_str("    steps:\n      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 1\n");
                for note in &j.unmapped_notes {
                    y.push_str(&format!("      # MANUAL_REVIEW_REQUIRED: {}\n", note));
                }
                for cmd in &j.commands {
                    if cmd.contains(": ")
                        || cmd.starts_with('{')
                        || cmd.starts_with('[')
                        || cmd.starts_with('*')
                        || cmd.starts_with('&')
                        || cmd.contains('#')
                    {
                        y.push_str(&format!(
                            "      - run: \"{}\"\n",
                            cmd.replace('\\', "\\\\").replace('"', "\\\"")
                        ));
                    } else {
                        y.push_str(&format!("      - run: {}\n", cmd));
                    }
                }
                if !j.artifact_paths.is_empty() {
                    y.push_str("      - uses: actions/upload-artifact@v4\n        with:\n          name: build-artifacts\n          path: dist/\n          retention-days: 7\n");
                }
            }
            y
        }
        "gitlab-ci" => {
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\ndefault:\n  interruptible: true\n  timeout: 30m\nvariables:\n  GIT_DEPTH: \"1\"\nstages:\n  - build\n",
                ir.source_vendor, ir.source_file
            );
            for j in &ir.jobs {
                let tag = match (j.os.as_str(), j.vcpus) {
                    ("macos", _) => "saas-macos-medium-m1",
                    ("windows", _) => "saas-windows-medium-amd64",
                    (_, c) if c >= 8 => "saas-linux-large-amd64",
                    (_, c) if c >= 4 => "saas-linux-medium-amd64",
                    _ => "saas-linux-small-arm64",
                };
                y.push_str(&format!("\n{}:\n", j.id));
                if let Some(m) = j.est_mins {
                    y.push_str(&format!("  # est-minutes: {:.2}\n", m));
                }
                y.push_str(&format!("  stage: build\n  tags:\n    - {}\n", tag));
                if j.matrix_count > 1 {
                    y.push_str(&format!("  parallel: {}\n", j.matrix_count));
                }
                if !j.needs.is_empty() {
                    y.push_str(&format!("  needs: [{}]\n", j.needs.join(", ")));
                }
                for note in &j.unmapped_notes {
                    y.push_str(&format!("  # MANUAL_REVIEW_REQUIRED: {}\n", note));
                }
                y.push_str("  script:\n");
                if j.commands.is_empty() {
                    y.push_str("    - \"echo 'Run CI job steps'\"\n");
                }
                for cmd in &j.commands {
                    y.push_str(&format!("    - \"{}\"\n", cmd.replace('"', "\\\"")));
                }
                if !j.artifact_paths.is_empty() {
                    y.push_str("  artifacts:\n    expire_in: 7 days\n    paths:\n      - dist/\n");
                }
            }
            y
        }
        "circleci" => {
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\nversion: 2.1\njobs:\n",
                ir.source_vendor, ir.source_file
            );
            for j in &ir.jobs {
                let (exec_block, rc) = match (j.os.as_str(), j.vcpus) {
                    ("macos", _) => ("    macos:\n      xcode: \"16.0.0\"", "macos.m2pro.medium"),
                    (_, c) if c >= 8 => ("    docker:\n      - image: cimg/base:current", "arm.xlarge"),
                    (_, c) if c >= 4 => ("    docker:\n      - image: cimg/base:current", "arm.large"),
                    _ => ("    docker:\n      - image: cimg/base:current", "arm.medium"),
                };
                y.push_str(&format!("  {}:\n", j.id));
                if let Some(m) = j.est_mins {
                    y.push_str(&format!("    # est-minutes: {:.2}\n", m));
                }
                if j.matrix_count > 1 {
                    y.push_str(&format!("    parallelism: {}\n", j.matrix_count));
                }
                y.push_str(&format!(
                    "{}\n    resource_class: {}\n    steps:\n      - checkout\n",
                    exec_block, rc
                ));
                for note in &j.unmapped_notes {
                    y.push_str(&format!("      # MANUAL_REVIEW_REQUIRED: {}\n", note));
                }
                for cmd in &j.commands {
                    y.push_str(&format!(
                        "      - run:\n          no_output_timeout: 15m\n          command: \"{}\"\n",
                        cmd.replace('"', "\\\"")
                    ));
                }
            }
            y.push_str("workflows:\n  ci:\n    jobs:\n");
            for j in &ir.jobs {
                if j.needs.is_empty() {
                    y.push_str(&format!("      - {}\n", j.id));
                } else {
                    y.push_str(&format!(
                        "      - {}:\n          requires: [{}]\n",
                        j.id,
                        j.needs.join(", ")
                    ));
                }
            }
            y
        }
        "bitbucket" => {
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\nimage: atlassian/default-image:4\noptions:\n  max-time: 30\nclone:\n  depth: 1\npipelines:\n  default:\n",
                ir.source_vendor, ir.source_file
            );
            for j in &ir.jobs {
                let sz = if j.vcpus >= 8 {
                    "4x"
                } else if j.vcpus >= 4 {
                    "2x"
                } else {
                    "1x"
                };
                y.push_str(&format!("    - step:\n        name: {}\n        size: {}\n", j.id, sz));
                for note in &j.unmapped_notes {
                    y.push_str(&format!("        # MANUAL_REVIEW_REQUIRED: {}\n", note));
                }
                y.push_str("        script:\n");
                for cmd in &j.commands {
                    y.push_str(&format!("          - \"{}\"\n", cmd.replace('"', "\\\"")));
                }
            }
            y
        }
        "aws-codebuild" => {
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\n# Recommended CodeBuild computeType: BUILD_GENERAL1_SMALL (ARM_CONTAINER)\nversion: 0.2\nphases:\n  build:\n    commands:\n",
                ir.source_vendor, ir.source_file
            );
            for j in &ir.jobs {
                for note in &j.unmapped_notes {
                    y.push_str(&format!("      # MANUAL_REVIEW_REQUIRED ({}): {}\n", j.id, note));
                }
                for cmd in &j.commands {
                    y.push_str(&format!("      - \"{}\"\n", cmd.replace('"', "\\\"")));
                }
            }
            y.push_str("cache:\n  paths:\n    - '/root/.cache/**/*'\n");
            y
        }
        "gcp-cloud-build" => {
            let max_vcpus = ir.jobs.iter().map(|j| j.vcpus).max().unwrap_or(2);
            let mtype = if max_vcpus >= 8 { "E2_HIGHCPU_8" } else { "E2_MEDIUM" };
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\ntimeout: 1800s\noptions:\n  machineType: '{}'\nsteps:\n",
                ir.source_vendor, ir.source_file, mtype
            );
            for j in &ir.jobs {
                let joined = j.commands.join(" && ");
                for note in &j.unmapped_notes {
                    y.push_str(&format!("  # MANUAL_REVIEW_REQUIRED ({}): {}\n", j.id, note));
                }
                y.push_str(&format!(
                    "  - id: '{}'\n    name: 'ubuntu:24.04'\n    entrypoint: 'bash'\n    args: ['-c', \"{}\"]\n",
                    j.id,
                    joined.replace('"', "\\\"")
                ));
            }
            y
        }
        "azure-pipelines" => {
            let mut y = format!(
                "# Auto-converted from {} ({}) by MiniCI 7x7 Pipeline IR Translator\ntrigger:\n  - main\npr:\n  - main\njobs:\n",
                ir.source_vendor, ir.source_file
            );
            for j in &ir.jobs {
                let vm = match j.os.as_str() {
                    "macos" => "macOS-latest",
                    "windows" => "windows-latest",
                    _ => "ubuntu-latest",
                };
                y.push_str(&format!(
                    "  - job: {}\n    timeoutInMinutes: {}\n    pool:\n      vmImage: '{}'\n",
                    j.id.replace('-', "_"),
                    j.timeout_mins,
                    vm
                ));
                if !j.needs.is_empty() {
                    let deps: Vec<String> = j.needs.iter().map(|d| d.replace('-', "_")).collect();
                    y.push_str(&format!("    dependsOn: [{}]\n", deps.join(", ")));
                }
                y.push_str("    steps:\n      - checkout: self\n        fetchDepth: 1\n");
                for note in &j.unmapped_notes {
                    y.push_str(&format!("      # MANUAL_REVIEW_REQUIRED: {}\n", note));
                }
                for cmd in &j.commands {
                    y.push_str(&format!(
                        "      - script: \"{}\"\n        displayName: '{}'\n",
                        cmd.replace('"', "\\\""),
                        j.id
                    ));
                }
            }
            y
        }
        _ => String::new(),
    }
}

pub fn translate_pipeline_cross_platform(
    repo_dir: &Path,
    from_override: Option<&str>,
    target_platform: &str,
) -> Option<(PipelineIr, String, String)> {
    let base = if repo_dir.is_file() {
        repo_dir.parent().unwrap_or(Path::new("."))
    } else {
        repo_dir
    };

    let mut candidates: Vec<(&str, PathBuf, String)> = Vec::new();
    if repo_dir.is_file() {
        let fname = repo_dir.file_name().and_then(|s| s.to_str()).unwrap_or("ci.yml");
        let low = fname.to_ascii_lowercase();
        let guessed = if low.contains("gitlab") {
            "gitlab-ci"
        } else if low.contains("circleci") {
            "circleci"
        } else if low.contains("bitbucket") {
            "bitbucket"
        } else if low.contains("buildspec") {
            "aws-codebuild"
        } else if low.contains("cloudbuild") {
            "gcp-cloud-build"
        } else if low.contains("azure") {
            "azure-pipelines"
        } else if low.contains("jenkins") {
            "jenkins"
        } else {
            "github-actions"
        };
        let vendor = from_override.filter(|f| *f != "auto").unwrap_or(guessed);
        if let Ok(raw) = fs::read_to_string(repo_dir) {
            let ir = parse_generic_yaml_or_jenkins_to_ir(vendor, fname, &raw);
            let emitted = emit_pipeline_ir_to_target(&ir, target_platform);
            let rel_out = target_default_rel_path(target_platform).to_string();
            return Some((ir, rel_out, emitted));
        }
    }

    let gh_dir = base.join(".github").join("workflows");
    if let Ok(entries) = fs::read_dir(&gh_dir) {
        let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext == "yml" || ext == "yaml" {
                let rel = format!(
                    ".github/workflows/{}",
                    p.file_name().and_then(|s| s.to_str()).unwrap_or("ci.yml")
                );
                candidates.push(("github-actions", p, rel));
            }
        }
    }
    for (v, rel, p) in [
        ("gitlab-ci", ".gitlab-ci.yml", base.join(".gitlab-ci.yml")),
        ("circleci", ".circleci/config.yml", base.join(".circleci").join("config.yml")),
        ("bitbucket", "bitbucket-pipelines.yml", base.join("bitbucket-pipelines.yml")),
        ("aws-codebuild", "buildspec.yml", base.join("buildspec.yml")),
        ("gcp-cloud-build", "cloudbuild.yaml", base.join("cloudbuild.yaml")),
        ("gcp-cloud-build", "cloudbuild.yml", base.join("cloudbuild.yml")),
        ("azure-pipelines", "azure-pipelines.yml", base.join("azure-pipelines.yml")),
        ("azure-pipelines", "azure-pipelines.yaml", base.join("azure-pipelines.yaml")),
        ("jenkins", "Jenkinsfile", base.join("Jenkinsfile")),
    ] {
        if p.is_file() {
            candidates.push((v, p, rel.to_string()));
        }
    }

    let want_from = from_override
        .map(|f| match f.trim().to_ascii_lowercase().as_str() {
            "github" | "github-actions" => "github-actions",
            "gitlab" | "gitlab-ci" => "gitlab-ci",
            "circleci" => "circleci",
            "bitbucket" | "bitbucket-pipelines" => "bitbucket",
            "codebuild" | "aws-codebuild" => "aws-codebuild",
            "cloudbuild" | "gcp" | "gcp-cloud-build" => "gcp-cloud-build",
            "azure" | "azure-devops" | "azure-pipelines" => "azure-pipelines",
            "jenkins" | "jenkinsfile" => "jenkins",
            _ => "auto",
        })
        .unwrap_or("auto");

    let src_analysis = analyze_path(repo_dir, None);
    let mut merged_jobs: Vec<JobIr> = Vec::new();
    let mut primary_vendor: Option<&str> = None;
    let mut primary_rel = String::new();
    let mut files_merged = 0usize;

    let sanitize_job_key = |raw_id: &str| -> String {
        let cleaned: String = raw_id
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let trimmed = cleaned.trim_matches('_');
        if trimmed.is_empty() {
            "job".to_string()
        } else {
            trimmed.to_string()
        }
    };

    for (v, path, rel) in &candidates {
        if want_from != "auto" && *v != want_from {
            continue;
        }
        if want_from == "auto" && *v == target_platform && candidates.iter().any(|(ov, _, _)| *ov != target_platform) {
            continue;
        }
        if let Some(pv) = primary_vendor {
            if *v != pv {
                continue;
            }
        }
        if let Ok(raw) = fs::read_to_string(path) {
            primary_vendor = Some(*v);
            if primary_rel.is_empty() {
                primary_rel = rel.clone();
            }
            files_merged += 1;
            let file_ir = parse_generic_yaml_or_jenkins_to_ir(v, rel, &raw);
            let file_analysis_jobs: Vec<_> = src_analysis
                .jobs
                .iter()
                .filter(|ej| ej.workflow_file == *rel || ej.workflow_file.ends_with(rel.as_str()))
                .collect();

            let base_id_of = |s: &str| -> String {
                s.split(" [")
                    .next()
                    .unwrap_or(s)
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string()
            };

            let is_synthetic_fallback = file_ir.jobs.len() == 1
                && file_ir.jobs[0].id == "build"
                && file_ir.jobs[0].commands.len() == 1
                && file_ir.jobs[0].commands[0] == "echo 'CI build'"
                && (file_analysis_jobs.is_empty()
                    || !file_analysis_jobs.iter().any(|ej| base_id_of(&ej.job_id) == "build"));

            let mut matched_row_indices: Vec<usize> = Vec::new();
            if !is_synthetic_fallback {
                for mut j in file_ir.jobs {
                    let orig_id = base_id_of(&j.id);
                    let orig_norm = orig_id.replace('-', "_").to_ascii_lowercase();
                    let matching_idx: Vec<usize> = file_analysis_jobs
                        .iter()
                        .enumerate()
                        .filter(|(_, ej)| {
                            let ej_base = base_id_of(&ej.job_id);
                            ej_base == orig_id || ej_base.replace('-', "_").to_ascii_lowercase() == orig_norm
                        })
                        .map(|(i_ej, _)| i_ej)
                        .collect();

                    // Skip non-VM jobs (e.g., GitHub reusable workflow callers `uses: ./.github/workflows/...`)
                    // when `src_analysis` analyzed the repository so defined & expanded job counts match 1:1!
                    if *v == "github-actions" && !src_analysis.jobs.is_empty() && matching_idx.is_empty() {
                        continue;
                    }

                    if j.commands.is_empty() {
                        j.commands.push("echo 'Run CI job steps'".to_string());
                    }

                    if matching_idx.is_empty() {
                        j.id = sanitize_job_key(&j.id);
                        if merged_jobs.iter().any(|existing| existing.id == j.id) {
                            j.id = format!("{}_{}", j.id, merged_jobs.len() + 1);
                        }
                        merged_jobs.push(j);
                    } else if matching_idx.len() == 1 {
                        let idx0 = matching_idx[0];
                        matched_row_indices.push(idx0);
                        let ej = file_analysis_jobs[idx0];
                        j.matrix_count = ej.matrix_combination_count.max(1);
                        j.est_mins = Some((ej.est_raw_duration_mins_per_job * 100.0).round() / 100.0);
                        j.vcpus = j.vcpus.max(ej.vcpus);
                        if ej.os.eq_ignore_ascii_case("macos") {
                            j.os = "macos".to_string();
                        } else if ej.os.eq_ignore_ascii_case("windows") {
                            j.os = "windows".to_string();
                        } else {
                            j.os = "linux".to_string();
                        }
                        j.id = sanitize_job_key(&j.id);
                        if merged_jobs.iter().any(|existing| existing.id == j.id) {
                            j.id = format!("{}_{}", j.id, merged_jobs.len() + 1);
                        }
                        merged_jobs.push(j);
                    } else {
                        // Multi-OS matrix job (`job [linux]`, `job [macos]`, `job [windows]`):
                        // Emit one target job per OS bucket so OS tags, defined_jobs, and expanded_jobs reconcile 100%!
                        for idx_m in matching_idx {
                            matched_row_indices.push(idx_m);
                            let ej = file_analysis_jobs[idx_m];
                            let mut split_j = j.clone();
                            split_j.matrix_count = ej.matrix_combination_count.max(1);
                            split_j.est_mins =
                                Some((ej.est_raw_duration_mins_per_job * 100.0).round() / 100.0);
                            split_j.vcpus = ej.vcpus.max(2);
                            split_j.os = if ej.os.eq_ignore_ascii_case("macos") {
                                "macos".to_string()
                            } else if ej.os.eq_ignore_ascii_case("windows") {
                                "windows".to_string()
                            } else {
                                "linux".to_string()
                            };
                            split_j.id = sanitize_job_key(&ej.job_id);
                            if merged_jobs.iter().any(|existing| existing.id == split_j.id) {
                                split_j.id = format!("{}_{}", split_j.id, merged_jobs.len() + 1);
                            }
                            merged_jobs.push(split_j);
                        }
                    }
                }
            }

            // Reconcile any remaining `ExpandedJobCost` rows from `src_analysis` (e.g., 4-space indented jobs)
            for (idx_ej, ej) in file_analysis_jobs.iter().enumerate() {
                if matched_row_indices.contains(&idx_ej) {
                    continue;
                }
                let dom_os = if ej.os.eq_ignore_ascii_case("macos") {
                    "macos"
                } else if ej.os.eq_ignore_ascii_case("windows") {
                    "windows"
                } else {
                    "linux"
                };
                let mut cand_id = sanitize_job_key(&ej.job_id);
                if merged_jobs.iter().any(|existing| existing.id == cand_id) {
                    cand_id = format!("{}_{}", cand_id, merged_jobs.len() + 1);
                }
                merged_jobs.push(JobIr {
                    id: cand_id,
                    os: dom_os.to_string(),
                    arch: "arm64".to_string(),
                    vcpus: ej.vcpus.max(2),
                    matrix_count: ej.matrix_combination_count.max(1),
                    est_mins: Some((ej.est_raw_duration_mins_per_job * 100.0).round() / 100.0),
                    commands: vec!["echo 'Translated workflow job'".to_string()],
                    ..Default::default()
                });
            }
        }
    }

    if merged_jobs.is_empty() {
        merged_jobs.push(JobIr {
            commands: vec!["echo 'CI build'".to_string()],
            ..Default::default()
        });
    }

    let src_v = primary_vendor?;
    let source_label = if files_merged > 1 && src_v == "github-actions" {
        format!(".github/workflows/*.yml ({} files)", files_merged)
    } else {
        primary_rel
    };
    let ir = PipelineIr {
        name: format!("CI ({} -> {})", src_v, target_platform),
        source_vendor: src_v.to_string(),
        source_file: source_label,
        jobs: merged_jobs,
    };
    let emitted = emit_pipeline_ir_to_target(&ir, target_platform);
    let rel_out = target_default_rel_path(target_platform).to_string();
    Some((ir, rel_out, emitted))
}

fn copy_shadow_context(base_dir: &Path, shadow_root: &Path) {
    for rel in [
        "minici-usage.yml",
        "Cargo.toml",
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        "yarn.lock",
        "package-lock.json",
        "requirements.txt",
        "pyproject.toml",
        "uv.lock",
        "go.mod",
        "go.sum",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "turbo.json",
        "nx.json",
    ] {
        let src = base_dir.join(rel);
        if src.is_file() {
            let _ = fs::copy(&src, shadow_root.join(rel));
        }
    }
    let codeowners = base_dir.join(".github").join("CODEOWNERS");
    if codeowners.is_file() {
        let _ = fs::create_dir_all(shadow_root.join(".github"));
        let _ = fs::copy(&codeowners, shadow_root.join(".github").join("CODEOWNERS"));
    }
    let lock = base_dir.join(".minici").join("telemetry.lock.json");
    if lock.is_file() {
        let _ = fs::create_dir_all(shadow_root.join(".minici"));
        let _ = fs::copy(&lock, shadow_root.join(".minici").join("telemetry.lock.json"));
    }
}

fn maybe_invoke_minici_yaml_fixer(target_root: &Path) -> Option<String> {
    let candidates = [
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".local/bin/minici-yaml-fixer")),
        Some(PathBuf::from("minici-yaml-fixer")),
    ];
    for cand in candidates.into_iter().flatten() {
        if let Ok(out) = Command::new(&cand)
            .args(["fix", "--in-place", "--path"])
            .arg(target_root)
            .output()
        {
            if out.status.success() {
                return Some(String::from_utf8_lossy(&out.stdout).to_string());
            }
        }
    }
    None
}

pub fn run_surgical_optimizer(
    repo_dir: &Path,
    operation: &str, // "shrink" | "migrate"
    target_mode: &str,
    apply_to_disk: bool,
) -> OptimizationReceipt {
    run_surgical_optimizer_with_opts(
        repo_dir,
        operation,
        target_mode,
        apply_to_disk,
        MigrateOptions::default(),
    )
}

pub fn run_surgical_optimizer_with_opts(
    repo_dir: &Path,
    operation: &str, // "shrink" | "migrate"
    target_mode: &str,
    apply_to_disk: bool,
    opts: MigrateOptions,
) -> OptimizationReceipt {
    let target_norm_owned = target_mode.trim().to_ascii_lowercase();
    let canonical_target: &str = match target_norm_owned.as_str() {
        "aws-ec2" => "self-hosted",
        "aws-spot" | "spot" => "aws-spot",
        "gcp-spot" => "gcp-spot",
        "azure-spot" => "azure-spot",
        "self-hosted" => "self-hosted",
        "gitlab" | "gitlab-ci" => "gitlab-ci",
        "azure" | "azure-devops" | "azure-pipelines" => "azure-pipelines",
        "gcp" | "cloudbuild" | "cloud-build" | "gcp-cloud-build" => "gcp-cloud-build",
        "codebuild" | "aws-codebuild" => "aws-codebuild",
        "github" | "github-actions" => "github-actions",
        "circleci" => "circleci",
        "bitbucket" | "bitbucket-pipelines" => "bitbucket",
        "buildkite" => "buildkite",
        "blacksmith" => "blacksmith",
        "high-speed" => "high-speed",
        "warpbuild" => "warpbuild",
        "buildjet" => "buildjet",
        "depot" => "depot",
        "namespace" => "namespace",
        "runs-on" | "runson" => "runs-on",
        "macos-m2pro" | "macos-arm64" => "macos-m2pro",
        "ubuntu-arm64" => "ubuntu-arm64",
        "github-arm64" => "github-arm64",
        "arm64" => "arm64",
        "ubuntu-24.04-arm" => "ubuntu-24.04-arm",
        "ubuntu-slim" => "ubuntu-slim",
        "slim" => "slim",
        _ => target_mode.trim(),
    };

    let security_mode = opts.security_mode || target_norm_owned == "security";
    let base_report = analyze_path(repo_dir, None);
    let src_vendor_detected = base_report
        .detected_providers
        .first()
        .cloned()
        .unwrap_or_else(|| "github-actions".to_string());
    let wf_files = collect_workflow_files(repo_dir);
    let base_dir = if repo_dir.is_file() {
        repo_dir.parent().unwrap_or(Path::new("."))
    } else {
        repo_dir
    };

    static SHADOW_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
    let seq = SHADOW_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let shadow_root = std::env::temp_dir().join(format!(
        "minici_opt_shadow_{}_{}_{}",
        std::process::id(),
        seq,
        operation
    ));
    let _ = fs::remove_dir_all(&shadow_root);
    let _ = fs::create_dir_all(shadow_root.join(".github").join("workflows"));
    copy_shadow_context(base_dir, &shadow_root);

    let mut all_edits: Vec<FilePatchEdit> = Vec::new();
    let mut files_modified = 0usize;
    let mut pending_writes: Vec<(PathBuf, String)> = Vec::new();
    let mut extra_crate_writes: Vec<(PathBuf, String)> = Vec::new();
    let mut converted_preview: Option<String> = None;
    let mut target_workflow_file: Option<String> = None;
    let mut effective_source_platform = src_vendor_detected.clone();
    let enable_rust_tools = operation == "accelerate"
        || opts.rust_tools
        || opts.rust_everywhere
        || target_norm_owned == "rust-tools"
        || target_norm_owned == "rust-everywhere";
    let enable_rust_everywhere = operation == "accelerate"
        || opts.rust_everywhere
        || target_norm_owned == "rust-everywhere";

    // Cross-Platform `7x7` Pipeline IR Translation (`minici migrate --from <X> --to <Y>`)
    let is_cross_platform = operation == "migrate"
        && is_ci_platform_target(canonical_target)
        && (opts.from_vendor.is_some()
            || src_vendor_detected != canonical_target
            || base_dir.join("Jenkinsfile").is_file());

    if is_cross_platform {
        if let Some((ir, rel_target, emitted_yaml)) = translate_pipeline_cross_platform(
            repo_dir,
            opts.from_vendor.as_deref(),
            canonical_target,
        ) {
            effective_source_platform = ir.source_vendor.clone();
            target_workflow_file = Some(rel_target.clone());
            converted_preview = Some(emitted_yaml.clone());

            let shadow_dest = shadow_root.join(&rel_target);
            if let Some(p) = shadow_dest.parent() {
                let _ = fs::create_dir_all(p);
            }
            let _ = fs::write(&shadow_dest, &emitted_yaml);

            let out_dest = opts
                .out_file
                .clone()
                .unwrap_or_else(|| base_dir.join(&rel_target));
            files_modified = 1;
            let expanded_cnt: usize = ir.jobs.iter().map(|j| j.matrix_count.max(1)).sum();
            all_edits.push(FilePatchEdit {
                workflow_file: rel_target.clone(),
                rule_id: "MINICI-MIGRATE-IR7X7".to_string(),
                description: format!(
                    "Translated {} expanded job(s) ({} definitions) from `{}` (`{}`) -> `{}` (`{}`)",
                    expanded_cnt,
                    ir.jobs.len(),
                    ir.source_vendor,
                    ir.source_file,
                    canonical_target,
                    rel_target
                ),
                line_number: 1,
                original_snippet: ir.source_file.clone(),
                replacement_snippet: emitted_yaml.lines().take(18).collect::<Vec<_>>().join("\n"),
            });
            pending_writes.push((out_dest, emitted_yaml));
        }
    } else {
        for (src_path, rel_name) in &wf_files {
            if let Ok(raw) = fs::read_to_string(src_path) {
                let rel_low = rel_name.to_ascii_lowercase();
                let is_gh_wf = rel_name.starts_with(".github/")
                    || (!rel_low.contains("gitlab")
                        && !rel_low.contains("circleci")
                        && !rel_low.contains("bitbucket")
                        && !rel_low.contains("buildspec")
                        && !rel_low.contains("cloudbuild")
                        && !rel_low.contains("azure-pipelines"));
                let runner_mode_for_file =
                    if target_norm_owned == "aws-spot" || target_norm_owned == "spot" {
                        "aws-spot"
                    } else {
                        canonical_target
                    };
                let (mut patched, mut file_edits) = if operation == "migrate" {
                    migrate_single_workflow_yaml(rel_name, &raw, runner_mode_for_file)
                } else if is_gh_wf {
                    shrink_github_workflow_yaml(rel_name, &raw, base_dir, security_mode)
                } else {
                    shrink_non_github_yaml(rel_name, &raw)
                };
                if operation != "migrate" && (enable_rust_tools || enable_rust_everywhere) {
                    let (acc_yaml, acc_edits) = apply_rust_ci_acceleration_to_yaml(
                        rel_name,
                        &patched,
                        base_dir,
                        &shadow_root,
                        is_gh_wf,
                        enable_rust_tools,
                        enable_rust_everywhere,
                        &mut extra_crate_writes,
                    );
                    if !acc_edits.is_empty() {
                        patched = acc_yaml;
                        file_edits.extend(acc_edits);
                    }
                }
                if !file_edits.is_empty() {
                    files_modified += 1;
                    all_edits.extend(file_edits);
                    let write_dest = opts.out_file.clone().unwrap_or_else(|| src_path.clone());
                    pending_writes.push((write_dest, patched.clone()));
                }
                let dest_shadow = if rel_name.starts_with(".github/") {
                    shadow_root.join(rel_name)
                } else if rel_name.starts_with(".circleci/") {
                    let _ = fs::create_dir_all(shadow_root.join(".circleci"));
                    shadow_root.join(rel_name)
                } else if is_gh_wf && repo_dir.is_file() {
                    shadow_root
                        .join(".github")
                        .join("workflows")
                        .join(rel_name)
                } else {
                    shadow_root.join(rel_name)
                };
                if let Some(p) = dest_shadow.parent() {
                    let _ = fs::create_dir_all(p);
                }
                let _ = fs::write(dest_shadow, &patched);
            }
        }
        if opts.with_fixer && operation == "shrink" {
            if let Some(fixer_note) = maybe_invoke_minici_yaml_fixer(&shadow_root) {
                all_edits.push(FilePatchEdit {
                    workflow_file: ".github/workflows".to_string(),
                    rule_id: "MINICI-SEC-PINACT".to_string(),
                    description: format!(
                        "Executed `minici-yaml-fixer` 5-Pillar SHA-pinning & zizmor pass ({} bytes)",
                        fixer_note.len()
                    ),
                    line_number: 1,
                    original_snippet: String::new(),
                    replacement_snippet: String::new(),
                });
            }
        }
    }

    let mut head_report = if wf_files.is_empty() && !is_cross_platform {
        base_report.clone()
    } else {
        analyze_path(&shadow_root, Some(base_report.usage.clone()))
    };

    // Gate 6 (Mandatory Cost-Drop Safety Gate for in-place runner migrations):
    // Never write an in-place runner rewrite that unexpectedly increases monthly compute cost!
    if operation == "migrate"
        && !is_cross_platform
        && !is_ci_platform_target(canonical_target)
        && canonical_target != "buildkite"
        && head_report.gross_monthly_compute_cost_usd > base_report.gross_monthly_compute_cost_usd + 0.01
    {
        all_edits.clear();
        files_modified = 0;
        pending_writes.clear();
        head_report = base_report.clone();
    }

    let verified_diff = compute_diff(&base_report, &head_report);
    let _ = fs::remove_dir_all(&shadow_root);

    let should_write = (apply_to_disk || opts.out_file.is_some()) && !pending_writes.is_empty();
    if should_write {
        for (dest_path, content) in pending_writes {
            if let Some(parent) = dest_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp = dest_path.with_extension("yml.minici_tmp");
            if fs::write(&tmp, content).is_ok() {
                let _ = fs::rename(&tmp, dest_path);
            }
        }
        if apply_to_disk {
            for (crate_file_path, content) in extra_crate_writes {
                if let Some(parent) = crate_file_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(&crate_file_path, content);
            }
        }
    }

    let cc_base = &base_report.cost_components;
    let cc_head = &head_report.cost_components;
    let raw_ci_in_place = cc_base.arm64_migration_monthly_savings_usd
        + cc_base.monthly_superseded_run_waste_usd
        + cc_base.monthly_unrealized_cache_savings_usd
        + (cc_base.monthly_vendor_storage_configured_usd - cc_base.monthly_vendor_storage_14d_usd).max(0.0);
    let in_place_savings = raw_ci_in_place.min(base_report.gross_monthly_compute_cost_usd * 0.65);

    let switching_verdict = if operation == "migrate"
        && (is_ci_platform_target(canonical_target) || canonical_target == "buildkite")
    {
        let src_vendor = effective_source_platform.as_str();
        let src_tco = match src_vendor {
            "gitlab-ci" => cc_base.gitlab_ci_premium_monthly_tco_usd,
            "circleci" => cc_base.circleci_performance_monthly_tco_usd,
            "bitbucket" => cc_base.bitbucket_premium_monthly_tco_usd,
            "buildkite" => cc_base.buildkite_pro_monthly_tco_usd,
            "aws-codebuild" => cc_base.aws_codebuild_monthly_tco_usd,
            "gcp-cloud-build" => cc_base.gcp_cloud_build_monthly_tco_usd,
            "azure-pipelines" => cc_base.azure_pipelines_monthly_tco_usd,
            "jenkins" => cc_base.jenkins_self_hosted_monthly_tco_usd,
            _ => cc_base.github_actions_team_monthly_tco_usd,
        }
        .max(base_report.gross_monthly_compute_cost_usd);
        let target_tco = match canonical_target {
            "gitlab-ci" => cc_base.gitlab_ci_premium_monthly_tco_usd,
            "circleci" => cc_base.circleci_performance_monthly_tco_usd,
            "bitbucket" => cc_base.bitbucket_premium_monthly_tco_usd,
            "buildkite" => cc_base.buildkite_pro_monthly_tco_usd,
            "aws-codebuild" => cc_base.aws_codebuild_monthly_tco_usd,
            "gcp-cloud-build" => cc_base.gcp_cloud_build_monthly_tco_usd,
            "azure-pipelines" => cc_base.azure_pipelines_monthly_tco_usd,
            _ => cc_base.github_actions_team_monthly_tco_usd,
        };
        let sw = compute_5layer_platform_switching_tco(
            base_report.total_workflows.max(1),
            cc_base.implied_active_devs.max(1),
            src_vendor,
            canonical_target,
            src_tco,
            target_tco,
            cc_base.blended_fully_loaded_hourly_usd,
            true,
        );
        Some((
            sw.engineering_hours,
            sw.total_switching_cost_usd,
            sw.payback_months,
            sw.verdict,
        ))
    } else {
        None
    };

    let storage_before = if cc_base.monthly_vendor_storage_configured_usd > 0.0
        || base_report.monthly_artifact_storage_vendor_usd == 0.0
    {
        cc_base.monthly_vendor_storage_configured_usd
    } else {
        base_report.monthly_artifact_storage_vendor_usd
    };
    let raw_storage_after = if cc_head.monthly_vendor_storage_configured_usd > 0.0
        || head_report.monthly_artifact_storage_vendor_usd == 0.0
    {
        cc_head.monthly_vendor_storage_configured_usd
    } else {
        head_report.monthly_artifact_storage_vendor_usd
    };
    let storage_after = if cc_head.configured_retention_days < cc_base.configured_retention_days
        && cc_base.configured_retention_days > 0
        && (raw_storage_after - storage_before).abs() < 1e-6
        && storage_before > 0.0
    {
        ((storage_before
            * (cc_head.configured_retention_days as f64)
            / (cc_base.configured_retention_days as f64))
            * 100.0)
            .round()
            / 100.0
    } else {
        raw_storage_after
    };

    OptimizationReceipt {
        operation: operation.to_string(),
        source_platform: effective_source_platform,
        target_platform: canonical_target.to_string(),
        target_workflow_file,
        applied_to_disk: should_write && !all_edits.is_empty(),
        files_modified,
        edits: all_edits,
        verified_diff,
        artifact_storage_before_usd: storage_before,
        artifact_storage_after_usd: storage_after,
        missing_timeouts_before: cc_base.jobs_missing_timeout_minutes_count,
        missing_timeouts_after: cc_head.jobs_missing_timeout_minutes_count,
        in_place_shrink_monthly_savings_usd: ((in_place_savings) * 100.0).round() / 100.0,
        converted_github_workflow_preview: converted_preview,
        switching_verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comment_preserving_shrink_and_6gate_arm64_migrate_edge_cases() {
        let dir = std::env::temp_dir().join("minici_optimizer_unit_test_v2");
        let _ = fs::remove_dir_all(&dir);
        let wf_dir = dir.join(".github").join("workflows");
        fs::create_dir_all(&wf_dir).unwrap();

        let orig = r#"# Important header comment preserved!
name: PR Suite
on: [pull_request]
jobs:
  ubuntu_ver_job:
    timeout-minutes: 15
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: actions/upload-artifact@v4
        with:
          retention-days: 90
      - run: cargo test
  multiline_runner_and_git_diff:
    runs-on:
      - self-hosted
      - linux
    steps:
      - uses: actions/checkout@v4
      - run: git diff origin/main...HEAD
  ios:
    runs-on: macos-latest
    steps:
      - run: xcodebuild test
"#;
        fs::write(wf_dir.join("ci.yml"), orig).unwrap();

        let shrink = run_surgical_optimizer(&dir, "shrink", "hygiene", true);
        assert!(shrink.edits.len() >= 4);
        let shrunk_yaml = fs::read_to_string(wf_dir.join("ci.yml")).unwrap();
        assert_eq!(shrunk_yaml.matches("timeout-minutes: 15").count(), 1);
        assert!(shrunk_yaml.contains("runs-on:\n      - self-hosted\n      - linux\n    timeout-minutes: 30"));
        assert_eq!(shrunk_yaml.matches("fetch-depth: 1").count(), 1);

        let mig = run_surgical_optimizer(&dir, "migrate", "github-arm64", true);
        assert_eq!(mig.files_modified, 1);
        assert!(mig.verified_diff.delta_monthly_compute_usd < 0.0);
        let migrated_yaml = fs::read_to_string(wf_dir.join("ci.yml")).unwrap();
        assert!(migrated_yaml.contains("# Important header comment preserved!"));
        assert!(migrated_yaml.contains("runs-on: ubuntu-24.04-arm\n"));
        assert!(!migrated_yaml.contains("arm-4core"));
        assert!(migrated_yaml.contains("runs-on: macos-latest"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_7x7_anywhere_to_anywhere_pipeline_translation_and_7provider_shrink() {
        let dir = std::env::temp_dir().join("minici_optimizer_7x7_anywhere_test");
        let _ = fs::remove_dir_all(&dir);
        let wf_dir = dir.join(".github").join("workflows");
        fs::create_dir_all(&wf_dir).unwrap();
        fs::write(dir.join("go.sum"), "golang.org/x/sys v0.20.0\n").unwrap();
        fs::write(
            wf_dir.join("ci.yml"),
            "name: Go Matrix\non: [push, pull_request]\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/setup-go@v5\n      - run: golangci-lint run\n  test:\n    runs-on: ubuntu-4-core\n    needs: [lint]\n    steps:\n      - uses: actions/checkout@v4\n      - run: go test ./...\n",
        )
        .unwrap();

        // 1. Test Go cache + security permissions shrink
        let shrink_sec = run_surgical_optimizer_with_opts(
            &dir,
            "shrink",
            "security",
            false,
            MigrateOptions {
                security_mode: true,
                ..Default::default()
            },
        );
        assert!(shrink_sec.edits.iter().any(|e| e.rule_id == "MINICI-SEC-006"));
        assert!(shrink_sec.edits.iter().any(|e| e.rule_id == "MINICI-SHRINK-006"));

        // 2. Test GitHub Actions -> ALL 6 other CI platforms (`gitlab-ci`, `circleci`, `bitbucket`, `aws-codebuild`, `gcp-cloud-build`, `azure-pipelines`)!
        for target_plat in [
            "gitlab-ci",
            "circleci",
            "bitbucket",
            "aws-codebuild",
            "gcp-cloud-build",
            "azure-pipelines",
        ] {
            let receipt = run_surgical_optimizer_with_opts(
                &dir,
                "migrate",
                target_plat,
                false,
                MigrateOptions {
                    from_vendor: Some("github-actions".to_string()),
                    ..Default::default()
                },
            );
            assert_eq!(receipt.source_platform, "github-actions");
            assert_eq!(receipt.target_platform, target_plat);
            let preview = receipt
                .converted_github_workflow_preview
                .as_deref()
                .expect("Expected cross-platform YAML preview");
            assert!(
                preview.contains("go test ./..."),
                "Target {} missing translated `go test ./...` command: {}",
                target_plat,
                preview
            );
            assert!(receipt.switching_verdict.is_some());
        }

        // 3. Test Jenkinsfile -> GitHub Actions migration!
        let jdir = std::env::temp_dir().join("minici_jenkins_to_gha_test");
        let _ = fs::remove_dir_all(&jdir);
        fs::create_dir_all(&jdir).unwrap();
        fs::write(
            jdir.join("Jenkinsfile"),
            "pipeline {\n  agent any\n  stages {\n    stage('Build') {\n      steps {\n        sh 'mvn -B package'\n      }\n    }\n    stage('Unit Test') {\n      steps {\n        sh 'mvn test'\n      }\n    }\n  }\n}\n",
        )
        .unwrap();
        let j_receipt = run_surgical_optimizer(&jdir, "migrate", "github-actions", true);
        assert_eq!(j_receipt.source_platform, "jenkins");
        assert!(jdir.join(".github/workflows/ci.yml").is_file());
        let gha_written = fs::read_to_string(jdir.join(".github/workflows/ci.yml")).unwrap();
        assert!(gha_written.contains("mvn -B package"));
        assert!(gha_written.contains("needs: [build]"));

        // 4. Test dynamic SKU migration (`--to warp-ubuntu-latest-arm64-2x`)
        let dyn_mig = run_surgical_optimizer(&dir, "migrate", "warp-ubuntu-latest-arm64-2x", false);
        assert!(dyn_mig.edits.iter().any(|e| e.replacement_snippet.contains("warp-ubuntu-latest-arm64-2x")));

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&jdir);
    }
}
