//! Unified Markdown, Table, HTML, CSV, Slack Block Kit & Launch Pack Renderer
//! (`crates/minici/src/renderer.rs` — Zero External Dependencies).

use crate::engine::{json_escape, normalize_ci_path, BreakdownReport, DiffReport};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy)]
pub struct GlyphSet {
    pub bullet: &'static str,
    pub sep: &'static str,
    pub times: &'static str,
    pub emdash: &'static str,
    pub micros: &'static str,
    pub check: &'static str,
    pub rule_char: char,
}

pub fn use_ascii_mode() -> bool {
    if let Ok(v) = env::var("MINICI_ASCII") {
        let low = v.trim().to_ascii_lowercase();
        if low == "1" || low == "true" || low == "yes" {
            return true;
        }
        if low == "0" || low == "false" || low == "no" {
            return false;
        }
    }
    if env::var("NO_UNICODE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return true;
    }
    if let Ok(lc_all) = env::var("LC_ALL") {
        let v = lc_all.trim();
        if v.eq_ignore_ascii_case("c") || v.eq_ignore_ascii_case("posix") {
            return true;
        }
    }
    #[cfg(windows)]
    {
        let modern_win_term = env::var("WT_SESSION").is_ok()
            || env::var("TERM_PROGRAM").is_ok()
            || env::var("VSCODE_PID").is_ok()
            || env::var("CURSOR_TRACE_ID").is_ok()
            || env::var("GITHUB_ACTIONS").is_ok()
            || env::var("GITLAB_CI").is_ok()
            || env::var("TF_BUILD").is_ok()
            || env::var("LANG")
                .map(|l| l.to_ascii_lowercase().contains("utf"))
                .unwrap_or(false);
        if !modern_win_term {
            return true;
        }
    }
    false
}

pub fn glyphs_for(ascii: bool) -> GlyphSet {
    if ascii {
        GlyphSet {
            bullet: "*",
            sep: " | ",
            times: "x",
            emdash: "--",
            micros: "us",
            check: "[OK]",
            rule_char: '-',
        }
    } else {
        GlyphSet {
            bullet: "•",
            sep: " • ",
            times: "×",
            emdash: "—",
            micros: "µs",
            check: "✓",
            rule_char: '─',
        }
    }
}

pub fn active_glyphs() -> GlyphSet {
    glyphs_for(use_ascii_mode())
}

pub fn short_workflow_label(raw: &str) -> String {
    let norm = normalize_ci_path(raw);
    if let Some(stripped) = norm.strip_prefix(".github/workflows/") {
        stripped.to_string()
    } else if let Some(stripped) = norm.strip_prefix("./.github/workflows/") {
        stripped.to_string()
    } else {
        norm
    }
}

fn parse_git_remote_host_and_slug(raw_remote: &str) -> Option<String> {
    let r = raw_remote.trim().trim_end_matches('/').trim_end_matches(".git");
    if r.is_empty() {
        return None;
    }
    if let Some(rest) = r.strip_prefix("https://").or_else(|| r.strip_prefix("http://")) {
        let clean = match rest.find('@') {
            Some(at_idx) => &rest[at_idx + 1..],
            None => rest,
        };
        if clean.contains('/') {
            return Some(clean.to_string());
        }
    } else if let Some(rest) = r.strip_prefix("ssh://git@") {
        if rest.contains('/') {
            return Some(rest.to_string());
        }
    } else if let Some(rest) = r.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            if !host.is_empty() && !path.is_empty() {
                return Some(format!("{}/{}", host, path.trim_start_matches('/')));
            }
        }
    }
    None
}

fn detect_github_account_and_git_user() -> String {
    let mut gh_users = Vec::new();
    if let Ok(home) = env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        let hosts_path = Path::new(&home).join(".config").join("gh").join("hosts.yml");
        if let Ok(raw) = fs::read_to_string(hosts_path) {
            for line in raw.lines() {
                let t = line.trim();
                if let Some(u) = t.strip_prefix("user:") {
                    let u = u.trim().trim_matches('"').trim_matches('\'');
                    if !u.is_empty() && !gh_users.contains(&u.to_string()) {
                        gh_users.push(u.to_string());
                    }
                }
            }
        }
    }
    let git_name = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let git_email = Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let gh_part = if !gh_users.is_empty() {
        format!("github.com/{}", gh_users.join(", github.com/"))
    } else {
        "github.com (local git)".to_string()
    };

    if !git_name.is_empty() || !git_email.is_empty() {
        format!("{} ({} <{}>)", gh_part, git_name, git_email)
    } else {
        gh_part
    }
}

fn detect_repo_identity(target_path: &str, ascii_mode: bool) -> String {
    let g = glyphs_for(ascii_mode);
    let norm_target = normalize_ci_path(target_path);
    let p = Path::new(target_path);
    let check_dir = if p.exists() && p.is_dir() {
        p
    } else {
        Path::new(".")
    };
    let remote_host_slug = Command::new("git")
        .arg("-C")
        .arg(check_dir)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .and_then(|s| parse_git_remote_host_and_slug(&s))
        .unwrap_or_default();

    let branch = Command::new("git")
        .arg("-C")
        .arg(check_dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    if norm_target.contains("[PUBLIC & ACCESSIBLE")
        || norm_target.starts_with("github.com/")
        || norm_target.starts_with("gitlab.com/")
        || norm_target.starts_with("bitbucket.org/")
    {
        norm_target
    } else if !remote_host_slug.is_empty()
        && (norm_target == "." || norm_target.contains("ci-casestudies") || p.join(".git").exists())
    {
        if !branch.is_empty() {
            format!(
                "{} (branch: {}{}path: {})",
                remote_host_slug, branch, g.sep, norm_target
            )
        } else {
            format!("{} (path: {})", remote_host_slug, norm_target)
        }
    } else {
        norm_target
    }
}

fn detect_binary_size_kb() -> u64 {
    env::current_exe()
        .ok()
        .and_then(|p| fs::metadata(p).ok())
        .map(|m| (m.len() + 512) / 1024)
        .unwrap_or(492)
}

fn show_full_job_table() -> bool {
    env::var("MINICI_FULL_TABLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("all"))
        .unwrap_or(false)
}

fn ranked_job_indices(r: &BreakdownReport) -> Vec<usize> {
    let app_costs = r.apportioned_job_monthly_costs();
    let mut indices: Vec<usize> = (0..r.jobs.len()).collect();
    indices.sort_by(|&a, &b| {
        app_costs[b]
            .partial_cmp(&app_costs[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                r.jobs[b]
                    .run_compute_cost_usd
                    .partial_cmp(&r.jobs[a].run_compute_cost_usd)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.cmp(&b))
    });
    indices
}

fn primary_change_request_label(providers: &[String]) -> &'static str {
    if providers.len() == 1 && providers[0] == "gitlab-ci" {
        "merged MR"
    } else {
        "merged PR"
    }
}

pub fn render_cli_compact_summary(r: &BreakdownReport) -> String {
    render_cli_compact_summary_with_opts(r, use_ascii_mode(), show_full_job_table())
}

pub fn render_cli_compact_summary_with_opts(
    r: &BreakdownReport,
    ascii_mode: bool,
    force_full_table: bool,
) -> String {
    let g = glyphs_for(ascii_mode);
    if r.total_workflows == 0 {
        return format!("{} minici: Skipped (0 CI workflow files found)", g.check);
    }

    let repo_info = detect_repo_identity(&r.target_path, ascii_mode);
    let mode_env = std::env::var("MINICI_CUSTOMER_MODE").unwrap_or_default();
    let is_owner_mode = mode_env == "0" || mode_env.eq_ignore_ascii_case("false");
    let is_explicit_customer_mode = mode_env == "1" || mode_env.eq_ignore_ascii_case("true");
    let customer_mode = !is_owner_mode;

    let view_suffix = if is_owner_mode {
        " [OWNER / INTERNAL DEBUG VIEW]"
    } else if is_explicit_customer_mode {
        " [CUSTOMER LAUNCH VIEW]"
    } else {
        ""
    };

    let prov_badge = if r.provenance_mode != "static_heuristic" {
        format!("{}{}", g.sep, r.provenance_mode)
    } else {
        String::new()
    };
    let mut out = String::new();
    out.push_str(&format!(
        "Project: {} ({} workflow(s), {} expanded jobs{}{:.2} ms{}){}\n\n",
        repo_info,
        r.total_workflows,
        r.total_expanded_matrix_jobs,
        g.sep,
        r.execution_time_ms,
        prov_badge,
        view_suffix
    ));

    let mat_hdr = format!("MATRIX {} RUNNER", g.times);
    let r42: String = std::iter::repeat(g.rule_char).take(42).collect();
    let r24: String = std::iter::repeat(g.rule_char).take(24).collect();
    let r11: String = std::iter::repeat(g.rule_char).take(11).collect();
    let r10: String = std::iter::repeat(g.rule_char).take(10).collect();
    let r13: String = std::iter::repeat(g.rule_char).take(13).collect();

    out.push_str(&format!(
        " {:<42}  {:<24}  {:>11}  {:>10}  {:>10}  {:>13}\n",
        "WORKFLOW / JOB", mat_hdr, "BILLED MINS", "RATE/MIN", "COST/RUN", "MONTHLY COST"
    ));
    out.push_str(&format!(
        " {:<42}  {:<24}  {:>11}  {:>10}  {:>10}  {:>13}\n",
        r42, r24, r11, r10, r10, r13
    ));

    let app_costs = r.apportioned_job_monthly_costs();
    let max_default_rows = 15usize;
    let full_table = force_full_table || r.jobs.len() <= max_default_rows;
    let display_indices: Vec<usize> = if full_table {
        (0..r.jobs.len()).collect()
    } else {
        ranked_job_indices(r).into_iter().take(max_default_rows).collect()
    };

    for &idx in &display_indices {
        let j = &r.jobs[idx];
        let wf_short = short_workflow_label(&j.workflow_file);
        let wf_job_raw = format!("{}{}{}", wf_short, g.sep, j.job_id);
        let wf_job = if wf_job_raw.chars().count() > 42 {
            let truncated: String = wf_job_raw.chars().take(39).collect();
            format!("{}...", truncated)
        } else {
            wf_job_raw
        };

        let clean_runner_lbl = j
            .runner_label
            .trim()
            .trim_end_matches(',')
            .trim_matches('"')
            .trim_matches('\'');
        let mat_runner_raw = format!("{}x {}", j.matrix_combination_count, clean_runner_lbl);
        let mat_runner = if mat_runner_raw.chars().count() > 24 {
            let truncated: String = mat_runner_raw.chars().take(21).collect();
            format!("{}...", truncated)
        } else {
            mat_runner_raw
        };

        let monthly_job_usd = app_costs[idx];
        let mins_col = format!("{:.1}m", j.total_billed_mins_all_matrix);
        let rate_col = format!("${:.4}/m", j.rate_per_min_usd);
        let run_col = format!("${:.3}", j.run_compute_cost_usd);
        let mo_col = format!("${:.2}/mo", monthly_job_usd);
        out.push_str(&format!(
            " {:<42}  {:<24}  {:>11}  {:>10}  {:>10}  {:>13}\n",
            wf_job, mat_runner, mins_col, rate_col, run_col, mo_col
        ));
    }

    if !full_table && r.jobs.len() > max_default_rows {
        let ranked = ranked_job_indices(r);
        let hidden = &ranked[max_default_rows..];
        let hidden_exp_jobs: usize = hidden
            .iter()
            .map(|&i| r.jobs[i].matrix_combination_count)
            .sum();
        let hidden_mins: f64 = hidden
            .iter()
            .map(|&i| r.jobs[i].total_billed_mins_all_matrix)
            .sum();
        let hidden_run_usd: f64 = hidden.iter().map(|&i| r.jobs[i].run_compute_cost_usd).sum();
        let hidden_mo_usd: f64 = hidden.iter().map(|&i| app_costs[i]).sum();
        let more_lbl = format!("... +{} lower-cost job(s) (--all)", hidden.len());
        let more_mat = format!("{} expanded job(s)", hidden_exp_jobs);
        out.push_str(&format!(
            " {:<42}  {:<24}  {:>11}  {:>10}  {:>10}  {:>13}\n",
            more_lbl,
            more_mat,
            format!("{:.1}m", hidden_mins),
            g.emdash,
            format!("${:.3}", hidden_run_usd),
            format!("${:.2}/mo", hidden_mo_usd)
        ));
    }

    out.push_str(&format!(
        " {:<42}  {:<24}  {:>11}  {:>10}  {:>10}  {:>13}\n",
        r42, r24, r11, r10, r10, r13
    ));
    let tier_short = if r.effective_tier.starts_with("free")
        || r.effective_tier.starts_with("public-free")
        || r.effective_tier.contains("within included quota")
    {
        "free tier"
    } else {
        "paid tier"
    };
    let total_label = format!(
        "OVERALL TOTAL ({:.0} runs/mo{}{})",
        r.monthly_runs, g.sep, tier_short
    );
    let total_label_trunc = if total_label.chars().count() > 42 {
        let t: String = total_label.chars().take(39).collect();
        format!("{}...", t)
    } else {
        total_label
    };
    let jobs_summary = format!("{} expanded job(s)", r.total_expanded_matrix_jobs);
    let tot_mins_col = format!("{:.1}m", r.billed_mins_per_run);
    let tot_run_col = format!("${:.3}", r.gross_compute_cost_per_run_usd);
    let tot_mo_col = format!("${:.2}/mo", r.gross_monthly_compute_cost_usd);
    out.push_str(&format!(
        " {:<42}  {:<24}  {:>11}  {:>10}  {:>10}  {:>13}\n\n",
        total_label_trunc,
        jobs_summary,
        tot_mins_col,
        g.emdash,
        tot_run_col,
        tot_mo_col
    ));

    let cc = &r.cost_components;
    let rounding_suffix = if cc.rounding_tax_mins_per_run > 0.0 {
        format!(
            "{}Rounding Overhead: {:.1}m/run (${:.2}/mo)",
            g.sep, cc.rounding_tax_mins_per_run, cc.monthly_rounding_tax_usd
        )
    } else {
        String::new()
    };
    if r.free_tier_included_mins.is_infinite() {
        out.push_str(&format!(
            " {} Free-Tier Quota  : {:.0} mins (unlimited public quota){}Net Billed: ${:.2}/mo{}\n",
            g.bullet,
            r.monthly_billed_mins,
            g.sep,
            r.net_billed_monthly_compute_cost_usd,
            rounding_suffix
        ));
    } else if (cc.monthly_os_weighted_free_tier_mins - r.monthly_billed_mins).abs() > 0.5
        && cc.monthly_os_weighted_free_tier_mins > 0.0
    {
        out.push_str(&format!(
            " {} Free-Tier Quota  : {:.0} / {:.0} pool mins ({:.1}% of monthly quota){}Net Billed: ${:.2}/mo{}\n",
            g.bullet,
            cc.monthly_os_weighted_free_tier_mins,
            r.free_tier_included_mins,
            r.free_tier_quota_burn_pct,
            g.sep,
            r.net_billed_monthly_compute_cost_usd,
            rounding_suffix
        ));
    } else {
        out.push_str(&format!(
            " {} Free-Tier Quota  : {:.0} / {:.0} mins ({:.1}% of monthly quota){}Net Billed: ${:.2}/mo{}\n",
            g.bullet,
            r.monthly_billed_mins,
            r.free_tier_included_mins,
            r.free_tier_quota_burn_pct,
            g.sep,
            r.net_billed_monthly_compute_cost_usd,
            rounding_suffix
        ));
    }

    let concurrency_suffix = if cc.parallelism_tier_upgrade_tax_monthly_usd > 0.0 {
        format!(
            "{}Concurrency: {} peak jobs ({} hosted > {} cap, +${:.2}/mo tier upgrade)",
            g.sep,
            cc.peak_concurrent_jobs_per_run,
            cc.hosted_concurrent_jobs_per_run,
            cc.vendor_free_concurrent_cap,
            cc.parallelism_tier_upgrade_tax_monthly_usd
        )
    } else if cc.self_hosted_or_microvm_jobs_per_run > 0 {
        format!(
            "{}Concurrency: {} peak jobs ({} hosted / {} custom)",
            g.sep,
            cc.peak_concurrent_jobs_per_run,
            cc.hosted_concurrent_jobs_per_run,
            cc.self_hosted_or_microvm_jobs_per_run
        )
    } else {
        String::new()
    };
    out.push_str(&format!(
        " {} Dev Wait-Time    : {:.1}m critical path (${:.2}/run{}${:.2}/mo at 40% DORA context-switch loss){}\n",
        g.bullet,
        r.critical_path_wait_mins_per_run,
        r.dev_wait_cost_per_run_usd,
        g.sep,
        r.monthly_dev_wait_cost_usd,
        concurrency_suffix
    ));

    if cc.jobs_uploading_artifacts > 0 {
        let retention_hint = if cc.configured_retention_days > 14 {
            format!(
                " (14d retention reduces storage to ${:.2}/mo)",
                cc.monthly_vendor_storage_14d_usd
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            " {} Artifact Storage : {} upload job(s){}{}d retention (${:.2}/mo, {:.2} GB steady-state){}\n",
            g.bullet,
            cc.jobs_uploading_artifacts,
            g.sep,
            cc.configured_retention_days,
            cc.monthly_vendor_storage_configured_usd,
            cc.steady_state_stored_gb_configured,
            retention_hint
        ));
    }

    let mut hygiene_parts = Vec::new();
    if cc.detected_pipeline_shape == "monorepo" && cc.detected_workspace_units_count >= 2 {
        if !cc.has_affected_only_graph_tool && cc.monthly_affected_only_savings_usd >= 10.0 {
            hygiene_parts.push(format!(
                "Monorepo ({} units, {} affected-only saves ~${:.2}/mo)",
                cc.detected_workspace_units_count,
                cc.recommended_affected_tool,
                cc.monthly_affected_only_savings_usd
            ));
        } else {
            hygiene_parts.push(format!(
                "Monorepo ({} units{}{})",
                cc.detected_workspace_units_count, g.sep, cc.detected_monorepo_tool
            ));
        }
    } else if cc.detected_pipeline_shape == "multi-service-microservices" {
        hygiene_parts.push(format!(
            "Multi-service ({} units, {:.0}% setup overhead)",
            cc.detected_workspace_units_count.max(r.total_workflows),
            cc.setup_overhead_share_pct
        ));
    }
    if cc.jobs_missing_cache > 0 && cc.monthly_unrealized_cache_savings_usd >= 1.0 {
        hygiene_parts.push(format!(
            "Cache {}/{} jobs (caching {} uncached saves ~${:.2}/mo)",
            cc.jobs_with_cache,
            r.total_expanded_matrix_jobs,
            cc.jobs_missing_cache,
            cc.monthly_unrealized_cache_savings_usd
        ));
    } else if cc.jobs_with_cache > 0 {
        hygiene_parts.push(format!(
            "Cache {}/{} jobs ({:.0}% est hit rate)",
            cc.jobs_with_cache, r.total_expanded_matrix_jobs, cc.estimated_cache_hit_rate_pct
        ));
    }
    if cc.workflows_missing_concurrency_cancel > 0 && cc.monthly_superseded_run_waste_usd >= 1.0 {
        hygiene_parts.push(format!(
            "Cancel-in-progress missing on {} wf (~${:.2}/mo superseded waste)",
            cc.workflows_missing_concurrency_cancel, cc.monthly_superseded_run_waste_usd
        ));
    }
    if cc.workflows_missing_path_filters > 0 && cc.monthly_unfiltered_check_savings_usd >= 1.0 {
        hygiene_parts.push(format!(
            "Path filters missing on {} wf (~${:.2}/mo selective savings)",
            cc.workflows_missing_path_filters, cc.monthly_unfiltered_check_savings_usd
        ));
    }

    let mut roi_and_safety_parts = Vec::new();
    if cc.pr_unconditional_matrix_cells >= 4 && cc.conditional_pr_matrix_monthly_savings_usd >= 10.0
    {
        roi_and_safety_parts.push(format!(
            "Conditional matrix ({} cells -> 1-cell PR saves ~${:.2}/mo)",
            cc.pr_unconditional_matrix_cells, cc.conditional_pr_matrix_monthly_savings_usd
        ));
    }
    if cc.best_first_optimization_monthly_savings_usd >= 5.0 {
        roi_and_safety_parts.push(format!(
            "Top ROI phase: {} (~{:.0}h effort saves ~${:.2}/mo, {:.1}m payback)",
            cc.best_first_optimization_phase,
            cc.best_first_optimization_effort_hours,
            cc.best_first_optimization_monthly_savings_usd,
            cc.best_first_optimization_payback_months
        ));
    }
    if cc.larger_runners_detected_count > 0 && cc.larger_runners_bypass_free_minutes_tax_usd >= 1.0 {
        roi_and_safety_parts.push(format!(
            "{} larger runner job(s) (${:.2}/mo metered from 1st sec)",
            cc.larger_runners_detected_count, cc.larger_runners_bypass_free_minutes_tax_usd
        ));
    }
    if cc.jobs_missing_timeout_minutes_count > 0 && cc.hung_job_360m_worst_case_exposure_usd >= 5.0 {
        roi_and_safety_parts.push(format!(
            "{} job(s) missing timeout-minutes (${:.2} 6h hung-run exposure)",
            cc.jobs_missing_timeout_minutes_count, cc.hung_job_360m_worst_case_exposure_usd
        ));
    }
    if cc.unpinned_template_refs_count > 0 {
        roi_and_safety_parts.push(format!(
            "{} unpinned @main/@master ref(s)",
            cc.unpinned_template_refs_count
        ));
    }
    if cc.spend_anomaly_alert_active {
        roi_and_safety_parts.push(format!(
            "Spend velocity {:.2}x 30d baseline (proj EOM ${:.2}/mo)",
            cc.burn_velocity_7d_vs_30d_ratio, cc.projected_eom_compute_usd
        ));
    }

    if !hygiene_parts.is_empty() {
        out.push_str(&format!(
            " {} Pipeline Hygiene : {}\n",
            g.bullet,
            hygiene_parts.join(g.sep)
        ));
    }
    if !roi_and_safety_parts.is_empty() {
        out.push_str(&format!(
            " {} Optimization ROI : {}\n",
            g.bullet,
            roi_and_safety_parts.join(g.sep)
        ));
    }

    if r.gross_monthly_compute_cost_usd > 0.0 && cc.cost_per_merged_pr_usd > 0.0 {
        let reg_part = if cc.container_images_built_count > 0
            && cc.monthly_registry_storage_and_pull_usd > 0.0
        {
            format!(
                "{}Container Registry: ${:.2}/mo ({:.1} GB, {} image(s))",
                g.sep,
                cc.monthly_registry_storage_and_pull_usd,
                cc.est_registry_storage_gb,
                cc.container_images_built_count
            )
        } else {
            String::new()
        };
        let idle_part = if cc.monthly_always_on_idle_waste_usd > 0.0 {
            format!(
                "{}Self-Hosted Idle: {:.0}% off-hours (${:.2}/mo waste)",
                g.sep, cc.off_hours_idle_capacity_pct, cc.monthly_always_on_idle_waste_usd
            )
        } else {
            String::new()
        };
        let cr_lbl = primary_change_request_label(&r.detected_providers);
        out.push_str(&format!(
            " {} Unit Economics   : ${:.2}/{}{}${:.2}/active dev/mo{}{}\n",
            g.bullet,
            cc.cost_per_merged_pr_usd,
            cr_lbl,
            g.sep,
            cc.cost_per_active_dev_monthly_usd,
            reg_part,
            idle_part
        ));
    }

    let uses_self_hosted_cloud_vpc = r.jobs.iter().any(|j| {
        j.provider.starts_with("self-hosted")
            || j.provider == "aws-codebuild"
            || j.provider == "gcp-cloud-build"
    });
    if uses_self_hosted_cloud_vpc && cc.monthly_transfer_gb > 0.0 {
        out.push_str(&format!(
            " {} Cloud Network    : {:.2} GB/mo ({}){}Private Subnet + NAT: ${:.2}/mo (VPC Endpoint + Cache: ${:.2}/mo)\n",
            g.bullet,
            cc.monthly_transfer_gb,
            cc.infracost_cloud_region,
            g.sep,
            cc.monthly_infracost_private_subnet_nat_worst_case_usd,
            cc.monthly_infracost_vpc_endpoint_cache_optimized_usd
        ));
    }

    out.push_str(&format!(
        " {} Recommendation   : {}",
        g.bullet,
        build_contextual_recommendation(r)
    ));

    if !customer_mode {
        let catalog_state = crate::catalog::pricing_catalog_status();
        let account_info = detect_github_account_and_git_user();
        let bin_kb = detect_binary_size_kb();
        let trace_lines = if r.provenance_mode == "static_heuristic" {
            format!(
                "               [MATH TRACE] Runs/Mo = round({:.1} PRs/wk * 4.33 wks * {:.1} pushes/PR) = {:.1} runs/mo\n\
               [MATH TRACE] Gross = sum_j(run_cost_j * runs_j) = ${:.2}/mo | Discount = ${:.2}/mo | Net = ${:.2}/mo",
                r.usage.prs_per_week,
                r.usage.pushes_per_pr,
                r.monthly_runs,
                r.gross_monthly_compute_cost_usd,
                r.discount_allowance_monthly_usd,
                r.net_billed_monthly_compute_cost_usd
            )
        } else {
            format!(
                "               [MATH TRACE] Provenance = {} ({:.1} observed runs/mo)\n\
               [MATH TRACE] Gross = ${:.2}/mo | Discount = ${:.2}/mo | Net = ${:.2}/mo",
                r.provenance_mode,
                r.monthly_runs,
                r.gross_monthly_compute_cost_usd,
                r.discount_allowance_monthly_usd,
                r.net_billed_monthly_compute_cost_usd
            )
        };
        out.push_str(&format!(
            "\n\n [OWNER / INTERNAL DEBUG VIEW{}{}{}{} KB Rust Binary]\n\
               [ON] feat.matrix_ast_expansion  : {} defined jobs -> {} expanded matrix combinations\n\
               [ON] feat.hot_reload_catalog    : {}\n{}",
            g.sep,
            account_info,
            g.sep,
            bin_kb,
            r.total_defined_jobs,
            r.total_expanded_matrix_jobs,
            catalog_state,
            trace_lines
        ));
    }

    out
}

fn csv_escape(s: &str) -> String {
    s.replace('"', "\"\"").replace(['\r', '\n'], " ")
}

pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn render_breakdown_csv(r: &BreakdownReport) -> String {
    let mut out = String::from("workflow_file,job_id,line_start,line_end,matrix_count,runner_label,provider,os,vcpus,raw_mins,billed_mins_total,rate_per_min_usd,cost_per_run_usd\n");
    for j in &r.jobs {
        let ls = j.line_start.max(1);
        let le = j.line_end.max(ls);
        out.push_str(&format!(
            "\"{}\",\"{}\",{},{},{},\"{}\",\"{}\",\"{}\",{},{:.2},{:.2},{:.4},{:.4}\n",
            csv_escape(&normalize_ci_path(&j.workflow_file)),
            csv_escape(&j.job_id),
            ls,
            le,
            j.matrix_combination_count,
            csv_escape(&j.runner_label),
            csv_escape(&j.provider),
            csv_escape(&j.os),
            j.vcpus,
            j.est_raw_duration_mins_per_job,
            j.total_billed_mins_all_matrix,
            j.rate_per_min_usd,
            j.run_compute_cost_usd
        ));
    }
    out
}

fn build_contextual_recommendation(r: &BreakdownReport) -> String {
    let cc = &r.cost_components;
    let hosted_total_jobs = r
        .total_expanded_matrix_jobs
        .saturating_sub(cc.self_hosted_or_microvm_jobs_per_run);
    if cc.self_hosted_or_microvm_jobs_per_run > 0
        && cc.self_hosted_or_microvm_jobs_per_run >= hosted_total_jobs
    {
        if cc.jobs_uploading_artifacts > 0 && cc.configured_retention_days > 14 {
            return format!(
                "Pipeline compute already optimized ({}/{} jobs on custom runners); set artifact `retention-days: 14` to cut storage from ${:.2}/mo to ${:.2}/mo",
                cc.self_hosted_or_microvm_jobs_per_run,
                r.total_expanded_matrix_jobs,
                cc.monthly_vendor_storage_configured_usd,
                cc.monthly_vendor_storage_14d_usd
            );
        }
        return format!(
            "Pipeline already optimized ({}/{} jobs on custom runners; remaining {} lightweight hosted job(s) are optimal as-is)",
            cc.self_hosted_or_microvm_jobs_per_run,
            r.total_expanded_matrix_jobs,
            hosted_total_jobs
        );
    }

    if r.exceeds_5000m_self_hosted_breakeven {
        let blacksmith_savings_usd =
            (r.gross_monthly_compute_cost_usd - r.monthly_blacksmith_cost_usd).max(0.0);
        let spot_savings_usd =
            (r.gross_monthly_compute_cost_usd - r.monthly_self_hosted_spot_cost_usd).max(0.0);
        if blacksmith_savings_usd >= 20.0 {
            format!(
                "Right-size heavy jobs to ARM64 / High-Speed Runners (${:.2}/mo, saves ${:.2}/mo) or Cloud Spot (${:.2}/mo, saves ${:.2}/mo)",
                r.monthly_blacksmith_cost_usd,
                blacksmith_savings_usd,
                r.monthly_self_hosted_spot_cost_usd,
                spot_savings_usd
            )
        } else if spot_savings_usd >= 1.0 {
            format!(
                "Move sustained compute ({:.0} mins/mo) to Cloud Spot (${:.2}/mo, saves ${:.2}/mo)",
                r.monthly_billed_mins,
                r.monthly_self_hosted_spot_cost_usd,
                spot_savings_usd
            )
        } else if cc.arm64_migration_monthly_savings_usd >= 1.0 {
            format!(
                "Switch eligible Linux x64 jobs to ARM64 runners (saves ${:.2}/mo)",
                cc.arm64_migration_monthly_savings_usd
            )
        } else if cc.jobs_uploading_artifacts > 0 && cc.configured_retention_days > 14 {
            format!(
                "Stay on hosted runners (already cheaper than dedicated Spot); set artifact `retention-days: 14` to reduce storage to ${:.2}/mo",
                cc.monthly_vendor_storage_14d_usd
            )
        } else {
            format!(
                "Stay on hosted runners (${:.2}/mo hosted is already below dedicated Cloud Spot cost)",
                r.gross_monthly_compute_cost_usd
            )
        }
    } else if cc.jobs_uploading_artifacts > 0 && cc.configured_retention_days > 14 {
        format!(
            "Stay on hosted runners ({:.0} / 5,000 mins/mo break-even); set artifact `retention-days: 14` to reduce storage to ${:.2}/mo",
            r.monthly_billed_mins,
            cc.monthly_vendor_storage_14d_usd
        )
    } else {
        format!(
            "Stay on hosted runners ({:.0} / 5,000 mins/mo self-hosted break-even threshold)",
            r.monthly_billed_mins
        )
    }
}

pub fn render_breakdown_html(r: &BreakdownReport) -> String {
    let mut rows = String::new();
    for j in &r.jobs {
        rows.push_str(&format!(
            "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}x (<code>{}</code>)</td><td>{:.1}m ({:.1}m billed)</td><td>${:.4}/m</td><td><strong>${:.3}/run</strong></td></tr>\n",
            html_escape(&j.workflow_file),
            html_escape(&j.job_id),
            j.matrix_combination_count,
            html_escape(&j.runner_label),
            j.est_raw_duration_mins_per_job,
            j.total_billed_mins_all_matrix,
            j.rate_per_min_usd,
            j.run_compute_cost_usd
        ));
    }
    let rec = html_escape(&build_contextual_recommendation(r));
    let safe_target = html_escape(&r.target_path);
    let cc = &r.cost_components;
    let mut extra_summary = String::new();
    if cc.jobs_uploading_artifacts > 0 {
        extra_summary.push_str(&format!(
            "<p style=\"margin:6px 0 0\"><strong>Artifact Storage:</strong> {} upload job(s) • {}d retention (<strong>${:.2}/mo</strong>, {:.2} GB steady-state)</p>",
            cc.jobs_uploading_artifacts,
            cc.configured_retention_days,
            cc.monthly_vendor_storage_configured_usd,
            cc.steady_state_stored_gb_configured
        ));
    }
    let mut hygiene_items = Vec::new();
    if cc.detected_pipeline_shape == "monorepo" && cc.detected_workspace_units_count >= 2 {
        if !cc.has_affected_only_graph_tool && cc.monthly_affected_only_savings_usd >= 10.0 {
            hygiene_items.push(format!(
                "Monorepo ({} units, {} affected-only saves ~${:.2}/mo)",
                cc.detected_workspace_units_count,
                html_escape(&cc.recommended_affected_tool),
                cc.monthly_affected_only_savings_usd
            ));
        } else {
            hygiene_items.push(format!(
                "Monorepo ({} units • {})",
                cc.detected_workspace_units_count,
                html_escape(&cc.detected_monorepo_tool)
            ));
        }
    } else if cc.detected_pipeline_shape == "multi-service-microservices" {
        hygiene_items.push(format!(
            "Multi-service ({} units, {:.0}% setup overhead)",
            cc.detected_workspace_units_count.max(r.total_workflows),
            cc.setup_overhead_share_pct
        ));
    }
    if cc.jobs_missing_cache > 0 && cc.monthly_unrealized_cache_savings_usd >= 1.0 {
        hygiene_items.push(format!(
            "Cache {}/{} jobs (caching {} uncached saves ~${:.2}/mo)",
            cc.jobs_with_cache,
            r.total_expanded_matrix_jobs,
            cc.jobs_missing_cache,
            cc.monthly_unrealized_cache_savings_usd
        ));
    }
    if cc.workflows_missing_concurrency_cancel > 0 && cc.monthly_superseded_run_waste_usd >= 1.0 {
        hygiene_items.push(format!(
            "Cancel-in-progress missing on {} wf (~${:.2}/mo superseded waste)",
            cc.workflows_missing_concurrency_cancel, cc.monthly_superseded_run_waste_usd
        ));
    }
    if cc.workflows_missing_path_filters > 0 && cc.monthly_unfiltered_check_savings_usd >= 1.0 {
        hygiene_items.push(format!(
            "Path filters missing on {} wf (~${:.2}/mo selective savings)",
            cc.workflows_missing_path_filters, cc.monthly_unfiltered_check_savings_usd
        ));
    }
    if cc.unpinned_template_refs_count > 0 {
        hygiene_items.push(format!(
            "{} unpinned @main/@master ref(s)",
            cc.unpinned_template_refs_count
        ));
    }
    if cc.larger_runners_detected_count > 0 && cc.larger_runners_bypass_free_minutes_tax_usd >= 1.0 {
        hygiene_items.push(format!(
            "{} larger runner job(s) (${:.2}/mo metered from 1st sec; never draw free mins)",
            cc.larger_runners_detected_count, cc.larger_runners_bypass_free_minutes_tax_usd
        ));
    }
    if cc.best_first_optimization_monthly_savings_usd >= 5.0 {
        hygiene_items.push(format!(
            "Top ROI phase: {} (~{:.0}h effort saves ~${:.2}/mo, {:.1}m payback)",
            html_escape(&cc.best_first_optimization_phase),
            cc.best_first_optimization_effort_hours,
            cc.best_first_optimization_monthly_savings_usd,
            cc.best_first_optimization_payback_months
        ));
    }
    if cc.jobs_missing_timeout_minutes_count > 0 && cc.hung_job_360m_worst_case_exposure_usd >= 5.0 {
        hygiene_items.push(format!(
            "{} job(s) missing timeout-minutes (${:.2} 6h hung-run exposure)",
            cc.jobs_missing_timeout_minutes_count, cc.hung_job_360m_worst_case_exposure_usd
        ));
    }
    if cc.spend_anomaly_alert_active {
        hygiene_items.push(format!(
            "Spend velocity {:.2}x baseline (proj EOM ${:.2}/mo)",
            cc.burn_velocity_7d_vs_30d_ratio, cc.projected_eom_compute_usd
        ));
    }
    if !hygiene_items.is_empty() {
        extra_summary.push_str(&format!(
            "<p style=\"margin:6px 0 0\"><strong>Pipeline Hygiene:</strong> {}</p>",
            hygiene_items.join(" • ")
        ));
    }
    if r.gross_monthly_compute_cost_usd > 0.0 && cc.cost_per_merged_pr_usd > 0.0 {
        let mut econ_parts = vec![
            format!("<strong>${:.2}</strong>/merged PR", cc.cost_per_merged_pr_usd),
            format!(
                "<strong>${:.2}</strong>/active dev/mo",
                cc.cost_per_active_dev_monthly_usd
            ),
        ];
        if cc.codeowners_detected && cc.top_codeowners_team != "unassigned" {
            econ_parts.push(format!(
                "Owner: <code>{}</code>",
                html_escape(&cc.top_codeowners_team)
            ));
        }
        if cc.container_images_built_count > 0 && cc.monthly_registry_storage_and_pull_usd > 0.0 {
            econ_parts.push(format!(
                "Container Registry: <strong>${:.2}/mo</strong> ({:.1} GB, {} image(s))",
                cc.monthly_registry_storage_and_pull_usd,
                cc.est_registry_storage_gb,
                cc.container_images_built_count
            ));
        }
        if cc.monthly_always_on_idle_waste_usd > 0.0 {
            econ_parts.push(format!(
                "Self-Hosted Idle: {:.0}% (<strong>${:.2}/mo</strong> waste)",
                cc.off_hours_idle_capacity_pct, cc.monthly_always_on_idle_waste_usd
            ));
        }
        extra_summary.push_str(&format!(
            "<p style=\"margin:6px 0 0\"><strong>Unit Economics:</strong> {}</p>",
            econ_parts.join(" • ")
        ));
    }
    let uses_self_hosted_cloud_vpc = r.jobs.iter().any(|j| {
        j.provider.starts_with("self-hosted")
            || j.provider == "aws-codebuild"
            || j.provider == "gcp-cloud-build"
    });
    if uses_self_hosted_cloud_vpc
        && cc.monthly_transfer_gb > 0.0
        && cc.monthly_infracost_private_subnet_nat_worst_case_usd > 0.0
    {
        extra_summary.push_str(&format!(
            "<p style=\"margin:6px 0 0\"><strong>Cloud Network:</strong> {:.2} GB/mo (<code>{}</code>) • Private Subnet + NAT: <strong>${:.2}/mo</strong> (VPC Endpoint + Cache: ${:.2}/mo)</p>",
            cc.monthly_transfer_gb,
            html_escape(&cc.infracost_cloud_region),
            cc.monthly_infracost_private_subnet_nat_worst_case_usd,
            cc.monthly_infracost_vpc_endpoint_cache_optimized_usd
        ));
    }
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><title>MiniCI FinOps Report — {}</title>\
         <style>body{{background:#080b10;color:#f1f5f9;font-family:-apple-system,BlinkMacSystemFont,sans-serif;padding:32px;max-width:1100px;margin:0 auto}}\
         .card{{background:#0f151f;border:1px solid #1e293b;border-radius:10px;padding:20px;margin-bottom:20px}}\
         table{{width:100%;border-collapse:collapse;font-size:14px}}th,td{{padding:10px;border-bottom:1px solid #1e293b;text-align:left}}\
         th{{color:#94a3b8}}code{{color:#38bdf8;font-family:monospace}}.kpi{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin-bottom:20px}}\
         .kpi div{{background:#151e2c;padding:14px;border-radius:8px;border:1px solid #1e293b}}.kpi h3{{margin:0 0 6px;font-size:12px;color:#94a3b8}}\
         .kpi p{{margin:0;font-size:20px;font-weight:700;color:#10b981}}</style></head><body>\
         <h1>MiniCI — CI/CD Cost &amp; Matrix FinOps Report</h1>\
         <p>Target: <code>{}</code> • Scanned in <code>{:.2} ms</code> • {} Workflows ({} Expanded Matrix Jobs)</p>\
         <div class=\"kpi\">\
           <div><h3>Monthly CI Bill</h3><p>${:.2}/mo (${:.3}/run)</p></div>\
           <div><h3>Free Quota Burn</h3><p>{:.0} / {:.0}m ({:.1}%)</p></div>\
           <div><h3>Dev Wait (${:.0}/hr • 40% Loss)</h3><p>${:.2}/mo (${:.2} loss)</p></div>\
           <div><h3>Peak Concurrency</h3><p>{} jobs ({} hosted)</p></div>\
         </div>\
         <div class=\"card\"><p style=\"margin:0\"><strong>Recommendation:</strong> {}</p>{}</div>\
         <div class=\"card\"><h2>Per-Job Matrix Breakdown</h2>\
         <table><thead><tr><th>Workflow</th><th>Job ID</th><th>Matrix × SKU</th><th>Duration (Billed)</th><th>Official Rate</th><th>Cost / Run</th></tr></thead>\
         <tbody>{}</tbody></table></div></body></html>\n",
        safe_target,
        safe_target,
        r.execution_time_ms,
        r.total_workflows,
        r.total_expanded_matrix_jobs,
        r.gross_monthly_compute_cost_usd,
        r.gross_compute_cost_per_run_usd,
        r.monthly_billed_mins,
        r.free_tier_included_mins,
        r.free_tier_quota_burn_pct,
        r.usage.dev_hourly_rate_usd,
        r.monthly_dev_wait_cost_usd,
        cc.monthly_productive_payroll_loss_usd,
        r.cost_components.peak_concurrent_jobs_per_run,
        r.cost_components.hosted_concurrent_jobs_per_run,
        rec,
        extra_summary,
        rows
    )
}

pub fn render_cli_compact_diff(d: &DiffReport) -> String {
    render_cli_compact_diff_with_opts(d, use_ascii_mode())
}

pub fn render_cli_compact_diff_with_opts(d: &DiffReport, ascii_mode: bool) -> String {
    let g = glyphs_for(ascii_mode);
    let sign = if d.delta_monthly_compute_usd >= 0.0 { "+" } else { "-" };
    let pct_sign = if d.delta_monthly_compute_pct >= 0.0 { "+" } else { "" };
    let wait_sign = if d.delta_monthly_dev_wait_usd >= 0.0 { "+" } else { "-" };
    let is_gitlab = d.head_report.detected_providers.len() == 1
        && d.head_report.detected_providers[0] == "gitlab-ci";
    let cr_upper = if is_gitlab { "MERGE REQUEST" } else { "PULL REQUEST" };
    let cr_short = if is_gitlab { "MR Head" } else { "PR Head" };
    let rule: String = std::iter::repeat(g.rule_char).take(86).collect();

    let changes_block = if d.job_changes.is_empty() {
        format!("   5. Key Change        : No workflow or matrix SKU changes between base and head.\n")
    } else {
        let mut lines = String::new();
        for (idx, ch) in d.job_changes.iter().take(3).enumerate() {
            if idx == 0 {
                lines.push_str(&format!(
                    "   5. Key Changes ({})   : {}\n",
                    d.job_changes.len(),
                    ch
                ));
            } else {
                lines.push_str(&format!("                          {}\n", ch));
            }
        }
        if d.job_changes.len() > 3 {
            lines.push_str(&format!(
                "                          ... +{} additional workflow/job delta(s)\n",
                d.job_changes.len() - 3
            ));
        }
        lines
    };

    format!(
        " {}\n\
         {} MINICI {} {} COST DIFF ({} {}{}Exit Code 0 Informational)\n\
         {}\n\
           Comparing            : `{}` (Base) -> `{}` ({})\n\
           1. Matrix Jobs       : {} jobs -> {} jobs ({:+} jobs)\n\
           2. Billed Mins / Run : {:.1}m -> {:.1}m ({:+.1}m / run)\n\
           3. Monthly CI Cost   : ${:.2}/mo -> ${:.2}/mo ({}${:.2}/mo{}{}{:.1}%)\n\
           4. Dev Wait Cost ($) : ${:.2}/mo -> ${:.2}/mo ({}${:.2}/mo) [Assumed Rate Configurable]\n\
        {}\
         {}\n",
        rule,
        " ",
        g.emdash,
        cr_upper,
        d.execution_time_us,
        g.micros,
        g.sep,
        format!(" {}", rule),
        normalize_ci_path(&d.base_path),
        normalize_ci_path(&d.head_path),
        cr_short,
        d.base_expanded_jobs,
        d.head_expanded_jobs,
        d.delta_expanded_jobs,
        d.base_billed_mins_per_run,
        d.head_billed_mins_per_run,
        d.delta_billed_mins_per_run,
        d.base_monthly_compute_usd,
        d.head_monthly_compute_usd,
        sign,
        d.delta_monthly_compute_usd.abs(),
        g.sep,
        pct_sign,
        d.delta_monthly_compute_pct,
        d.base_monthly_dev_wait_usd,
        d.head_monthly_dev_wait_usd,
        wait_sign,
        d.delta_monthly_dev_wait_usd.abs(),
        changes_block,
        format!(" {}", rule)
    )
}

fn render_applicable_summary_notes_markdown(r: &BreakdownReport) -> String {
    let cc = &r.cost_components;
    let mut notes = Vec::new();
    if cc.rounding_tax_mins_per_run > 0.0 {
        notes.push(format!(
            "- **1-Minute Ceiling Rounding:** `{:.1}m/run` (`${:.2}/mo`)",
            cc.rounding_tax_mins_per_run, cc.monthly_rounding_tax_usd
        ));
    }
    if cc.jobs_uploading_artifacts > 0 {
        notes.push(format!(
            "- **Artifact Storage:** `{}` upload job(s) at `{}d` retention (`${:.2}/mo`, `{:.2} GB` steady-state)",
            cc.jobs_uploading_artifacts,
            cc.configured_retention_days,
            cc.monthly_vendor_storage_configured_usd,
            cc.steady_state_stored_gb_configured
        ));
    }
    if cc.parallelism_tier_upgrade_tax_monthly_usd > 0.0 {
        notes.push(format!(
            "- **Concurrency Cap:** `{}` hosted concurrent jobs exceed `{}` tier cap (`+${:.2}/mo` tier upgrade)",
            cc.hosted_concurrent_jobs_per_run,
            cc.vendor_free_concurrent_cap,
            cc.parallelism_tier_upgrade_tax_monthly_usd
        ));
    }
    if cc.detected_pipeline_shape == "monorepo" && cc.detected_workspace_units_count >= 2 {
        if !cc.has_affected_only_graph_tool && cc.monthly_affected_only_savings_usd >= 10.0 {
            notes.push(format!(
                "- **Monorepo Topology:** `{}` workspace units detected; adopting `{}` affected-only execution saves **`~${:.2}/mo`**",
                cc.detected_workspace_units_count,
                cc.recommended_affected_tool,
                cc.monthly_affected_only_savings_usd
            ));
        } else {
            notes.push(format!(
                "- **Monorepo Topology:** `{}` workspace units (`{}` active)",
                cc.detected_workspace_units_count, cc.detected_monorepo_tool
            ));
        }
    } else if cc.detected_pipeline_shape == "multi-service-microservices" {
        notes.push(format!(
            "- **Multi-Service Topology:** `{}` service units (`{:.0}%` per-job setup overhead; shared base image saves `~${:.2}/mo`)",
            cc.detected_workspace_units_count.max(r.total_workflows),
            cc.setup_overhead_share_pct,
            cc.monthly_shared_infra_setup_savings_usd
        ));
    }
    if cc.jobs_missing_cache > 0 && cc.monthly_unrealized_cache_savings_usd >= 1.0 {
        notes.push(format!(
            "- **Build Cache Opportunity:** `{}/{}` jobs cached; caching remaining `{}` job(s) saves **`~${:.2}/mo`** (`{:.0}-{:.0}%` typical for `{}`)",
            cc.jobs_with_cache,
            r.total_expanded_matrix_jobs,
            cc.jobs_missing_cache,
            cc.monthly_unrealized_cache_savings_usd,
            cc.realistic_saving_pct_min,
            cc.realistic_saving_pct_max,
            cc.stack_cache_profile
        ));
    }
    if cc.workflows_missing_concurrency_cancel > 0 && cc.monthly_superseded_run_waste_usd >= 1.0 {
        notes.push(format!(
            "- **Superseded Push Runs:** `{}` workflow(s) lack `cancel-in-progress: true` (`~${:.2}/mo` wasted on obsolete pushes)",
            cc.workflows_missing_concurrency_cancel,
            cc.monthly_superseded_run_waste_usd
        ));
    }
    if cc.workflows_missing_path_filters > 0 && cc.monthly_unfiltered_check_savings_usd >= 1.0 {
        notes.push(format!(
            "- **Selective PR Checks:** `{}` workflow(s) run on all pushes without `paths:` filters (`~${:.2}/mo` savings with path filtering)",
            cc.workflows_missing_path_filters,
            cc.monthly_unfiltered_check_savings_usd
        ));
    }
    if cc.unpinned_template_refs_count > 0 {
        notes.push(format!(
            "- **Template Pinning:** `{}` unpinned `@main`/`@master` reference(s) detected",
            cc.unpinned_template_refs_count
        ));
    }
    if cc.larger_runners_detected_count > 0 && cc.larger_runners_bypass_free_minutes_tax_usd >= 1.0 {
        notes.push(format!(
            "- **Larger Runners Billing:** `{}` larger runner job(s) (`${:.2}/mo` metered from first second; never consume included free minutes)",
            cc.larger_runners_detected_count, cc.larger_runners_bypass_free_minutes_tax_usd
        ));
    }
    if cc.pr_unconditional_matrix_cells >= 4 && cc.conditional_pr_matrix_monthly_savings_usd >= 10.0
    {
        notes.push(format!(
            "- **Conditional PR Matrix (`{}`):** `{}` matrix cells run on every PR; running `1` cell on PRs and full matrix on `main`/nightly saves **`~${:.2}/mo`**",
            cc.detected_repo_matrix_profile,
            cc.pr_unconditional_matrix_cells,
            cc.conditional_pr_matrix_monthly_savings_usd
        ));
    }
    if cc.best_first_optimization_monthly_savings_usd >= 5.0 {
        notes.push(format!(
            "- **Best First Optimization (`{}`):** `~{:.0}h` effort reclaims **`~${:.2}/mo`** (`{:.1} mo` payback; `${:.2}/mo` total reducible across phases)",
            cc.best_first_optimization_phase,
            cc.best_first_optimization_effort_hours,
            cc.best_first_optimization_monthly_savings_usd,
            cc.best_first_optimization_payback_months,
            cc.monthly_total_reducible_phase_savings_usd
        ));
    }
    if r.gross_monthly_compute_cost_usd > 0.0 && cc.cost_per_merged_pr_usd > 0.0 {
        let cr_lbl = primary_change_request_label(&r.detected_providers);
        notes.push(format!(
            "- **Unit Economics:** **`${:.2}` / {}** • **`${:.2}` / active developer / mo** (`{:.0}%` top-workflow share)",
            cc.cost_per_merged_pr_usd,
            cr_lbl,
            cc.cost_per_active_dev_monthly_usd,
            cc.top_workflow_cost_share_pct
        ));
    }
    if cc.jobs_missing_timeout_minutes_count > 0 && cc.hung_job_360m_worst_case_exposure_usd >= 5.0 {
        notes.push(format!(
            "- **Hung-Job Guardrail:** `{}` job(s) lack `timeout-minutes` (`${:.2}` worst-case 6-hour default timeout exposure)",
            cc.jobs_missing_timeout_minutes_count,
            cc.hung_job_360m_worst_case_exposure_usd
        ));
    }
    if cc.spend_anomaly_alert_active {
        notes.push(format!(
            "- **Spend Velocity Alert:** 7-day burn is **`{:.2}x`** 30-day baseline (projected EOM **`${:.2}/mo`**)",
            cc.burn_velocity_7d_vs_30d_ratio,
            cc.projected_eom_compute_usd
        ));
    }
    if cc.container_images_built_count > 0 && cc.monthly_registry_storage_and_pull_usd > 0.0 {
        notes.push(format!(
            "- **Container Registry:** `{}` image(s) built (`~{:.1} GB` active layers, **`${:.2}/mo`** storage)",
            cc.container_images_built_count,
            cc.est_registry_storage_gb,
            cc.monthly_registry_storage_and_pull_usd
        ));
    }
    if cc.monthly_always_on_idle_waste_usd > 0.0 {
        notes.push(format!(
            "- **Self-Hosted Idle Capacity:** `{:.0}%` off-hours idle (**`${:.2}/mo`** always-on waste; scale-to-zero recommended)",
            cc.off_hours_idle_capacity_pct,
            cc.monthly_always_on_idle_waste_usd
        ));
    }
    let uses_self_hosted_cloud_vpc = r.jobs.iter().any(|j| {
        j.provider.starts_with("self-hosted")
            || j.provider == "aws-codebuild"
            || j.provider == "gcp-cloud-build"
    });
    if uses_self_hosted_cloud_vpc
        && cc.monthly_transfer_gb > 0.0
        && cc.monthly_infracost_private_subnet_nat_worst_case_usd > 0.0
    {
        notes.push(format!(
            "- **Cloud Network:** `{:.2} GB/mo` (`{}`) • Private Subnet + NAT: **`${:.2}/mo`** (VPC Endpoint + Cache: `${:.2}/mo`)",
            cc.monthly_transfer_gb,
            cc.infracost_cloud_region,
            cc.monthly_infracost_private_subnet_nat_worst_case_usd,
            cc.monthly_infracost_vpc_endpoint_cache_optimized_usd
        ));
    }
    notes.push(format!("- **Recommendation:** {}", build_contextual_recommendation(r)));
    format!("\n{}\n", notes.join("\n"))
}

fn build_agent_meta_comment_breakdown(r: &BreakdownReport) -> String {
    let prov = r
        .detected_providers
        .first()
        .map(|s| s.as_str())
        .unwrap_or("github-actions");
    format!(
        "<!-- minici-agent-meta: {{\"schema\":\"1.0\",\"provider\":\"{}\",\"workflows\":{},\"expanded_jobs\":{},\"gross_monthly_usd\":{:.2},\"net_monthly_usd\":{:.2},\"dev_wait_monthly_usd\":{:.2},\"reducible_monthly_usd\":{:.2},\"top_phase\":\"{}\",\"mcp_tools\":[\"minici_breakdown\",\"minici_shrink\",\"minici_migrate\"]}} -->\n",
        json_escape(prov),
        r.total_workflows,
        r.total_expanded_matrix_jobs,
        r.gross_monthly_compute_cost_usd,
        r.net_billed_monthly_compute_cost_usd,
        r.monthly_dev_wait_cost_usd,
        r.cost_components.monthly_total_reducible_phase_savings_usd,
        json_escape(&r.cost_components.best_first_optimization_phase)
    )
}

fn build_agent_meta_comment_diff(d: &DiffReport, platform: &str) -> String {
    let nz = |v: f64| if v.abs() < 0.0005 { 0.0 } else { v };
    format!(
        "<!-- minici-agent-meta: {{\"schema\":\"1.0\",\"platform\":\"{}\",\"delta_expanded_jobs\":{},\"delta_billed_mins_per_run\":{:.2},\"base_monthly_usd\":{:.2},\"head_monthly_usd\":{:.2},\"delta_monthly_usd\":{:.2},\"delta_dev_wait_usd\":{:.2},\"reducible_monthly_usd\":{:.2},\"fix_cmd\":\"minici shrink --path . --apply\"}} -->\n",
        json_escape(platform),
        d.delta_expanded_jobs,
        nz(d.delta_billed_mins_per_run),
        nz(d.base_monthly_compute_usd),
        nz(d.head_monthly_compute_usd),
        nz(d.delta_monthly_compute_usd),
        nz(d.delta_monthly_dev_wait_usd),
        nz(d.head_report.cost_components.monthly_total_reducible_phase_savings_usd)
    )
}

pub fn render_breakdown_markdown(r: &BreakdownReport) -> String {
    if r.total_workflows == 0 {
        return "<!-- minici-cost-comment -->\n✓ **MiniCI:** Skipped (0 CI workflow files found)\n".to_string();
    }

    let max_default_rows = 15usize;
    let full_table = show_full_job_table() || r.jobs.len() <= max_default_rows;
    let ranked = ranked_job_indices(r);
    let primary_indices: Vec<usize> = if full_table {
        (0..r.jobs.len()).collect()
    } else {
        ranked.iter().copied().take(max_default_rows).collect()
    };

    let app_costs = r.apportioned_job_monthly_costs();
    let mut rows = String::new();
    for &idx in &primary_indices {
        let j = &r.jobs[idx];
        let monthly_job_usd = app_costs[idx];
        rows.push_str(&format!(
            "| `{}` | `{}` | `{}x {}` | `{:.1}m` | `${:.4}/m` | `${:.3}` | **`${:.2}/mo`** |\n",
            short_workflow_label(&j.workflow_file),
            j.job_id,
            j.matrix_combination_count,
            j.runner_label,
            j.total_billed_mins_all_matrix,
            j.rate_per_min_usd,
            j.run_compute_cost_usd,
            monthly_job_usd
        ));
    }

    let mut overflow_details = String::new();
    if !full_table && r.jobs.len() > max_default_rows {
        let rest_indices = &ranked[max_default_rows..];
        let mut rest_rows = String::new();
        let mut rest_mo_usd = 0.0_f64;
        for &idx in rest_indices {
            let j = &r.jobs[idx];
            let monthly_job_usd = app_costs[idx];
            rest_mo_usd += monthly_job_usd;
            rest_rows.push_str(&format!(
                "| `{}` | `{}` | `{}x {}` | `{:.1}m` | `${:.4}/m` | `${:.3}` | **`${:.2}/mo`** |\n",
                short_workflow_label(&j.workflow_file),
                j.job_id,
                j.matrix_combination_count,
                j.runner_label,
                j.total_billed_mins_all_matrix,
                j.rate_per_min_usd,
                j.run_compute_cost_usd,
                monthly_job_usd
            ));
        }
        overflow_details = format!(
            "\n<details>\n<summary>Expand remaining <code>{}</code> lower-cost workflow job(s) (<code>${:.2}/mo</code> combined)</summary>\n\n\
| Workflow | Job ID | Matrix × Runner | Billed Mins | Rate/Min | Cost/Run | Monthly (`{:.0}` runs) |\n\
| :--- | :--- | :--- | ---: | ---: | ---: | ---: |\n\
{}\n</details>\n",
            rest_indices.len(),
            rest_mo_usd,
            r.monthly_runs,
            rest_rows.trim_end()
        );
    }

    let notes_md = render_applicable_summary_notes_markdown(r);
    let agent_meta = build_agent_meta_comment_breakdown(r);

    format!(
        "<!-- minici-cost-comment -->\n\
### 💰 MiniCI CI/CD Cost & Quota Breakdown (`{:.2} ms`)\n\n\
**Summary:** `{}` workflow(s) • `{}` expanded matrix job(s) • **`${:.2}/mo`** gross compute (**`${:.2}/mo`** net billed, `{:.1}%` of `{:.0}m` free quota) • **Dev Wait:** `${:.2}/mo` (`{:.1}m` critical path)\n\n\
| Workflow | Job ID | Matrix × Runner | Billed Mins | Rate/Min | Cost/Run | Monthly (`{:.0}` runs) |\n\
| :--- | :--- | :--- | ---: | ---: | ---: | ---: |\n\
{rows}| **OVERALL TOTAL** | **`{} job(s)`** | **`{}`** | **`{:.1}m`** | — | **`${:.3}`** | **`${:.2}/mo`** |\n\
{overflow_details}{notes_md}{agent_meta}",
        r.execution_time_ms,
        r.total_workflows,
        r.total_expanded_matrix_jobs,
        r.gross_monthly_compute_cost_usd,
        r.net_billed_monthly_compute_cost_usd,
        r.free_tier_quota_burn_pct,
        r.free_tier_included_mins,
        r.monthly_dev_wait_cost_usd,
        r.critical_path_wait_mins_per_run,
        r.monthly_runs,
        r.total_expanded_matrix_jobs,
        r.effective_tier,
        r.billed_mins_per_run,
        r.gross_compute_cost_per_run_usd,
        r.gross_monthly_compute_cost_usd
    )
}

pub fn render_diff_markdown(d: &DiffReport) -> String {
    render_diff_markdown_for_platform(d, "github")
}

pub fn render_diff_markdown_for_platform(d: &DiffReport, platform: &str) -> String {
    let plat = platform.trim().to_ascii_lowercase();
    let is_gitlab = plat == "gitlab" || plat == "gitlab-ci";
    let is_bitbucket = plat == "bitbucket" || plat == "bitbucket-pipelines";

    let heading_cr = if is_gitlab {
        "Merge Request"
    } else {
        "Pull Request"
    };
    let head_col = if is_gitlab {
        "MR Branch (Head)"
    } else {
        "PR Branch (Head)"
    };
    let cr_short = if is_gitlab { "MR" } else { "PR" };

    let usd_sign = if d.delta_monthly_compute_usd >= 0.0 { "+" } else { "-" };
    let pct_sign = if d.delta_monthly_compute_pct >= 0.0 { "+" } else { "-" };
    let wait_sign = if d.delta_monthly_dev_wait_usd >= 0.0 { "+" } else { "-" };

    let max_visible_changes = 10usize;
    let change_lines = if d.job_changes.is_empty() {
        "- *No workflow or matrix SKU changes detected between base and head.*".to_string()
    } else if d.job_changes.len() <= max_visible_changes || show_full_job_table() {
        d.job_changes
            .iter()
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n")
    } else if is_bitbucket {
        // Bitbucket Cloud PR comments strip raw HTML `<details>`, so keep pure CommonMark
        let mut lines: Vec<String> = d
            .job_changes
            .iter()
            .take(max_visible_changes)
            .map(|c| format!("- {}", c))
            .collect();
        lines.push(format!(
            "- *...and `{}` additional workflow/matrix change(s)*",
            d.job_changes.len() - max_visible_changes
        ));
        lines.join("\n")
    } else {
        let top_part = d
            .job_changes
            .iter()
            .take(max_visible_changes)
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n");
        let rest_part = d
            .job_changes
            .iter()
            .skip(max_visible_changes)
            .map(|c| format!("- {}", c))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{}\n<details>\n<summary>Show remaining <code>{}</code> workflow/matrix change(s)</summary>\n\n{}\n</details>",
            top_part,
            d.job_changes.len() - max_visible_changes,
            rest_part
        )
    };
    let six_comp_md = render_applicable_summary_notes_markdown(&d.head_report);
    let comment_marker = if is_bitbucket {
        "[//]: # (minici-cost-comment)\n"
    } else {
        "<!-- minici-cost-comment -->\n"
    };
    let agent_meta = if is_bitbucket {
        String::new()
    } else {
        build_agent_meta_comment_diff(d, &plat)
    };

    format!(
        "{comment_marker}\
### 💰 MiniCI {heading_cr} CI/CD Cost Diff (`{} µs` Rust Engine)\n\n\
| Metric | Base Branch | {head_col} | Delta (`Before vs. After`) |\n\
| :--- | :--- | :--- | :--- |\n\
| **Expanded Matrix Jobs** | `{} jobs` | **`{} jobs`** | **`{:+} jobs`** |\n\
| **Billed Minutes / Run** | `{:.1} mins` | **`{:.1} mins`** | **`{:+.1} mins / run`** |\n\
| **Free-Tier Quota Burn %** | `{:.1}%` | **`{:.1}%`** | **`{:+.1}%` monthly quota** |\n\
| **Monthly CI Compute Cost** | `${:.2}/mo` | **`${:.2}/mo`** | **`{}${:.2}/mo` (`{}{:.1}%`)** |\n\
| **Monthly Developer Wait-Time (`$`)** | `${:.2}/mo` | **`${:.2}/mo`** | **`{}${:.2}/mo`** |\n\n\
#### 🔍 Workflow & Matrix Cost Drivers Changed in This {cr_short}\n\
{}\n\
{}{agent_meta}",
        d.execution_time_us,
        d.base_expanded_jobs,
        d.head_expanded_jobs,
        d.delta_expanded_jobs,
        d.base_billed_mins_per_run,
        d.head_billed_mins_per_run,
        d.delta_billed_mins_per_run,
        d.base_free_quota_burn_pct,
        d.head_free_quota_burn_pct,
        d.head_free_quota_burn_pct - d.base_free_quota_burn_pct,
        d.base_monthly_compute_usd,
        d.head_monthly_compute_usd,
        usd_sign,
        d.delta_monthly_compute_usd.abs(),
        pct_sign,
        d.delta_monthly_compute_pct.abs(),
        d.base_monthly_dev_wait_usd,
        d.head_monthly_dev_wait_usd,
        wait_sign,
        d.delta_monthly_dev_wait_usd.abs(),
        change_lines,
        six_comp_md
    )
}

fn truncate_slack_header(raw: &str) -> String {
    if raw.chars().count() <= 145 {
        raw.to_string()
    } else {
        let t: String = raw.chars().take(142).collect();
        format!("{}...", t)
    }
}

pub fn render_slack_breakdown_payload(r: &BreakdownReport) -> String {
    let rec = build_contextual_recommendation(r);
    let repo_lbl = short_workflow_label(&r.target_path);
    let hdr = truncate_slack_header(&format!("MiniCI CI/CD FinOps Breakdown — {}", repo_lbl));
    let cr_lbl = if r.detected_providers.len() == 1 && r.detected_providers[0] == "gitlab-ci" {
        "MR"
    } else {
        "PR"
    };
    let top_driver = ranked_job_indices(r)
        .first()
        .and_then(|&i| r.jobs.get(i))
        .map(|j| {
            format!(
                "`{} • {}` (`${:.2}/mo` on `{}`)",
                short_workflow_label(&j.workflow_file),
                j.job_id,
                j.run_compute_cost_usd * r.monthly_runs,
                j.runner_label
            )
        })
        .unwrap_or_else(|| "None (0 active CI jobs)".to_string());

    let anomaly_line = if r.cost_components.spend_anomaly_alert_active {
        format!(
            "\\n*Spend Velocity Alert:* `{:.2}x` 30d baseline (proj EOM `${:.2}/mo`)",
            r.cost_components.burn_velocity_7d_vs_30d_ratio,
            r.cost_components.projected_eom_compute_usd
        )
    } else {
        String::new()
    };
    let prov = r
        .detected_providers
        .first()
        .map(|s| s.as_str())
        .unwrap_or("ci");

    let text = format!(
        "MiniCI FinOps Breakdown ({}): ${:.2}/mo gross (${:.2}/mo net) | {} jobs | ${:.2}/{} | {}",
        repo_lbl,
        r.gross_monthly_compute_cost_usd,
        r.net_billed_monthly_compute_cost_usd,
        r.total_expanded_matrix_jobs,
        r.cost_components.cost_per_merged_pr_usd,
        cr_lbl,
        rec
    );
    format!(
        "{{\n  \"text\": \"{}\",\n  \"blocks\": [\n    {{\n      \"type\": \"header\",\n      \"text\": {{ \"type\": \"plain_text\", \"text\": \"{}\" }}\n    }},\n    {{\n      \"type\": \"section\",\n      \"fields\": [\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Monthly Compute:*\\n`${:.2}/mo` (`${:.2}/mo` net)\" }},\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Unit Economics:*\\n`${:.2}/{}` • `${:.2}/dev`\" }},\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Quota Burn:*\\n`{:.1}%` of `{:.0}m`\" }},\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Dev Wait Cost:*\\n`${:.2}/mo` (`{:.1}m` CP)\" }}\n      ]\n    }},\n    {{\n      \"type\": \"section\",\n      \"text\": {{ \"type\": \"mrkdwn\", \"text\": \"*Top Cost Driver:* {}{}\\n*Recommendation:* {}\" }}\n    }},\n    {{\n      \"type\": \"context\",\n      \"elements\": [\n        {{ \"type\": \"mrkdwn\", \"text\": \"Provider: `{}` • Provenance: `{}` • `{:.2} ms` • Quick fix: `minici shrink --path .`\" }}\n      ]\n    }}\n  ]\n}}",
        json_escape(&text),
        json_escape(&hdr),
        r.gross_monthly_compute_cost_usd,
        r.net_billed_monthly_compute_cost_usd,
        r.cost_components.cost_per_merged_pr_usd,
        cr_lbl,
        r.cost_components.cost_per_active_dev_monthly_usd,
        r.free_tier_quota_burn_pct,
        r.free_tier_included_mins,
        r.monthly_dev_wait_cost_usd,
        r.critical_path_wait_mins_per_run,
        json_escape(&top_driver),
        anomaly_line,
        json_escape(&rec),
        json_escape(prov),
        json_escape(&r.provenance_mode),
        r.execution_time_ms
    )
}

pub fn render_slack_payload(d: &DiffReport) -> String {
    let usd_sign = if d.delta_monthly_compute_usd >= 0.0 { "+" } else { "-" };
    let pct_sign = if d.delta_monthly_compute_pct >= 0.0 { "+" } else { "-" };
    let is_gitlab = d.head_report.detected_providers.len() == 1
        && d.head_report.detected_providers[0] == "gitlab-ci";
    let cr_word = if is_gitlab { "Merge Request" } else { "Pull Request" };
    let hdr = truncate_slack_header(&format!("MiniCI {} CI/CD Cost Diff", cr_word));
    let top_change = d
        .job_changes
        .first()
        .cloned()
        .unwrap_or_else(|| "No workflow or matrix SKU changes detected.".to_string());
    let rec = build_contextual_recommendation(&d.head_report);
    let prov = d
        .head_report
        .detected_providers
        .first()
        .map(|s| s.as_str())
        .unwrap_or("ci");

    let text = format!(
        "MiniCI {} Cost Diff: {}${:.2}/mo ({}{:.1}%) | Jobs: {} -> {} | Dev Wait: ${:.2}/mo",
        cr_word,
        usd_sign,
        d.delta_monthly_compute_usd.abs(),
        pct_sign,
        d.delta_monthly_compute_pct.abs(),
        d.base_expanded_jobs,
        d.head_expanded_jobs,
        d.head_monthly_dev_wait_usd
    );
    format!(
        "{{\n  \"text\": \"{}\",\n  \"blocks\": [\n    {{\n      \"type\": \"header\",\n      \"text\": {{ \"type\": \"plain_text\", \"text\": \"{}\" }}\n    }},\n    {{\n      \"type\": \"section\",\n      \"fields\": [\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Monthly Compute Delta:*\\n`{}${:.2}/mo` (`{}{:.1}%`)\" }},\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Expanded Matrix Jobs:*\\n`{} -> {} jobs` (`{:+} jobs`)\" }},\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Free-Tier Quota Burn:*\\n`{:.1}% -> {:.1}%`\" }},\n        {{ \"type\": \"mrkdwn\", \"text\": \"*Developer Wait Cost:*\\n`${:.2}/mo -> ${:.2}/mo`\" }}\n      ]\n    }},\n    {{\n      \"type\": \"section\",\n      \"text\": {{ \"type\": \"mrkdwn\", \"text\": \"*Key Driver:* {}\\n*Recommendation:* {}\" }}\n    }},\n    {{\n      \"type\": \"context\",\n      \"elements\": [\n        {{ \"type\": \"mrkdwn\", \"text\": \"Base `{}` -> Head `{}` • Provider `{}` • `{} µs`\" }}\n      ]\n    }}\n  ]\n}}",
        json_escape(&text),
        json_escape(&hdr),
        usd_sign,
        d.delta_monthly_compute_usd.abs(),
        pct_sign,
        d.delta_monthly_compute_pct.abs(),
        d.base_expanded_jobs,
        d.head_expanded_jobs,
        d.delta_expanded_jobs,
        d.base_free_quota_burn_pct,
        d.head_free_quota_burn_pct,
        d.base_monthly_dev_wait_usd,
        d.head_monthly_dev_wait_usd,
        json_escape(&top_change),
        json_escape(&rec),
        json_escape(&normalize_ci_path(&d.base_path)),
        json_escape(&normalize_ci_path(&d.head_path)),
        json_escape(prov),
        d.execution_time_us
    )
}

pub fn generate_post_tool_use_launch_pack(feature_name: &str, sample_latency_us: u128) -> String {
    format!(
        "# 🚀 AUTOMATIC `PostToolUse` 6-CHANNEL NATURAL-LANGUAGE LAUNCH & PARTNER PROFILE PACK (`{feature_name}`)\n\n\
> **Founder & Publisher Contact:** MiniCI maintainers (`github.com/minici-dev`)\n\
> **Category Positioning:** **CI/CD with Taste** (`taste-ci-1`) — *\"If we were building CI/CD for software built with AI, MiniCI is what it would look like.\"*\n\
> **Pricing Model:** **Open-Source Core (`$0` CLI + Action + MCP)** • **Team Pro (`$79/mo` flat per GitHub Org — 1-Click Auto-Fix PR Bot + 30-Day Drift Memory + Weekly Slack CFO Digest)** • **Enterprise (`Custom`)**\n\
> **Empirically Measured Portable Rust Benchmark:** `{sample_latency_us} µs` (`{:.3} ms`) on 1–2 CPU threads • Zero External Dependencies\n\n\
---\n\n\
## 1. Y Combinator (YC) Application — Product & Technical Moat Profile\n\
- **Founder:** MiniCI maintainers (`github.com/minici-dev`)\n\
- **Company / Project:** MiniCI — **Deterministic CI/CD FinOps & Workflow Economics**\n\
- **50-Character Description:** Sub-ms Rust CI/CD cost breakdown & PR diff engine.\n\
- **What we built (`{feature_name}`):** AI coding agents push 4x more commits per day, silently expanding CI matrices and burning runner budgets. We built `minici` 100% in **Rust** (`{sample_latency_us} µs` execution, zero runtime dependencies). Every `git push` or PR gets an instant `< 1 ms` receipt showing **Free-Tier Quota Burn %**, **Developer Wait-Time Dollars (`$65/hr` default, 40% DORA context-switch loss)**, **Billed Compute Cost (with GitHub's 1-minute per-job ceiling rounding and macOS 10x multiplier)**, **Artifact Storage Optimization (`$0.25/GB-mo` vs `$0.023` S3)**, and **Self-Hosted Spot Break-Even (`5,000 mins/mo`)**.\n\n\
## 2. Product Hunt — Maker Launch Listing\n\
- **Name:** MiniCI — Sub-Millisecond CI/CD Cost & PR Diff Engine\n\
- **Maker Contact:** MiniCI maintainers (`github.com/minici-dev`)\n\
- **Tagline:** Catch expensive Pull Request CI matrix spikes in `< 1ms` with zero-dependency Rust.\n\
- **Maker Comment (Natural Voice):** Hey Product Hunt! AI coding agents are shipping code faster than ever, but CI/CD hasn't caught up—one tiny `strategy.matrix` edit in a PR can silently multiply your GitHub Actions or GitLab bill by 10x. I built `minici` in pure Rust (`{sample_latency_us} µs` execution, zero dependencies): it shows your exact Free Quota Burn %, Developer Wait-Time cost, and Per-PR Compute Invoice before merge.\n\n\
## 3. Reddit (`r/devops`, `r/rust`, `r/sre`) Technical Post (Natural Engineer Voice)\n\
- **Title:** `I built a zero-dependency Rust CLI that expands GitHub/GitLab/CircleCI workflow matrices and calculates PR cost diffs in {sample_latency_us} µs`\n\
- **Body:** GitHub Actions and GitLab CI only show you a lump-sum bill at the end of the month—they never tell you which PR or `strategy.matrix` change spiked your runner bill or burned your 2,000 free minutes. I wrote `minici` in pure Rust with an embedded zero-copy SKU catalog (supporting 1-minute per-job ceiling rounding, macOS 10x multipliers, `$0.25/GB-mo` artifact storage overage, developer wait-time cost, and the 5,000 min/mo self-hosted break-even threshold). Always exits `0` so it never blocks a build. Feedback welcome: [GitHub Issues](https://github.com/minici-dev/minici-cost-action/issues).\n\n\
## 4. LinkedIn — Founder Build-in-Public Post (Natural Staff SRE Voice)\n\
- **Post:** AI coding agents expanded PR volume 4x, and one `strategy.matrix` change can silently multiply your GitHub Actions bill overnight. So I built **MiniCI (`minici`)** in pure Rust. In **{sample_latency_us} microseconds** (`< 1 ms`), it expands your CI matrices and posts the exact Per-PR & Monthly Cost Diff, Free-Tier Quota Burn %, and Developer Wait-Time cost right on the PR or local `pre-push` hook. Open-source CLI, Action, and MCP server (`Apache-2.0`). Reach out anytime via [GitHub Issues](https://github.com/minici-dev/minici-cost-action/issues).\n\n\
## 5. AI Coding Agent & IDE (`PreToolUse` / `PostToolUse` / MCP) Profile\n\
- **Integration Name:** `minici` — Sub-Millisecond CI/CD with Taste Guard for AI Coding Agents\n\
- **Maintainer:** MiniCI maintainers (`github.com/minici-dev`)\n\
- **Hook Specification:** Plugs directly into IDE & Git hooks (`PreToolUse` / `PostToolUse` on `git push` or `.github/workflows/*.yml` edits) in `{sample_latency_us} µs` with `exit 0` informational receipts so coding agents respect your repo's CI budget (`taste-ci-1`) before pushing.\n\n\
## 6. Mozilla Builders / Open-Source AI & DevTools Grant Profile\n\
- **Applicant:** MiniCI maintainers (`github.com/minici-dev`)\n\
- **Project Summary:** Open-source, zero-telemetry-egress Rust binary (`Apache-2.0`) that brings `CI/CD with Taste` locally to any developer machine or cheap 1–2 vCPU container without sending proprietary source code to third-party servers.\n",
        (sample_latency_us as f64) / 1000.0
    )
}
