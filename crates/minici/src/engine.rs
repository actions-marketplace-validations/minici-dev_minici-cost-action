//! Universal Combinatorial CI/CD DAG, Matrix & Cost Engine (`crates/minici/src/engine.rs`).
//!
//! Pure Cost Domain Engine (`Deterministic CI/CD FinOps` — Zero External Crate Dependencies):
//! 1. Parses any repository's CI/CD workflows across GitHub Actions (`.github/workflows/*.yml`),
//!    GitLab CI (`.gitlab-ci.yml`), CircleCI (`.circleci/config.yml`), Bitbucket (`bitbucket-pipelines.yml`),
//!    and AWS CodeBuild (`buildspec.yml`).
//! 2. Expands Cartesian `strategy.matrix` (`OS × Runtime × Version`), `include` additions, `exclude` filters,
//!    and GitLab `parallel: matrix:` combinations in microseconds.
//! 3. Applies Official Vendor Billing Rules (including GitHub Actions' 1-minute ceiling rounding per job!).
//! 4. Calculates BOTH:
//!    - **Free-Tier User Metrics:** Free Quota Burn % (`2,000` GH / `400` GL / `3,000` CircleCI) + Developer Wait-Time Engineering Cost (`$` at `$65/hr` default).
//!    - **Paid-Tier / FinOps Metrics:** Billed Runner Compute Cost (`$`), Hidden Artifact Storage Markup (`$0.25/GB` vs `$0.023` S3),
//!      and the `5,000 min/mo` Self-Hosted / Blacksmith Break-Even analysis.

use crate::catalog::{
    infracost_aws_nat_gateway_rate_usd, infracost_ec2_runner_profile, resolve_runner_sku,
    stack_cache_savings_profile, vendor_cache_mechanics,
    ARTIFACT_STORAGE_S3_INTELLIGENT_USD_PER_GB_MO, ARTIFACT_STORAGE_S3_USD_PER_GB_MO,
    ARTIFACT_STORAGE_VENDOR_USD_PER_GB_MO, AWS_CROSS_AZ_USD_PER_GB, AWS_CROSS_REGION_USD_PER_GB,
    AWS_INTERNET_EGRESS_USD_PER_GB, AWS_VPC_ENDPOINT_USD_PER_GB,
    SELF_HOSTED_BREAKEVEN_MINS_PER_MONTH, SELF_HOSTED_C5_4XLARGE_SPOT_BREAKEVEN_MINS,
    SELF_HOSTED_VPS_BASE_MONTHLY_USD,
};
use crate::providers::{
    compute_5layer_platform_switching_tco, is_github_larger_runner_job, select_provider_plugin,
    RepoVisibility,
};
use crate::telemetry::CanonicalTelemetrySnapshot;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct UsageConfig {
    pub tier: String, // "auto", "free", "paid"
    pub prs_per_week: f64,
    pub pushes_per_pr: f64,
    pub dev_hourly_rate_usd: f64,
    pub artifact_gb_per_run: f64,
    pub artifact_retention_days: u32,
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self {
            tier: "auto".to_string(),
            prs_per_week: 25.0,
            pushes_per_pr: 4.0,
            dev_hourly_rate_usd: 65.0,
            artifact_gb_per_run: 0.25,
            artifact_retention_days: 30,
        }
    }
}

impl UsageConfig {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"tier\":\"{}\",\"prs_per_week\":{:.1},\"pushes_per_pr\":{:.1},\"dev_hourly_rate_usd\":{:.2},\"artifact_gb_per_run\":{:.2},\"artifact_retention_days\":{}}}",
            json_escape(&self.tier),
            self.prs_per_week,
            self.pushes_per_pr,
            self.dev_hourly_rate_usd,
            self.artifact_gb_per_run,
            self.artifact_retention_days
        )
    }
}

pub fn normalize_ci_path(raw: &str) -> String {
    raw.replace('\\', "/")
}

#[derive(Debug, Clone)]
pub struct ExpandedJobCost {
    pub workflow_file: String,
    pub job_id: String,
    pub line_start: usize,
    pub line_end: usize,
    pub matrix_combination_count: usize,
    pub runner_label: String,
    pub provider: String,
    pub os: String,
    pub vcpus: u32,
    pub est_raw_duration_mins_per_job: f64,
    pub billed_mins_per_job: f64,
    pub total_billed_mins_all_matrix: f64,
    pub rate_per_min_usd: f64,
    pub run_compute_cost_usd: f64,
    pub blacksmith_run_cost_usd: f64,
    pub self_hosted_spot_run_cost_usd: f64,
    pub has_concurrency_cancel: bool,
    pub has_timeout_minutes: bool,
    pub has_sparse_or_shallow_clone: bool,
    pub uses_cache: bool,
    pub uploads_artifacts: bool,
    pub artifact_retention_days: Option<u32>,
    pub monthly_runs_for_job: Option<f64>,
}

impl ExpandedJobCost {
    pub fn effective_monthly_runs(&self, fallback_monthly_runs: f64) -> f64 {
        self.monthly_runs_for_job
            .unwrap_or(fallback_monthly_runs)
            .max(0.0)
    }

    pub fn monthly_compute_cost(&self, fallback_monthly_runs: f64) -> f64 {
        norm_zero(
            ((self.run_compute_cost_usd * self.effective_monthly_runs(fallback_monthly_runs))
                * 100.0)
                .round()
                / 100.0,
        )
    }

    pub fn to_json_with_monthly(&self, apportioned_monthly_usd: Option<f64>) -> String {
        let ret_json = self
            .artifact_retention_days
            .map(|d| d.to_string())
            .unwrap_or_else(|| "null".to_string());
        let runs_json = self
            .monthly_runs_for_job
            .map(|r| format!("{:.2}", r))
            .unwrap_or_else(|| "null".to_string());
        let mo_cost_json = apportioned_monthly_usd
            .or_else(|| {
                self.monthly_runs_for_job
                    .map(|r| norm_zero(((self.run_compute_cost_usd * r.max(0.0)) * 100.0).round() / 100.0))
            })
            .map(|v| format!("{:.2}", norm_zero(v)))
            .unwrap_or_else(|| "null".to_string());
        let norm_wf = normalize_ci_path(&self.workflow_file);
        let ls = self.line_start.max(1);
        let le = self.line_end.max(ls);
        format!(
            "{{\"workflow_file\":\"{}\",\"job_id\":\"{}\",\"line_start\":{},\"line_end\":{},\"matrix_combination_count\":{},\"runner_label\":\"{}\",\"provider\":\"{}\",\"os\":\"{}\",\"vcpus\":{},\"est_raw_duration_mins_per_job\":{:.2},\"billed_mins_per_job\":{:.2},\"total_billed_mins_all_matrix\":{:.2},\"rate_per_min_usd\":{:.6},\"run_compute_cost_usd\":{:.4},\"monthly_runs_for_job\":{},\"monthly_compute_cost_usd\":{},\"high_speed_microvm_run_cost_usd\":{:.4},\"self_hosted_spot_run_cost_usd\":{:.4},\"has_concurrency_cancel\":{},\"has_timeout_minutes\":{},\"has_sparse_or_shallow_clone\":{},\"uses_cache\":{},\"uploads_artifacts\":{},\"artifact_retention_days\":{}}}",
            json_escape(&norm_wf),
            json_escape(&self.job_id),
            ls,
            le,
            self.matrix_combination_count,
            json_escape(&self.runner_label),
            json_escape(&self.provider),
            json_escape(&self.os),
            self.vcpus,
            self.est_raw_duration_mins_per_job,
            self.billed_mins_per_job,
            self.total_billed_mins_all_matrix,
            self.rate_per_min_usd,
            self.run_compute_cost_usd,
            runs_json,
            mo_cost_json,
            self.blacksmith_run_cost_usd,
            self.self_hosted_spot_run_cost_usd,
            self.has_concurrency_cancel,
            self.has_timeout_minutes,
            self.has_sparse_or_shallow_clone,
            self.uses_cache,
            self.uploads_artifacts,
            ret_json
        )
    }

    pub fn to_json(&self) -> String {
        self.to_json_with_monthly(None)
    }
}

#[derive(Debug, Clone)]
pub struct CostComponentsBreakdown {
    // 1. Build Minutes Cost (published cloud & vendor rate cards)
    pub rounding_tax_mins_per_run: f64,
    pub monthly_rounding_tax_usd: f64,
    pub linux_mins_per_run: f64,
    pub windows_mins_per_run: f64,
    pub macos_mins_per_run: f64,
    pub os_weighted_free_tier_mins_per_run: f64,
    pub monthly_os_weighted_free_tier_mins: f64,
    pub monthly_flake_retry_10pct_overhead_usd: f64,
    // 2. CI Runner Cost — Cloud Compute Catalog (published cloud & vendor rate cards)
    pub infracost_iac_files_detected: bool,
    pub hosted_effective_hourly_rate_usd: f64,
    pub infracost_recommended_ec2_instance: String,
    pub infracost_ec2_on_demand_hourly_usd: f64,
    pub infracost_ec2_on_demand_monthly_usd: f64,
    pub infracost_ec2_spot_hourly_usd: f64,
    pub infracost_ec2_surrounding_overhead_monthly_usd: f64,
    pub macos_self_hosted_mac_mini_monthly_usd: f64,
    pub self_hosted_breakeven_mins_t3_medium: f64,
    pub self_hosted_breakeven_mins_c5_4xlarge_spot: f64,
    pub always_on_runner_utilization_pct: f64,
    pub ephemeral_runner_recommended_mode: String,
    pub ephemeral_cold_start_wait_tax_monthly_usd: f64,
    pub k8s_eks_control_plane_monthly_usd: f64,
    pub k8s_gke_aks_free_tier_control_plane_monthly_usd: f64,
    pub spot_interruption_band_pct: String,
    pub spot_retry_overhead_monthly_usd: f64,
    pub spot_ineligible_jobs_count: usize,
    pub hybrid_spot_plus_on_demand_monthly_usd: f64,
    // 3. Artifact Storage Cost — Cloud Catalog S3/ECR Delegated (published cloud & vendor rate cards)
    pub jobs_uploading_artifacts: usize,
    pub default_retention_days: u32,
    pub configured_retention_days: u32,
    pub steady_state_stored_gb_configured: f64,
    pub monthly_vendor_storage_configured_usd: f64,
    pub net_billed_vendor_storage_configured_usd: f64,
    pub monthly_infracost_s3_configured_usd: f64,
    pub steady_state_stored_gb_90d: f64,
    pub steady_state_stored_gb_14d: f64,
    pub monthly_vendor_storage_90d_usd: f64,
    pub monthly_vendor_storage_14d_usd: f64,
    pub monthly_infracost_s3_standard_90d_usd: f64,
    pub monthly_infracost_s3_intelligent_14d_usd: f64,
    // 4. Parallelism & Matrix Cost Strategy
    pub peak_concurrent_jobs_per_run: usize,
    pub hosted_concurrent_jobs_per_run: usize,
    pub macos_concurrent_jobs_per_run: usize,
    pub self_hosted_or_microvm_jobs_per_run: usize,
    pub vendor_free_concurrent_cap: usize,
    pub vendor_team_concurrent_cap: usize,
    pub parallelism_tier_upgrade_tax_monthly_usd: f64,
    pub detected_repo_matrix_profile: String,
    pub matrix_jobs_count: usize,
    pub pr_unconditional_matrix_cells: usize,
    pub conditional_pr_matrix_monthly_savings_usd: f64,
    // 5. Log Retention Cost — Cloud Catalog CloudWatch/S3 Delegated (published cloud & vendor rate cards)
    pub detected_log_language_profile: String,
    pub est_log_mb_per_run: f64,
    pub monthly_log_volume_gb: f64,
    pub vendor_default_log_retention_days: u32,
    pub monthly_vendor_default_log_cost_usd: f64,
    pub monthly_datadog_30d_log_shipping_usd: f64,
    pub monthly_datadog_90d_log_shipping_usd: f64,
    pub monthly_splunk_30d_log_shipping_usd: f64,
    pub monthly_infracost_loki_s3_usd: f64,
    // 6. Cross-Region Egress Cost
    pub infracost_cloud_region: String,
    pub est_transfer_gb_per_run: f64,
    pub monthly_transfer_gb: f64,
    pub infracost_aws_internet_egress_per_gb_usd: f64,
    pub infracost_aws_nat_gateway_per_gb_usd: f64,
    pub infracost_aws_cross_az_per_gb_usd: f64,
    pub infracost_aws_cross_region_per_gb_usd: f64,
    pub monthly_infracost_private_subnet_nat_worst_case_usd: f64,
    pub monthly_infracost_cross_region_registry_usd: f64,
    pub monthly_infracost_vpc_endpoint_cache_optimized_usd: f64,
    // 7. Developer Wait Time Cost (Payroll & Context-Switch Loss)
    pub productivity_loss_factor: f64,
    pub blended_fully_loaded_hourly_usd: f64,
    pub monthly_wait_hours: f64,
    pub monthly_productive_payroll_loss_usd: f64,
    pub monthly_blended_100hr_productive_loss_usd: f64,
    pub wait_to_compute_cost_ratio: f64,
    pub lead_time_feedback_tier: String,
    // 8. Failed Build Rerun Cost (Flake Rerun Compute + Investigation Payroll)
    pub estimated_flake_rate_pct: f64,
    pub has_job_level_auto_retry: bool,
    pub monthly_flaky_reruns: f64,
    pub monthly_rerun_compute_cost_usd: f64,
    pub avg_investigation_mins_per_flake: f64,
    pub monthly_flake_investigation_hours: f64,
    pub monthly_flake_investigation_payroll_usd: f64,
    pub monthly_total_immediate_flake_cost_usd: f64,
    pub monthly_savings_at_1pct_target_usd: f64,
    // 9. Build Cache Savings (Per-Stack Realistic Savings & Cache Hit-Rate KPI)
    pub stack_cache_profile: String,
    pub realistic_saving_pct_min: f64,
    pub realistic_saving_pct_max: f64,
    pub jobs_with_cache: usize,
    pub jobs_missing_cache: usize,
    pub has_restore_keys_fallback: bool,
    pub has_docker_buildkit_cache: bool,
    pub estimated_cache_hit_rate_pct: f64,
    pub vendor_cache_size_limit: String,
    pub vendor_cache_eviction_policy: String,
    pub monthly_uncached_compute_usd: f64,
    pub monthly_unrealized_cache_savings_usd: f64,
    pub next_recommended_caching_layer: String,
    pub distributed_remote_cache_monthly_infra_usd: f64,
    pub container_base_image_mirror_monthly_savings_usd: f64,
    pub pr_cache_poisoning_risk_jobs: usize,
    // 10. Flaky Test CI/CD Tax (Immediate + Deferred Velocity Drag & Deflake ROI)
    pub deferred_velocity_drag_multiplier: f64,
    pub monthly_deferred_velocity_drag_usd: f64,
    pub monthly_combined_flake_tax_usd: f64,
    pub deflake_project_est_cost_usd: f64,
    pub deflake_payback_weeks: f64,
    // 11. Branch Protection Cost (Required-Check Multiplication, Concurrency Cancel & Path Filters)
    pub required_checks_count: usize,
    pub monthly_required_check_executions: usize,
    pub workflows_with_concurrency_cancel: usize,
    pub workflows_missing_concurrency_cancel: usize,
    pub workflows_with_path_filters: usize,
    pub workflows_missing_path_filters: usize,
    pub has_affected_only_graph_tool: bool,
    pub slow_blocking_checks_over_10m: usize,
    pub monthly_superseded_run_waste_usd: f64,
    pub monthly_unfiltered_check_savings_usd: f64,
    // 12. Startup Stage Cost (1-15 Devs, Free-Tier Burn, Upgrade Triggers & Early Traps)
    pub inferred_team_stage: String,
    pub implied_active_devs: usize,
    pub free_tier_burn_pct: f64,
    pub upgrade_trigger_80pct_reached: bool,
    pub premature_self_hosting_trap: bool,
    pub premature_multi_os_pr_matrix_trap: bool,
    pub monthly_multi_os_pr_to_nightly_savings_usd: f64,
    pub mac_mini_hardware_payback_months: f64,
    // 13. Scaleup Stage Cost (25-100 Devs, Tier-Crisis Decisions, Required-Check Audit & Platform Eng ROI)
    pub self_hosted_breakeven_reached: bool,
    pub team_to_enterprise_seat_jump_monthly_usd: f64,
    pub quarterly_required_check_audit_savings_usd: f64,
    pub platform_engineer_monthly_fully_loaded_usd: f64,
    pub recoverable_ci_and_wait_monthly_usd: f64,
    pub platform_engineer_roi_coverage_pct: f64,
    // 14. Enterprise Stage Cost (250+ Devs, Compliance Overhead, Regional Residency & Dedicated Tenancy)
    pub detected_compliance_controls: Vec<String>,
    pub compliance_controls_count: usize,
    pub detected_cloud_regions: Vec<String>,
    pub regional_residency_multiplier: f64,
    pub enterprise_250_seat_licence_monthly_usd: f64,
    pub siem_1yr_audit_log_retention_monthly_usd: f64,
    pub dedicated_tenancy_premium_pct_min: f64,
    pub dedicated_tenancy_premium_pct_max: f64,
    // 15. Monorepo CI Cost (Dynamic Workspace Detection, Unconstrained Blast Radius & Affected-Only Fix)
    pub detected_pipeline_shape: String,
    pub detected_workspace_units_count: usize,
    pub detected_monorepo_tool: String,
    pub recommended_affected_tool: String,
    pub has_nightly_full_suite_safety_net: bool,
    pub affected_only_reduction_pct_min: f64,
    pub affected_only_reduction_pct_max: f64,
    pub monthly_affected_only_savings_usd: f64,
    // 16. Microservices CI Cost (Per-Service Setup Overhead, Reusable Templates, Pinning & Shared Cache)
    pub est_setup_overhead_sec_per_job: f64,
    pub monthly_setup_overhead_hours: f64,
    pub setup_overhead_share_pct: f64,
    pub monthly_setup_overhead_compute_usd: f64,
    pub monthly_shared_infra_setup_savings_usd: f64,
    pub reusable_workflow_calls_count: usize,
    pub unpinned_template_refs_count: usize,
    pub cross_repo_triggers_count: usize,
    pub est_aggregate_cache_gb: f64,
    pub exceeds_10gb_repo_cache_limit: bool,
    // 17. Multi-Vendor TCO & Migration Comparison Engine (Seats + Included Quota + Overage + Migration Payback)
    pub cheapest_vendor_platform: String,
    pub cheapest_vendor_monthly_tco_usd: f64,
    pub github_actions_team_monthly_tco_usd: f64,
    pub gitlab_ci_premium_monthly_tco_usd: f64,
    pub circleci_performance_monthly_tco_usd: f64,
    pub bitbucket_premium_monthly_tco_usd: f64,
    pub buildkite_pro_monthly_tco_usd: f64,
    pub jenkins_self_hosted_monthly_tco_usd: f64,
    pub aws_codebuild_monthly_tco_usd: f64,
    pub gcp_cloud_build_monthly_tco_usd: f64,
    pub azure_pipelines_monthly_tco_usd: f64,
    pub migration_estimated_hours: f64,
    pub migration_estimated_cost_usd: f64,
    pub migration_payback_months: f64,
    pub migration_worth_it_2x_annual_rule: bool,
    pub migration_verdict: String,
    // 18. 6-Phase Build-Minute Time-Share & Best-Payback Optimization Ranker
    pub phase_clone_checkout_share_pct: f64,
    pub phase_dependency_install_share_pct: f64,
    pub phase_compile_typecheck_share_pct: f64,
    pub phase_test_suite_share_pct: f64,
    pub phase_docker_build_push_share_pct: f64,
    pub phase_deploy_cleanup_share_pct: f64,
    pub monthly_total_reducible_phase_savings_usd: f64,
    pub best_first_optimization_phase: String,
    pub best_first_optimization_effort_hours: f64,
    pub best_first_optimization_monthly_savings_usd: f64,
    pub best_first_optimization_payback_months: f64,
    // 19. Compute-Class Right-Sizing Explorer (Amdahl Speedup vs Larger-Runner Billing & Dev-Wait Net ROI)
    pub detected_workload_compute_profile: String,
    pub larger_runners_detected_count: usize,
    pub larger_runners_bypass_free_minutes_tax_usd: f64,
    pub arm64_2core_monthly_compute_usd: f64,
    pub arm64_migration_monthly_savings_usd: f64,
    pub standard_2core_monthly_compute_usd: f64,
    pub larger_4core_monthly_compute_usd: f64,
    pub larger_8core_monthly_compute_usd: f64,
    pub larger_16core_monthly_compute_usd: f64,
    pub best_net_roi_runner_tier: String,
    pub best_net_roi_monthly_delta_usd: f64,
    // 20. Language-Stack Deep-Dive Intelligence (iOS / Android / JVM / Rust / Node / Python / Go / IaC)
    pub detected_stack_archetype: String,
    pub ios_codemagic_annual_m2_monthly_usd: f64,
    pub ios_buildkite_mac_mini_fleet_monthly_tco_usd: f64,
    pub android_emulator_jobs_count: usize,
    pub android_kvm_or_gradle_cache_savings_usd: f64,
    pub jvm_warm_cache_50pct_savings_usd: f64,
    pub stack_specific_monthly_savings_usd: f64,
    pub stack_specific_primary_lever: String,
    // 21. Unit Economics & Team Chargeback (`$/PR`, `$/Deploy`, `$/Active Dev`, Cadence-Weighted Spend & CODEOWNERS)
    pub cost_per_merged_pr_usd: f64,
    pub cost_per_deploy_usd: f64,
    pub cost_per_active_dev_monthly_usd: f64,
    pub cadence_weighted_monthly_compute_usd: f64,
    pub top_workflow_cost_share_pct: f64,
    pub codeowners_detected: bool,
    pub top_codeowners_team: String,
    // 22. Spend Anomaly, Quota Forecast & Hung-Job Exposure (`7d/30d` Velocity, EOM Forecast & Missing `timeout-minutes`)
    pub burn_velocity_7d_vs_30d_ratio: f64,
    pub spend_anomaly_alert_active: bool,
    pub projected_eom_compute_usd: f64,
    pub projected_quota_exhaustion_day: u32,
    pub jobs_missing_timeout_minutes_count: usize,
    pub hung_job_360m_worst_case_exposure_usd: f64,
    // 23. Container Registry & Package Storage Economics (`GHCR / ECR / GCR` Retention & Untagged Layer Cleanup)
    pub container_images_built_count: usize,
    pub est_registry_storage_gb: f64,
    pub monthly_registry_storage_and_pull_usd: f64,
    pub untagged_image_cleanup_monthly_savings_usd: f64,
    // 24. Self-Hosted Runner Idle Waste & Queue SLA (`Off-Hours` Idle Capacity & `P95` Queue Wait Breach Risk)
    pub off_hours_idle_capacity_pct: f64,
    pub monthly_always_on_idle_waste_usd: f64,
    pub est_p95_queue_wait_sec: f64,
    pub sla_queue_breach_risk: bool,
}

impl CostComponentsBreakdown {
    pub fn to_json(&self) -> String {
        let compliance_json = self
            .detected_compliance_controls
            .iter()
            .map(|c| format!("\"{}\"", json_escape(c)))
            .collect::<Vec<_>>()
            .join(",");
        let regions_json = self
            .detected_cloud_regions
            .iter()
            .map(|r| format!("\"{}\"", json_escape(r)))
            .collect::<Vec<_>>()
            .join(",");
        let models_21_to_24_json = format!(
            "\"unit_economics_and_chargeback\": {{\"cost_per_merged_pr_usd\":{:.2},\"cost_per_deploy_usd\":{:.2},\"cost_per_active_dev_monthly_usd\":{:.2},\"cadence_weighted_monthly_compute_usd\":{:.2},\"top_workflow_cost_share_pct\":{:.1},\"codeowners_detected\":{},\"top_codeowners_team\":\"{}\"}},\n    \"spend_anomaly_and_forecast\": {{\"burn_velocity_7d_vs_30d_ratio\":{:.2},\"spend_anomaly_alert_active\":{},\"projected_eom_compute_usd\":{:.2},\"projected_quota_exhaustion_day\":{},\"jobs_missing_timeout_minutes_count\":{},\"hung_job_360m_worst_case_exposure_usd\":{:.2}}},\n    \"container_registry_economics\": {{\"container_images_built_count\":{},\"est_registry_storage_gb\":{:.2},\"monthly_registry_storage_and_pull_usd\":{:.2},\"untagged_image_cleanup_monthly_savings_usd\":{:.2}}},\n    \"self_hosted_fleet_idle_and_sla\": {{\"off_hours_idle_capacity_pct\":{:.1},\"monthly_always_on_idle_waste_usd\":{:.2},\"est_p95_queue_wait_sec\":{:.1},\"sla_queue_breach_risk\":{}}}",
            self.cost_per_merged_pr_usd,
            self.cost_per_deploy_usd,
            self.cost_per_active_dev_monthly_usd,
            self.cadence_weighted_monthly_compute_usd,
            self.top_workflow_cost_share_pct,
            self.codeowners_detected,
            json_escape(&self.top_codeowners_team),
            self.burn_velocity_7d_vs_30d_ratio,
            self.spend_anomaly_alert_active,
            self.projected_eom_compute_usd,
            self.projected_quota_exhaustion_day,
            self.jobs_missing_timeout_minutes_count,
            if self.hung_job_360m_worst_case_exposure_usd.abs() < 0.0005 {
                0.0
            } else {
                self.hung_job_360m_worst_case_exposure_usd
            },
            self.container_images_built_count,
            self.est_registry_storage_gb,
            self.monthly_registry_storage_and_pull_usd,
            self.untagged_image_cleanup_monthly_savings_usd,
            self.off_hours_idle_capacity_pct,
            self.monthly_always_on_idle_waste_usd,
            self.est_p95_queue_wait_sec,
            self.sla_queue_breach_risk,
        );
        format!(
            "{{\n    \"build_minutes_cost\": {{\"rounding_tax_mins_per_run\":{:.2},\"monthly_rounding_tax_usd\":{:.2},\"linux_mins_per_run\":{:.2},\"windows_mins_per_run\":{:.2},\"macos_mins_per_run\":{:.2},\"os_weighted_free_tier_mins_per_run\":{:.2},\"monthly_os_weighted_free_tier_mins\":{:.2},\"monthly_flake_retry_10pct_overhead_usd\":{:.2}}},\n    \"runner_cost\": {{\"cloud_catalog_delegated\":true,\"iac_files_detected\":{},\"cloud_resources\":[\"aws_instance\",\"aws_ebs_volume\",\"google_compute_instance\",\"azurerm_linux_virtual_machine\"],\"hosted_effective_hourly_rate_usd\":{:.2},\"recommended_cloud_instance\":\"{}\",\"cloud_vm_on_demand_hourly_usd\":{:.4},\"cloud_vm_on_demand_monthly_usd\":{:.2},\"cloud_vm_spot_hourly_usd\":{:.3},\"cloud_vm_surrounding_overhead_monthly_usd\":{:.2},\"macos_self_hosted_mac_mini_monthly_usd\":{:.2},\"self_hosted_breakeven_mins_t3_medium\":{:.0},\"self_hosted_breakeven_mins_c5_4xlarge_spot\":{:.0},\"always_on_runner_utilization_pct\":{:.1},\"ephemeral_runner_recommended_mode\":\"{}\",\"ephemeral_cold_start_wait_tax_monthly_usd\":{:.2},\"k8s_eks_control_plane_monthly_usd\":{:.2},\"k8s_gke_aks_free_tier_control_plane_monthly_usd\":{:.2},\"spot_interruption_band_pct\":\"{}\",\"spot_retry_overhead_monthly_usd\":{:.2},\"spot_ineligible_jobs_count\":{},\"hybrid_spot_plus_on_demand_monthly_usd\":{:.2}}},\n    \"artifact_storage_cost\": {{\"cloud_catalog_delegated\":true,\"cloud_resources\":[\"aws_s3_bucket\",\"aws_ecr_repository\"],\"jobs_uploading_artifacts\":{},\"default_retention_days\":{},\"configured_retention_days\":{},\"steady_state_stored_gb_configured\":{:.2},\"monthly_vendor_storage_configured_usd\":{:.2},\"net_billed_vendor_storage_configured_usd\":{:.2},\"monthly_s3_configured_usd\":{:.2},\"steady_state_stored_gb_90d\":{:.2},\"steady_state_stored_gb_14d\":{:.2},\"monthly_vendor_storage_90d_usd\":{:.2},\"monthly_vendor_storage_14d_usd\":{:.2},\"monthly_s3_standard_90d_usd\":{:.2},\"monthly_s3_intelligent_14d_usd\":{:.2}}},\n    \"parallelism_cost\": {{\"peak_concurrent_jobs_per_run\":{},\"hosted_concurrent_jobs_per_run\":{},\"macos_concurrent_jobs_per_run\":{},\"self_hosted_or_microvm_jobs_per_run\":{},\"vendor_free_concurrent_cap\":{},\"vendor_team_concurrent_cap\":{},\"parallelism_tier_upgrade_tax_monthly_usd\":{:.2},\"detected_repo_matrix_profile\":\"{}\",\"matrix_jobs_count\":{},\"pr_unconditional_matrix_cells\":{},\"conditional_pr_matrix_monthly_savings_usd\":{:.2}}},\n    \"log_retention_cost\": {{\"cloud_catalog_delegated\":true,\"cloud_resources\":[\"aws_cloudwatch_log_group\",\"aws_s3_bucket\"],\"detected_log_language_profile\":\"{}\",\"est_log_mb_per_run\":{:.2},\"monthly_log_volume_gb\":{:.2},\"vendor_default_log_retention_days\":{},\"monthly_vendor_default_log_cost_usd\":{:.2},\"monthly_external_siem_30d_log_shipping_usd\":{:.2},\"monthly_external_siem_90d_log_shipping_usd\":{:.2},\"monthly_enterprise_siem_30d_log_shipping_usd\":{:.2},\"monthly_s3_archive_log_usd\":{:.2}}},\n    \"cross_region_egress_cost\": {{\"cloud_catalog_delegated\":true,\"cloud_resources\":[\"aws_nat_gateway\",\"aws_vpc_endpoint\",\"google_compute_router_nat\",\"azurerm_nat_gateway\"],\"cloud_region\":\"{}\",\"est_transfer_gb_per_run\":{:.2},\"monthly_transfer_gb\":{:.2},\"aws_internet_egress_per_gb_usd\":{:.3},\"aws_nat_gateway_per_gb_usd\":{:.3},\"aws_cross_az_per_gb_usd\":{:.3},\"aws_cross_region_per_gb_usd\":{:.3},\"monthly_private_subnet_nat_worst_case_usd\":{:.2},\"monthly_cross_region_registry_usd\":{:.2},\"monthly_vpc_endpoint_cache_optimized_usd\":{:.2}}},\n    \"developer_wait_time_cost\": {{\"productivity_loss_factor\":{:.2},\"blended_fully_loaded_hourly_usd\":{:.2},\"monthly_wait_hours\":{:.2},\"monthly_productive_payroll_loss_usd\":{:.2},\"monthly_blended_100hr_productive_loss_usd\":{:.2},\"wait_to_compute_cost_ratio\":{:.2},\"lead_time_feedback_tier\":\"{}\"}},\n    \"failed_build_rerun_cost\": {{\"estimated_flake_rate_pct\":{:.1},\"has_job_level_auto_retry\":{},\"monthly_flaky_reruns\":{:.2},\"monthly_rerun_compute_cost_usd\":{:.2},\"avg_investigation_mins_per_flake\":{:.1},\"monthly_flake_investigation_hours\":{:.2},\"monthly_flake_investigation_payroll_usd\":{:.2},\"monthly_total_immediate_flake_cost_usd\":{:.2},\"monthly_savings_at_1pct_target_usd\":{:.2}}},\n    \"build_cache_savings\": {{\"stack_cache_profile\":\"{}\",\"realistic_saving_pct_min\":{:.1},\"realistic_saving_pct_max\":{:.1},\"jobs_with_cache\":{},\"jobs_missing_cache\":{},\"has_restore_keys_fallback\":{},\"has_docker_buildkit_cache\":{},\"estimated_cache_hit_rate_pct\":{:.1},\"vendor_cache_size_limit\":\"{}\",\"vendor_cache_eviction_policy\":\"{}\",\"monthly_uncached_compute_usd\":{:.2},\"monthly_unrealized_cache_savings_usd\":{:.2},\"next_recommended_caching_layer\":\"{}\",\"distributed_remote_cache_monthly_infra_usd\":{:.2},\"container_base_image_mirror_monthly_savings_usd\":{:.2},\"pr_cache_poisoning_risk_jobs\":{}}},\n    \"flaky_test_ci_cd_tax\": {{\"monthly_immediate_flake_cost_usd\":{:.2},\"deferred_velocity_drag_multiplier\":{:.1},\"monthly_deferred_velocity_drag_usd\":{:.2},\"monthly_combined_flake_tax_usd\":{:.2},\"deflake_project_est_cost_usd\":{:.2},\"deflake_payback_weeks\":{:.2}}},\n    \"branch_protection_cost\": {{\"required_checks_count\":{},\"monthly_required_check_executions\":{},\"workflows_with_concurrency_cancel\":{},\"workflows_missing_concurrency_cancel\":{},\"workflows_with_path_filters\":{},\"workflows_missing_path_filters\":{},\"has_affected_only_graph_tool\":{},\"slow_blocking_checks_over_10m\":{},\"monthly_superseded_run_waste_usd\":{:.2},\"monthly_unfiltered_check_savings_usd\":{:.2}}},\n    \"startup_stage_cost\": {{\"inferred_team_stage\":\"{}\",\"implied_active_devs\":{},\"free_tier_burn_pct\":{:.2},\"upgrade_trigger_80pct_reached\":{},\"premature_self_hosting_trap\":{},\"premature_multi_os_pr_matrix_trap\":{},\"monthly_multi_os_pr_to_nightly_savings_usd\":{:.2},\"mac_mini_hardware_payback_months\":{:.2}}},\n    \"scaleup_stage_cost\": {{\"self_hosted_breakeven_reached\":{},\"team_to_enterprise_seat_jump_monthly_usd\":{:.2},\"quarterly_required_check_audit_savings_usd\":{:.2},\"platform_engineer_monthly_fully_loaded_usd\":{:.2},\"recoverable_ci_and_wait_monthly_usd\":{:.2},\"platform_engineer_roi_coverage_pct\":{:.2}}},\n    \"enterprise_stage_cost\": {{\"detected_compliance_controls\":[{}],\"compliance_controls_count\":{},\"detected_cloud_regions\":[{}],\"regional_residency_multiplier\":{:.2},\"enterprise_250_seat_licence_monthly_usd\":{:.2},\"siem_1yr_audit_log_retention_monthly_usd\":{:.2},\"dedicated_tenancy_premium_pct_min\":{:.1},\"dedicated_tenancy_premium_pct_max\":{:.1}}},\n    \"monorepo_ci_cost\": {{\"detected_pipeline_shape\":\"{}\",\"detected_workspace_units_count\":{},\"detected_monorepo_tool\":\"{}\",\"recommended_affected_tool\":\"{}\",\"has_nightly_full_suite_safety_net\":{},\"affected_only_reduction_pct_min\":{:.1},\"affected_only_reduction_pct_max\":{:.1},\"monthly_affected_only_savings_usd\":{:.2}}},\n    \"microservices_ci_cost\": {{\"est_setup_overhead_sec_per_job\":{:.1},\"monthly_setup_overhead_hours\":{:.2},\"setup_overhead_share_pct\":{:.2},\"monthly_setup_overhead_compute_usd\":{:.2},\"monthly_shared_infra_setup_savings_usd\":{:.2},\"reusable_workflow_calls_count\":{},\"unpinned_template_refs_count\":{},\"cross_repo_triggers_count\":{},\"est_aggregate_cache_gb\":{:.2},\"exceeds_10gb_repo_cache_limit\":{}}},\n    \"vendor_tco_comparison\": {{\"cheapest_vendor_platform\":\"{}\",\"cheapest_vendor_monthly_tco_usd\":{:.2},\"github_actions_team_monthly_tco_usd\":{:.2},\"gitlab_ci_premium_monthly_tco_usd\":{:.2},\"circleci_performance_monthly_tco_usd\":{:.2},\"bitbucket_premium_monthly_tco_usd\":{:.2},\"buildkite_pro_monthly_tco_usd\":{:.2},\"jenkins_self_hosted_monthly_tco_usd\":{:.2},\"aws_codebuild_monthly_tco_usd\":{:.2},\"gcp_cloud_build_monthly_tco_usd\":{:.2},\"azure_pipelines_monthly_tco_usd\":{:.2},\"migration_estimated_hours\":{:.1},\"migration_estimated_cost_usd\":{:.2},\"migration_payback_months\":{:.2},\"migration_worth_it_2x_annual_rule\":{},\"migration_verdict\":\"{}\"}},\n    \"build_phase_breakdown\": {{\"phase_clone_checkout_share_pct\":{:.1},\"phase_dependency_install_share_pct\":{:.1},\"phase_compile_typecheck_share_pct\":{:.1},\"phase_test_suite_share_pct\":{:.1},\"phase_docker_build_push_share_pct\":{:.1},\"phase_deploy_cleanup_share_pct\":{:.1},\"monthly_total_reducible_phase_savings_usd\":{:.2},\"best_first_optimization_phase\":\"{}\",\"best_first_optimization_effort_hours\":{:.1},\"best_first_optimization_monthly_savings_usd\":{:.2},\"best_first_optimization_payback_months\":{:.2}}},\n    \"compute_class_explorer\": {{\"detected_workload_compute_profile\":\"{}\",\"larger_runners_detected_count\":{},\"larger_runners_bypass_free_minutes_tax_usd\":{:.2},\"arm64_2core_monthly_compute_usd\":{:.2},\"arm64_migration_monthly_savings_usd\":{:.2},\"standard_2core_monthly_compute_usd\":{:.2},\"larger_4core_monthly_compute_usd\":{:.2},\"larger_8core_monthly_compute_usd\":{:.2},\"larger_16core_monthly_compute_usd\":{:.2},\"best_net_roi_runner_tier\":\"{}\",\"best_net_roi_monthly_delta_usd\":{:.2}}},\n    \"stack_specific_economics\": {{\"detected_stack_archetype\":\"{}\",\"ios_cloud_annual_m2_monthly_usd\":{:.2},\"ios_mac_mini_fleet_monthly_tco_usd\":{:.2},\"android_emulator_jobs_count\":{},\"android_kvm_or_gradle_cache_savings_usd\":{:.2},\"jvm_warm_cache_50pct_savings_usd\":{:.2},\"stack_specific_monthly_savings_usd\":{:.2},\"stack_specific_primary_lever\":\"{}\"}},\n    {}\n  }}",
            self.rounding_tax_mins_per_run,
            self.monthly_rounding_tax_usd,
            self.linux_mins_per_run,
            self.windows_mins_per_run,
            self.macos_mins_per_run,
            self.os_weighted_free_tier_mins_per_run,
            self.monthly_os_weighted_free_tier_mins,
            self.monthly_flake_retry_10pct_overhead_usd,
            self.infracost_iac_files_detected,
            self.hosted_effective_hourly_rate_usd,
            json_escape(&self.infracost_recommended_ec2_instance),
            self.infracost_ec2_on_demand_hourly_usd,
            self.infracost_ec2_on_demand_monthly_usd,
            self.infracost_ec2_spot_hourly_usd,
            self.infracost_ec2_surrounding_overhead_monthly_usd,
            self.macos_self_hosted_mac_mini_monthly_usd,
            self.self_hosted_breakeven_mins_t3_medium,
            self.self_hosted_breakeven_mins_c5_4xlarge_spot,
            self.always_on_runner_utilization_pct,
            json_escape(&self.ephemeral_runner_recommended_mode),
            self.ephemeral_cold_start_wait_tax_monthly_usd,
            self.k8s_eks_control_plane_monthly_usd,
            self.k8s_gke_aks_free_tier_control_plane_monthly_usd,
            json_escape(&self.spot_interruption_band_pct),
            self.spot_retry_overhead_monthly_usd,
            self.spot_ineligible_jobs_count,
            self.hybrid_spot_plus_on_demand_monthly_usd,
            self.jobs_uploading_artifacts,
            self.default_retention_days,
            self.configured_retention_days,
            self.steady_state_stored_gb_configured,
            self.monthly_vendor_storage_configured_usd,
            self.net_billed_vendor_storage_configured_usd,
            self.monthly_infracost_s3_configured_usd,
            self.steady_state_stored_gb_90d,
            self.steady_state_stored_gb_14d,
            self.monthly_vendor_storage_90d_usd,
            self.monthly_vendor_storage_14d_usd,
            self.monthly_infracost_s3_standard_90d_usd,
            self.monthly_infracost_s3_intelligent_14d_usd,
            self.peak_concurrent_jobs_per_run,
            self.hosted_concurrent_jobs_per_run,
            self.macos_concurrent_jobs_per_run,
            self.self_hosted_or_microvm_jobs_per_run,
            self.vendor_free_concurrent_cap,
            self.vendor_team_concurrent_cap,
            self.parallelism_tier_upgrade_tax_monthly_usd,
            json_escape(&self.detected_repo_matrix_profile),
            self.matrix_jobs_count,
            self.pr_unconditional_matrix_cells,
            self.conditional_pr_matrix_monthly_savings_usd,
            json_escape(&self.detected_log_language_profile),
            self.est_log_mb_per_run,
            self.monthly_log_volume_gb,
            self.vendor_default_log_retention_days,
            self.monthly_vendor_default_log_cost_usd,
            self.monthly_datadog_30d_log_shipping_usd,
            self.monthly_datadog_90d_log_shipping_usd,
            self.monthly_splunk_30d_log_shipping_usd,
            self.monthly_infracost_loki_s3_usd,
            json_escape(&self.infracost_cloud_region),
            self.est_transfer_gb_per_run,
            self.monthly_transfer_gb,
            self.infracost_aws_internet_egress_per_gb_usd,
            self.infracost_aws_nat_gateway_per_gb_usd,
            self.infracost_aws_cross_az_per_gb_usd,
            self.infracost_aws_cross_region_per_gb_usd,
            self.monthly_infracost_private_subnet_nat_worst_case_usd,
            self.monthly_infracost_cross_region_registry_usd,
            self.monthly_infracost_vpc_endpoint_cache_optimized_usd,
            self.productivity_loss_factor,
            self.blended_fully_loaded_hourly_usd,
            self.monthly_wait_hours,
            self.monthly_productive_payroll_loss_usd,
            self.monthly_blended_100hr_productive_loss_usd,
            self.wait_to_compute_cost_ratio,
            json_escape(&self.lead_time_feedback_tier),
            self.estimated_flake_rate_pct,
            self.has_job_level_auto_retry,
            self.monthly_flaky_reruns,
            self.monthly_rerun_compute_cost_usd,
            self.avg_investigation_mins_per_flake,
            self.monthly_flake_investigation_hours,
            self.monthly_flake_investigation_payroll_usd,
            self.monthly_total_immediate_flake_cost_usd,
            self.monthly_savings_at_1pct_target_usd,
            json_escape(&self.stack_cache_profile),
            self.realistic_saving_pct_min,
            self.realistic_saving_pct_max,
            self.jobs_with_cache,
            self.jobs_missing_cache,
            self.has_restore_keys_fallback,
            self.has_docker_buildkit_cache,
            self.estimated_cache_hit_rate_pct,
            json_escape(&self.vendor_cache_size_limit),
            json_escape(&self.vendor_cache_eviction_policy),
            self.monthly_uncached_compute_usd,
            self.monthly_unrealized_cache_savings_usd,
            json_escape(&self.next_recommended_caching_layer),
            self.distributed_remote_cache_monthly_infra_usd,
            self.container_base_image_mirror_monthly_savings_usd,
            self.pr_cache_poisoning_risk_jobs,
            self.monthly_total_immediate_flake_cost_usd,
            self.deferred_velocity_drag_multiplier,
            self.monthly_deferred_velocity_drag_usd,
            self.monthly_combined_flake_tax_usd,
            self.deflake_project_est_cost_usd,
            self.deflake_payback_weeks,
            self.required_checks_count,
            self.monthly_required_check_executions,
            self.workflows_with_concurrency_cancel,
            self.workflows_missing_concurrency_cancel,
            self.workflows_with_path_filters,
            self.workflows_missing_path_filters,
            self.has_affected_only_graph_tool,
            self.slow_blocking_checks_over_10m,
            self.monthly_superseded_run_waste_usd,
            self.monthly_unfiltered_check_savings_usd,
            json_escape(&self.inferred_team_stage),
            self.implied_active_devs,
            self.free_tier_burn_pct,
            self.upgrade_trigger_80pct_reached,
            self.premature_self_hosting_trap,
            self.premature_multi_os_pr_matrix_trap,
            self.monthly_multi_os_pr_to_nightly_savings_usd,
            self.mac_mini_hardware_payback_months,
            self.self_hosted_breakeven_reached,
            self.team_to_enterprise_seat_jump_monthly_usd,
            self.quarterly_required_check_audit_savings_usd,
            self.platform_engineer_monthly_fully_loaded_usd,
            self.recoverable_ci_and_wait_monthly_usd,
            self.platform_engineer_roi_coverage_pct,
            compliance_json,
            self.compliance_controls_count,
            regions_json,
            self.regional_residency_multiplier,
            self.enterprise_250_seat_licence_monthly_usd,
            self.siem_1yr_audit_log_retention_monthly_usd,
            self.dedicated_tenancy_premium_pct_min,
            self.dedicated_tenancy_premium_pct_max,
            json_escape(&self.detected_pipeline_shape),
            self.detected_workspace_units_count,
            json_escape(&self.detected_monorepo_tool),
            json_escape(&self.recommended_affected_tool),
            self.has_nightly_full_suite_safety_net,
            self.affected_only_reduction_pct_min,
            self.affected_only_reduction_pct_max,
            self.monthly_affected_only_savings_usd,
            self.est_setup_overhead_sec_per_job,
            self.monthly_setup_overhead_hours,
            self.setup_overhead_share_pct,
            self.monthly_setup_overhead_compute_usd,
            self.monthly_shared_infra_setup_savings_usd,
            self.reusable_workflow_calls_count,
            self.unpinned_template_refs_count,
            self.cross_repo_triggers_count,
            self.est_aggregate_cache_gb,
            self.exceeds_10gb_repo_cache_limit,
            json_escape(&self.cheapest_vendor_platform),
            self.cheapest_vendor_monthly_tco_usd,
            self.github_actions_team_monthly_tco_usd,
            self.gitlab_ci_premium_monthly_tco_usd,
            self.circleci_performance_monthly_tco_usd,
            self.bitbucket_premium_monthly_tco_usd,
            self.buildkite_pro_monthly_tco_usd,
            self.jenkins_self_hosted_monthly_tco_usd,
            self.aws_codebuild_monthly_tco_usd,
            self.gcp_cloud_build_monthly_tco_usd,
            self.azure_pipelines_monthly_tco_usd,
            self.migration_estimated_hours,
            self.migration_estimated_cost_usd,
            self.migration_payback_months,
            self.migration_worth_it_2x_annual_rule,
            json_escape(&self.migration_verdict),
            self.phase_clone_checkout_share_pct,
            self.phase_dependency_install_share_pct,
            self.phase_compile_typecheck_share_pct,
            self.phase_test_suite_share_pct,
            self.phase_docker_build_push_share_pct,
            self.phase_deploy_cleanup_share_pct,
            self.monthly_total_reducible_phase_savings_usd,
            json_escape(&self.best_first_optimization_phase),
            self.best_first_optimization_effort_hours,
            self.best_first_optimization_monthly_savings_usd,
            self.best_first_optimization_payback_months,
            json_escape(&self.detected_workload_compute_profile),
            self.larger_runners_detected_count,
            self.larger_runners_bypass_free_minutes_tax_usd,
            self.arm64_2core_monthly_compute_usd,
            self.arm64_migration_monthly_savings_usd,
            self.standard_2core_monthly_compute_usd,
            self.larger_4core_monthly_compute_usd,
            self.larger_8core_monthly_compute_usd,
            self.larger_16core_monthly_compute_usd,
            json_escape(&self.best_net_roi_runner_tier),
            self.best_net_roi_monthly_delta_usd,
            json_escape(&self.detected_stack_archetype),
            self.ios_codemagic_annual_m2_monthly_usd,
            self.ios_buildkite_mac_mini_fleet_monthly_tco_usd,
            self.android_emulator_jobs_count,
            self.android_kvm_or_gradle_cache_savings_usd,
            self.jvm_warm_cache_50pct_savings_usd,
            self.stack_specific_monthly_savings_usd,
            json_escape(&self.stack_specific_primary_lever),
            models_21_to_24_json
        )
    }
}

#[derive(Debug, Clone)]
pub struct BreakdownReport {
    pub target_path: String,
    pub provenance_mode: String,
    pub execution_time_us: u128,
    pub execution_time_ms: f64,
    pub detected_providers: Vec<String>,
    pub usage: UsageConfig,
    pub total_workflows: usize,
    pub total_defined_jobs: usize,
    pub total_expanded_matrix_jobs: usize,
    pub raw_duration_mins_per_run: f64,
    pub critical_path_wait_mins_per_run: f64,
    pub billed_mins_per_run: f64,
    pub monthly_runs: f64,
    pub monthly_billed_mins: f64,
    pub free_tier_included_mins: f64,
    pub free_tier_quota_burn_pct: f64,
    pub effective_tier: String,
    pub gross_compute_cost_per_run_usd: f64,
    pub gross_monthly_compute_cost_usd: f64,
    pub discount_allowance_monthly_usd: f64,
    pub net_billed_monthly_compute_cost_usd: f64,
    pub third_party_runner_monthly_cost_usd: f64,
    pub shard_setup_duplication_mins_per_run: f64,
    pub clones_per_ci_run: usize,
    pub dev_wait_cost_per_run_usd: f64,
    pub monthly_dev_wait_cost_usd: f64,
    pub monthly_artifact_storage_vendor_usd: f64,
    pub monthly_artifact_storage_s3_usd: f64,
    pub monthly_blacksmith_cost_usd: f64,
    pub monthly_self_hosted_spot_cost_usd: f64,
    pub exceeds_5000m_self_hosted_breakeven: bool,
    pub cost_components: CostComponentsBreakdown,
    pub jobs: Vec<ExpandedJobCost>,
}

impl BreakdownReport {
    /// Deterministic Hamilton (Largest-Remainder) cent apportionment of `gross_monthly_compute_cost_usd`
    /// across `self.jobs` so every rendered CLI, Markdown, HTML, CSV, and JSON job table sums to
    /// `gross_monthly_compute_cost_usd` with ZERO 1-cent rounding drift (`H1`).
    pub fn apportioned_job_monthly_costs(&self) -> Vec<f64> {
        let n = self.jobs.len();
        if n == 0 {
            return Vec::new();
        }
        let target_cents = (self.gross_monthly_compute_cost_usd.max(0.0) * 100.0).round() as i64;
        if target_cents <= 0 {
            return vec![0.0; n];
        }
        let weights: Vec<f64> = self
            .jobs
            .iter()
            .map(|j| (j.run_compute_cost_usd * j.effective_monthly_runs(self.monthly_runs)).max(0.0))
            .collect();
        let total_w: f64 = weights.iter().sum();
        if total_w <= 0.0 {
            let base = target_cents / (n as i64);
            let rem = (target_cents % (n as i64)) as usize;
            return (0..n)
                .map(|i| ((base + if i < rem { 1 } else { 0 }) as f64) / 100.0)
                .collect();
        }
        let mut floors: Vec<i64> = Vec::with_capacity(n);
        let mut remainders: Vec<(usize, f64, f64)> = Vec::with_capacity(n);
        let mut floor_sum: i64 = 0;
        for (i, &w) in weights.iter().enumerate() {
            let exact_cents = (target_cents as f64) * (w / total_w);
            let fl = (exact_cents + 1e-11).floor() as i64;
            let rem = (exact_cents - (fl as f64)).max(0.0);
            floors.push(fl);
            floor_sum += fl;
            remainders.push((i, rem, w));
        }
        let leftover = (target_cents - floor_sum).max(0) as usize;
        remainders.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.0.cmp(&b.0))
        });
        for &(idx, _, _) in remainders.iter().take(leftover) {
            floors[idx] += 1;
        }
        floors.into_iter().map(|c| norm_zero((c as f64) / 100.0)).collect()
    }

    pub fn agent_digest_json(&self) -> String {
        let app_costs = self.apportioned_job_monthly_costs();
        let mut ranked_indices: Vec<usize> = (0..self.jobs.len()).collect();
        ranked_indices.sort_by(|&a, &b| {
            app_costs[b]
                .partial_cmp(&app_costs[a])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(&b))
        });

        let top_jobs: Vec<String> = ranked_indices
            .into_iter()
            .take(5)
            .map(|idx| {
                let j = &self.jobs[idx];
                let mut missing = Vec::new();
                if !j.has_timeout_minutes {
                    missing.push("\"timeout-minutes\"");
                }
                if !j.has_concurrency_cancel {
                    missing.push("\"concurrency-cancel\"");
                }
                if !j.uses_cache {
                    missing.push("\"cache\"");
                }
                let ls = j.line_start.max(1);
                let le = j.line_end.max(ls);
                format!(
                    "{{\"workflow_file\":\"{}\",\"job_id\":\"{}\",\"line_start\":{},\"line_end\":{},\"runner_label\":\"{}\",\"monthly_cost_usd\":{:.2},\"missing_guardrails\":[{}]}}",
                    json_escape(&normalize_ci_path(&j.workflow_file)),
                    json_escape(&j.job_id),
                    ls,
                    le,
                    json_escape(&j.runner_label),
                    app_costs[idx],
                    missing.join(",")
                )
            })
            .collect();

        let cc = &self.cost_components;
        let mut actions: Vec<String> = Vec::new();
        let compound_compute_sav = if self.gross_monthly_compute_cost_usd > 0.0 {
            let r_cancel = (cc.monthly_superseded_run_waste_usd / self.gross_monthly_compute_cost_usd)
                .clamp(0.0, 0.50);
            let r_cache = (cc.monthly_unrealized_cache_savings_usd / self.gross_monthly_compute_cost_usd)
                .clamp(0.0, 0.70);
            (self.gross_monthly_compute_cost_usd * (1.0 - (1.0 - r_cancel) * (1.0 - r_cache)))
                .min(self.gross_monthly_compute_cost_usd * 0.85)
        } else {
            0.0
        };
        let storage_sav = if cc.configured_retention_days > 14 {
            (cc.monthly_vendor_storage_configured_usd - cc.monthly_vendor_storage_14d_usd).max(0.0)
        } else {
            0.0
        };
        let shrink_savings = ((compound_compute_sav + storage_sav) * 100.0).round() / 100.0;
        if shrink_savings >= 1.0 || cc.jobs_missing_timeout_minutes_count > 0 {
            actions.push(format!(
                "{{\"mcp_tool\":\"minici_shrink\",\"cli_cmd\":\"minici shrink --path .\",\"est_monthly_savings_usd\":{:.2},\"fixes_missing_timeouts\":{}}}",
                shrink_savings, cc.jobs_missing_timeout_minutes_count
            ));
        }
        if cc.arm64_migration_monthly_savings_usd >= 1.0 {
            actions.push(format!(
                "{{\"mcp_tool\":\"minici_migrate\",\"cli_cmd\":\"minici migrate --path . --to github-arm64\",\"est_monthly_savings_usd\":{:.2}}}",
                cc.arm64_migration_monthly_savings_usd
            ));
        }
        if cc.monthly_unfiltered_check_savings_usd >= 5.0 {
            actions.push(format!(
                "{{\"lever\":\"path-filters\",\"workflows_affected\":{},\"est_monthly_savings_usd\":{:.2}}}",
                cc.workflows_missing_path_filters, cc.monthly_unfiltered_check_savings_usd
            ));
        }

        format!(
            "{{\"top_optimization_phase\":\"{}\",\"reducible_monthly_usd\":{:.2},\"top_cost_jobs\":[{}],\"next_actions\":[{}]}}",
            json_escape(&cc.best_first_optimization_phase),
            cc.monthly_total_reducible_phase_savings_usd,
            top_jobs.join(","),
            actions.join(",")
        )
    }

    pub fn to_compact_json(&self) -> String {
        let provs: Vec<String> = self
            .detected_providers
            .iter()
            .map(|p| format!("\"{}\"", json_escape(p)))
            .collect();
        format!(
            "{{\n  \"schema\": \"minici.compact.v1\",\n  \"target_path\": \"{}\",\n  \"provenance_mode\": \"{}\",\n  \"execution_time_ms\": {:.3},\n  \"detected_providers\": [{}],\n  \"total_workflows\": {},\n  \"total_defined_jobs\": {},\n  \"total_expanded_matrix_jobs\": {},\n  \"billed_mins_per_run\": {:.2},\n  \"critical_path_wait_mins_per_run\": {:.2},\n  \"monthly_runs\": {:.1},\n  \"gross_monthly_compute_cost_usd\": {:.2},\n  \"net_billed_monthly_compute_cost_usd\": {:.2},\n  \"free_tier_quota_burn_pct\": {:.1},\n  \"monthly_dev_wait_cost_usd\": {:.2},\n  \"cost_per_merged_pr_usd\": {:.2},\n  \"spend_anomaly_alert_active\": {},\n  \"agent_digest\": {}\n}}",
            json_escape(&normalize_ci_path(&self.target_path)),
            json_escape(&self.provenance_mode),
            self.execution_time_ms,
            provs.join(", "),
            self.total_workflows,
            self.total_defined_jobs,
            self.total_expanded_matrix_jobs,
            self.billed_mins_per_run,
            self.critical_path_wait_mins_per_run,
            self.monthly_runs,
            self.gross_monthly_compute_cost_usd,
            self.net_billed_monthly_compute_cost_usd,
            self.free_tier_quota_burn_pct,
            self.monthly_dev_wait_cost_usd,
            self.cost_components.cost_per_merged_pr_usd,
            self.cost_components.spend_anomaly_alert_active,
            self.agent_digest_json()
        )
    }

    pub fn to_json(&self) -> String {
        let provs: Vec<String> = self
            .detected_providers
            .iter()
            .map(|p| format!("\"{}\"", json_escape(p)))
            .collect();
        let app_costs = self.apportioned_job_monthly_costs();
        let job_strs: Vec<String> = self
            .jobs
            .iter()
            .zip(app_costs.iter())
            .map(|(j, &mc)| j.to_json_with_monthly(Some(mc)))
            .collect();
        format!(
            "{{\n  \"target_path\": \"{}\",\n  \"provenance_mode\": \"{}\",\n  \"execution_time_us\": {},\n  \"execution_time_ms\": {:.3},\n  \"detected_providers\": [{}],\n  \"usage\": {},\n  \"total_workflows\": {},\n  \"total_defined_jobs\": {},\n  \"total_expanded_matrix_jobs\": {},\n  \"raw_duration_mins_per_run\": {:.2},\n  \"critical_path_wait_mins_per_run\": {:.2},\n  \"billed_mins_per_run\": {:.2},\n  \"shard_setup_duplication_mins_per_run\": {:.2},\n  \"clones_per_ci_run\": {},\n  \"monthly_runs\": {:.1},\n  \"monthly_billed_mins\": {:.2},\n  \"free_tier_included_mins\": {:.1},\n  \"free_tier_quota_burn_pct\": {:.1},\n  \"effective_tier\": \"{}\",\n  \"gross_compute_cost_per_run_usd\": {:.4},\n  \"gross_monthly_compute_cost_usd\": {:.2},\n  \"discount_allowance_monthly_usd\": {:.2},\n  \"net_billed_monthly_compute_cost_usd\": {:.2},\n  \"third_party_runner_monthly_cost_usd\": {:.2},\n  \"dev_wait_cost_per_run_usd\": {:.2},\n  \"monthly_dev_wait_cost_usd\": {:.2},\n  \"monthly_artifact_storage_vendor_usd\": {:.2},\n  \"monthly_artifact_storage_s3_usd\": {:.2},\n  \"monthly_high_speed_microvm_cost_usd\": {:.2},\n  \"monthly_self_hosted_spot_cost_usd\": {:.2},\n  \"exceeds_5000m_self_hosted_breakeven\": {},\n  \"agent_digest\": {},\n  \"cost_components\": {},\n  \"jobs\": [\n    {}\n  ]\n}}",
            json_escape(&self.target_path),
            json_escape(&self.provenance_mode),
            self.execution_time_us,
            self.execution_time_ms,
            provs.join(", "),
            self.usage.to_json(),
            self.total_workflows,
            self.total_defined_jobs,
            self.total_expanded_matrix_jobs,
            self.raw_duration_mins_per_run,
            self.critical_path_wait_mins_per_run,
            self.billed_mins_per_run,
            self.shard_setup_duplication_mins_per_run,
            self.clones_per_ci_run,
            self.monthly_runs,
            self.monthly_billed_mins,
            self.free_tier_included_mins,
            self.free_tier_quota_burn_pct,
            json_escape(&self.effective_tier),
            self.gross_compute_cost_per_run_usd,
            self.gross_monthly_compute_cost_usd,
            self.discount_allowance_monthly_usd,
            self.net_billed_monthly_compute_cost_usd,
            self.third_party_runner_monthly_cost_usd,
            self.dev_wait_cost_per_run_usd,
            self.monthly_dev_wait_cost_usd,
            self.monthly_artifact_storage_vendor_usd,
            self.monthly_artifact_storage_s3_usd,
            self.monthly_blacksmith_cost_usd,
            self.monthly_self_hosted_spot_cost_usd,
            self.exceeds_5000m_self_hosted_breakeven,
            self.agent_digest_json(),
            self.cost_components.to_json(),
            job_strs.join(",\n    ")
        )
    }
}

#[derive(Debug, Clone)]
pub struct DiffReport {
    pub base_path: String,
    pub head_path: String,
    pub execution_time_us: u128,
    pub base_expanded_jobs: usize,
    pub head_expanded_jobs: usize,
    pub delta_expanded_jobs: isize,
    pub base_billed_mins_per_run: f64,
    pub head_billed_mins_per_run: f64,
    pub delta_billed_mins_per_run: f64,
    pub base_monthly_compute_usd: f64,
    pub head_monthly_compute_usd: f64,
    pub delta_monthly_compute_usd: f64,
    pub delta_monthly_compute_pct: f64,
    pub base_monthly_dev_wait_usd: f64,
    pub head_monthly_dev_wait_usd: f64,
    pub delta_monthly_dev_wait_usd: f64,
    pub base_free_quota_burn_pct: f64,
    pub head_free_quota_burn_pct: f64,
    pub job_changes: Vec<String>,
    pub head_report: BreakdownReport,
}

impl DiffReport {
    pub fn to_compact_json(&self) -> String {
        let ch: Vec<String> = self
            .job_changes
            .iter()
            .take(10)
            .map(|c| format!("\"{}\"", json_escape(c)))
            .collect();
        let nz = |v: f64| if v.abs() < 0.0005 { 0.0 } else { v };
        format!(
            "{{\n  \"schema\": \"minici.diff.compact.v1\",\n  \"base_path\": \"{}\",\n  \"head_path\": \"{}\",\n  \"execution_time_us\": {},\n  \"base_expanded_jobs\": {},\n  \"head_expanded_jobs\": {},\n  \"delta_expanded_jobs\": {},\n  \"delta_billed_mins_per_run\": {:.2},\n  \"base_monthly_compute_usd\": {:.2},\n  \"head_monthly_compute_usd\": {:.2},\n  \"delta_monthly_compute_usd\": {:.2},\n  \"delta_monthly_compute_pct\": {:.1},\n  \"delta_monthly_dev_wait_usd\": {:.2},\n  \"total_job_changes\": {},\n  \"top_job_changes\": [{}],\n  \"head_agent_digest\": {}\n}}",
            json_escape(&normalize_ci_path(&self.base_path)),
            json_escape(&normalize_ci_path(&self.head_path)),
            self.execution_time_us,
            self.base_expanded_jobs,
            self.head_expanded_jobs,
            self.delta_expanded_jobs,
            nz(self.delta_billed_mins_per_run),
            nz(self.base_monthly_compute_usd),
            nz(self.head_monthly_compute_usd),
            nz(self.delta_monthly_compute_usd),
            nz(self.delta_monthly_compute_pct),
            nz(self.delta_monthly_dev_wait_usd),
            self.job_changes.len(),
            ch.join(", "),
            self.head_report.agent_digest_json()
        )
    }

    pub fn to_json(&self) -> String {
        let ch: Vec<String> = self
            .job_changes
            .iter()
            .map(|c| format!("\"{}\"", json_escape(c)))
            .collect();
        let nz = |v: f64| if v.abs() < 0.0005 { 0.0 } else { v };
        format!(
            "{{\n  \"base_path\": \"{}\",\n  \"head_path\": \"{}\",\n  \"execution_time_us\": {},\n  \"base_expanded_jobs\": {},\n  \"head_expanded_jobs\": {},\n  \"delta_expanded_jobs\": {},\n  \"base_billed_mins_per_run\": {:.2},\n  \"head_billed_mins_per_run\": {:.2},\n  \"delta_billed_mins_per_run\": {:.2},\n  \"base_monthly_compute_usd\": {:.2},\n  \"head_monthly_compute_usd\": {:.2},\n  \"delta_monthly_compute_usd\": {:.2},\n  \"delta_monthly_compute_pct\": {:.1},\n  \"base_monthly_dev_wait_usd\": {:.2},\n  \"head_monthly_dev_wait_usd\": {:.2},\n  \"delta_monthly_dev_wait_usd\": {:.2},\n  \"base_free_quota_burn_pct\": {:.1},\n  \"head_free_quota_burn_pct\": {:.1},\n  \"job_changes\": [{}],\n  \"agent_digest\": {},\n  \"cost_components\": {}\n}}",
            json_escape(&self.base_path),
            json_escape(&self.head_path),
            self.execution_time_us,
            self.base_expanded_jobs,
            self.head_expanded_jobs,
            self.delta_expanded_jobs,
            nz(self.base_billed_mins_per_run),
            nz(self.head_billed_mins_per_run),
            nz(self.delta_billed_mins_per_run),
            nz(self.base_monthly_compute_usd),
            nz(self.head_monthly_compute_usd),
            nz(self.delta_monthly_compute_usd),
            nz(self.delta_monthly_compute_pct),
            nz(self.base_monthly_dev_wait_usd),
            nz(self.head_monthly_dev_wait_usd),
            nz(self.delta_monthly_dev_wait_usd),
            nz(self.base_free_quota_burn_pct),
            nz(self.head_free_quota_burn_pct),
            ch.join(", "),
            self.head_report.agent_digest_json(),
            self.head_report.cost_components.to_json()
        )
    }
}

fn load_usage_config(root: &Path, override_cfg: Option<UsageConfig>) -> UsageConfig {
    let mut cfg = override_cfg.unwrap_or_default();
    let base_dir = if root.is_file() {
        root.parent().unwrap_or(Path::new("."))
    } else {
        root
    };
    let usage_file = base_dir.join("minici-usage.yml");
    if usage_file.is_file() {
        if let Ok(raw) = fs::read_to_string(&usage_file) {
            for line in raw.lines() {
                let t = line.trim();
                if t.starts_with('#') || !t.contains(':') {
                    continue;
                }
                let parts: Vec<&str> = t.splitn(2, ':').collect();
                let k = parts[0].trim();
                let v = parts[1]
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                match k {
                    "tier" => cfg.tier = v.to_string(),
                    "prs_per_week" => {
                        if let Ok(n) = v.parse::<f64>() {
                            cfg.prs_per_week = n;
                        }
                    }
                    "pushes_per_pr" => {
                        if let Ok(n) = v.parse::<f64>() {
                            cfg.pushes_per_pr = n;
                        }
                    }
                    "dev_hourly_rate_usd" => {
                        if let Ok(n) = v.parse::<f64>() {
                            cfg.dev_hourly_rate_usd = n;
                        }
                    }
                    "artifact_gb_per_run" => {
                        if let Ok(n) = v.parse::<f64>() {
                            cfg.artifact_gb_per_run = n;
                        }
                    }
                    "artifact_retention_days" => {
                        if let Ok(n) = v.parse::<u32>() {
                            cfg.artifact_retention_days = n.clamp(1, 400);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    cfg
}

/// Parses GitHub Actions `retention-days: <N>` or GitLab CI `expire_in: <duration>` into days (`1..=400`).
pub fn parse_retention_days_value(raw: &str) -> Option<u32> {
    let clean = raw
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase();
    if clean.is_empty() {
        return None;
    }
    if clean == "never" {
        return Some(400);
    }
    if let Ok(days) = clean.parse::<u32>() {
        return Some(days.clamp(1, 400));
    }
    let num_part: String = clean.chars().take_while(|c| c.is_ascii_digit()).collect();
    if let Ok(n) = num_part.parse::<u32>() {
        let rest = clean[num_part.len()..].trim();
        let days = if rest.starts_with("sec") || rest == "s" {
            1
        } else if rest.starts_with("min") || rest == "m" || rest == "mins" {
            1
        } else if rest.starts_with("week") || rest == "w" || rest == "wks" {
            n.saturating_mul(7)
        } else if rest.starts_with("month") || rest == "mo" || rest == "mos" {
            n.saturating_mul(30)
        } else if rest.starts_with("year") || rest == "y" || rest == "yr" || rest == "yrs" {
            n.saturating_mul(365)
        } else if rest.starts_with("hour") || rest.starts_with("hr") || rest == "h" {
            (n.saturating_add(23)) / 24
        } else {
            n
        };
        return Some(days.clamp(1, 400));
    }
    None
}

fn extract_os_from_flow_map(s: &str) -> Option<String> {
    let inner = s.trim().strip_prefix('{')?.strip_suffix('}')?;
    for part in inner.split(',') {
        let kv = part.trim();
        if let Some(val) = kv
            .strip_prefix("os:")
            .or_else(|| kv.strip_prefix("runner:"))
            .or_else(|| kv.strip_prefix("runs-on:"))
        {
            let clean = val.trim().trim_matches('"').trim_matches('\'');
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }
    None
}

fn parse_inline_yaml_list(val: &str) -> Vec<String> {
    let trimmed = val.trim();
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if end > start {
                let inner = &trimmed[start + 1..end];
                return inner
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    Vec::new()
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

fn find_unquoted_comment_idx(line: &str) -> Option<usize> {
    let mut in_single = false;
    let mut in_double = false;
    let mut prev_backslash = false;
    for (i, ch) in line.char_indices() {
        if in_double {
            if ch == '"' && !prev_backslash {
                in_double = false;
            }
            prev_backslash = ch == '\\' && !prev_backslash;
            continue;
        }
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        if ch == '"' {
            in_double = true;
            prev_backslash = false;
        } else if ch == '\'' {
            in_single = true;
        } else if ch == '#' {
            if i == 0 || line.as_bytes()[i - 1].is_ascii_whitespace() {
                return Some(i);
            }
        }
    }
    None
}

fn strip_comment(line: &str) -> &str {
    if let Some(idx) = find_unquoted_comment_idx(line) {
        if idx == 0 {
            line
        } else {
            line[..idx].trim_end()
        }
    } else {
        line
    }
}

#[inline]
fn norm_zero(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        0.0
    } else {
        x
    }
}

fn extract_comment_est_minutes(raw_line: &str) -> Option<f64> {
    let hash_idx = find_unquoted_comment_idx(raw_line)?;
    let comment_body = raw_line[hash_idx + 1..].trim();
    let low = comment_body.to_ascii_lowercase();
    let rest = if let Some(r) = low.strip_prefix("est-minutes:") {
        r
    } else if let Some(r) = low.strip_prefix("estimated-minutes:") {
        r
    } else {
        return None;
    };
    let num_tok = rest
        .trim()
        .split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .next()
        .unwrap_or("");
    if let Ok(m) = num_tok.parse::<f64>() {
        if m.is_finite() && m > 0.0 {
            return Some(m.max(0.2));
        }
    }
    None
}

fn load_repo_visibility(root: &Path) -> RepoVisibility {
    if let Ok(env_vis) = std::env::var("MINICI_REPO_VISIBILITY") {
        let parsed = RepoVisibility::from_str_opt(&env_vis);
        if parsed != RepoVisibility::Unknown {
            return parsed;
        }
    }
    let base_dir = if root.is_file() {
        root.parent().unwrap_or(Path::new("."))
    } else {
        root
    };
    let usage_file = base_dir.join("minici-usage.yml");
    if usage_file.is_file() {
        if let Ok(raw) = fs::read_to_string(&usage_file) {
            for line in raw.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("visibility:") {
                    let v = rest
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    let parsed = RepoVisibility::from_str_opt(v);
                    if parsed != RepoVisibility::Unknown {
                        return parsed;
                    }
                }
            }
        }
    }
    RepoVisibility::Unknown
}

fn line_has_cache_signal(low_line: &str) -> bool {
    // Fixes Bug #26:
    // 1. `cache: false` / `cache: ""` explicitly DISABLES caching!
    // 2. Local `./.github/actions/` composite actions do NOT automatically imply dependency caching!
    // 3. Bare `setup-node` / `setup-python` without `cache:` do NOT cache by default (`setup-go` with `go.sum` is handled at repo level).
    if low_line.contains("cache: false")
        || low_line.contains("cache: 'false'")
        || low_line.contains("cache: \"false\"")
        || low_line.contains("cache: ''")
        || low_line.contains("cache: \"\"")
    {
        return false;
    }
    low_line.contains("actions/cache")
        || low_line.contains("cache:")
        || low_line.contains("cache-key:")
        || low_line.contains("cache-from")
        || low_line.contains("cache-to")
        || low_line.contains("setup-docker-builder")
        || low_line.contains("rust-cache")
        || low_line.contains("sccache")
        || low_line.contains("ccache")
        || low_line.contains("setup-gradle")
        || low_line.contains("gradle-build-action")
        || low_line.contains("save_cache")
        || low_line.contains("restore_cache")
        || (low_line.contains("./.github/actions/")
            && (low_line.contains("cache")
                || low_line.contains("setup")
                || low_line.contains("install")
                || low_line.contains("corepack")
                || low_line.contains("pnpm")
                || low_line.contains("node")
                || low_line.contains("rust")
                || low_line.contains("python")
                || low_line.contains("go")))
}

#[derive(Default)]
#[allow(dead_code)]
struct WorkflowAstMeta {
    dag_nodes: Vec<(String, f64, usize, Vec<String>)>,
    job_display_names: Vec<(String, String)>, // (job_id, display_name)
    matrix_job_ids: Vec<String>,
}

/// Fast zero-allocation scanner for a single GitHub Actions job block (`lines` belonging to one job).
/// Returns `Some((raw_mins_per_job, expanded_matrix_count, needs_job_ids, display_name, is_matrix_job))` when the job allocates a runner VM.
fn analyze_github_job_block(
    wf_rel: &str,
    job_id: &str,
    job_lines: &[&str],
    line_start: usize,
    line_end: usize,
    has_top_concurrency: bool,
    has_top_cache: bool,
    has_repo_gosum: bool,
    out_jobs: &mut Vec<ExpandedJobCost>,
) -> Option<(f64, usize, Vec<String>, Option<String>, bool)> {
    let mut raw_runs_on = "ubuntu-latest".to_string();
    let mut saw_runs_on = false;
    let mut in_runs_on_list = false;
    let mut runs_on_indent = 0usize;
    let mut runs_on_list_items: Vec<String> = Vec::new();
    let mut uses_local_reusable_workflow = false;
    let mut has_job_concurrency = has_top_concurrency;
    let mut in_job_concurrency_block = false;
    let mut job_conc_indent = 0usize;
    let mut has_timeout_minutes = false;
    let mut timeout_minutes_val: Option<f64> = None;
    let mut job_display_name: Option<String> = None;
    let mut uses_cache = has_top_cache;
    let mut explicit_cache_disabled = false;
    let mut explicit_est_mins: Option<f64> = None;
    let mut step_est_mins_sum = 0.0_f64;
    let mut has_sparse = false;
    let mut uploads_artifacts = false;
    let mut artifact_retention_days: Option<u32> = None;
    let mut step_mins = 0.5_f64;
    let mut saw_steps = false;
    let mut step_item_indent: Option<usize> = None;
    let mut cur_step_counted_test = false;
    let mut cur_step_counted_build = false;
    let mut needs_ids: Vec<String> = Vec::new();
    let mut in_needs_list = false;
    let mut needs_indent = 0usize;

    // Matrix tracking (including full Cartesian `exclude:` hyperplanes & `include:` cells per D4 & D5!)
    let mut in_matrix = false;
    let mut matrix_indent = 0usize;
    let mut current_matrix_axis = String::new();
    let mut axis_indent = 0usize;
    let mut axis_counts: Vec<(String, Vec<String>)> = Vec::new();
    let mut include_entries: Vec<Vec<(String, String)>> = Vec::new();
    let mut exclude_entries: Vec<Vec<(String, String)>> = Vec::new();

    for &raw_line in job_lines {
        if let Some(m) = extract_comment_est_minutes(raw_line) {
            if saw_steps {
                step_est_mins_sum += m;
            } else {
                explicit_est_mins = Some(m);
            }
        }
        let clean = strip_comment(raw_line);
        let trimmed = clean.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent_of(clean);
        let low_trimmed = trimmed.to_ascii_lowercase();
        if low_trimmed.contains("cache: false")
            || low_trimmed.contains("cache: 'false'")
            || low_trimmed.contains("cache: \"false\"")
        {
            explicit_cache_disabled = true;
            uses_cache = false;
        } else if line_has_cache_signal(&low_trimmed)
            || (!explicit_cache_disabled && has_repo_gosum && low_trimmed.contains("actions/setup-go"))
        {
            uses_cache = true;
        }

        // Parse multi-line `runs-on:` (both `- self-hosted` sequence AND `group:` / `labels:` mapping form)
        if in_runs_on_list {
            if ind > runs_on_indent {
                if trimmed.starts_with('-') {
                    let lbl = trimmed
                        .trim_start_matches('-')
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !lbl.is_empty() {
                        runs_on_list_items.push(lbl.to_string());
                        raw_runs_on = runs_on_list_items.join("-");
                    }
                } else if let Some((k, v)) = trimmed.split_once(':') {
                    let key_low = k.trim().to_ascii_lowercase();
                    let vt = v.trim();
                    if key_low == "labels" && !vt.is_empty() {
                        let parsed = parse_inline_yaml_list(vt);
                        if !parsed.is_empty() {
                            for item in parsed {
                                runs_on_list_items.push(item);
                            }
                        } else {
                            let clean_v = vt.trim_matches('"').trim_matches('\'');
                            if !clean_v.is_empty() {
                                runs_on_list_items.push(clean_v.to_string());
                            }
                        }
                        if !runs_on_list_items.is_empty() {
                            raw_runs_on = runs_on_list_items.join("-");
                        }
                    } else if key_low == "group" && !vt.is_empty() && runs_on_list_items.is_empty() {
                        let clean_g = vt.trim_matches('"').trim_matches('\'');
                        if !clean_g.is_empty() {
                            raw_runs_on = clean_g.to_string();
                        }
                    }
                }
                continue;
            } else {
                in_runs_on_list = false;
            }
        }

        if in_job_concurrency_block {
            if ind > job_conc_indent {
                if let Some(cip) = low_trimmed.strip_prefix("cancel-in-progress:") {
                    let cv = cip.trim().trim_matches('"').trim_matches('\'');
                    if cv == "true" || cv.contains("${{") {
                        has_job_concurrency = true;
                    }
                }
                continue;
            } else {
                in_job_concurrency_block = false;
            }
        }

        // Parse job-level `name:`, `timeout-minutes:` & `needs:` BEFORE `steps:`
        if !saw_steps && !in_matrix {
            if trimmed.starts_with("name:") {
                if let Some(rhs) = trimmed.splitn(2, ':').nth(1) {
                    let nm = rhs.trim().trim_matches('"').trim_matches('\'').trim();
                    if !nm.is_empty() && !nm.contains("${{") {
                        job_display_name = Some(nm.to_string());
                    }
                }
            }
            if trimmed.starts_with("timeout-minutes:") {
                has_timeout_minutes = true;
                if let Some(rhs) = trimmed.splitn(2, ':').nth(1) {
                    if let Ok(tm) = rhs.trim().trim_matches('"').trim_matches('\'').parse::<f64>() {
                        if tm.is_finite() && tm > 0.0 {
                            timeout_minutes_val = Some(tm);
                        }
                    }
                }
            }
            if in_needs_list {
                if ind > needs_indent && trimmed.starts_with('-') {
                    let dep = trimmed
                        .trim_start_matches('-')
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !dep.is_empty() {
                        needs_ids.push(dep.to_string());
                    }
                    continue;
                } else {
                    in_needs_list = false;
                }
            }
            if trimmed.starts_with("needs:") {
                if let Some(rhs) = trimmed.splitn(2, ':').nth(1) {
                    let rhs_t = rhs.trim();
                    if rhs_t.is_empty() {
                        in_needs_list = true;
                        needs_indent = ind;
                    } else if rhs_t.starts_with('[') {
                        for dep in parse_inline_yaml_list(rhs_t) {
                            needs_ids.push(dep);
                        }
                    } else {
                        let single = rhs_t.trim_matches('"').trim_matches('\'');
                        if !single.is_empty() {
                            needs_ids.push(single.to_string());
                        }
                    }
                }
            }
        }

        if !saw_steps && trimmed.starts_with("concurrency:") {
            let rhs = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
            if rhs.is_empty() {
                in_job_concurrency_block = true;
                job_conc_indent = ind;
            } else if low_trimmed.contains("cancel-in-progress: true")
                || low_trimmed.contains("cancel-in-progress: ${{")
            {
                has_job_concurrency = true;
            }
        }
        if trimmed.starts_with("est-minutes:") || trimmed.starts_with("estimated-minutes:") {
            if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                if let Ok(m) = val.trim().parse::<f64>() {
                    explicit_est_mins = Some(m.max(0.2));
                }
            }
        }
        if !saw_steps && !in_matrix && trimmed.starts_with("uses:") {
            if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                let target = val.trim().trim_matches('"').trim_matches('\'');
                if target.starts_with("./.github/workflows/")
                    || target.starts_with(".github/workflows/")
                {
                    uses_local_reusable_workflow = true;
                }
            }
        }
        if !saw_steps && trimmed.starts_with("runs-on:") {
            saw_runs_on = true;
            if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                let vt = val.trim();
                if vt.is_empty() {
                    in_runs_on_list = true;
                    runs_on_indent = ind;
                    runs_on_list_items.clear();
                } else {
                    let list = parse_inline_yaml_list(vt);
                    if !list.is_empty() {
                        raw_runs_on = list.join("-");
                    } else {
                        raw_runs_on = vt
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string();
                    }
                }
            }
        }

        // Matrix block detection
        if trimmed.starts_with("matrix:") {
            in_matrix = true;
            matrix_indent = ind;
            current_matrix_axis.clear();
            continue;
        }
        if in_matrix {
            if ind <= matrix_indent && !trimmed.starts_with('-') {
                in_matrix = false;
            } else {
                if trimmed.starts_with('-') && !current_matrix_axis.is_empty() {
                    let item_val = trimmed
                        .trim_start_matches('-')
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if current_matrix_axis == "include" || current_matrix_axis == "exclude" {
                        if ind <= axis_indent + 4 {
                            let mut pairs: Vec<(String, String)> = Vec::new();
                            if item_val.starts_with('{') && item_val.ends_with('}') {
                                for kv in item_val[1..item_val.len() - 1].split(',') {
                                    if let Some((k, v)) = kv.split_once(':') {
                                        pairs.push((
                                            k.trim().trim_matches('"').trim_matches('\'').to_string(),
                                            v.trim().trim_matches('"').trim_matches('\'').to_string(),
                                        ));
                                    }
                                }
                            } else if let Some((k, v)) = item_val.split_once(':') {
                                pairs.push((
                                    k.trim().trim_matches('"').trim_matches('\'').to_string(),
                                    v.trim().trim_matches('"').trim_matches('\'').to_string(),
                                ));
                            }
                            if current_matrix_axis == "include" {
                                include_entries.push(pairs);
                            } else {
                                exclude_entries.push(pairs);
                            }
                        }
                    } else if let Some(last) = axis_counts.last_mut() {
                        if last.0 == current_matrix_axis
                            && (!item_val.contains(':') || item_val.starts_with('{'))
                        {
                            last.1.push(item_val.to_string());
                        }
                    }
                } else if (current_matrix_axis == "include" || current_matrix_axis == "exclude")
                    && !trimmed.starts_with('-')
                    && ind > axis_indent + 2
                    && trimmed.contains(':')
                {
                    let target_vec = if current_matrix_axis == "include" {
                        &mut include_entries
                    } else {
                        &mut exclude_entries
                    };
                    if let Some(last_ent) = target_vec.last_mut() {
                        if let Some((k, v)) = trimmed.split_once(':') {
                            last_ent.push((
                                k.trim().trim_matches('"').trim_matches('\'').to_string(),
                                v.trim().trim_matches('"').trim_matches('\'').to_string(),
                            ));
                        }
                    }
                } else if !trimmed.starts_with('-') && trimmed.contains(':') && ind <= matrix_indent + 4 {
                    let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                    let key = parts[0].trim();
                    let rhs = parts[1].trim();
                    current_matrix_axis = key.to_string();
                    axis_indent = ind;
                    if key != "include" && key != "exclude" {
                        let inline = parse_inline_yaml_list(rhs);
                        if !inline.is_empty() {
                            axis_counts.push((key.to_string(), inline));
                            current_matrix_axis.clear();
                        } else {
                            axis_counts.push((key.to_string(), Vec::new()));
                        }
                    }
                }
                continue;
            }
        }

        // Steps & duration heuristics (deduplicated per `- ` step boundary per D6 & H2!)
        if trimmed.starts_with("steps:") {
            saw_steps = true;
            continue;
        }
        if saw_steps {
            if trimmed.starts_with('-') {
                let s_ind = *step_item_indent.get_or_insert(ind);
                if ind <= s_ind {
                    step_item_indent = Some(ind);
                    cur_step_counted_test = false;
                    cur_step_counted_build = false;
                }
            }
            if trimmed.contains("actions/checkout") {
                step_mins += 0.35;
            } else if trimmed.starts_with("filter:")
                || trimmed.starts_with("sparse-checkout:")
                || trimmed
                    .strip_prefix("fetch-depth:")
                    .map(|v| v.trim().trim_matches('"').trim_matches('\'') == "1")
                    .unwrap_or(false)
            {
                if !has_sparse {
                    has_sparse = true;
                    step_mins = (step_mins - 0.3).max(0.5);
                }
            } else if trimmed.contains("upload-artifact") {
                uploads_artifacts = true;
                step_mins += 0.35;
            } else if trimmed.starts_with("retention-days:") {
                if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                    if let Some(days) = parse_retention_days_value(val) {
                        artifact_retention_days = Some(days);
                    }
                }
            } else if low_trimmed.contains("swatinem/rust-cache")
                || low_trimmed.contains("actions/cache")
            {
                // Dedicated cache actions do not add step duration penalty (they speed up warm builds!)
            } else {
                let is_name_line = trimmed.starts_with("- name:") || trimmed.starts_with("name:");
                let has_test_cmd = !is_name_line
                    && (low_trimmed.contains("pytest")
                        || low_trimmed.contains("cargo test")
                        || low_trimmed.contains("cargo nextest")
                        || low_trimmed.contains("npm test")
                        || low_trimmed.contains("pnpm test")
                        || low_trimmed.contains("yarn test")
                        || low_trimmed.contains("bun test")
                        || low_trimmed.contains("jest")
                        || low_trimmed.contains("playwright test")
                        || low_trimmed.contains("cypress run")
                        || (low_trimmed.contains("vitest")
                            && !low_trimmed.contains(".vitest-reports")
                            && !low_trimmed.contains("--merge-reports")
                            && !low_trimmed.starts_with("path:"))
                        || low_trimmed.contains("go test")
                        || low_trimmed.contains("mvn test")
                        || low_trimmed.contains("gradlew test")
                        || low_trimmed.contains("connectedandroidtest")
                        || low_trimmed.contains("dotnet test")
                        || low_trimmed.contains("swift test")
                        || low_trimmed.contains("flutter test")
                        || low_trimmed.contains("ctest")
                        || low_trimmed.contains("bazel test")
                        || low_trimmed.contains("nx test")
                        || low_trimmed.contains("nx affected -t test")
                        || low_trimmed.contains("turbo test")
                        || low_trimmed.contains("turbo run test")
                        || (low_trimmed.contains("xcodebuild") && low_trimmed.contains("test")));

                let has_build_cmd = !is_name_line
                    && (low_trimmed.contains("docker build")
                        || low_trimmed.contains("docker compose build")
                        || low_trimmed.contains("buildx build")
                        || low_trimmed.contains("cargo build --release")
                        || low_trimmed.contains("npm run build")
                        || low_trimmed.contains("pnpm build")
                        || low_trimmed.contains("pnpm run build")
                        || low_trimmed.contains("yarn build")
                        || low_trimmed.contains("bun run build")
                        || low_trimmed.contains("gradlew assemble")
                        || low_trimmed.contains("gradlew build")
                        || low_trimmed.contains("dotnet build")
                        || low_trimmed.contains("swift build")
                        || low_trimmed.contains("flutter build")
                        || low_trimmed.contains("cmake --build")
                        || low_trimmed.contains("make -j")
                        || low_trimmed.contains("bazel build")
                        || low_trimmed.contains("nx affected")
                        || low_trimmed.contains("nx run-many")
                        || low_trimmed.contains("turbo build")
                        || low_trimmed.contains("turbo run build")
                        || (low_trimmed.contains("xcodebuild") && !low_trimmed.contains("test")));

                let raw_low = raw_line.to_ascii_lowercase();
                if raw_low.contains("# minici-rust-bootstrap") {
                    // Cached Rust bootstrap action (< 2s)
                } else if raw_low.contains("# minici-rust-accelerated:nextest") {
                    step_mins += 1.45;
                } else if raw_low.contains("# minici-rust-accelerated:")
                    || raw_low.contains("# minici-rust-everywhere")
                {
                    step_mins = (step_mins - 0.20).max(0.45);
                } else if has_test_cmd || has_build_cmd {
                    if has_test_cmd && !cur_step_counted_test {
                        step_mins += 2.5;
                        cur_step_counted_test = true;
                    }
                    if has_build_cmd && !cur_step_counted_build {
                        step_mins += 3.5;
                        cur_step_counted_build = true;
                    }
                } else if trimmed.starts_with("- name:")
                    || trimmed.starts_with("- run:")
                    || trimmed.starts_with("- uses:")
                {
                    step_mins += 0.25;
                }
            }
        }
    }

    if uses_local_reusable_workflow && !saw_runs_on && !saw_steps {
        return None;
    }

    // Apply warm-cache speedup discount (~15%) when `uses_cache` is active on multi-minute build/test jobs (fixes H2!)
    let adjusted_step_mins = if uses_cache && step_mins > 1.5 {
        0.85 + (step_mins - 0.85) * 0.82
    } else {
        step_mins
    };
    let mut raw_mins = if let Some(em) = explicit_est_mins {
        em
    } else if step_est_mins_sum > 0.0 {
        step_est_mins_sum.max(0.2)
    } else if saw_steps {
        adjusted_step_mins
    } else {
        3.5
    };
    if explicit_est_mins.is_none() && step_est_mins_sum <= 0.0 {
        if let Some(tm) = timeout_minutes_val {
            raw_mins = raw_mins.min(tm.max(0.2));
        }
    }
    raw_mins = (raw_mins * 100.0).round() / 100.0;

    let non_empty_axis_defs: Vec<&(String, Vec<String>)> = axis_counts
        .iter()
        .filter(|(_, items)| !items.is_empty())
        .collect();
    let non_empty_axes = non_empty_axis_defs.len();
    let mut product_count = 1usize;
    for (_, items) in &non_empty_axis_defs {
        product_count = product_count.saturating_mul(items.len());
    }

    let cell_matches_kv = |cell: &[(String, String)], ek: &str, ev: &str| -> bool {
        if let Some((_, cv)) = cell.iter().find(|(ck, _)| ck == ek) {
            return cv == ev;
        }
        if ek == "os" || ek == "runner" || ek == "runs-on" {
            for (_, cv) in cell {
                if cv.starts_with('{') {
                    if let Some(flow_os) = extract_os_from_flow_map(cv) {
                        if flow_os == ev {
                            return true;
                        }
                    }
                }
            }
        }
        false
    };
    let extract_cell_os = |cell: &[(String, String)]| -> Option<String> {
        for (ck, cv) in cell {
            if ck == "os" || ck == "runner" || ck == "runs-on" {
                return Some(cv.clone());
            }
            if cv.starts_with('{') {
                if let Some(flow_os) = extract_os_from_flow_map(cv) {
                    return Some(flow_os);
                }
            }
        }
        None
    };

    let (matrix_count, bucket_jobs_exact, is_matrix_job) = if non_empty_axes > 0 && product_count <= 4096 {
        let mut cells: Vec<Vec<(String, String)>> = vec![Vec::new()];
        for (ax_name, ax_vals) in &non_empty_axis_defs {
            let mut next_cells = Vec::with_capacity(cells.len() * ax_vals.len());
            for prefix in &cells {
                for v in ax_vals {
                    let mut c = prefix.clone();
                    c.push((ax_name.clone(), v.clone()));
                    next_cells.push(c);
                }
            }
            cells = next_cells;
        }
        if !exclude_entries.is_empty() {
            cells.retain(|cell| {
                !exclude_entries.iter().any(|ex| {
                    !ex.is_empty() && ex.iter().all(|(ek, ev)| cell_matches_kv(cell, ek, ev))
                })
            });
        }
        for inc_pairs in &include_entries {
            if inc_pairs.is_empty() {
                continue;
            }
            let mut adds_new_cell = false;
            let mut matched_any_axis = false;
            for (k, v) in inc_pairs {
                if let Some((_, existing_vals)) =
                    non_empty_axis_defs.iter().find(|(ak, _)| ak == k)
                {
                    matched_any_axis = true;
                    if !existing_vals.iter().any(|ev| ev == v) {
                        adds_new_cell = true;
                    }
                }
            }
            let already_present = cells.iter().any(|c| {
                inc_pairs.iter().all(|(ik, iv)| cell_matches_kv(c, ik, iv))
            });
            if (!matched_any_axis || adds_new_cell) && !already_present {
                cells.push(inc_pairs.clone());
            }
        }
        let m_cnt = cells.len().max(1);
        let mut os_buckets: Vec<(String, usize)> = Vec::new();
        for c in &cells {
            if let Some(os_lbl) = extract_cell_os(c) {
                if let Some(found) = os_buckets.iter_mut().find(|x| x.0 == os_lbl) {
                    found.1 += 1;
                } else {
                    os_buckets.push((os_lbl, 1));
                }
            }
        }
        let is_mat = product_count > 1 || m_cnt > 1 || os_buckets.len() > 1;
        (m_cnt, os_buckets, is_mat)
    } else {
        let mut include_os_list: Vec<String> = Vec::new();
        let mut extra_include_items = 0usize;
        for inc_pairs in &include_entries {
            let inc_os = inc_pairs
                .iter()
                .find(|(k, _)| k == "os" || k == "runner" || k == "runs-on")
                .map(|(_, v)| v.clone());
            if non_empty_axes == 0 {
                extra_include_items += 1;
                include_os_list.push(inc_os.unwrap_or_else(|| "ubuntu-latest".to_string()));
            } else {
                extra_include_items += 1;
                if let Some(os_v) = inc_os {
                    include_os_list.push(os_v);
                }
            }
        }
        let m_cnt = if non_empty_axes == 0 && extra_include_items > 0 {
            extra_include_items
        } else {
            product_count.saturating_sub(exclude_entries.len()) + extra_include_items
        }
        .max(1);
        let mut os_buckets: Vec<(String, usize)> = Vec::new();
        for os_l in &include_os_list {
            if let Some(found) = os_buckets.iter_mut().find(|x| x.0 == *os_l) {
                found.1 += 1;
            } else {
                os_buckets.push((os_l.clone(), 1));
            }
        }
        let is_mat = m_cnt > 1 || os_buckets.len() > 1;
        (m_cnt, os_buckets, is_mat)
    };

    if !bucket_jobs_exact.is_empty() && raw_runs_on.contains("${{") {
        for (os_label, per_os_jobs) in bucket_jobs_exact {
            if per_os_jobs == 0 {
                continue;
            }
            let sku = resolve_runner_sku(&os_label);
            let clean_os_lbl = os_label
                .trim()
                .trim_end_matches(',')
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            let billed_per_job = (select_provider_plugin(sku.provider)
                .round_job_minutes(raw_mins, sku.ceil_to_whole_minute)
                * 100.0)
                .round()
                / 100.0;
            let total_billed = (billed_per_job * (per_os_jobs as f64) * 100.0).round() / 100.0;
            out_jobs.push(ExpandedJobCost {
                workflow_file: normalize_ci_path(wf_rel),
                job_id: format!("{} [{}]", job_id, clean_os_lbl),
                line_start,
                line_end,
                matrix_combination_count: per_os_jobs,
                runner_label: clean_os_lbl,
                provider: sku.provider.to_string(),
                os: sku.os.to_string(),
                vcpus: sku.vcpus,
                est_raw_duration_mins_per_job: (raw_mins * 100.0).round() / 100.0,
                billed_mins_per_job: billed_per_job,
                total_billed_mins_all_matrix: total_billed,
                rate_per_min_usd: sku.rate_per_min_usd,
                run_compute_cost_usd: ((total_billed * sku.rate_per_min_usd) * 10000.0).round()
                    / 10000.0,
                blacksmith_run_cost_usd: ((raw_mins
                    * (per_os_jobs as f64)
                    * sku.blacksmith_equiv_rate_usd)
                    * 10000.0)
                    .round()
                    / 10000.0,
                self_hosted_spot_run_cost_usd: ((raw_mins
                    * (per_os_jobs as f64)
                    * sku.self_hosted_spot_rate_usd)
                    * 10000.0)
                    .round()
                    / 10000.0,
                has_concurrency_cancel: has_job_concurrency,
                has_timeout_minutes,
                has_sparse_or_shallow_clone: has_sparse,
                uses_cache,
                uploads_artifacts,
                artifact_retention_days,
                monthly_runs_for_job: None,
            });
        }
    } else {
        let sku = resolve_runner_sku(&raw_runs_on);
        let billed_per_job = (select_provider_plugin(sku.provider)
            .round_job_minutes(raw_mins, sku.ceil_to_whole_minute)
            * 100.0)
            .round()
            / 100.0;
        let total_billed = (billed_per_job * (matrix_count as f64) * 100.0).round() / 100.0;
        let display_runner = if raw_runs_on.contains("${{") {
            sku.sku_label.to_string()
        } else {
            raw_runs_on
                .trim()
                .trim_end_matches(',')
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string()
        };
        out_jobs.push(ExpandedJobCost {
            workflow_file: normalize_ci_path(wf_rel),
            job_id: job_id.to_string(),
            line_start,
            line_end,
            matrix_combination_count: matrix_count,
            runner_label: display_runner,
            provider: sku.provider.to_string(),
            os: sku.os.to_string(),
            vcpus: sku.vcpus,
            est_raw_duration_mins_per_job: (raw_mins * 100.0).round() / 100.0,
            billed_mins_per_job: billed_per_job,
            total_billed_mins_all_matrix: total_billed,
            rate_per_min_usd: sku.rate_per_min_usd,
            run_compute_cost_usd: ((total_billed * sku.rate_per_min_usd) * 10000.0).round()
                / 10000.0,
            blacksmith_run_cost_usd: ((raw_mins
                * (matrix_count as f64)
                * sku.blacksmith_equiv_rate_usd)
                * 10000.0)
                .round()
                / 10000.0,
            self_hosted_spot_run_cost_usd: ((raw_mins
                * (matrix_count as f64)
                * sku.self_hosted_spot_rate_usd)
                * 10000.0)
                .round()
                / 10000.0,
            has_concurrency_cancel: has_job_concurrency,
            has_timeout_minutes,
            has_sparse_or_shallow_clone: has_sparse,
            uses_cache,
            uploads_artifacts,
            artifact_retention_days,
            monthly_runs_for_job: None,
        });
    }
    Some((raw_mins, matrix_count, needs_ids, job_display_name, is_matrix_job))
}

/// Computes BOTH `(critical_path_mins, max_parallel_stage_width)` on a workflow's `needs:` DAG (Fixes Bug #9 & Bug #23).
fn compute_dag_critical_path_and_width(dag_nodes: &[(String, f64, usize, Vec<String>)]) -> (f64, usize) {
    if dag_nodes.is_empty() {
        return (0.0, 0);
    }
    let mut dur_map: HashMap<&str, f64> = HashMap::new();
    let mut deps_map: HashMap<&str, &[String]> = HashMap::new();
    for (id, dur, _, deps) in dag_nodes {
        dur_map.insert(id.as_str(), *dur);
        deps_map.insert(id.as_str(), deps.as_slice());
    }
    fn longest_and_level<'a>(
        node: &'a str,
        dur_map: &HashMap<&'a str, f64>,
        deps_map: &HashMap<&'a str, &'a [String]>,
        memo: &mut HashMap<&'a str, (f64, usize)>,
        visiting: &mut Vec<&'a str>,
        depth: usize,
    ) -> (f64, usize, bool) {
        if let Some(&cached) = memo.get(node) {
            return (cached.0, cached.1, false);
        }
        let own = dur_map.get(node).copied().unwrap_or(0.0);
        if depth > 32 {
            return (own, 0, true);
        }
        visiting.push(node);
        let (max_dep_dur, max_dep_lvl, had_dep, hit_cycle) = deps_map
            .get(node)
            .map(|deps| {
                let mut md = 0.0_f64;
                let mut ml = 0usize;
                let mut any = false;
                let mut cyc = false;
                for d in *deps {
                    if dur_map.contains_key(d.as_str()) {
                        if visiting.contains(&d.as_str()) {
                            cyc = true;
                            continue;
                        }
                        any = true;
                        let (dd, dl, dc) = longest_and_level(
                            d.as_str(),
                            dur_map,
                            deps_map,
                            memo,
                            visiting,
                            depth + 1,
                        );
                        if dc {
                            cyc = true;
                        }
                        md = md.max(dd);
                        ml = ml.max(dl);
                    }
                }
                (md, ml, any, cyc)
            })
            .unwrap_or((0.0, 0, false, false));
        visiting.pop();
        let level = if had_dep { max_dep_lvl + 1 } else { 0 };
        let res = (own + max_dep_dur, level);
        if !hit_cycle {
            memo.insert(node, res);
        }
        (res.0, res.1, hit_cycle)
    }
    let mut memo: HashMap<&str, (f64, usize)> = HashMap::new();
    let mut visiting: Vec<&str> = Vec::new();
    let mut stage_widths: HashMap<usize, usize> = HashMap::new();
    let mut max_cp = 0.0_f64;
    for (id, _, exp_cnt, _) in dag_nodes {
        visiting.clear();
        let (cp, lvl, _) = longest_and_level(id.as_str(), &dur_map, &deps_map, &mut memo, &mut visiting, 0);
        max_cp = max_cp.max(cp);
        *stage_widths.entry(lvl).or_insert(0) += *exp_cnt;
    }
    let max_width = stage_widths.values().copied().max().unwrap_or(0);
    (max_cp, max_width)
}

/// Returns `(critical_path_mins, max_parallel_stage_width, triggers_on_pr_or_push, ast_meta)` for a GitHub Actions workflow file.
fn parse_github_workflow_file(
    wf_path: &Path,
    rel_name: &str,
    has_repo_gosum: bool,
    out_jobs: &mut Vec<ExpandedJobCost>,
) -> (f64, usize, bool, WorkflowAstMeta) {
    let content = match fs::read_to_string(wf_path) {
        Ok(c) => c,
        Err(_) => return (0.0, 0, false, WorkflowAstMeta::default()),
    };
    let triggers_pr_or_push = workflow_triggers_on_pr_or_push(&content.to_ascii_lowercase());
    let all_lines: Vec<&str> = content.lines().collect();
    let mut has_top_concurrency = false;
    let mut in_top_conc = false;
    let mut has_top_cache = false;
    let mut in_jobs = false;
    let mut jobs_indent = 0usize;
    let mut job_header_indent: Option<usize> = None;
    let mut current_job_id: Option<String> = None;
    let mut current_job_start_line = 1usize;
    let mut current_job_end_line = 1usize;
    let mut current_job_lines: Vec<&str> = Vec::new();
    let mut meta = WorkflowAstMeta::default();

    for (line_idx, &line) in all_lines.iter().enumerate() {
        let line_no = line_idx + 1;
        if in_jobs && current_job_id.is_some() && extract_comment_est_minutes(line).is_some() {
            let pre_clean = strip_comment(line).trim();
            if pre_clean.is_empty() || pre_clean.starts_with('#') {
                current_job_lines.push(line);
                current_job_end_line = line_no;
                continue;
            }
        }
        let clean = strip_comment(line);
        let trimmed = clean.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent_of(clean);

        if !in_jobs {
            if ind == 0 {
                in_top_conc = trimmed.starts_with("concurrency:");
                if in_top_conc {
                    let low_t = trimmed.to_ascii_lowercase();
                    if low_t.contains("cancel-in-progress: true")
                        || low_t.contains("cancel-in-progress: ${{")
                    {
                        has_top_concurrency = true;
                    }
                }
            } else if in_top_conc {
                let low_t = trimmed.to_ascii_lowercase();
                if let Some(cip) = low_t.strip_prefix("cancel-in-progress:") {
                    let cv = cip.trim().trim_matches('"').trim_matches('\'');
                    if cv == "true" || cv.contains("${{") {
                        has_top_concurrency = true;
                    }
                }
            }
            if line_has_cache_signal(&trimmed.to_ascii_lowercase()) {
                has_top_cache = true;
            }
        }
        if ind == 0 && trimmed == "jobs:" {
            in_jobs = true;
            in_top_conc = false;
            jobs_indent = 0;
            continue;
        }
        if in_jobs {
            if ind == 0 && !trimmed.starts_with('#') {
                if let Some(jid) = current_job_id.take() {
                    if let Some((dur, exp_c, deps, disp_nm, is_mat)) = analyze_github_job_block(
                        rel_name,
                        &jid,
                        &current_job_lines,
                        current_job_start_line,
                        current_job_end_line,
                        has_top_concurrency,
                        has_top_cache,
                        has_repo_gosum,
                        out_jobs,
                    ) {
                        if let Some(dn) = disp_nm {
                            meta.job_display_names.push((jid.clone(), dn));
                        }
                        if is_mat {
                            meta.matrix_job_ids.push(jid.clone());
                        }
                        meta.dag_nodes.push((jid, dur, exp_c, deps));
                    }
                    current_job_lines.clear();
                }
                in_jobs = false;
                continue;
            }
            if job_header_indent.is_none()
                && ind > jobs_indent
                && trimmed.ends_with(':')
                && !trimmed.starts_with('-')
            {
                let cand = trimmed
                    .trim_end_matches(':')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim();
                if !cand.is_empty() && !cand.contains(':') {
                    job_header_indent = Some(ind);
                }
            }
            if Some(ind) == job_header_indent && trimmed.ends_with(':') && !trimmed.starts_with('-') {
                let cand_raw = trimmed.trim_end_matches(':').trim();
                let cand_clean = cand_raw.trim_matches('"').trim_matches('\'').trim();
                if let Some(jid) = current_job_id.take() {
                    if let Some((dur, exp_c, deps, disp_nm, is_mat)) = analyze_github_job_block(
                        rel_name,
                        &jid,
                        &current_job_lines,
                        current_job_start_line,
                        current_job_end_line,
                        has_top_concurrency,
                        has_top_cache,
                        has_repo_gosum,
                        out_jobs,
                    ) {
                        if let Some(dn) = disp_nm {
                            meta.job_display_names.push((jid.clone(), dn));
                        }
                        if is_mat {
                            meta.matrix_job_ids.push(jid.clone());
                        }
                        meta.dag_nodes.push((jid, dur, exp_c, deps));
                    }
                    current_job_lines.clear();
                }
                if !cand_clean.is_empty() && !cand_clean.contains(':') {
                    current_job_id = Some(cand_clean.to_string());
                    current_job_start_line = line_no;
                    current_job_end_line = line_no;
                    if let Some(hash_idx) = find_unquoted_comment_idx(line) {
                        if extract_comment_est_minutes(line).is_some() {
                            current_job_lines.push(&line[hash_idx..]);
                        }
                    }
                }
            } else if current_job_id.is_some() {
                current_job_lines.push(line);
                current_job_end_line = line_no;
            }
        }
    }

    if let Some(jid) = current_job_id.take() {
        if let Some((dur, exp_c, deps, disp_nm, is_mat)) = analyze_github_job_block(
            rel_name,
            &jid,
            &current_job_lines,
            current_job_start_line,
            current_job_end_line,
            has_top_concurrency,
            has_top_cache,
            has_repo_gosum,
            out_jobs,
        ) {
            if let Some(dn) = disp_nm {
                meta.job_display_names.push((jid.clone(), dn));
            }
            if is_mat {
                meta.matrix_job_ids.push(jid.clone());
            }
            meta.dag_nodes.push((jid, dur, exp_c, deps));
        }
    }

    let (cp, width) = compute_dag_critical_path_and_width(&meta.dag_nodes);
    (cp, width, triggers_pr_or_push, meta)
}

/// Multi-vendor per-job parser for `.gitlab-ci.yml`, `.circleci/config.yml`, `bitbucket-pipelines.yml`, and `buildspec.yml`
/// (Fixes Bug #12 completely: per-job runner & multi-line `tags:`, step command duration heuristics,
/// GitLab `parallel: matrix:` combinatorics, CircleCI `docker_layer_caching: true` `+$0.12` fee, and `CiProvider::round_job_minutes`).
fn parse_gitlab_or_other_ci(
    ci_path: &Path,
    rel_name: &str,
    provider_hint: &str,
    out_jobs: &mut Vec<ExpandedJobCost>,
) -> (f64, usize) {
    let content = match fs::read_to_string(ci_path) {
        Ok(c) => c,
        Err(_) => return (0.0, 0),
    };

    #[derive(Clone)]
    struct ParsedVendorJob {
        name: String,
        stage: String,
        line_start: usize,
        line_end: usize,
        sku_label: String,
        parallel_count: usize,
        explicit_mins: Option<f64>,
        step_mins: f64,
        saw_script_steps: bool,
        has_dlc: bool,
        has_timeout: bool,
        uploads_artifacts: bool,
        retention_days: Option<u32>,
        has_interruptible: bool,
        has_cache: bool,
        in_tags_list: bool,
        in_parallel_matrix: bool,
        matrix_item_axes: Vec<Vec<usize>>,
    }

    let is_codebuild = provider_hint == "BUILD_GENERAL1_SMALL";
    let mut default_sku = provider_hint.to_string();
    let mut default_cache = false;
    let mut default_interruptible = false;
    let mut default_timeout = false;
    let mut jobs_list: Vec<ParsedVendorJob> = Vec::new();
    let mut current: Option<ParsedVendorJob> = None;
    let mut in_circle_jobs_section = false;
    let mut circle_job_indent: Option<usize> = None;
    let mut in_default_section = false;
    let total_lines = content.lines().count().max(1);

    let flush_job = |cur: &mut Option<ParsedVendorJob>, list: &mut Vec<ParsedVendorJob>| {
        if let Some(mut j) = cur.take() {
            if !j.matrix_item_axes.is_empty() {
                let sum_prods: usize = j
                    .matrix_item_axes
                    .iter()
                    .map(|axes| axes.iter().copied().filter(|&n| n > 0).product::<usize>())
                    .sum();
                if sum_prods > j.parallel_count {
                    j.parallel_count = sum_prods;
                }
            }
            list.push(j);
        }
    };

    for (line_idx, line) in content.lines().enumerate() {
        let line_no = line_idx + 1;
        if let Some(m) = extract_comment_est_minutes(line) {
            if let Some(ref mut job) = current {
                job.explicit_mins = Some(m);
                job.line_end = line_no;
            }
        }
        let clean = strip_comment(line);
        let trimmed = clean.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent_of(clean);
        let low = trimmed.to_ascii_lowercase();

        if low.starts_with("max-time:") {
            default_timeout = true;
        }

        if ind == 0 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            let key = trimmed.trim_end_matches(':').trim();
            if is_codebuild && key == "artifacts" {
                if let Some(ref mut job) = current {
                    job.uploads_artifacts = true;
                    job.step_mins += 0.35;
                }
                continue;
            }
            if is_codebuild && key == "cache" {
                default_cache = true;
                if let Some(ref mut job) = current {
                    job.has_cache = true;
                }
                continue;
            }
            flush_job(&mut current, &mut jobs_list);
            in_default_section = key == "default";
            in_circle_jobs_section = key == "jobs";
            if is_codebuild && key == "phases" {
                current = Some(ParsedVendorJob {
                    name: "codebuild-phases".to_string(),
                    stage: "test".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    sku_label: default_sku.clone(),
                    parallel_count: 1,
                    explicit_mins: None,
                    step_mins: 0.5,
                    saw_script_steps: true,
                    has_dlc: false,
                    has_timeout: default_timeout,
                    uploads_artifacts: false,
                    retention_days: None,
                    has_interruptible: default_interruptible,
                    has_cache: default_cache,
                    in_tags_list: false,
                    in_parallel_matrix: false,
                    matrix_item_axes: Vec::new(),
                });
                continue;
            }
            if !is_codebuild
                && !key.is_empty()
                && !key.starts_with('.')
                && !key.starts_with(':')
                && !key.contains(": ")
                && !key.contains(" :")
                && !matches!(
                    key,
                    "stages"
                        | "variables"
                        | "default"
                        | "include"
                        | "workflow"
                        | "workflows"
                        | "image"
                        | "version"
                        | "orbs"
                        | "executors"
                        | "commands"
                        | "pipelines"
                        | "options"
                        | "definitions"
                        | "phases"
                        | "jobs"
                        | "cache"
                        | "env"
                        | "reports"
                        | "artifacts"
                        | "logs"
                        | "batch"
                )
            {
                current = Some(ParsedVendorJob {
                    name: key.to_string(),
                    stage: "test".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    sku_label: default_sku.clone(),
                    parallel_count: 1,
                    explicit_mins: None,
                    step_mins: 0.5,
                    saw_script_steps: false,
                    has_dlc: false,
                    has_timeout: default_timeout,
                    uploads_artifacts: false,
                    retention_days: None,
                    has_interruptible: default_interruptible,
                    has_cache: default_cache,
                    in_tags_list: false,
                    in_parallel_matrix: false,
                    matrix_item_axes: Vec::new(),
                });
            }
            if key == "cache" {
                default_cache = true;
            }
            continue;
        }

        if in_circle_jobs_section {
            if circle_job_indent.is_none() && ind > 0 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
                circle_job_indent = Some(ind);
            }
            if Some(ind) == circle_job_indent && trimmed.ends_with(':') && !trimmed.starts_with('-') {
                flush_job(&mut current, &mut jobs_list);
                let jname = trimmed.trim_end_matches(':').trim().to_string();
                current = Some(ParsedVendorJob {
                    name: jname,
                    stage: "test".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    sku_label: default_sku.clone(),
                    parallel_count: 1,
                    explicit_mins: None,
                    step_mins: 0.5,
                    saw_script_steps: false,
                    has_dlc: false,
                    has_timeout: default_timeout,
                    uploads_artifacts: false,
                    retention_days: None,
                    has_interruptible: default_interruptible,
                    has_cache: default_cache,
                    in_tags_list: false,
                    in_parallel_matrix: false,
                    matrix_item_axes: Vec::new(),
                });
                continue;
            }
        }

        // Fixes D8: Flush previous step on every `- step:` in Bitbucket Pipelines!
        if trimmed.starts_with("- step:") && !in_circle_jobs_section {
            flush_job(&mut current, &mut jobs_list);
            let step_idx = jobs_list.len() + 1;
            current = Some(ParsedVendorJob {
                name: format!("step-{}", step_idx),
                stage: format!("step-stage-{}", step_idx),
                line_start: line_no,
                line_end: line_no,
                sku_label: default_sku.clone(),
                parallel_count: 1,
                explicit_mins: None,
                step_mins: 0.5,
                saw_script_steps: false,
                has_dlc: false,
                has_timeout: default_timeout,
                uploads_artifacts: false,
                retention_days: None,
                has_interruptible: default_interruptible,
                has_cache: default_cache,
                in_tags_list: false,
                in_parallel_matrix: false,
                matrix_item_axes: Vec::new(),
            });
            continue;
        }

        let target_cache_signal = line_has_cache_signal(&low);
        if let Some(ref mut job) = current {
            job.line_end = line_no;
            if job.in_tags_list {
                if trimmed.starts_with('-') {
                    let tg = trimmed
                        .trim_start_matches('-')
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !tg.is_empty() {
                        job.sku_label = tg.to_string();
                        job.in_tags_list = false;
                    }
                    continue;
                } else {
                    job.in_tags_list = false;
                }
            }
            if job.in_parallel_matrix {
                if trimmed.starts_with('-') && trimmed.contains(':') && trimmed.contains('[') {
                    let list = parse_inline_yaml_list(trimmed);
                    job.matrix_item_axes.push(if list.is_empty() { Vec::new() } else { vec![list.len()] });
                    continue;
                } else if trimmed == "-" {
                    job.matrix_item_axes.push(Vec::new());
                    continue;
                } else if trimmed.starts_with('-') && trimmed.contains(':') {
                    job.matrix_item_axes.push(vec![0]);
                    continue;
                } else if trimmed.contains(':') && trimmed.contains('[') {
                    let list = parse_inline_yaml_list(trimmed);
                    if !list.is_empty() {
                        if job.matrix_item_axes.is_empty() {
                            job.matrix_item_axes.push(Vec::new());
                        }
                        if let Some(cur_item) = job.matrix_item_axes.last_mut() {
                            cur_item.push(list.len());
                        }
                    }
                    continue;
                } else if trimmed.starts_with('-') && !trimmed.contains(':') {
                    if let Some(cur_item) = job.matrix_item_axes.last_mut() {
                        if let Some(last_ax) = cur_item.last_mut() {
                            *last_ax += 1;
                        }
                    }
                    continue;
                } else if trimmed.ends_with(':') && !trimmed.starts_with('-') && trimmed != "matrix:" && trimmed != "script:" && trimmed != "steps:" {
                    if job.matrix_item_axes.is_empty() {
                        job.matrix_item_axes.push(Vec::new());
                    }
                    if let Some(cur_item) = job.matrix_item_axes.last_mut() {
                        cur_item.push(0);
                    }
                    continue;
                } else if trimmed == "script:" || trimmed == "steps:" || trimmed.starts_with("tags:") || trimmed.starts_with("stage:") {
                    job.in_parallel_matrix = false;
                }
            }

            if trimmed.starts_with("stage:") {
                if let Some(st) = trimmed.splitn(2, ':').nth(1) {
                    let st_clean = st.trim().trim_matches('"').trim_matches('\'');
                    if !st_clean.is_empty() {
                        job.stage = st_clean.to_string();
                    }
                }
            }

            if target_cache_signal || low.starts_with("- caches") || low == "- docker" {
                job.has_cache = true;
            }
            if low.contains("docker_layer_caching: true") || low.contains("dlc: true") {
                job.has_dlc = true;
            }
            if low.starts_with("timeout:") || low.starts_with("max-time:") || low.starts_with("no_output_timeout:") {
                job.has_timeout = true;
            }
            if trimmed.starts_with("name:") && job.name.starts_with("step-") {
                if let Some(nm) = trimmed.splitn(2, ':').nth(1) {
                    let clean_nm = nm.trim().trim_matches('"').trim_matches('\'');
                    if !clean_nm.is_empty() {
                        job.name = clean_nm.to_string();
                    }
                }
            }
            if trimmed.starts_with("tags:") {
                let rhs = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
                if rhs.is_empty() {
                    job.in_tags_list = true;
                } else {
                    let list = parse_inline_yaml_list(rhs);
                    if let Some(first) = list.first() {
                        job.sku_label = first.clone();
                    }
                }
            } else if trimmed.starts_with("resource_class:") {
                if let Some(rc) = trimmed.splitn(2, ':').nth(1) {
                    let rc_clean = rc.trim().trim_matches('"').trim_matches('\'');
                    job.sku_label = if rc_clean.contains("macos") || rc_clean.contains("arm.") || rc_clean.contains("gpu") {
                        rc_clean.to_string()
                    } else {
                        format!("docker-{}", rc_clean)
                    };
                }
            } else if trimmed.starts_with("macos:") {
                job.sku_label = "macos.m2pro.medium".to_string();
            } else if trimmed.starts_with("size:") {
                if let Some(sz) = trimmed.splitn(2, ':').nth(1) {
                    let sz_clean = sz.trim().trim_matches('"').trim_matches('\'');
                    job.sku_label = format!("bitbucket-{}", sz_clean);
                }
            } else if trimmed.starts_with("parallel:") || trimmed.starts_with("parallelism:") {
                let rhs = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
                if rhs.is_empty() {
                    job.in_parallel_matrix = true;
                } else if let Ok(n) = rhs.parse::<usize>() {
                    job.parallel_count = n.max(1);
                }
            } else if trimmed == "matrix:" {
                job.in_parallel_matrix = true;
            } else if trimmed.starts_with("est-minutes:") || trimmed.starts_with("estimated-minutes:") {
                if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                    if let Ok(m) = val.trim().parse::<f64>() {
                        job.explicit_mins = Some(m.max(0.2));
                    }
                }
            } else if trimmed == "script:" || trimmed == "steps:" || trimmed == "commands:" {
                job.saw_script_steps = true;
            } else if trimmed.starts_with("artifacts:") || trimmed.contains("store_artifacts") {
                job.uploads_artifacts = true;
                job.step_mins += 0.35;
            } else if trimmed.starts_with("expire_in:") || trimmed.starts_with("retention-days:") {
                if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                    if let Some(days) = parse_retention_days_value(val) {
                        job.retention_days = Some(days);
                        job.uploads_artifacts = true;
                    }
                }
            } else if trimmed.starts_with("interruptible: true") || low.contains("auto-cancel") {
                job.has_interruptible = true;
            } else if job.saw_script_steps && (trimmed.starts_with('-') || trimmed.starts_with("run:") || trimmed.starts_with("command:")) {
                if low.contains("checkout") {
                    job.step_mins += 0.35;
                } else {
                    let is_test = low.contains("pytest")
                        || low.contains("cargo test")
                        || low.contains("npm test")
                        || low.contains("pnpm test")
                        || low.contains("yarn test")
                        || low.contains("jest")
                        || low.contains("vitest")
                        || low.contains("go test")
                        || low.contains("mvn test")
                        || low.contains("gradlew test")
                        || low.contains("dotnet test")
                        || low.contains("flutter test");
                    let is_build = low.contains("docker build")
                        || low.contains("cargo build")
                        || low.contains("npm run build")
                        || low.contains("pnpm build")
                        || low.contains("gradlew assemble")
                        || low.contains("gradlew build")
                        || low.contains("cmake --build")
                        || low.contains("make -j");
                    if is_test || is_build {
                        if is_test {
                            job.step_mins += 2.5;
                        }
                        if is_build {
                            job.step_mins += 3.5;
                        }
                    } else {
                        job.step_mins += 0.35;
                    }
                }
            }
        } else if in_default_section {
            if target_cache_signal {
                default_cache = true;
            }
            if trimmed.starts_with("tags:") {
                let list = parse_inline_yaml_list(trimmed);
                if let Some(first) = list.first() {
                    default_sku = first.clone();
                }
            } else if trimmed.starts_with("interruptible: true") {
                default_interruptible = true;
            } else if low.starts_with("timeout:") {
                default_timeout = true;
            }
        } else if target_cache_signal {
            default_cache = true;
        }
    }
    flush_job(&mut current, &mut jobs_list);

    if jobs_list.is_empty() {
        jobs_list.push(ParsedVendorJob {
            name: "default-pipeline".to_string(),
            stage: "test".to_string(),
            line_start: 1,
            line_end: total_lines,
            sku_label: default_sku,
            parallel_count: 1,
            explicit_mins: Some(4.0),
            step_mins: 4.0,
            saw_script_steps: false,
            has_dlc: false,
            has_timeout: default_timeout,
            uploads_artifacts: false,
            retention_days: None,
            has_interruptible: default_interruptible,
            has_cache: default_cache,
            in_tags_list: false,
            in_parallel_matrix: false,
            matrix_item_axes: Vec::new(),
        });
    }

    let mut stage_max_dur: Vec<(String, f64)> = Vec::new();
    let mut stage_widths: Vec<(String, usize)> = Vec::new();
    for pj in jobs_list {
        let raw_mins = (pj.explicit_mins.unwrap_or_else(|| {
            if pj.saw_script_steps {
                pj.step_mins.max(0.5)
            } else {
                4.0
            }
        }) * 100.0)
            .round()
            / 100.0;
        if let Some(sm) = stage_max_dur.iter_mut().find(|(s, _)| *s == pj.stage) {
            sm.1 = sm.1.max(raw_mins);
        } else {
            stage_max_dur.push((pj.stage.clone(), raw_mins));
        }
        if let Some(sw) = stage_widths.iter_mut().find(|(s, _)| *s == pj.stage) {
            sw.1 += pj.parallel_count;
        } else {
            stage_widths.push((pj.stage.clone(), pj.parallel_count));
        }
        let sku = resolve_runner_sku(&pj.sku_label);
        let billed_per_job = (select_provider_plugin(sku.provider)
            .round_job_minutes(raw_mins, sku.ceil_to_whole_minute)
            * 100.0)
            .round()
            / 100.0;
        let total_billed = (billed_per_job * (pj.parallel_count as f64) * 100.0).round() / 100.0;
        // CircleCI Docker Layer Caching (`DLC`) adds 200 credits = $0.12 per job run!
        let dlc_fee = if pj.has_dlc && sku.provider == "circleci" {
            0.12 * (pj.parallel_count as f64)
        } else {
            0.0
        };
        out_jobs.push(ExpandedJobCost {
            workflow_file: normalize_ci_path(rel_name),
            job_id: pj.name,
            line_start: pj.line_start,
            line_end: pj.line_end,
            matrix_combination_count: pj.parallel_count,
            runner_label: sku.sku_label.to_string(),
            provider: sku.provider.to_string(),
            os: sku.os.to_string(),
            vcpus: sku.vcpus,
            est_raw_duration_mins_per_job: (raw_mins * 100.0).round() / 100.0,
            billed_mins_per_job: billed_per_job,
            total_billed_mins_all_matrix: total_billed,
            rate_per_min_usd: sku.rate_per_min_usd,
            run_compute_cost_usd: (((total_billed * sku.rate_per_min_usd) + dlc_fee) * 10000.0)
                .round()
                / 10000.0,
            blacksmith_run_cost_usd: ((raw_mins
                * (pj.parallel_count as f64)
                * sku.blacksmith_equiv_rate_usd)
                * 10000.0)
                .round()
                / 10000.0,
            self_hosted_spot_run_cost_usd: ((raw_mins
                * (pj.parallel_count as f64)
                * sku.self_hosted_spot_rate_usd)
                * 10000.0)
                .round()
                / 10000.0,
            has_concurrency_cancel: pj.has_interruptible,
            has_timeout_minutes: pj.has_timeout,
            has_sparse_or_shallow_clone: false,
            uses_cache: pj.has_cache,
            uploads_artifacts: pj.uploads_artifacts,
            artifact_retention_days: pj.retention_days,
            monthly_runs_for_job: None,
        });
    }
    let cp_sum: f64 = stage_max_dur.iter().map(|(_, d)| *d).sum();
    let peak_stage_w: usize = stage_widths.iter().map(|(_, w)| *w).max().unwrap_or(0);
    (cp_sum, peak_stage_w)
}

/// Native Google Cloud Build (`cloudbuild.yaml` / `cloudbuild.yml`) parser with `machineType` SKU rates,
/// step duration heuristics, `# est-minutes:` overrides, and exact sub-minute billing (`provider = "gcp-cloud-build"`).
fn parse_gcp_cloudbuild_file(
    ci_path: &Path,
    rel_name: &str,
    out_jobs: &mut Vec<ExpandedJobCost>,
) -> (f64, usize) {
    let content = match fs::read_to_string(ci_path) {
        Ok(c) => c,
        Err(_) => return (0.0, 0),
    };

    let mut runner_label = "cloudbuild-e2-standard-2".to_string();
    let mut rate_per_min = 0.0060_f64;
    let mut vcpus = 2_u32;
    let mut bs_equiv = 0.0040_f64;
    let mut spot_equiv = 0.0010_f64;
    let mut explicit_mins: Option<f64> = None;
    let mut step_mins = 0.50_f64;
    let mut saw_steps = false;
    let mut in_steps = false;
    let mut has_timeout = false;
    let mut has_cache = false;
    let mut uploads_artifacts = false;
    let mut cur_step_docker_builder = false;
    let mut cur_step_counted_build = false;

    for line in content.lines() {
        if let Some(m) = extract_comment_est_minutes(line) {
            explicit_mins = Some(m);
        }
        let clean = strip_comment(line);
        let trimmed = clean.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent_of(clean);
        let low = trimmed.to_ascii_lowercase();

        if ind == 0 && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            in_steps = trimmed == "steps:";
        }

        if low.starts_with("est-minutes:") || low.starts_with("estimated-minutes:") {
            if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                if let Ok(m) = val.trim().parse::<f64>() {
                    if m.is_finite() && m > 0.0 {
                        explicit_mins = Some(m.max(0.2));
                    }
                }
            }
            continue;
        }

        if low.starts_with("machinetype:") || low.starts_with("machine_type:") {
            if let Some(val) = trimmed.splitn(2, ':').nth(1) {
                let mt = val
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_ascii_uppercase();
                match mt.as_str() {
                    "E2_HIGHCPU_8" => {
                        runner_label = "E2_HIGHCPU_8".to_string();
                        rate_per_min = 0.0156;
                        vcpus = 8;
                        bs_equiv = 0.0160;
                        spot_equiv = 0.0040;
                    }
                    "E2_HIGHCPU_32" => {
                        runner_label = "E2_HIGHCPU_32".to_string();
                        rate_per_min = 0.0624;
                        vcpus = 32;
                        bs_equiv = 0.0640;
                        spot_equiv = 0.0160;
                    }
                    "N1_HIGHCPU_8" => {
                        runner_label = "N1_HIGHCPU_8".to_string();
                        rate_per_min = 0.0160;
                        vcpus = 8;
                        bs_equiv = 0.0160;
                        spot_equiv = 0.0040;
                    }
                    "N1_HIGHCPU_32" => {
                        runner_label = "N1_HIGHCPU_32".to_string();
                        rate_per_min = 0.0640;
                        vcpus = 32;
                        bs_equiv = 0.0640;
                        spot_equiv = 0.0160;
                    }
                    "E2_MEDIUM" => {
                        runner_label = "E2_MEDIUM".to_string();
                        rate_per_min = 0.0030;
                        vcpus = 2;
                        bs_equiv = 0.0040;
                        spot_equiv = 0.0010;
                    }
                    _ => {
                        runner_label = "cloudbuild-e2-standard-2".to_string();
                        rate_per_min = 0.0060;
                        vcpus = 2;
                        bs_equiv = 0.0040;
                        spot_equiv = 0.0010;
                    }
                }
            }
            continue;
        }

        if low.starts_with("timeout:") {
            has_timeout = true;
        }
        if low.starts_with("artifacts:") {
            uploads_artifacts = true;
            step_mins += 0.35;
        }
        if line_has_cache_signal(&low) || low.contains("kaniko") {
            has_cache = true;
        }

        let is_step_start = trimmed.starts_with("- name:")
            || trimmed.starts_with("- id:")
            || trimmed.starts_with("- entrypoint:");
        if is_step_start {
            in_steps = true;
            saw_steps = true;
            step_mins += 0.25;
            cur_step_docker_builder = low.contains("cloud-builders/docker");
            cur_step_counted_build = false;
        } else if in_steps && trimmed.starts_with("name:") && low.contains("cloud-builders/docker") {
            cur_step_docker_builder = true;
        }

        if in_steps {
            let is_test = low.contains("pytest")
                || low.contains("npm test")
                || low.contains("pnpm test")
                || low.contains("yarn test")
                || low.contains("go test")
                || low.contains("cargo test")
                || low.contains("mvn test")
                || low.contains("gradlew test")
                || low.contains("dotnet test")
                || low.contains("jest")
                || low.contains("vitest");
            let is_build = low.contains("docker build")
                || low.contains("gcloud builds")
                || low.contains("buildx build")
                || low.contains("kaniko")
                || low.contains("cargo build --release")
                || low.contains("npm run build")
                || low.contains("gradlew build")
                || low.contains("cmake --build")
                || (cur_step_docker_builder
                    && (low.contains("'build'")
                        || low.contains("\"build\"")
                        || low.contains("[build")
                        || low == "- build"));
            if is_test {
                step_mins += 2.50;
            }
            if is_build && !cur_step_counted_build {
                step_mins += 3.50;
                cur_step_counted_build = true;
            }
        }
    }

    let total_cb_lines = content.lines().count().max(1);
    let raw_mins = (explicit_mins.unwrap_or(if saw_steps { step_mins.max(0.5) } else { 3.5 })
        * 100.0)
        .round()
        / 100.0;
    let billed_per_job = (select_provider_plugin("gcp-cloud-build")
        .round_job_minutes(raw_mins, false)
        * 100.0)
        .round()
        / 100.0;
    out_jobs.push(ExpandedJobCost {
        workflow_file: normalize_ci_path(rel_name),
        job_id: "cloud-build".to_string(),
        line_start: 1,
        line_end: total_cb_lines,
        matrix_combination_count: 1,
        runner_label,
        provider: "gcp-cloud-build".to_string(),
        os: "Linux".to_string(),
        vcpus,
        est_raw_duration_mins_per_job: (raw_mins * 100.0).round() / 100.0,
        billed_mins_per_job: billed_per_job,
        total_billed_mins_all_matrix: billed_per_job,
        rate_per_min_usd: rate_per_min,
        run_compute_cost_usd: ((billed_per_job * rate_per_min) * 10000.0).round() / 10000.0,
        blacksmith_run_cost_usd: ((raw_mins * bs_equiv) * 10000.0).round() / 10000.0,
        self_hosted_spot_run_cost_usd: ((raw_mins * spot_equiv) * 10000.0).round() / 10000.0,
        has_concurrency_cancel: false,
        has_timeout_minutes: has_timeout,
        has_sparse_or_shallow_clone: false,
        uses_cache: has_cache,
        uploads_artifacts,
        artifact_retention_days: None,
        monthly_runs_for_job: None,
    });
    (raw_mins, 1)
}

/// Native Azure Pipelines (`azure-pipelines.yml` / `azure-pipelines.yaml`) parser supporting both
/// `jobs:` (`- job: <name>`) and flat `steps:`/`script:` pipelines, `vmImage:` OS resolution,
/// static `strategy: matrix:` combinatorics, and `# est-minutes:` overrides (`provider = "azure-pipelines"`).
fn parse_azure_pipelines_file(
    ci_path: &Path,
    rel_name: &str,
    out_jobs: &mut Vec<ExpandedJobCost>,
) -> (f64, usize) {
    let content = match fs::read_to_string(ci_path) {
        Ok(c) => c,
        Err(_) => return (0.0, 0),
    };

    #[derive(Clone)]
    struct AzJob {
        name: String,
        line_start: usize,
        line_end: usize,
        vm_image: String,
        explicit_mins: Option<f64>,
        step_mins: f64,
        saw_steps: bool,
        has_timeout: bool,
        has_cache: bool,
        has_shallow: bool,
        uploads_artifacts: bool,
        in_matrix: bool,
        matrix_indent: usize,
        matrix_entry_indent: Option<usize>,
        named_matrix_legs: usize,
        leg_vm_images: Vec<String>,
        list_matrix_axes: Vec<usize>,
    }

    let mut default_vm_image = "ubuntu-latest".to_string();
    let mut default_explicit_mins: Option<f64> = None;
    let mut default_timeout = false;
    let mut top_in_matrix = false;
    let mut top_matrix_indent = 0usize;
    let mut top_matrix_entry_indent: Option<usize> = None;
    let mut top_named_legs = 0usize;
    let mut top_list_axes: Vec<usize> = Vec::new();
    let total_az_lines = content.lines().count().max(1);

    let mut jobs_list: Vec<AzJob> = Vec::new();
    let mut current: Option<AzJob> = None;

    for (line_idx, line) in content.lines().enumerate() {
        let line_no = line_idx + 1;
        if let Some(m) = extract_comment_est_minutes(line) {
            if let Some(ref mut j) = current {
                j.explicit_mins = Some(m);
                j.line_end = line_no;
            } else {
                default_explicit_mins = Some(m);
            }
        }
        let clean = strip_comment(line);
        let trimmed = clean.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent_of(clean);
        let low = trimmed.to_ascii_lowercase();

        if trimmed.starts_with("- job:") || trimmed == "- job" {
            if let Some(prev) = current.take() {
                jobs_list.push(prev);
            }
            let raw_name = trimmed
                .split_once(':')
                .map(|(_, v)| v.trim().trim_matches('"').trim_matches('\''))
                .unwrap_or("");
            let jname = if raw_name.is_empty() {
                format!("job-{}", jobs_list.len() + 1)
            } else {
                raw_name.to_string()
            };
            current = Some(AzJob {
                name: jname,
                line_start: line_no,
                line_end: line_no,
                vm_image: default_vm_image.clone(),
                explicit_mins: None,
                step_mins: 0.50,
                saw_steps: false,
                has_timeout: default_timeout,
                has_cache: false,
                has_shallow: false,
                uploads_artifacts: false,
                in_matrix: false,
                matrix_indent: 0,
                matrix_entry_indent: None,
                named_matrix_legs: 0,
                leg_vm_images: Vec::new(),
                list_matrix_axes: Vec::new(),
            });
            continue;
        }

        // Flat pipeline fallback when `steps:` or `- script:` appears without `- job:`
        if current.is_none()
            && jobs_list.is_empty()
            && (trimmed == "steps:"
                || trimmed.starts_with("- script:")
                || trimmed.starts_with("- bash:")
                || trimmed.starts_with("- pwsh:")
                || trimmed.starts_with("- powershell:")
                || trimmed.starts_with("- task:"))
        {
            current = Some(AzJob {
                name: "pipeline".to_string(),
                line_start: line_no,
                line_end: line_no,
                vm_image: default_vm_image.clone(),
                explicit_mins: default_explicit_mins,
                step_mins: 0.50,
                saw_steps: true,
                has_timeout: default_timeout,
                has_cache: false,
                has_shallow: false,
                uploads_artifacts: false,
                in_matrix: false,
                matrix_indent: 0,
                matrix_entry_indent: None,
                named_matrix_legs: top_named_legs,
                leg_vm_images: Vec::new(),
                list_matrix_axes: top_list_axes.clone(),
            });
            if trimmed == "steps:" {
                continue;
            }
        }

        if let Some(ref mut j) = current {
            j.line_end = line_no;
            if j.in_matrix {
                if ind <= j.matrix_indent && !trimmed.starts_with('-') {
                    j.in_matrix = false;
                } else {
                    let e_ind = *j.matrix_entry_indent.get_or_insert(ind);
                    if ind == e_ind && trimmed.contains(':') && !trimmed.starts_with('-') {
                        let (k, rhs) = trimmed.split_once(':').unwrap_or((trimmed, ""));
                        let kl = k.trim().to_ascii_lowercase();
                        if kl != "maxparallel" {
                            let inline = parse_inline_yaml_list(rhs);
                            if !inline.is_empty() {
                                j.list_matrix_axes.push(inline.len());
                            } else {
                                j.named_matrix_legs += 1;
                            }
                        }
                    } else if ind > e_ind && low.starts_with("vmimage:") {
                        if let Some(v) = trimmed.splitn(2, ':').nth(1) {
                            let vc = v.trim().trim_matches('"').trim_matches('\'');
                            if !vc.is_empty() && !vc.contains('$') {
                                j.leg_vm_images.push(vc.to_string());
                            }
                        }
                    }
                    continue;
                }
            }

            if trimmed == "matrix:" {
                j.in_matrix = true;
                j.matrix_indent = ind;
                j.matrix_entry_indent = None;
                continue;
            }
            if low.starts_with("vmimage:") {
                if let Some(v) = trimmed.splitn(2, ':').nth(1) {
                    let vc = v.trim().trim_matches('"').trim_matches('\'');
                    if !vc.is_empty() && !vc.contains('$') {
                        j.vm_image = vc.to_string();
                    }
                }
                continue;
            }
            if low.starts_with("est-minutes:") || low.starts_with("estimated-minutes:") {
                if let Some(v) = trimmed.splitn(2, ':').nth(1) {
                    if let Ok(m) = v.trim().parse::<f64>() {
                        if m.is_finite() && m > 0.0 {
                            j.explicit_mins = Some(m.max(0.2));
                        }
                    }
                }
                continue;
            }
            if low.starts_with("timeoutinminutes:") {
                j.has_timeout = true;
                continue;
            }
            if line_has_cache_signal(&low) || low.contains("cache@2") {
                j.has_cache = true;
            }
            if low.contains("fetchdepth: 1") || low.contains("fetchdepth:1") {
                j.has_shallow = true;
            }
            if low.contains("publishbuildartifacts")
                || low.contains("publishpipelineartifact")
                || low.starts_with("- publish:")
            {
                j.uploads_artifacts = true;
                j.step_mins += 0.35;
            }
            if trimmed == "steps:" {
                j.saw_steps = true;
                continue;
            }
            if trimmed.starts_with("- checkout:") {
                j.saw_steps = true;
                j.step_mins += 0.35;
                continue;
            }

            let is_test = low.contains("pytest")
                || low.contains("npm test")
                || low.contains("pnpm test")
                || low.contains("yarn test")
                || low.contains("go test")
                || low.contains("cargo test")
                || low.contains("dotnet test")
                || low.contains("mvn test")
                || low.contains("gradlew test")
                || low.contains("jest")
                || low.contains("vitest")
                || (low.contains("xcodebuild") && low.contains("test"));
            let is_build = low.contains("docker build")
                || low.contains("docker@2")
                || low.contains("dotnet build")
                || low.contains("cargo build")
                || low.contains("npm run build")
                || low.contains("pnpm build")
                || low.contains("gradlew build")
                || low.contains("cmake --build")
                || (low.contains("xcodebuild") && !low.contains("test"));
            if is_test || is_build {
                j.saw_steps = true;
                if is_test {
                    j.step_mins += 2.50;
                }
                if is_build {
                    j.step_mins += 3.50;
                }
            } else if trimmed.starts_with("- script:")
                || trimmed.starts_with("- bash:")
                || trimmed.starts_with("- pwsh:")
                || trimmed.starts_with("- powershell:")
                || trimmed.starts_with("- task:")
            {
                j.saw_steps = true;
                j.step_mins += 0.25;
            }
        } else {
            if top_in_matrix {
                if ind <= top_matrix_indent && !trimmed.starts_with('-') {
                    top_in_matrix = false;
                } else {
                    let e_ind = *top_matrix_entry_indent.get_or_insert(ind);
                    if ind == e_ind && trimmed.contains(':') && !trimmed.starts_with('-') {
                        let (k, rhs) = trimmed.split_once(':').unwrap_or((trimmed, ""));
                        if !k.trim().eq_ignore_ascii_case("maxparallel") {
                            let inline = parse_inline_yaml_list(rhs);
                            if !inline.is_empty() {
                                top_list_axes.push(inline.len());
                            } else {
                                top_named_legs += 1;
                            }
                        }
                    }
                    continue;
                }
            }
            if trimmed == "matrix:" {
                top_in_matrix = true;
                top_matrix_indent = ind;
                top_matrix_entry_indent = None;
            } else if low.starts_with("vmimage:") {
                if let Some(v) = trimmed.splitn(2, ':').nth(1) {
                    let vc = v.trim().trim_matches('"').trim_matches('\'');
                    if !vc.is_empty() && !vc.contains('$') {
                        default_vm_image = vc.to_string();
                    }
                }
            } else if low.starts_with("est-minutes:") || low.starts_with("estimated-minutes:") {
                if let Some(v) = trimmed.splitn(2, ':').nth(1) {
                    if let Ok(m) = v.trim().parse::<f64>() {
                        if m.is_finite() && m > 0.0 {
                            default_explicit_mins = Some(m.max(0.2));
                        }
                    }
                }
            } else if low.starts_with("timeoutinminutes:") {
                default_timeout = true;
            }
        }
    }

    if let Some(last) = current.take() {
        jobs_list.push(last);
    }
    if jobs_list.is_empty() {
        jobs_list.push(AzJob {
            name: "pipeline".to_string(),
            line_start: 1,
            line_end: total_az_lines,
            vm_image: default_vm_image,
            explicit_mins: default_explicit_mins.or(Some(3.5)),
            step_mins: 3.5,
            saw_steps: false,
            has_timeout: default_timeout,
            has_cache: false,
            has_shallow: false,
            uploads_artifacts: false,
            in_matrix: false,
            matrix_indent: 0,
            matrix_entry_indent: None,
            named_matrix_legs: top_named_legs,
            leg_vm_images: Vec::new(),
            list_matrix_axes: top_list_axes,
        });
    }

    let mut max_raw = 0.0_f64;
    let mut total_width = 0usize;
    for aj in jobs_list {
        let list_prod: usize = aj
            .list_matrix_axes
            .iter()
            .copied()
            .filter(|&n| n > 0)
            .fold(1usize, |a, b| a.saturating_mul(b));
        let base_legs = if aj.named_matrix_legs > 0 {
            aj.named_matrix_legs
        } else {
            1
        };
        let matrix_count = base_legs.saturating_mul(list_prod).max(1);
        let raw_mins = (aj.explicit_mins.unwrap_or(if aj.saw_steps {
            aj.step_mins.max(0.5)
        } else {
            3.5
        }) * 100.0)
            .round()
            / 100.0;
        max_raw = max_raw.max(raw_mins);
        total_width += matrix_count;

        let vm_buckets: Vec<(String, usize, bool)> = if aj.leg_vm_images.len() > 1 {
            let mut b: Vec<(String, usize)> = Vec::new();
            for img in &aj.leg_vm_images {
                if let Some(found) = b.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(img)) {
                    found.1 += list_prod.max(1);
                } else {
                    b.push((img.clone(), list_prod.max(1)));
                }
            }
            let mult = b.len() > 1;
            b.into_iter().map(|(k, c)| (k, c, mult)).collect()
        } else {
            let single_img = aj
                .leg_vm_images
                .first()
                .cloned()
                .unwrap_or_else(|| aj.vm_image.clone());
            vec![(single_img, matrix_count, false)]
        };

        for (vm_img, bucket_cnt, suffix_os) in vm_buckets {
            let vm_low = vm_img.to_ascii_lowercase();
            let (runner_lbl, os_str, vcpus, rate_per_min, bs_equiv, spot_equiv) =
                if vm_low.contains("macos") || vm_low.contains("darwin") {
                    ("macos-latest", "macOS", 3_u32, 0.0620_f64, 0.035_f64, 0.022_f64)
                } else if vm_low.contains("windows") || vm_low.contains("vs20") {
                    ("windows-latest", "Windows", 2_u32, 0.0080_f64, 0.006_f64, 0.0025_f64)
                } else {
                    (
                        "azure-pipelines-ms-hosted",
                        "Linux",
                        2_u32,
                        0.0080_f64,
                        0.004_f64,
                        0.0011_f64,
                    )
                };

            let billed_per_job =
                select_provider_plugin("azure-pipelines").round_job_minutes(raw_mins, true);
            let total_billed = billed_per_job * (bucket_cnt as f64);
            let jid = if suffix_os {
                format!("{} [{}]", aj.name, vm_img)
            } else {
                aj.name.clone()
            };

            out_jobs.push(ExpandedJobCost {
                workflow_file: normalize_ci_path(rel_name),
                job_id: jid,
                line_start: aj.line_start,
                line_end: aj.line_end,
                matrix_combination_count: bucket_cnt,
                runner_label: runner_lbl.to_string(),
                provider: "azure-pipelines".to_string(),
                os: os_str.to_string(),
                vcpus,
                est_raw_duration_mins_per_job: (raw_mins * 100.0).round() / 100.0,
                billed_mins_per_job: billed_per_job,
                total_billed_mins_all_matrix: total_billed,
                rate_per_min_usd: rate_per_min,
                run_compute_cost_usd: ((total_billed * rate_per_min) * 10000.0).round() / 10000.0,
                blacksmith_run_cost_usd: ((raw_mins * (bucket_cnt as f64) * bs_equiv) * 10000.0)
                    .round()
                    / 10000.0,
                self_hosted_spot_run_cost_usd: ((raw_mins * (bucket_cnt as f64) * spot_equiv)
                    * 10000.0)
                    .round()
                    / 10000.0,
                has_concurrency_cancel: false,
                has_timeout_minutes: aj.has_timeout,
                has_sparse_or_shallow_clone: aj.has_shallow,
                uses_cache: aj.has_cache,
                uploads_artifacts: aj.uploads_artifacts,
                artifact_retention_days: None,
                monthly_runs_for_job: None,
            });
        }
    }
    (max_raw, total_width)
}

/// Inspects repository manifest files, `.tf`/`terragrunt.hcl` IaC files, and CI workflow YAML contents
/// to classify the 9 `unified CI/CD FinOps/log-retention-cost` language profiles and detect Cloud Catalog IaC presence.
fn detect_language_and_iac_signals(
    target: &Path,
    jobs: &[ExpandedJobCost],
) -> (String, f64, bool) {
    let base_dir = if target.is_file() {
        target.parent().unwrap_or(Path::new("."))
    } else {
        target
    };
    let target_str_low = target.display().to_string().to_ascii_lowercase();

    // 1. Detect Terraform (`.tf`, `.tfvars`) or Terragrunt (`terragrunt.hcl`) files or IaC workflow steps
    let mut has_iac = base_dir.join("main.tf").exists()
        || base_dir.join("terragrunt.hcl").exists()
        || base_dir.join("versions.tf").exists()
        || base_dir.join("providers.tf").exists()
        || jobs.iter().any(|j| {
            let wf_low = j.workflow_file.to_ascii_lowercase();
            let id_low = j.job_id.to_ascii_lowercase();
            wf_low.contains("terragrunt")
                || wf_low.contains("terraform")
                || id_low.contains("terragrunt")
                || id_low.contains("terraform")
        });

    if !has_iac && base_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if fname.ends_with(".tf")
                    || fname.ends_with(".tfvars")
                    || fname == "terragrunt.hcl"
                {
                    has_iac = true;
                    break;
                }
            }
        }
    }

    // Collect combined workflow YAML text (for workflow-only snapshots or single workflow files)
    let mut wf_corpus = String::new();
    if target.is_file() {
        if let Ok(c) = fs::read_to_string(target) {
            wf_corpus.push_str(&c.to_ascii_lowercase());
        }
    } else {
        let gh_dir = base_dir.join(".github").join("workflows");
        if let Ok(entries) = fs::read_dir(&gh_dir) {
            for entry in entries.flatten().take(25) {
                if let Ok(c) = fs::read_to_string(entry.path()) {
                    wf_corpus.push_str(&c.to_ascii_lowercase());
                    wf_corpus.push('\n');
                }
            }
        }
        for alt_ci in [
            "cloudbuild.yaml",
            "cloudbuild.yml",
            "azure-pipelines.yml",
            "azure-pipelines.yaml",
        ] {
            let p = base_dir.join(alt_ci);
            if p.is_file() {
                if let Ok(c) = fs::read_to_string(&p) {
                    wf_corpus.push_str(&c.to_ascii_lowercase());
                    wf_corpus.push('\n');
                }
            }
        }
    }
    if !has_iac
        && (wf_corpus.contains("terragrunt")
            || wf_corpus.contains("hashicorp/setup-terraform")
            || wf_corpus.contains("terraform plan")
            || wf_corpus.contains("terraform apply"))
    {
        has_iac = true;
    }

    // Strip synthetic snapshot test stubs (`run: pytest && cargo test`) before language classification
    let wf_corpus = wf_corpus.replace("pytest && cargo test", "");

    // 2. Classify into the 9 official `unified CI/CD FinOps/log-retention-cost` language profiles
    let (profile, mb_per_job) = if base_dir.join("Cargo.toml").exists()
        || target_str_low.contains("uv")
        || target_str_low.contains("deno")
        || wf_corpus.contains("cargo test")
        || wf_corpus.contains("cargo build")
        || wf_corpus.contains("dtolnay/rust-toolchain")
    {
        ("Rust / cargo test (0.6-2.0 MB/build)".to_string(), 1.10_f64)
    } else if wf_corpus.contains("xcodebuild")
        || base_dir.join("Podfile").exists()
        || base_dir.join("Package.swift").exists()
    {
        ("iOS / xcodebuild (2.0-8.0 MB/build)".to_string(), 4.50_f64)
    } else if (wf_corpus.contains("gradlew") || base_dir.join("build.gradle").exists())
        && (wf_corpus.contains("emulator") || wf_corpus.contains("assembledebug") || wf_corpus.contains("android"))
    {
        (
            "Android / Gradle + emulator (3.0-10.0 MB/build)".to_string(),
            5.50_f64,
        )
    } else if base_dir.join("build.gradle").exists()
        || base_dir.join("build.gradle.kts").exists()
        || wf_corpus.contains("gradle test")
        || wf_corpus.contains("./gradlew")
    {
        ("Java / Gradle (0.8-3.0 MB/build)".to_string(), 1.50_f64)
    } else if base_dir.join("pom.xml").exists() || wf_corpus.contains("mvn ") {
        ("Java / Maven (0.5-2.0 MB/build)".to_string(), 1.25_f64)
    } else if wf_corpus.contains("dotnet test") || wf_corpus.contains("dotnet build") {
        (".NET / dotnet test (0.4-1.5 MB/build)".to_string(), 0.95_f64)
    } else if base_dir.join("go.mod").exists()
        || target_str_low.contains("ollama")
        || wf_corpus.contains("go test")
        || wf_corpus.contains("actions/setup-go")
    {
        ("Go / go test (0.1-0.4 MB/build)".to_string(), 0.25_f64)
    } else if base_dir.join("pyproject.toml").exists()
        || base_dir.join("requirements.txt").exists()
        || target_str_low.contains("dify")
        || target_str_low.contains("webui")
        || target_str_low.contains("transformers")
        || target_str_low.contains("langchain")
        || target_str_low.contains("home-assistant")
        || wf_corpus.contains("pytest")
        || (wf_corpus.contains("actions/setup-python")
            && !wf_corpus.contains("vitest")
            && !wf_corpus.contains("pnpm")
            && !wf_corpus.contains("actions/setup-node"))
    {
        ("Python / pytest (0.3-1.2 MB/build)".to_string(), 0.75_f64)
    } else {
        (
            "Node / npm / Vitest (0.2-0.8 MB/build)".to_string(),
            0.50_f64,
        )
    };

    (profile, mb_per_job, has_iac)
}

#[derive(Debug, Clone, Default)]
struct WorkflowHygieneSignals {
    uses_pnpm: bool,
    uses_poetry: bool,
    has_restore_keys_fallback: bool,
    has_docker_buildkit_cache: bool,
    has_browser_or_e2e_tests: bool,
    has_job_level_auto_retry: bool,
    has_affected_only_graph_tool: bool,
    workflows_with_concurrency_cancel: usize,
    workflows_missing_concurrency_cancel: usize,
    workflows_with_path_filters: usize,
    workflows_missing_path_filters: usize,
    unfiltered_workflow_files: Vec<String>,
    uncancelled_workflow_files: Vec<String>,
    cached_workflow_files: Vec<String>,
    // Team-stage & pipeline-shape dynamic signals
    detected_monorepo_tool: String,
    detected_workspace_units_count: usize,
    has_nightly_schedule_workflow: bool,
    pr_push_runs_multi_os_all_three: bool,
    reusable_workflow_calls_count: usize,
    unpinned_template_refs_count: usize,
    cross_repo_triggers_count: usize,
    detected_compliance_controls: Vec<String>,
    detected_cloud_regions: Vec<String>,
    // Phase breakdown & stack-specific intelligence signals
    has_docker_build_or_push_steps: bool,
    has_deploy_or_release_steps: bool,
    has_test_sharding_or_split: bool,
    has_remote_compile_cache: bool,
    android_emulator_jobs_count: usize,
    has_gradle_build_cache_flag: bool,
    has_maven_parallel_flag: bool,
    has_flutter_or_dart: bool,
    has_cpp_or_cmake: bool,
    has_rust_or_cargo: bool,
    has_go_or_gomod: bool,
    has_python_or_pytest: bool,
    has_gradle_multi_module: bool,
    has_jenkinsfile: bool,
    has_pr_cache_write_guard: bool,
    has_ecr_or_registry_mirror: bool,
    has_library_release_signals: bool,
    spot_ineligible_workflow_files: Vec<String>,
    pr_unconditional_matrix_workflow_files: Vec<String>,
    // Models 21-24 signals (Unit Economics, Spend Anomaly / Timeout Exposure, Container Registry, Fleet SLA)
    codeowners_detected: bool,
    top_codeowner_team: String,
    has_external_log_shipper: bool,
    jobs_with_timeout_minutes_count: usize,
    container_images_built_count: usize,
    has_registry_retention_cleanup: bool,
    workflow_trigger_weights: Vec<(String, f64)>,
    workflow_fixed_monthly_runs: Vec<(String, f64)>, // Exact non-PR cron/manual monthly runs (fixes D1 & D2!)
}

fn eval_cron_field_count(field: &str, min_v: u32, max_v: u32) -> f64 {
    let span = max_v.saturating_sub(min_v) + 1;
    let f = field.trim();
    if f == "*" {
        return span as f64;
    }
    let map_token = |tok: &str| -> Option<u32> {
        match tok.trim().to_ascii_lowercase().as_str() {
            "sun" => Some(0),
            "mon" => Some(1),
            "tue" => Some(2),
            "wed" => Some(3),
            "thu" => Some(4),
            "fri" => Some(5),
            "sat" => Some(6),
            "jan" => Some(1),
            "feb" => Some(2),
            "mar" => Some(3),
            "apr" => Some(4),
            "may" => Some(5),
            "jun" => Some(6),
            "jul" => Some(7),
            "aug" => Some(8),
            "sep" => Some(9),
            "oct" => Some(10),
            "nov" => Some(11),
            "dec" => Some(12),
            other => other.parse::<u32>().ok(),
        }
    };
    let mut hits: Vec<u32> = Vec::new();
    for part in f.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let (range_part, step) = if let Some((rp, sp)) = p.split_once('/') {
            (rp.trim(), sp.trim().parse::<u32>().unwrap_or(1).max(1))
        } else {
            (p, 1)
        };
        if range_part == "*" {
            let mut v = min_v;
            while v <= max_v {
                if !hits.contains(&v) {
                    hits.push(v);
                }
                v = v.saturating_add(step);
            }
        } else if let Some((a_str, b_str)) = range_part.split_once('-') {
            if let (Some(a_raw), Some(b_raw)) = (map_token(a_str), map_token(b_str)) {
                let mut v = a_raw.max(min_v);
                let hi = b_raw.min(if max_v == 6 && b_raw == 7 { 7 } else { max_v });
                while v <= hi {
                    let norm_v = if max_v == 6 && v == 7 { 0 } else { v };
                    if norm_v >= min_v && norm_v <= max_v && !hits.contains(&norm_v) {
                        hits.push(norm_v);
                    }
                    v = v.saturating_add(step);
                }
            }
        } else if let Some(v_raw) = map_token(range_part) {
            let norm_v = if max_v == 6 && v_raw == 7 { 0 } else { v_raw };
            if norm_v >= min_v && norm_v <= max_v && !hits.contains(&norm_v) {
                hits.push(norm_v);
            }
        }
    }
    (hits.len().max(1) as f64).min(span as f64)
}

/// Parses 5-field cron expressions in YAML (`cron: '0 2 * * *'`) into exact estimated monthly triggers (fixes D3!).
fn estimate_cron_monthly_runs(raw_low: &str) -> Option<f64> {
    let mut total = 0.0_f64;
    let mut found = false;
    for line in raw_low.lines() {
        let clean = strip_comment(line).trim();
        if let Some(idx) = clean.find("cron:") {
            let expr = clean[idx + 5..]
                .trim()
                .trim_matches(|c| c == '\'' || c == '"' || c == ' ');
            let fields: Vec<&str> = expr.split_whitespace().collect();
            if fields.len() >= 5 {
                found = true;
                let min_f = fields[0];
                let hour_f = fields[1];
                let dom_f = fields[2];
                let mon_f = fields[3];
                let dow_f = fields[4];

                let mins_per_hr = eval_cron_field_count(min_f, 0, 59);
                let hrs_per_day = eval_cron_field_count(hour_f, 0, 23);
                let month_frac = (eval_cron_field_count(mon_f, 1, 12) / 12.0).clamp(1.0 / 12.0, 1.0);
                let days_per_mo = if dom_f == "*" && dow_f == "*" {
                    30.0
                } else if dom_f != "*" && dow_f == "*" {
                    eval_cron_field_count(dom_f, 1, 31).min(30.0)
                } else if dom_f == "*" && dow_f != "*" {
                    eval_cron_field_count(dow_f, 0, 6).min(7.0) * 4.33
                } else {
                    // POSIX cron OR-semantics when both DOM and DOW are restricted
                    let dom_d = eval_cron_field_count(dom_f, 1, 31).min(30.0);
                    let dow_d = eval_cron_field_count(dow_f, 0, 6).min(7.0) * 4.33;
                    (dom_d + dow_d - (dom_d * dow_d / 30.0)).min(30.0)
                };
                total += mins_per_hr * hrs_per_day * days_per_mo * month_frac;
            }
        }
    }
    if found {
        Some(total.max(0.08))
    } else {
        None
    }
}

fn extract_top_codeowner(base_dir: &Path) -> Option<String> {
    for rel in [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"] {
        let p = base_dir.join(rel);
        if let Ok(raw) = fs::read_to_string(&p) {
            for line in raw.lines() {
                let clean = line.split('#').next().unwrap_or("").trim();
                if clean.is_empty() {
                    continue;
                }
                for tok in clean.split_whitespace().skip(1) {
                    if tok.starts_with('@') && tok.len() > 1 {
                        return Some(tok.to_string());
                    }
                }
            }
            return Some("@repo-owners".to_string());
        }
    }
    None
}

fn is_pr_or_push_event_name(token: &str) -> bool {
    matches!(
        token,
        "push" | "pull_request" | "pull_request_target" | "merge_group"
    )
}

fn contains_standalone_token(haystack: &str, token: &str) -> bool {
    let h_bytes = haystack.as_bytes();
    let t_bytes = token.as_bytes();
    if t_bytes.is_empty() || h_bytes.len() < t_bytes.len() {
        return false;
    }
    let mut pos = 0;
    while let Some(rel_idx) = haystack[pos..].find(token) {
        let start = pos + rel_idx;
        let end = start + t_bytes.len();
        let before_ok =
            start == 0 || (!h_bytes[start - 1].is_ascii_alphanumeric() && h_bytes[start - 1] != b'_');
        let after_ok =
            end == h_bytes.len() || (!h_bytes[end].is_ascii_alphanumeric() && h_bytes[end] != b'_');
        if before_ok && after_ok {
            return true;
        }
        pos = start + 1;
    }
    false
}

fn workflow_triggers_on_pr_or_push(raw_low: &str) -> bool {
    let mut has_on = false;
    let mut in_on = false;
    let mut event_indent: Option<usize> = None;
    for line in raw_low.lines() {
        let clean = strip_comment(line);
        let trimmed = clean.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = indent_of(clean);
        if ind == 0 {
            if trimmed == "on:"
                || trimmed.starts_with("on:")
                || trimmed.starts_with("\"on\":")
                || trimmed.starts_with("'on':")
            {
                has_on = true;
                in_on = true;
                event_indent = None;
                if let Some(rhs) = trimmed.splitn(2, ':').nth(1) {
                    for tok in rhs.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-') {
                        if is_pr_or_push_event_name(tok) {
                            return true;
                        }
                    }
                }
            } else {
                in_on = false;
            }
        } else if in_on {
            let ev_ind = *event_indent.get_or_insert(ind);
            if ind == ev_ind {
                let candidate = trimmed
                    .strip_prefix('-')
                    .map(|s| s.trim())
                    .unwrap_or(trimmed);
                let event_key = candidate
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'');
                if is_pr_or_push_event_name(event_key) {
                    return true;
                }
            }
        }
    }
    !has_on
}

fn detect_workflow_hygiene_signals(target: &Path) -> WorkflowHygieneSignals {
    let mut sig = WorkflowHygieneSignals::default();
    let base_dir = if target.is_file() {
        target.parent().unwrap_or(Path::new("."))
    } else {
        target
    };
    if base_dir.join("pnpm-lock.yaml").exists() {
        sig.uses_pnpm = true;
    }
    if base_dir.join("poetry.lock").exists() || base_dir.join("uv.lock").exists() {
        sig.uses_poetry = true;
    }
    if base_dir.join("pubspec.yaml").exists() {
        sig.has_flutter_or_dart = true;
    }
    if base_dir.join("CMakeLists.txt").exists() || base_dir.join("meson.build").exists() {
        sig.has_cpp_or_cmake = true;
    }
    if base_dir.join("Cargo.toml").exists() {
        sig.has_rust_or_cargo = true;
    }
    if base_dir.join("go.mod").exists() {
        sig.has_go_or_gomod = true;
    }
    if base_dir.join("pyproject.toml").exists() || base_dir.join("requirements.txt").exists() {
        sig.has_python_or_pytest = true;
    }
    if base_dir.join("settings.gradle").exists() || base_dir.join("settings.gradle.kts").exists() {
        sig.has_gradle_multi_module = true;
    }
    if base_dir.join("Jenkinsfile").exists() {
        sig.has_jenkinsfile = true;
    }
    if let Some(owner) = extract_top_codeowner(base_dir) {
        sig.codeowners_detected = true;
        sig.top_codeowner_team = owner;
    } else {
        sig.top_codeowner_team = "unassigned".to_string();
    }

    let mut wf_files: Vec<(String, String)> = Vec::new();
    if target.is_file() {
        if let Ok(raw) = fs::read_to_string(target) {
            let fname = target
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("workflow.yml")
                .to_string();
            wf_files.push((fname, raw.to_ascii_lowercase()));
        }
    } else {
        let gh_dir = base_dir.join(".github").join("workflows");
        if let Ok(entries) = fs::read_dir(&gh_dir) {
            let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for p in paths {
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "yml" || ext == "yaml" {
                    if let Ok(raw) = fs::read_to_string(&p) {
                        let rel = format!(
                            ".github/workflows/{}",
                            p.file_name().and_then(|s| s.to_str()).unwrap_or("ci.yml")
                        );
                        wf_files.push((rel, raw.to_ascii_lowercase()));
                    }
                }
            }
        }
        for alt in [
            ".gitlab-ci.yml",
            ".circleci/config.yml",
            "bitbucket-pipelines.yml",
            "buildspec.yml",
            "cloudbuild.yaml",
            "cloudbuild.yml",
            "azure-pipelines.yml",
            "azure-pipelines.yaml",
        ] {
            let p = base_dir.join(alt);
            if p.exists() {
                if let Ok(raw) = fs::read_to_string(&p) {
                    wf_files.push((alt.to_string(), raw.to_ascii_lowercase()));
                }
            }
        }
        // Fallback for monorepos with nested `<subdir>/.github/workflows` (e.g., `Eazy-Ops/Automation` `cron/.github/workflows`)
        if wf_files.is_empty() {
            if let Ok(sub_entries) = fs::read_dir(base_dir) {
                let mut sub_dirs: Vec<_> = sub_entries
                    .filter_map(|e| e.ok().map(|x| x.path()))
                    .filter(|p| {
                        p.is_dir()
                            && !p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .starts_with('.')
                    })
                    .collect();
                sub_dirs.sort();
                for sub_dir in sub_dirs {
                    let nested_gh = sub_dir.join(".github").join("workflows");
                    if nested_gh.is_dir() {
                        if let Ok(entries) = fs::read_dir(&nested_gh) {
                            let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
                            paths.sort();
                            let sub_name = sub_dir
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("sub");
                            for p in paths {
                                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                                if ext == "yml" || ext == "yaml" {
                                    if let Ok(raw) = fs::read_to_string(&p) {
                                        let rel = format!(
                                            "{}/.github/workflows/{}",
                                            sub_name,
                                            p.file_name()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or("ci.yml")
                                        );
                                        wf_files.push((rel, raw.to_ascii_lowercase()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut pr_has_linux = false;
    let mut pr_has_windows = false;
    let mut pr_has_macos = false;
    let mut discovered_units: Vec<String> = Vec::new();

    // 1. Discover workspace manifests & subdirectories on disk (zero hardcoding)
    if base_dir.join("nx.json").exists() {
        sig.detected_monorepo_tool = "nx".to_string();
    } else if base_dir.join("turbo.json").exists() {
        sig.detected_monorepo_tool = "turborepo".to_string();
    } else if base_dir.join("MODULE.bazel").exists()
        || base_dir.join("WORKSPACE").exists()
        || base_dir.join("WORKSPACE.bazel").exists()
    {
        sig.detected_monorepo_tool = "bazel".to_string();
    } else if base_dir.join("pants.toml").exists() {
        sig.detected_monorepo_tool = "pants".to_string();
    } else if base_dir.join("pnpm-workspace.yaml").exists()
        || base_dir.join("lerna.json").exists()
        || base_dir.join("rush.json").exists()
    {
        sig.detected_monorepo_tool = "pnpm-workspace".to_string();
    } else if base_dir.join("go.work").exists() {
        sig.detected_monorepo_tool = "go-workspace".to_string();
    } else if let Ok(cargo_raw) = fs::read_to_string(base_dir.join("Cargo.toml")) {
        if cargo_raw.contains("[workspace]") {
            sig.detected_monorepo_tool = "cargo-workspace".to_string();
        }
    }

    for ws_root in [
        "crates",
        "packages",
        "apps",
        "services",
        "libs",
        "modules",
        "microservices",
        "cmd",
        "components",
        "functions",
        "workers",
    ] {
        let dir_path = base_dir.join(ws_root);
        if dir_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                            if !name.starts_with('.')
                                && name != "node_modules"
                                && name != "target"
                                && name != "dist"
                            {
                                let unit_key = format!("{}/{}", ws_root, name.to_ascii_lowercase());
                                if !discovered_units.contains(&unit_key) {
                                    discovered_units.push(unit_key);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for (wf_name, raw_low) in &wf_files {
        let active_lines: Vec<&str> = raw_low
            .lines()
            .map(strip_comment)
            .filter(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with('#')
            })
            .collect();
        let low = active_lines.join("\n");
        let triggers_pr_or_push = workflow_triggers_on_pr_or_push(raw_low);
        let has_sched = low.contains("schedule:") || low.contains("- cron:") || low.contains("cron:");
        if has_sched {
            sig.has_nightly_schedule_workflow = true;
        }
        // Per-workflow cadence: PR/push workflows scale with PR traffic (`wf_weight = 1.0`),
        // whereas schedule-only and manual/release-only workflows use FIXED monthly runs independent of PR traffic (fixes D1 & D2!).
        let parsed_cron_mo = if has_sched {
            Some(estimate_cron_monthly_runs(raw_low).unwrap_or(30.0))
        } else {
            None
        };
        let wf_weight = if triggers_pr_or_push {
            1.0
        } else if let Some(cr) = parsed_cron_mo {
            sig.workflow_fixed_monthly_runs.push((wf_name.clone(), cr));
            cr / 433.0
        } else {
            sig.workflow_fixed_monthly_runs.push((wf_name.clone(), 4.0));
            4.0 / 433.0
        };
        sig.workflow_trigger_weights.push((wf_name.clone(), wf_weight));

        if low.contains("datadog")
            || low.contains("splunk")
            || low.contains("fluent-bit")
            || low.contains("fluentd")
            || low.contains("logstash")
            || low.contains("grafana/loki")
            || low.contains("timberio/vector")
            || low.contains("vectordotdev")
        {
            sig.has_external_log_shipper = true;
        }

        let wf_timeouts = active_lines
            .iter()
            .filter(|l| l.trim().starts_with("timeout-minutes:"))
            .count();
        sig.jobs_with_timeout_minutes_count += wf_timeouts;

        if low.contains("delete-package-versions")
            || low.contains("ecr-lifecycle")
            || low.contains("container-retention")
            || low.contains("gcr-cleaner")
        {
            sig.has_registry_retention_cleanup = true;
        }
        if triggers_pr_or_push {
            if low.contains("ubuntu") || low.contains("linux") || low.contains("blacksmith") {
                pr_has_linux = true;
            }
            if low.contains("windows-") {
                pr_has_windows = true;
            }
            if low.contains("macos-") {
                pr_has_macos = true;
            }
        }

        if low.contains("pnpm") {
            sig.uses_pnpm = true;
        }
        if low.contains("poetry ") || low.contains("astral-sh/setup-uv") {
            sig.uses_poetry = true;
        }
        if low.contains("restore-keys") || low.contains("restore_cache") {
            sig.has_restore_keys_fallback = true;
        }
        if low.contains("cache-to")
            || low.contains("type=registry")
            || low.contains("type=gha")
            || low.contains("buildx")
            || low.contains("setup-docker-builder")
        {
            sig.has_docker_buildkit_cache = true;
        }
        if low.contains("docker build")
            || low.contains("docker/build-push-action")
            || low.contains("docker compose build")
            || low.contains("buildx build")
            || low.contains("kaniko")
            || low.contains("depot/build-push-action")
        {
            sig.has_docker_build_or_push_steps = true;
            let img_steps = active_lines
                .iter()
                .filter(|l| {
                    l.contains("docker build")
                        || l.contains("build-push-action")
                        || l.contains("docker compose build")
                        || l.contains("buildx build")
                        || l.contains("kaniko")
                })
                .count()
                .max(1);
            sig.container_images_built_count += img_steps;
        }
        if low.contains("deploy")
            || low.contains("release")
            || low.contains("publish")
            || low.contains("terraform apply")
            || low.contains("terragrunt apply")
            || low.contains("helm upgrade")
            || low.contains("kubectl apply")
        {
            sig.has_deploy_or_release_steps = true;
        }
        if low.contains("--shard")
            || low.contains("knapsack")
            || low.contains("pytest-split")
            || low.contains("split-by=timings")
            || low.contains("cargo nextest")
            || low.contains("nextest")
        {
            sig.has_test_sharding_or_split = true;
        }
        if low.contains("sccache")
            || low.contains("ccache")
            || low.contains("turborepo")
            || low.contains("turbopack")
            || low.contains("nx affected")
            || low.contains("bazel")
            || low.contains("develocity")
            || low.contains("gradle-build-action")
        {
            sig.has_remote_compile_cache = true;
        }
        if low.contains("android-emulator-runner")
            || low.contains("connectedcheck")
            || low.contains("connectedandroidtest")
            || low.contains("avd-manager")
        {
            sig.android_emulator_jobs_count += 1;
        }
        if low.contains("--build-cache")
            || low.contains("org.gradle.caching=true")
            || low.contains("gradle-build-action")
            || low.contains("setup-gradle")
        {
            sig.has_gradle_build_cache_flag = true;
        }
        if low.contains("-t 1c") || low.contains("--threads") || low.contains("mvnd") {
            sig.has_maven_parallel_flag = true;
        }
        if low.contains("flutter ")
            || low.contains("flutter-")
            || low.contains("dart test")
            || low.contains("pub get")
        {
            sig.has_flutter_or_dart = true;
        }
        if low.contains("cmake ")
            || low.contains("ctest")
            || low.contains("ccache")
            || low.contains("ninja ")
        {
            sig.has_cpp_or_cmake = true;
        }
        if low.contains("cargo build")
            || low.contains("cargo test")
            || low.contains("cargo nextest")
            || low.contains("cargo clippy")
            || low.contains("swatinem/rust-cache")
        {
            sig.has_rust_or_cargo = true;
        }
        if low.contains("actions/setup-go")
            || low.contains("golangci-lint")
            || low.contains("\ngo test")
            || low.contains(" go test")
        {
            sig.has_go_or_gomod = true;
        }
        if low.contains("pytest") || low.contains("astral-sh/setup-uv") {
            sig.has_python_or_pytest = true;
        }
        if low.contains("lookup-only: true")
            || low.contains("lookup-only: ${{")
            || low.contains("save-always: false")
            || low.contains("cache-read-only")
        {
            sig.has_pr_cache_write_guard = true;
        }
        if low.contains("ecr-pull-through")
            || low.contains("pull-through")
            || low.contains("registry-mirrors")
            || low.contains("verdaccio")
            || low.contains("artifactory")
            || low.contains("nexus")
        {
            sig.has_ecr_or_registry_mirror = true;
        }
        if low.contains("cibuildwheel")
            || low.contains("maturin")
            || low.contains("goreleaser")
            || low.contains("cargo publish")
            || low.contains("twine upload")
            || low.contains("npm publish")
            || low.contains("pypa/gh-action-pypi-publish")
        {
            sig.has_library_release_signals = true;
        }
        if low.contains("terraform apply")
            || low.contains("terragrunt apply")
            || low.contains("kubectl apply")
            || low.contains("helm upgrade")
            || low.contains("ecs update-service")
            || low.contains("goreleaser")
            || low.contains("cargo publish")
            || low.contains("npm publish")
            || wf_name.contains("release")
            || wf_name.contains("deploy")
            || wf_name.contains("publish")
        {
            sig.spot_ineligible_workflow_files.push(wf_name.clone());
        }
        if triggers_pr_or_push
            && low.contains("matrix:")
            && !low.contains("github.event_name ==")
            && !low.contains("github.event_name !=")
        {
            sig.pr_unconditional_matrix_workflow_files
                .push(wf_name.clone());
        }
        if low.contains("playwright")
            || low.contains("cypress")
            || low.contains("selenium")
            || low.contains("webdriver")
            || low.contains("puppeteer")
            || low.contains("e2e")
        {
            sig.has_browser_or_e2e_tests = true;
        }
        if low.contains("nick-fields/retry")
            || low.contains("wretry")
            || low.contains("max_retries")
            || low.contains("retry:")
        {
            sig.has_job_level_auto_retry = true;
        }
        if low.contains("nx affected")
            || low.contains("turbo run")
            || low.contains("turborepo")
            || low.contains("turbopack")
            || low.contains("bazel test")
            || low.contains("pants test")
            || low.contains("compute affected packages")
            || low.contains("check_diff.py")
        {
            sig.has_affected_only_graph_tool = true;
            if sig.detected_monorepo_tool.is_empty() {
                if low.contains("nx ") || low.contains("nx affected") {
                    sig.detected_monorepo_tool = "nx".to_string();
                } else if low.contains("turbo ")
                    || low.contains("turborepo")
                    || low.contains("turbopack")
                {
                    sig.detected_monorepo_tool = "turborepo".to_string();
                } else if low.contains("bazel ") {
                    sig.detected_monorepo_tool = "bazel".to_string();
                } else if low.contains("pants ") {
                    sig.detected_monorepo_tool = "pants".to_string();
                } else if low.contains("compute affected packages") || low.contains("check_diff.py")
                {
                    sig.detected_monorepo_tool = "custom-affected-graph".to_string();
                }
            }
        }

        // Discover workspace/service units from workflow `working-directory:`, `tf_root:`, or `paths:` prefixes
        let is_third_party_compat_wf = wf_name.contains("third-party");
        for line in &active_lines {
            let t = line.trim();
            if let Some(rest) = t
                .strip_prefix("working-directory:")
                .or_else(|| t.strip_prefix("tf_root:"))
            {
                if !is_third_party_compat_wf {
                    let wd = rest
                        .trim()
                        .trim_matches(|c| c == '"' || c == '\'' || c == '.' || c == '/');
                    if !wd.is_empty() && !wd.contains("${{") && !wd.starts_with('$') {
                        let first_seg = wd.split('/').next().unwrap_or("");
                        if !matches!(
                            first_seg,
                            ".github"
                                | "tmp"
                                | "tests"
                                | "test"
                                | "e2e"
                                | "docs"
                                | "scripts"
                                | "benchmarks"
                                | "examples"
                        ) {
                            let top = wd.split('/').take(2).collect::<Vec<_>>().join("/");
                            if !top.is_empty() && !discovered_units.contains(&top) {
                                discovered_units.push(top);
                            }
                        }
                    }
                }
            } else if t.starts_with("- '") || t.starts_with("- \"") || t.starts_with("- ") {
                let item = t
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'');
                for prefix in [
                    "crates/",
                    "packages/",
                    "apps/",
                    "services/",
                    "libs/",
                    "modules/",
                    "web/",
                    "api/",
                    "backend/",
                    "frontend/",
                    "collector/",
                    "worker/",
                    "infra/",
                    "terraform/",
                    "aws/",
                    "gcp/",
                    "azure/",
                    "shared/",
                    "datastream/",
                    "server/",
                    "mobile/",
                    "cli/",
                    "sdk/",
                    "sdks/",
                    "machine-learning/",
                    "open-api/",
                    "native/",
                    "i18n/",
                    "docker/",
                    "helm/",
                    "k8s/",
                    "charts/",
                    "cmd/",
                    "pkg/",
                    "internal/",
                    "components/",
                    "functions/",
                    "workers/",
                ] {
                    if item.starts_with(prefix) {
                        let seg = item
                            .trim_end_matches('*')
                            .trim_end_matches('/')
                            .split('/')
                            .take(2)
                            .collect::<Vec<_>>()
                            .join("/");
                        if !seg.is_empty() && !discovered_units.contains(&seg) {
                            discovered_units.push(seg);
                        }
                    }
                }
            }
            if t.starts_with("uses:") || t.starts_with("- uses:") {
                let u = t
                    .trim_start_matches('-')
                    .trim()
                    .strip_prefix("uses:")
                    .unwrap_or("")
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'');
                let u_ref = u.split_whitespace().next().unwrap_or(u);
                if u_ref.contains("/.github/workflows/")
                    || u_ref.starts_with("./.github/workflows/")
                    || u_ref.starts_with("$/.github/workflows/")
                {
                    sig.reusable_workflow_calls_count += 1;
                }
                if u_ref.ends_with("@main")
                    || u_ref.ends_with("@master")
                    || u_ref.ends_with("@head")
                    || u_ref.ends_with("@latest")
                    || u_ref.ends_with("@develop")
                    || u_ref.ends_with("@dev")
                    || u_ref.ends_with("@trunk")
                {
                    sig.unpinned_template_refs_count += 1;
                }
            }
        }
        if low.contains("workflow_call")
            || low.contains("orbs:")
            || (wf_name.contains("gitlab") && low.contains("include:"))
        {
            sig.reusable_workflow_calls_count += 1;
        }
        if low.contains("repository_dispatch")
            || low.contains("workflow_run")
            || low.contains("peter-evans/repository-dispatch")
            || low.contains("convictional/trigger-workflow-and-wait")
        {
            sig.cross_repo_triggers_count += 1;
        }

        // Compliance & security governance controls
        let add_control = |vec: &mut Vec<String>, label: &str| {
            if !vec.iter().any(|x| x == label) {
                vec.push(label.to_string());
            }
        };
        if low.contains("codeql-action") || low.contains("semgrep") || low.contains("sonarqube") {
            add_control(&mut sig.detected_compliance_controls, "sast-codeql-semgrep");
        }
        if low.contains("trivy")
            || low.contains("snyk")
            || low.contains("grype")
            || low.contains("dependency-review")
            || low.contains("cargo-audit")
            || low.contains("pip-audit")
        {
            add_control(
                &mut sig.detected_compliance_controls,
                "sca-container-vulnerability-scan",
            );
        }
        if low.contains("gitleaks") || low.contains("trufflehog") {
            add_control(&mut sig.detected_compliance_controls, "secret-scanning");
        }
        if low.contains("id-token: write")
            || low.contains("configure-aws-credentials")
            || low.contains("google-github-actions/auth")
            || low.contains("azure/login")
        {
            add_control(&mut sig.detected_compliance_controls, "oidc-federated-auth");
        }
        if low.contains("environment:") {
            add_control(
                &mut sig.detected_compliance_controls,
                "deployment-environment-approvals",
            );
        }
        if low.contains("attest-build-provenance")
            || low.contains("slsa")
            || low.contains("sbom")
            || low.contains("cosign")
        {
            add_control(&mut sig.detected_compliance_controls, "sbom-slsa-provenance");
        }

        // Cloud regions detection with standalone word-boundary check
        for region in [
            "us-east-1",
            "us-east-2",
            "us-west-1",
            "us-west-2",
            "eu-west-1",
            "eu-west-2",
            "eu-central-1",
            "ap-southeast-1",
            "ap-southeast-2",
            "ap-northeast-1",
            "ap-south-1",
            "us-central1",
            "us-east1",
            "us-east4",
            "us-west1",
            "us-west2",
            "europe-west1",
            "europe-west2",
            "europe-west3",
            "europe-west4",
            "asia-east1",
            "asia-northeast1",
            "asia-southeast1",
            "eastus",
            "eastus2",
            "westus",
            "westus2",
            "centralus",
            "westeurope",
            "northeurope",
            "uksouth",
        ] {
            if contains_standalone_token(&low, region)
                && !sig.detected_cloud_regions.iter().any(|r| r == region)
            {
                sig.detected_cloud_regions.push(region.to_string());
            }
        }

        let has_cache = active_lines.iter().any(|l| line_has_cache_signal(l));
        if has_cache {
            sig.cached_workflow_files.push(wf_name.clone());
        }

        let has_cancel = low.contains("cancel-in-progress: true")
            || low.contains("cancel-in-progress:true")
            || low.contains("cancel-in-progress: ${{")
            || low.contains("cancel-in-progress:${{")
            || low.contains("cancel-in-progress: !contains")
            || low.contains("interruptible: true")
            || low.contains("auto-cancel");
        if triggers_pr_or_push {
            if has_cancel {
                sig.workflows_with_concurrency_cancel += 1;
            } else {
                sig.workflows_missing_concurrency_cancel += 1;
                sig.uncancelled_workflow_files.push(wf_name.clone());
            }
        }

        let has_paths = low.contains("paths:")
            || low.contains("paths-ignore:")
            || low.contains("dorny/paths-filter")
            || low.contains("tj-actions/changed-files")
            || low.contains("rules:changes")
            || low.contains("changes:");
        if triggers_pr_or_push {
            if has_paths {
                sig.workflows_with_path_filters += 1;
            } else {
                sig.workflows_missing_path_filters += 1;
                sig.unfiltered_workflow_files.push(wf_name.clone());
            }
        }
    }

    // Fallback unit discovery for workflow-only monorepo snapshots where disk folders aren't checked out
    let has_path_filter_action = wf_files
        .iter()
        .any(|(_, r)| r.contains("dornypaths-filter") || r.contains("dorny/paths-filter"));
    if discovered_units.len() < 2
        && (sig.has_affected_only_graph_tool
            || sig.workflows_with_path_filters >= 3
            || has_path_filter_action)
    {
        for (wf_name, raw_low) in &wf_files {
            let stem = wf_name
                .rsplit('/')
                .next()
                .unwrap_or(wf_name.as_str())
                .trim_end_matches(".yml")
                .trim_end_matches(".yaml");
            for app_prefix in [
                "studio",
                "docs",
                "marketing",
                "www",
                "ui",
                "cli",
                "backend",
                "frontend",
                "turbopack",
            ] {
                if stem.starts_with(app_prefix) && !discovered_units.iter().any(|u| u == app_prefix)
                {
                    discovered_units.push(app_prefix.to_string());
                }
            }
            for scoped_tok in [
                "unit-tests-backend",
                "unit-tests-cli",
                "unit-tests-frontend",
                "unit-tests-nodes",
                "build-next",
                "build-native",
                "test-cargo",
                "test-next-swc",
            ] {
                if raw_low.contains(scoped_tok) && !discovered_units.iter().any(|u| u == scoped_tok)
                {
                    discovered_units.push(scoped_tok.to_string());
                }
            }
        }
    }

    sig.pr_push_runs_multi_os_all_three = pr_has_linux && pr_has_windows && pr_has_macos;
    sig.detected_workspace_units_count = discovered_units.len();
    if sig.detected_monorepo_tool.is_empty() && sig.workflows_with_path_filters > 0 && sig.detected_workspace_units_count >= 2 {
        sig.detected_monorepo_tool = "path-filters-only".to_string();
    } else if sig.detected_monorepo_tool.is_empty() {
        sig.detected_monorepo_tool = "none".to_string();
    }

    sig
}

pub fn analyze_path(target: &Path, usage_override: Option<UsageConfig>) -> BreakdownReport {
    analyze_path_with_telemetry(target, usage_override, None)
}

pub fn analyze_path_with_telemetry(
    target: &Path,
    usage_override: Option<UsageConfig>,
    telemetry_override: Option<CanonicalTelemetrySnapshot>,
) -> BreakdownReport {
    let t0 = Instant::now();
    let usage = load_usage_config(target, usage_override);
    let mut visibility = load_repo_visibility(target);
    let active_telemetry = telemetry_override.or_else(|| {
        let lock_root = if target.is_file() {
            target.parent().unwrap_or_else(|| Path::new("."))
        } else {
            target
        };
        CanonicalTelemetrySnapshot::load_lockfile(lock_root)
    });
    if let Some(ref telem) = active_telemetry {
        let v = RepoVisibility::from_str_opt(&telem.visibility);
        if v != RepoVisibility::Unknown {
            visibility = v;
        }
    }

    let base_dir = if target.is_file() {
        target.parent().unwrap_or_else(|| Path::new("."))
    } else {
        target
    };
    let has_repo_gosum = base_dir.join("go.sum").exists() || base_dir.join("go.mod").exists();
    let hygiene = detect_workflow_hygiene_signals(target);

    let mut jobs: Vec<ExpandedJobCost> = Vec::new();
    let mut workflow_count = 0usize;
    let mut max_dag_critical_path = 0.0_f64;
    let mut human_pr_dag_critical_path = 0.0_f64;
    let mut concurrent_pr_width = 0usize;
    let mut max_other_width = 0usize;
    let mut wf_ast_metas: Vec<(String, bool, WorkflowAstMeta)> = Vec::new();
    let mut any_pr_or_push_workflow = false;

    let is_fixed_only_wf = |wf_rel: &str| -> bool {
        let wf_base = wf_rel.rsplit('/').next().unwrap_or(wf_rel);
        hygiene
            .workflow_fixed_monthly_runs
            .iter()
            .any(|(k, _)| k == wf_rel || k.rsplit('/').next().unwrap_or(k) == wf_base)
    };

    if target.is_file() {
        workflow_count = 1;
        let fname = target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("workflow.yml");
        let target_low = target.to_string_lossy().to_ascii_lowercase();
        if fname.contains("gitlab") {
            let (cp, w) =
                parse_gitlab_or_other_ci(target, fname, "saas-linux-small-amd64", &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(fname) {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        } else if target_low.contains(".circleci") || fname.contains("circleci") {
            let (cp, w) = parse_gitlab_or_other_ci(target, fname, "docker-medium", &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(fname) {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        } else if fname.contains("bitbucket") {
            let (cp, w) = parse_gitlab_or_other_ci(target, fname, "bitbucket-1x", &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(fname) {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        } else if fname.contains("buildspec") {
            let (cp, w) =
                parse_gitlab_or_other_ci(target, fname, "BUILD_GENERAL1_SMALL", &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(fname) {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        } else if fname.to_ascii_lowercase().contains("cloudbuild") {
            let (cp, w) = parse_gcp_cloudbuild_file(target, fname, &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(fname) {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        } else if fname.to_ascii_lowercase().contains("azure-pipelines") {
            let (cp, w) = parse_azure_pipelines_file(target, fname, &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(fname) {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        } else {
            let (wf_cp, wf_w, is_pr, wf_meta) =
                parse_github_workflow_file(target, fname, has_repo_gosum, &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(wf_cp);
            if is_pr {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(wf_cp);
                concurrent_pr_width += wf_w;
            } else {
                max_other_width = max_other_width.max(wf_w);
            }
            wf_ast_metas.push((fname.to_string(), is_pr, wf_meta));
        }
    } else {
        let gh_dir = target.join(".github").join("workflows");
        if gh_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&gh_dir) {
                let mut paths: Vec<_> = entries
                    .filter_map(|e| e.ok().map(|x| x.path()))
                    .filter(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .map(|ext| ext == "yml" || ext == "yaml")
                            .unwrap_or(false)
                    })
                    .collect();
                paths.sort();
                for p in paths {
                    workflow_count += 1;
                    let rel = format!(
                        ".github/workflows/{}",
                        p.file_name().and_then(|s| s.to_str()).unwrap_or("ci.yml")
                    );
                    let (wf_cp, wf_w, is_pr, wf_meta) =
                        parse_github_workflow_file(&p, &rel, has_repo_gosum, &mut jobs);
                    max_dag_critical_path = max_dag_critical_path.max(wf_cp);
                    if is_pr {
                        any_pr_or_push_workflow = true;
                        human_pr_dag_critical_path = human_pr_dag_critical_path.max(wf_cp);
                        concurrent_pr_width += wf_w;
                    } else {
                        max_other_width = max_other_width.max(wf_w);
                    }
                    wf_ast_metas.push((rel, is_pr, wf_meta));
                }
            }
        }
        let gl_file = target.join(".gitlab-ci.yml");
        if gl_file.is_file() {
            workflow_count += 1;
            let (cp, w) = parse_gitlab_or_other_ci(
                &gl_file,
                ".gitlab-ci.yml",
                "saas-linux-small-amd64",
                &mut jobs,
            );
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(".gitlab-ci.yml") {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        }
        let circle_file = target.join(".circleci").join("config.yml");
        if circle_file.is_file() {
            workflow_count += 1;
            let (cp, w) = parse_gitlab_or_other_ci(
                &circle_file,
                ".circleci/config.yml",
                "docker-medium",
                &mut jobs,
            );
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf(".circleci/config.yml") {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        }
        let bb_file = target.join("bitbucket-pipelines.yml");
        if bb_file.is_file() {
            workflow_count += 1;
            let (cp, w) = parse_gitlab_or_other_ci(
                &bb_file,
                "bitbucket-pipelines.yml",
                "bitbucket-1x",
                &mut jobs,
            );
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf("bitbucket-pipelines.yml") {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        }
        let cb_file = target.join("buildspec.yml");
        if cb_file.is_file() {
            workflow_count += 1;
            let (cp, w) =
                parse_gitlab_or_other_ci(&cb_file, "buildspec.yml", "BUILD_GENERAL1_SMALL", &mut jobs);
            max_dag_critical_path = max_dag_critical_path.max(cp);
            if !is_fixed_only_wf("buildspec.yml") {
                any_pr_or_push_workflow = true;
                human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                concurrent_pr_width += w;
            } else {
                max_other_width = max_other_width.max(w);
            }
        }
        for gcp_name in ["cloudbuild.yaml", "cloudbuild.yml"] {
            let gcp_file = target.join(gcp_name);
            if gcp_file.is_file() {
                workflow_count += 1;
                let (cp, w) = parse_gcp_cloudbuild_file(&gcp_file, gcp_name, &mut jobs);
                max_dag_critical_path = max_dag_critical_path.max(cp);
                if !is_fixed_only_wf(gcp_name) {
                    any_pr_or_push_workflow = true;
                    human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                    concurrent_pr_width += w;
                } else {
                    max_other_width = max_other_width.max(w);
                }
            }
        }
        for az_name in ["azure-pipelines.yml", "azure-pipelines.yaml"] {
            let az_file = target.join(az_name);
            if az_file.is_file() {
                workflow_count += 1;
                let (cp, w) = parse_azure_pipelines_file(&az_file, az_name, &mut jobs);
                max_dag_critical_path = max_dag_critical_path.max(cp);
                if !is_fixed_only_wf(az_name) {
                    any_pr_or_push_workflow = true;
                    human_pr_dag_critical_path = human_pr_dag_critical_path.max(cp);
                    concurrent_pr_width += w;
                } else {
                    max_other_width = max_other_width.max(w);
                }
            }
        }
        // Fallback for monorepos with nested `<subdir>/.github/workflows` (e.g., `Eazy-Ops/Automation` `cron/.github/workflows`)
        if workflow_count == 0 {
            if let Ok(sub_entries) = fs::read_dir(target) {
                let mut sub_dirs: Vec<_> = sub_entries
                    .filter_map(|e| e.ok().map(|x| x.path()))
                    .filter(|p| p.is_dir() && !p.file_name().and_then(|n| n.to_str()).unwrap_or("").starts_with('.'))
                    .collect();
                sub_dirs.sort();
                for sub_dir in sub_dirs {
                    let nested_gh = sub_dir.join(".github").join("workflows");
                    if nested_gh.is_dir() {
                        if let Ok(entries) = fs::read_dir(&nested_gh) {
                            let mut paths: Vec<_> = entries
                                .filter_map(|e| e.ok().map(|x| x.path()))
                                .filter(|p| {
                                    p.extension()
                                        .and_then(|e| e.to_str())
                                        .map(|ext| ext == "yml" || ext == "yaml")
                                        .unwrap_or(false)
                                })
                                .collect();
                            paths.sort();
                            let sub_name = sub_dir.file_name().and_then(|s| s.to_str()).unwrap_or("sub");
                            for p in paths {
                                workflow_count += 1;
                                let rel = format!(
                                    "{}/.github/workflows/{}",
                                    sub_name,
                                    p.file_name().and_then(|s| s.to_str()).unwrap_or("ci.yml")
                                );
                                let sub_gosum =
                                    sub_dir.join("go.sum").exists() || sub_dir.join("go.mod").exists();
                                let (wf_cp, wf_w, is_pr, wf_meta) = parse_github_workflow_file(
                                    &p,
                                    &rel,
                                    has_repo_gosum || sub_gosum,
                                    &mut jobs,
                                );
                                max_dag_critical_path = max_dag_critical_path.max(wf_cp);
                                if is_pr {
                                    any_pr_or_push_workflow = true;
                                    human_pr_dag_critical_path =
                                        human_pr_dag_critical_path.max(wf_cp);
                                    concurrent_pr_width += wf_w;
                                } else {
                                    max_other_width = max_other_width.max(wf_w);
                                }
                                wf_ast_metas.push((rel, is_pr, wf_meta));
                            }
                        }
                    }
                }
            }
        }
    }

    fn split_base_jid(jid: &str) -> (&str, Option<&'static str>) {
        if let Some(idx) = jid.rfind(" [") {
            if jid.ends_with(']') {
                let tag = jid[idx + 2..jid.len() - 1].trim();
                if !tag.is_empty() {
                    let low = tag.to_ascii_lowercase();
                    let os_kind = if low == "macos"
                        || low.contains("macos")
                        || low.contains("darwin")
                        || low.contains("osx")
                    {
                        "macos"
                    } else if low == "windows" || low.contains("windows") || low.contains("win") {
                        "windows"
                    } else {
                        "linux"
                    };
                    return (&jid[..idx], Some(os_kind));
                }
            }
        }
        (jid, None)
    }

    // Overlay real observed job labels, dynamic `fromJSON` matrix counts, and exact ceiled durations (`A1, A2, C2, C3, C4, C6`)
    if let Some(ref telem) = active_telemetry {
        let rt_has_scoped = telem.job_runtime_labels.iter().any(|(k, _)| k.contains('#'));
        let dur_has_scoped = telem.job_durations_mins.iter().any(|(k, _)| k.contains('#'));
        let mat_has_scoped = telem.job_observed_matrix_counts.iter().any(|(k, _)| k.contains('#'));
        let bil_has_scoped = telem.job_billed_mins.iter().any(|(k, _)| k.contains('#'));

        let match_wf_scope = |key: &str, wf_file: &str, has_scoped: bool| -> Option<String> {
            let wf_base = wf_file.rsplit('/').next().unwrap_or(wf_file);
            if let Some((k_wf, k_job)) = key.split_once('#') {
                let k_wf_base = k_wf.rsplit('/').next().unwrap_or(k_wf);
                if k_wf == wf_file || k_wf_base == wf_base {
                    Some(k_job.to_string())
                } else {
                    None
                }
            } else if workflow_count <= 1 || !has_scoped {
                Some(key.to_string())
            } else {
                None
            }
        };

        let leg_matches_os = |k_job: &str, os_tag: &str| -> bool {
            let low = k_job.to_ascii_lowercase();
            match os_tag {
                "macos" => low.contains("macos") || low.contains("darwin") || low.contains("osx"),
                "windows" => low.contains("windows") || low.contains("win32") || low.contains("win64"),
                _ => low.contains("ubuntu") || low.contains("linux") || low.contains("debian"),
            }
        };

        for j in &mut jobs {
            let (base_jid, multi_os_tag) = split_base_jid(&j.job_id);
            let wf_meta_opt = wf_ast_metas
                .iter()
                .find(|(wf, _, _)| wf == &j.workflow_file)
                .map(|(_, _, m)| m);
            let display_name = wf_meta_opt.and_then(|m| {
                m.job_display_names
                    .iter()
                    .find(|(id, _)| id == base_jid)
                    .map(|(_, dn)| dn.as_str())
            });
            let is_static_matrix_job = wf_meta_opt
                .map(|m| m.matrix_job_ids.iter().any(|id| id == base_jid))
                .unwrap_or(j.matrix_combination_count > 1 || multi_os_tag.is_some());

            let cand_matches_exact_or_prefix = |k_job: &str, allow_prefix: bool| -> bool {
                let cands: [&str; 2] = [base_jid, display_name.unwrap_or(base_jid)];
                for cand in cands {
                    if k_job.eq_ignore_ascii_case(cand)
                        || k_job.eq_ignore_ascii_case(&j.job_id)
                    {
                        return true;
                    }
                    if allow_prefix && !k_job.contains(" [os:") {
                        let pref = format!("{} (", cand);
                        if k_job.len() > pref.len()
                            && k_job[..pref.len()].eq_ignore_ascii_case(&pref)
                        {
                            return true;
                        }
                    }
                }
                false
            };

            // 1. Dynamic `fromJSON` matrix fan-out calibration (`C3`)
            if !is_static_matrix_job
                && j.matrix_combination_count == 1
                && !telem.job_observed_matrix_counts.is_empty()
            {
                if let Some(obs_cnt) = telem
                    .job_observed_matrix_counts
                    .iter()
                    .filter_map(|(k, cnt)| {
                        let kj = match_wf_scope(k, &j.workflow_file, mat_has_scoped)?;
                        if cand_matches_exact_or_prefix(&kj, false) {
                            Some(*cnt)
                        } else {
                            None
                        }
                    })
                    .max()
                {
                    if obs_cnt > 1 {
                        j.matrix_combination_count = obs_cnt;
                        let total_billed = j.billed_mins_per_job * (obs_cnt as f64);
                        let sku = resolve_runner_sku(&j.runner_label);
                        j.total_billed_mins_all_matrix = total_billed;
                        j.run_compute_cost_usd =
                            ((total_billed * j.rate_per_min_usd) * 10000.0).round() / 10000.0;
                        j.blacksmith_run_cost_usd = ((j.est_raw_duration_mins_per_job
                            * (obs_cnt as f64)
                            * sku.blacksmith_equiv_rate_usd)
                            * 10000.0)
                            .round()
                            / 10000.0;
                        j.self_hosted_spot_run_cost_usd = ((j.est_raw_duration_mins_per_job
                            * (obs_cnt as f64)
                            * sku.self_hosted_spot_rate_usd)
                            * 10000.0)
                            .round()
                            / 10000.0;
                    }
                }
            }

            // 2. Runtime runner label override (`C4`, preserving unceiled Spot/MicroVM math `A2`)
            if !telem.job_runtime_labels.is_empty()
                && multi_os_tag.is_none()
                && (j.runner_label.contains("${{") || j.runner_label == "ubuntu-latest")
            {
                if let Some(rt_lbl) = telem.job_runtime_labels.iter().find_map(|(k, lbl)| {
                    let kj = match_wf_scope(k, &j.workflow_file, rt_has_scoped)?;
                    if cand_matches_exact_or_prefix(&kj, true) {
                        Some(lbl)
                    } else {
                        None
                    }
                }) {
                    if j.runner_label.contains("${{")
                        || !rt_lbl.eq_ignore_ascii_case("ubuntu-latest")
                    {
                        let sku = resolve_runner_sku(rt_lbl);
                        j.runner_label = rt_lbl.clone();
                        j.provider = sku.provider.to_string();
                        j.os = sku.os.to_string();
                        j.vcpus = sku.vcpus;
                        j.rate_per_min_usd = sku.rate_per_min_usd;
                        let billed_per_job = select_provider_plugin(&j.provider)
                            .round_job_minutes(j.est_raw_duration_mins_per_job, sku.ceil_to_whole_minute);
                        let total_billed = billed_per_job * (j.matrix_combination_count as f64);
                        let total_raw =
                            j.est_raw_duration_mins_per_job * (j.matrix_combination_count as f64);
                        j.billed_mins_per_job = billed_per_job;
                        j.total_billed_mins_all_matrix = total_billed;
                        j.run_compute_cost_usd =
                            ((total_billed * j.rate_per_min_usd) * 10000.0).round() / 10000.0;
                        j.blacksmith_run_cost_usd =
                            ((total_raw * sku.blacksmith_equiv_rate_usd) * 10000.0).round()
                                / 10000.0;
                        j.self_hosted_spot_run_cost_usd =
                            ((total_raw * sku.self_hosted_spot_rate_usd) * 10000.0).round()
                                / 10000.0;
                    }
                }
            }

            // 3. Duration & exact mean-of-ceilings billed minutes (`A1, A2, C2`)
            if !telem.job_durations_mins.is_empty() {
                let collect_vals = |vec: &[(String, f64)], has_scoped: bool| -> Vec<f64> {
                    if let Some(os_tag) = multi_os_tag {
                        let cands: [&str; 2] = [base_jid, display_name.unwrap_or(base_jid)];
                        let os_bucket_vals: Vec<f64> = vec
                            .iter()
                            .filter_map(|(k, v)| {
                                let kj = match_wf_scope(k, &j.workflow_file, has_scoped)?;
                                for cand in cands {
                                    let want = format!("{} [os:{}]", cand, os_tag);
                                    if kj.eq_ignore_ascii_case(&want) {
                                        return Some(*v);
                                    }
                                }
                                None
                            })
                            .collect();
                        if !os_bucket_vals.is_empty() {
                            return os_bucket_vals;
                        }
                        let os_leg_vals: Vec<f64> = vec
                            .iter()
                            .filter_map(|(k, v)| {
                                let kj = match_wf_scope(k, &j.workflow_file, has_scoped)?;
                                if cand_matches_exact_or_prefix(&kj, true)
                                    && leg_matches_os(&kj, os_tag)
                                {
                                    Some(*v)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !os_leg_vals.is_empty() {
                            return os_leg_vals;
                        }
                    }
                    vec.iter()
                        .filter_map(|(k, v)| {
                            let kj = match_wf_scope(k, &j.workflow_file, has_scoped)?;
                            if cand_matches_exact_or_prefix(&kj, true) {
                                Some(*v)
                            } else {
                                None
                            }
                        })
                        .collect()
                };

                let matching_raw = collect_vals(&telem.job_durations_mins, dur_has_scoped);
                if !matching_raw.is_empty() {
                    let raw_m = matching_raw.iter().sum::<f64>() / (matching_raw.len() as f64);
                    let raw_clamped = (raw_m.max(0.05) * 100.0).round() / 100.0;
                    let sku = resolve_runner_sku(&j.runner_label);
                    let matching_billed = collect_vals(&telem.job_billed_mins, bil_has_scoped);
                    let billed_per_job = if !matching_billed.is_empty() {
                        let bm =
                            matching_billed.iter().sum::<f64>() / (matching_billed.len() as f64);
                        let min_floor = if sku.ceil_to_whole_minute {
                            raw_clamped.max(1.0)
                        } else {
                            raw_clamped
                        };
                        ((bm.max(min_floor)) * 100.0).round() / 100.0
                    } else {
                        select_provider_plugin(&j.provider)
                            .round_job_minutes(raw_clamped, sku.ceil_to_whole_minute)
                    };
                    let mat_f = j.matrix_combination_count as f64;
                    let total_billed = (billed_per_job * mat_f * 100.0).round() / 100.0;
                    let total_raw = raw_clamped * mat_f;
                    j.est_raw_duration_mins_per_job = raw_clamped;
                    j.billed_mins_per_job = billed_per_job;
                    j.total_billed_mins_all_matrix = total_billed;
                    j.run_compute_cost_usd =
                        ((total_billed * j.rate_per_min_usd) * 10000.0).round() / 10000.0;
                    j.blacksmith_run_cost_usd =
                        ((total_raw * sku.blacksmith_equiv_rate_usd) * 10000.0).round() / 10000.0;
                    j.self_hosted_spot_run_cost_usd =
                        ((total_raw * sku.self_hosted_spot_rate_usd) * 10000.0).round() / 10000.0;
                }
            }
        }

        // Recompute exact DAG critical path per workflow from overlaid durations (`C6`)
        if !telem.job_durations_mins.is_empty() && !wf_ast_metas.is_empty() {
            let mut new_max_cp = 0.0_f64;
            let mut new_human_cp = 0.0_f64;
            for (wf_rel, is_pr, meta) in &wf_ast_metas {
                let updated_nodes: Vec<(String, f64, usize, Vec<String>)> = meta
                    .dag_nodes
                    .iter()
                    .map(|(nid, orig_dur, exp_c, deps)| {
                        let overlaid_dur = jobs
                            .iter()
                            .filter(|j| {
                                &j.workflow_file == wf_rel && split_base_jid(&j.job_id).0 == nid
                            })
                            .map(|j| j.est_raw_duration_mins_per_job)
                            .fold(0.0_f64, f64::max);
                        let eff_dur = if overlaid_dur > 0.0 {
                            overlaid_dur
                        } else {
                            *orig_dur
                        };
                        (nid.clone(), eff_dur, *exp_c, deps.clone())
                    })
                    .collect();
                let (cp, _) = compute_dag_critical_path_and_width(&updated_nodes);
                new_max_cp = new_max_cp.max(cp);
                if *is_pr {
                    new_human_cp = new_human_cp.max(cp);
                }
            }
            if new_max_cp > 0.0 {
                max_dag_critical_path = new_max_cp;
            }
            if new_human_cp > 0.0 {
                human_pr_dag_critical_path = new_human_cp;
            }
        }
    }

    let mut providers: Vec<String> = jobs.iter().map(|j| j.provider.clone()).collect();
    providers.sort();
    providers.dedup();
    let primary_provider = if providers.iter().any(|s| s == "github-actions")
        || jobs
            .iter()
            .any(|j| j.workflow_file.contains(".github/workflows"))
    {
        "github-actions"
    } else {
        providers
            .first()
            .map(|s| s.as_str())
            .unwrap_or("github-actions")
    };

    // Count distinct `(workflow_file, base_jid)` so multi-OS matrix buckets count as 1 defined YAML job (`D5`)
    let mut distinct_defined_jobs: Vec<(&str, &str)> = Vec::new();
    for j in &jobs {
        let pair = (j.workflow_file.as_str(), split_base_jid(&j.job_id).0);
        if !distinct_defined_jobs.contains(&pair) {
            distinct_defined_jobs.push(pair);
        }
    }
    let total_defined_jobs = distinct_defined_jobs.len();
    let total_expanded_matrix_jobs: usize = jobs.iter().map(|j| j.matrix_combination_count).sum();
    let raw_duration_mins_per_run: f64 = jobs
        .iter()
        .map(|j| j.est_raw_duration_mins_per_job * (j.matrix_combination_count as f64))
        .sum();
    let max_single_job_mins = jobs
        .iter()
        .map(|j| j.est_raw_duration_mins_per_job)
        .fold(0.0_f64, f64::max);
    let critical_path_wait_mins_per_run: f64 = max_dag_critical_path.max(max_single_job_mins);
    let human_wait_mins_per_run: f64 = if !any_pr_or_push_workflow {
        0.0
    } else if human_pr_dag_critical_path > 0.0 {
        human_pr_dag_critical_path
    } else {
        critical_path_wait_mins_per_run
    };
    let billed_mins_per_run: f64 = jobs.iter().map(|j| j.total_billed_mins_all_matrix).sum();

    let pr_monthly_runs = (usage.prs_per_week * usage.pushes_per_pr * 4.33).round();
    let lookup_fixed_wf_runs = |wf_file: &str| -> Option<f64> {
        let wf_base = wf_file.rsplit('/').next().unwrap_or(wf_file);
        hygiene
            .workflow_fixed_monthly_runs
            .iter()
            .find(|(k, _)| k == wf_file || k.rsplit('/').next().unwrap_or(k) == wf_base)
            .map(|(_, v)| *v)
    };
    let has_live_wf_runs = active_telemetry
        .as_ref()
        .map(|t| !t.workflow_monthly_runs.is_empty())
        .unwrap_or(false);
    let is_truncated_live_window = active_telemetry
        .as_ref()
        .and_then(|t| Some((t.observed_monthly_runs?, t.observed_runs_in_window? as f64)))
        .map(|(mo, obs)| obs > 0.0 && mo > obs * 1.5)
        .unwrap_or(false);
    let lookup_live_wf_runs = |wf_file: &str| -> Option<f64> {
        let telem = active_telemetry.as_ref()?;
        if telem.workflow_monthly_runs.is_empty() {
            return None;
        }
        let wf_base = wf_file.rsplit('/').next().unwrap_or(wf_file);
        telem
            .workflow_monthly_runs
            .iter()
            .find(|(k, _)| k == wf_file || k.rsplit('/').next().unwrap_or(k) == wf_base)
            .map(|(_, v)| *v)
    };

    let monthly_runs = if workflow_count == 0 {
        0.0
    } else if let Some(obs_runs) = active_telemetry.as_ref().and_then(|t| t.observed_monthly_runs) {
        obs_runs.max(0.0)
    } else if !any_pr_or_push_workflow && !hygiene.workflow_fixed_monthly_runs.is_empty() {
        (hygiene
            .workflow_fixed_monthly_runs
            .iter()
            .map(|(_, r)| *r)
            .sum::<f64>()
            * 100.0)
            .round()
            / 100.0
    } else {
        pr_monthly_runs
    };

    // Populate exact per-workflow `monthly_runs_for_job` on EVERY job (`C1, C7, D1, D2`)
    for j in &mut jobs {
        let wf_r = if has_live_wf_runs {
            if let Some(r) = lookup_live_wf_runs(&j.workflow_file) {
                r.max(0.0)
            } else if is_truncated_live_window {
                // High-velocity repo where 300-run sample covered <20d: preserve periodic cron schedules (`C7`)
                lookup_fixed_wf_runs(&j.workflow_file)
                    .filter(|&cr| cr > 4.0)
                    .unwrap_or(0.0)
            } else {
                0.0
            }
        } else if active_telemetry
            .as_ref()
            .and_then(|t| t.observed_monthly_runs)
            .is_some()
        {
            monthly_runs
        } else if let Some(fixed_r) = lookup_fixed_wf_runs(&j.workflow_file) {
            fixed_r.max(0.0)
        } else {
            pr_monthly_runs
        };
        j.monthly_runs_for_job = Some((wf_r * 100.0).round() / 100.0);
    }

    let monthly_billed_mins = if !jobs.is_empty() {
        let m: f64 = jobs
            .iter()
            .map(|j| j.total_billed_mins_all_matrix * j.effective_monthly_runs(monthly_runs))
            .sum();
        (m * 100.0).round() / 100.0
    } else if let Some(inv_mins) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_monthly_runs)
    {
        inv_mins
    } else {
        (billed_mins_per_run * monthly_runs * 100.0).round() / 100.0
    };

    let provider_plugin = select_provider_plugin(primary_provider);
    let invoice_receipt =
        provider_plugin.compute_invoice_ledger(&jobs, monthly_runs, &usage.tier, visibility);
    let free_tier_included_mins = invoice_receipt.free_tier_included_mins;
    let free_tier_quota_burn_pct = invoice_receipt.free_tier_quota_burn_pct;
    let effective_tier = invoice_receipt.effective_tier;

    let gross_compute_cost_per_run_usd = norm_zero(
        (jobs.iter().map(|j| j.run_compute_cost_usd).sum::<f64>() * 10000.0).round() / 10000.0,
    );
    let is_invoice_reconciled = active_telemetry
        .as_ref()
        .map(|t| t.provenance_mode == "invoice_reconciled")
        .unwrap_or(false);
    let mut gross_monthly_compute_cost_usd = norm_zero(
        if is_invoice_reconciled {
            if let Some(inv_gross) = active_telemetry
                .as_ref()
                .and_then(|t| t.invoice_compute_gross_usd.or(t.invoice_gross_usd))
            {
                (inv_gross * 100.0).round() / 100.0
            } else {
                invoice_receipt.gross_monthly_compute_cost_usd
            }
        } else {
            invoice_receipt.gross_monthly_compute_cost_usd
        },
    );

    let (raw_net_billed, raw_discount) = if is_invoice_reconciled {
        if let Some(inv_net) = active_telemetry
            .as_ref()
            .and_then(|t| t.invoice_compute_net_usd.or(t.invoice_net_billed_usd))
        {
            let net_r = (inv_net.max(0.0) * 100.0).round() / 100.0;
            if net_r > gross_monthly_compute_cost_usd {
                gross_monthly_compute_cost_usd = net_r;
            }
            let disc_r = ((gross_monthly_compute_cost_usd - net_r).max(0.0) * 100.0).round() / 100.0;
            (net_r, disc_r)
        } else {
            (
                invoice_receipt.net_billed_monthly_compute_cost_usd,
                invoice_receipt.discount_allowance_monthly_usd,
            )
        }
    } else {
        (
            invoice_receipt.net_billed_monthly_compute_cost_usd,
            invoice_receipt.discount_allowance_monthly_usd,
        )
    };
    let net_billed_monthly_compute_cost_usd = norm_zero(raw_net_billed);
    let discount_allowance_monthly_usd = norm_zero(raw_discount);
    let third_party_runner_monthly_cost_usd =
        norm_zero(invoice_receipt.third_party_or_self_hosted_monthly_usd);
    let shard_setup_duplication_mins_per_run = norm_zero(
        ((jobs
            .iter()
            .filter(|j| j.matrix_combination_count > 1)
            .map(|j| {
                let dup_shards = (j.matrix_combination_count.saturating_sub(1)) as f64;
                (dup_shards * 1.25).min(j.est_raw_duration_mins_per_job * dup_shards * 0.75)
            })
            .sum::<f64>())
            * 100.0)
            .round()
            / 100.0,
    );
    let clones_per_ci_run = total_expanded_matrix_jobs;

    // DORA-anchored 40% productivity-loss factor applied without cascading intermediate rounding (`G1`)
    let productivity_loss_factor = 0.40_f64;
    let human_monthly_runs = if !any_pr_or_push_workflow {
        0.0
    } else {
        active_telemetry
            .as_ref()
            .and_then(|t| t.observed_human_pr_monthly_runs)
            .unwrap_or(if has_live_wf_runs { monthly_runs } else { pr_monthly_runs })
    };
    let raw_dev_wait_per_run =
        (human_wait_mins_per_run / 60.0) * usage.dev_hourly_rate_usd * productivity_loss_factor;
    let dev_wait_cost_per_run_usd = norm_zero((raw_dev_wait_per_run * 100.0).round() / 100.0);
    let monthly_dev_wait_cost_usd =
        norm_zero(((raw_dev_wait_per_run * human_monthly_runs) * 100.0).round() / 100.0);

    let any_artifacts = jobs.iter().any(|j| j.uploads_artifacts);

    let monthly_blacksmith_cost_usd = norm_zero(
        ((jobs
            .iter()
            .map(|j| j.blacksmith_run_cost_usd * j.effective_monthly_runs(monthly_runs))
            .sum::<f64>())
            * 100.0)
            .round()
            / 100.0,
    );
    let monthly_self_hosted_spot_cost_usd = if jobs.is_empty() {
        0.0
    } else {
        norm_zero(
            ((jobs
                .iter()
                .map(|j| j.self_hosted_spot_run_cost_usd * j.effective_monthly_runs(monthly_runs))
                .sum::<f64>()
                + SELF_HOSTED_VPS_BASE_MONTHLY_USD)
                * 100.0)
                .round()
                / 100.0,
        )
    };

    // --- Compute Line-by-Line CI/CD Invoice Cost Components (per-job runner rate for rounding tax `A3`) ---
    let rounding_tax_mins_per_run =
        ((billed_mins_per_run - raw_duration_mins_per_run).max(0.0) * 100.0).round() / 100.0;
    let avg_rate_per_min = if billed_mins_per_run > 0.0 {
        gross_compute_cost_per_run_usd / billed_mins_per_run
    } else {
        0.006
    };
    let monthly_rounding_tax_usd = norm_zero(
        ((jobs
            .iter()
            .map(|j| {
                let raw_all =
                    j.est_raw_duration_mins_per_job * (j.matrix_combination_count as f64);
                let job_tax_mins = (j.total_billed_mins_all_matrix - raw_all).max(0.0);
                job_tax_mins * j.rate_per_min_usd * j.effective_monthly_runs(monthly_runs)
            })
            .sum::<f64>())
            * 100.0)
            .round()
            / 100.0,
    );

    let mut linux_mins_per_run = 0.0_f64;
    let mut windows_mins_per_run = 0.0_f64;
    let mut macos_mins_per_run = 0.0_f64;
    let mut max_vcpus = 2_u32;
    let mut has_arm_jobs = false;
    let mut has_linux_arm_jobs = false;
    let mut jobs_uploading_artifacts = 0_usize;
    let mut weighted_ret_days_num = 0.0_f64;
    let mut weighted_ret_days_den = 0.0_f64;
    let mut macos_concurrent_jobs_per_run = 0_usize;
    let mut self_hosted_or_microvm_jobs_per_run = 0_usize;
    let mut true_self_hosted_jobs_per_run = 0_usize;

    for j in &jobs {
        if j.vcpus > max_vcpus {
            max_vcpus = j.vcpus;
        }
        if j.uploads_artifacts {
            jobs_uploading_artifacts += j.matrix_combination_count;
            if let Some(d) = j.artifact_retention_days {
                let w = (j.matrix_combination_count.max(1) as f64)
                    * j.effective_monthly_runs(monthly_runs).max(1.0);
                weighted_ret_days_num += (d as f64) * w;
                weighted_ret_days_den += w;
            }
        } else if let Some(d) = j.artifact_retention_days {
            if weighted_ret_days_den == 0.0 {
                weighted_ret_days_num += d as f64;
                weighted_ret_days_den += 1.0;
            }
        }
        let rl_low = j.runner_label.to_ascii_lowercase();
        let is_self_or_3p = j.provider.starts_with("self-hosted")
            || j.provider == "blacksmith"
            || j.provider == "third-party-microvm"
            || rl_low.contains("blacksmith")
            || rl_low.contains("depot")
            || rl_low.contains("warpbuild")
            || rl_low.contains("buildjet")
            || rl_low.contains("namespace")
            || rl_low.contains("self-hosted");

        let os_low = j.os.to_ascii_lowercase();
        if os_low.contains("mac") {
            macos_mins_per_run += j.total_billed_mins_all_matrix;
            // Only vendor-hosted macOS jobs count against GitHub's 5-concurrent hosted macOS cap (`G4`)
            if !is_self_or_3p {
                macos_concurrent_jobs_per_run += j.matrix_combination_count;
            }
        } else if os_low.contains("win") {
            windows_mins_per_run += j.total_billed_mins_all_matrix;
        } else {
            linux_mins_per_run += j.total_billed_mins_all_matrix;
        }

        let is_job_arm = rl_low.contains("arm")
            || rl_low.contains("aarch64")
            || rl_low.contains("c7g")
            || rl_low.contains("c8g")
            || resolve_runner_sku(&j.runner_label).arch == "arm64";
        if is_job_arm {
            has_arm_jobs = true;
            if os_low.contains("linux") || (!os_low.contains("mac") && !os_low.contains("win")) {
                has_linux_arm_jobs = true;
            }
        }
        if j.provider.starts_with("self-hosted") || rl_low.contains("self-hosted") {
            true_self_hosted_jobs_per_run += j.matrix_combination_count;
        }
        if is_self_or_3p {
            self_hosted_or_microvm_jobs_per_run += j.matrix_combination_count;
        }
    }
    let yaml_configured_retention_days: Option<u32> = if weighted_ret_days_den > 0.0 {
        Some(((weighted_ret_days_num / weighted_ret_days_den).round() as u32).max(1))
    } else {
        None
    };

    // Use provider_plugin's exact free-tier pool math so Larger Runners do NOT inflate OS-weighted pool mins (Fixes Bug #6)
    let os_weighted_free_tier_mins_per_run = invoice_receipt.os_weighted_pool_mins_per_run;
    let monthly_os_weighted_free_tier_mins = invoice_receipt.monthly_os_weighted_pool_mins;
    let monthly_flake_retry_10pct_overhead_usd =
        ((gross_monthly_compute_cost_usd * 0.10) * 100.0).round() / 100.0;

    let hosted_effective_hourly_rate_usd = ((avg_rate_per_min * 60.0) * 100.0).round() / 100.0;
    let is_macos_only_workload =
        macos_mins_per_run > 0.0 && linux_mins_per_run == 0.0 && windows_mins_per_run == 0.0;
    let (ec2_label, ec2_od_hr, ec2_od_mo, ec2_spot_hr) =
        infracost_ec2_runner_profile(max_vcpus, has_linux_arm_jobs, is_macos_only_workload);
    let infracost_ec2_surrounding_overhead_monthly_usd = if jobs.is_empty() { 0.0 } else { 8.40 };
    let macos_self_hosted_mac_mini_monthly_usd = if macos_mins_per_run > 0.0 {
        25.00
    } else {
        0.0
    };

    // Primary-source verified Ephemeral Runner & Spot Instance Economics (`G3`: 720h/mo capacity per node)
    let active_runner_hours_per_month = monthly_billed_mins / 60.0;
    let fleet_nodes = if jobs.is_empty() {
        0_usize
    } else {
        ((active_runner_hours_per_month / 720.0).ceil() as usize).max(1)
    };
    let always_on_runner_utilization_pct = if fleet_nodes == 0 {
        0.0
    } else {
        (((active_runner_hours_per_month / ((fleet_nodes as f64) * 720.0)) * 100.0 * 10.0).round()
            / 10.0)
            .min(100.0)
    };
    // Slow-path cold start (90s = 1.5m on ~10% of burst runs) at 40% DORA productivity factor
    let ephemeral_cold_start_wait_tax_monthly_usd = if jobs.is_empty() {
        0.0
    } else {
        (((monthly_runs * 0.10 * (1.5 / 60.0)) * usage.dev_hourly_rate_usd * 0.40) * 100.0).round()
            / 100.0
    };
    let ephemeral_runner_recommended_mode = if jobs.is_empty() {
        "none".to_string()
    } else if monthly_billed_mins < SELF_HOSTED_BREAKEVEN_MINS_PER_MONTH
        && gross_monthly_compute_cost_usd < 150.0
    {
        "hosted-saas".to_string()
    } else if always_on_runner_utilization_pct < 45.0 {
        if ephemeral_cold_start_wait_tax_monthly_usd >= 25.0 || monthly_billed_mins >= 15000.0 {
            "hybrid-warm-pool-plus-ephemeral-spot".to_string()
        } else {
            "ephemeral-arc-karpenter".to_string()
        }
    } else if active_runner_hours_per_month >= 700.0 && always_on_runner_utilization_pct >= 65.0 {
        "always-on-horizontal".to_string()
    } else {
        "hybrid-warm-pool-plus-ephemeral-spot".to_string()
    };

    // Primary-source verified Kubernetes Ephemeral Runner Control-Plane Pricing:
    // AWS EKS Standard Support is $0.10/hr = $73.00/mo ($0.60/hr = $438.00/mo Extended Support),
    // whereas GKE Free Tier (1 zonal cluster) and Azure AKS Free Tier are $0.00/mo.
    let k8s_eks_control_plane_monthly_usd = if jobs.is_empty() { 0.0 } else { 73.00 };
    let k8s_gke_aks_free_tier_control_plane_monthly_usd = 0.00_f64;
    let (spot_interruption_band_pct, spot_retry_factor) = if jobs.is_empty() {
        ("none".to_string(), 0.0)
    } else if has_arm_jobs || ec2_label.starts_with("c7g") || ec2_label.starts_with("c6g") {
        (
            "5-10% (Graviton c7g / AMD c7a / diversified Karpenter)".to_string(),
            0.0125,
        )
    } else {
        (
            ">20% single-family x86 in us-east-1 (use diversified Karpenter c7a+c6a+c7g for <10%)"
                .to_string(),
            0.025,
        )
    };
    let spot_retry_overhead_monthly_usd =
        ((monthly_self_hosted_spot_cost_usd * spot_retry_factor) * 100.0).round() / 100.0;

    // 3. Artifact Storage (90d default vs configured YAML retention-days vs plan-included allowance + S3, `G2`)
    let default_retention_days = if providers.iter().any(|p| p == "gitlab-ci" || p == "circleci") {
        30
    } else {
        90
    };
    let configured_retention_days = yaml_configured_retention_days.unwrap_or({
        if usage.artifact_retention_days != 30 {
            usage.artifact_retention_days
        } else {
            default_retention_days
        }
    });
    let monthly_created_gb = if jobs.is_empty() || !any_artifacts {
        0.0
    } else {
        let mut wf_art_jobs: Vec<(String, usize, f64)> = Vec::new();
        for j in &jobs {
            if j.uploads_artifacts {
                let jr = j.effective_monthly_runs(monthly_runs);
                if let Some(entry) = wf_art_jobs.iter_mut().find(|(w, _, _)| w == &j.workflow_file)
                {
                    entry.1 += j.matrix_combination_count;
                } else {
                    wf_art_jobs.push((j.workflow_file.clone(), j.matrix_combination_count, jr));
                }
            }
        }
        if has_live_wf_runs {
            wf_art_jobs
                .iter()
                .map(|(_, cnt, r)| usage.artifact_gb_per_run * (*r) * ((*cnt).max(1) as f64).min(3.0))
                .sum()
        } else {
            let total_art_runs = if wf_art_jobs.len() == 1 {
                wf_art_jobs[0].2
            } else {
                let weighted_r: f64 = wf_art_jobs
                    .iter()
                    .map(|(_, cnt, r)| (*cnt as f64) * (*r))
                    .sum();
                weighted_r / (jobs_uploading_artifacts.max(1) as f64)
            };
            usage.artifact_gb_per_run * total_art_runs * (jobs_uploading_artifacts.max(1) as f64).min(3.0)
        }
    };
    let observed_art_gb_opt = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_artifact_storage_gb);
    let steady_state_stored_gb_configured = if let Some(obs_gb) = observed_art_gb_opt {
        (obs_gb * 100.0).round() / 100.0
    } else {
        ((monthly_created_gb * (configured_retention_days as f64 / 30.0)) * 100.0).round() / 100.0
    };
    let monthly_vendor_storage_configured_usd =
        ((steady_state_stored_gb_configured * ARTIFACT_STORAGE_VENDOR_USD_PER_GB_MO) * 100.0)
            .round()
            / 100.0;
    let included_storage_gb = if visibility == RepoVisibility::Public {
        f64::INFINITY
    } else {
        provider_plugin.plan_included_storage_gb(&usage.tier)
    };
    let net_billable_storage_gb = if included_storage_gb.is_infinite() {
        0.0
    } else {
        (steady_state_stored_gb_configured - included_storage_gb).max(0.0)
    };
    let net_billed_vendor_storage_configured_usd =
        ((net_billable_storage_gb * ARTIFACT_STORAGE_VENDOR_USD_PER_GB_MO) * 100.0).round() / 100.0;
    let monthly_infracost_s3_configured_usd =
        ((steady_state_stored_gb_configured * ARTIFACT_STORAGE_S3_USD_PER_GB_MO) * 100.0).round()
            / 100.0;
    let monthly_artifact_storage_vendor_usd = monthly_vendor_storage_configured_usd;
    let monthly_artifact_storage_s3_usd = monthly_infracost_s3_configured_usd;
    let (steady_state_stored_gb_90d, steady_state_stored_gb_14d) =
        if let Some(obs_gb) = observed_art_gb_opt {
            let base_days = (configured_retention_days.max(1)) as f64;
            (
                ((obs_gb * (90.0 / base_days)) * 100.0).round() / 100.0,
                ((obs_gb * (14.0 / base_days)) * 100.0).round() / 100.0,
            )
        } else {
            (
                ((monthly_created_gb * 3.0) * 100.0).round() / 100.0,
                ((monthly_created_gb * (14.0 / 30.0)) * 100.0).round() / 100.0,
            )
        };
    let monthly_vendor_storage_90d_usd =
        ((steady_state_stored_gb_90d * ARTIFACT_STORAGE_VENDOR_USD_PER_GB_MO) * 100.0).round()
            / 100.0;
    let monthly_vendor_storage_14d_usd =
        ((steady_state_stored_gb_14d * ARTIFACT_STORAGE_VENDOR_USD_PER_GB_MO) * 100.0).round()
            / 100.0;
    let monthly_infracost_s3_standard_90d_usd =
        ((steady_state_stored_gb_90d * ARTIFACT_STORAGE_S3_USD_PER_GB_MO) * 100.0).round() / 100.0;
    let monthly_infracost_s3_intelligent_14d_usd =
        ((steady_state_stored_gb_14d * ARTIFACT_STORAGE_S3_INTELLIGENT_USD_PER_GB_MO) * 100.0)
            .round()
            / 100.0;

    // 4. Parallelism Cost (DAG-aware concurrent stage width & vendor concurrency caps — Fixes Bug #23)
    let dag_stage_width = concurrent_pr_width.max(max_other_width);
    let static_peak_concurrent = if dag_stage_width > 0 {
        dag_stage_width.min(total_expanded_matrix_jobs)
    } else {
        total_expanded_matrix_jobs
    };
    let peak_concurrent_jobs_per_run = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_peak_concurrent_jobs)
        .map(|obs_p| static_peak_concurrent.max(obs_p))
        .unwrap_or(static_peak_concurrent);
    let hosted_ratio = if total_expanded_matrix_jobs > 0 {
        (total_expanded_matrix_jobs.saturating_sub(self_hosted_or_microvm_jobs_per_run) as f64)
            / (total_expanded_matrix_jobs as f64)
    } else {
        0.0
    };
    let hosted_concurrent_jobs_per_run =
        ((peak_concurrent_jobs_per_run as f64) * hosted_ratio).ceil() as usize;
    let (vendor_free_concurrent_cap, vendor_team_concurrent_cap) =
        provider_plugin.concurrency_caps();
    let parallelism_tier_upgrade_tax_monthly_usd =
        if hosted_concurrent_jobs_per_run > vendor_team_concurrent_cap
            || macos_concurrent_jobs_per_run > 5
        {
            510.00
        } else if hosted_concurrent_jobs_per_run > vendor_free_concurrent_cap {
            100.00
        } else {
            0.00
        };

    // 5. Log Retention Cost (Native vendor retention is $0; external SIEM only when detected — GEMINI.md Rule #3)
    let (detected_log_language_profile, base_log_mb_per_job, infracost_iac_files_detected) =
        detect_language_and_iac_signals(target, &jobs);
    let est_log_mb_per_run = if total_expanded_matrix_jobs == 0 {
        0.0
    } else {
        ((base_log_mb_per_job * (total_expanded_matrix_jobs as f64) * 1.15) * 100.0).round() / 100.0
    };
    let monthly_log_volume_gb =
        (((est_log_mb_per_run * monthly_runs) / 1024.0) * 100.0).round() / 100.0;
    let vendor_default_log_retention_days = if providers.iter().any(|p| p == "aws-codebuild") {
        14
    } else if providers.iter().any(|p| p == "gitlab-ci" || p == "circleci") {
        30
    } else {
        90
    };
    let (
        monthly_datadog_30d_log_shipping_usd,
        monthly_datadog_90d_log_shipping_usd,
        monthly_splunk_30d_log_shipping_usd,
        monthly_infracost_loki_s3_usd,
    ) = if hygiene.has_external_log_shipper {
        (
            ((monthly_log_volume_gb * 3.10) * 100.0).round() / 100.0,
            ((monthly_log_volume_gb * 4.60) * 100.0).round() / 100.0,
            ((monthly_log_volume_gb * 2.10) * 100.0).round() / 100.0,
            ((monthly_log_volume_gb * ARTIFACT_STORAGE_S3_USD_PER_GB_MO) * 100.0).round() / 100.0,
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    // 6. Cross-Region Egress & NAT Gateway Cost (Strictly gated to cloud VPC / self-hosted / explicit cloud registry — GEMINI.md Rule #3)
    let infracost_cloud_region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|_| "us-east-1".to_string());
    let infracost_aws_nat_gateway_per_gb_usd =
        infracost_aws_nat_gateway_rate_usd(&infracost_cloud_region);
    let has_cloud_vpc_egress = true_self_hosted_jobs_per_run > 0
        || primary_provider == "aws-codebuild"
        || primary_provider == "gcp-cloud-build"
        || hygiene.has_ecr_or_registry_mirror;
    let est_transfer_gb_per_run = if total_expanded_matrix_jobs == 0 || !has_cloud_vpc_egress {
        0.0
    } else {
        (((total_expanded_matrix_jobs as f64) * 0.18 + (jobs_uploading_artifacts as f64) * 0.35)
            * 100.0)
            .round()
            / 100.0
    };
    let monthly_transfer_gb = if total_expanded_matrix_jobs == 0 || !has_cloud_vpc_egress {
        0.0
    } else {
        let g: f64 = jobs
            .iter()
            .map(|j| {
                let r = j.effective_monthly_runs(monthly_runs);
                let art_c = if j.uploads_artifacts {
                    j.matrix_combination_count as f64
                } else {
                    0.0
                };
                ((j.matrix_combination_count as f64) * 0.18 + art_c * 0.35) * r
            })
            .sum();
        (g * 100.0).round() / 100.0
    };
    let worst_case_rate_per_gb = 0.80 * infracost_aws_nat_gateway_per_gb_usd
        + 0.20 * (infracost_aws_nat_gateway_per_gb_usd + AWS_INTERNET_EGRESS_USD_PER_GB);
    let (
        monthly_infracost_private_subnet_nat_worst_case_usd,
        monthly_infracost_cross_region_registry_usd,
        monthly_infracost_vpc_endpoint_cache_optimized_usd,
    ) = if has_cloud_vpc_egress {
        (
            ((monthly_transfer_gb * worst_case_rate_per_gb) * 100.0).round() / 100.0,
            ((monthly_transfer_gb * AWS_CROSS_REGION_USD_PER_GB) * 100.0).round() / 100.0,
            ((monthly_transfer_gb * 0.30 * AWS_VPC_ENDPOINT_USD_PER_GB) * 100.0).round() / 100.0,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    // 7. Developer Wait Time Cost (DORA-Anchored 40% Productivity-Loss Factor & Human PR Runs)
    let blended_fully_loaded_hourly_usd = 100.0_f64;
    let monthly_wait_hours =
        (((human_wait_mins_per_run * human_monthly_runs) / 60.0) * 100.0).round() / 100.0;
    let monthly_productive_payroll_loss_usd = monthly_dev_wait_cost_usd;
    let monthly_blended_100hr_productive_loss_usd =
        ((monthly_wait_hours * blended_fully_loaded_hourly_usd * productivity_loss_factor) * 100.0)
            .round()
            / 100.0;
    let wait_to_compute_cost_ratio = if gross_monthly_compute_cost_usd > 0.0 {
        ((monthly_productive_payroll_loss_usd / gross_monthly_compute_cost_usd) * 100.0).round()
            / 100.0
    } else {
        0.0
    };
    let lead_time_feedback_tier = if human_wait_mins_per_run <= 0.0 {
        "none (0m)".to_string()
    } else if human_wait_mins_per_run < 5.0 {
        "best-in-class (<5m)".to_string()
    } else if human_wait_mins_per_run <= 10.0 {
        "tight-feedback (5-10m)".to_string()
    } else if human_wait_mins_per_run <= 30.0 {
        "moderate-drag (10-30m)".to_string()
    } else {
        "severe-context-switch-drag (>30m)".to_string()
    };

    // 8. Failed Build Rerun Cost (Live SHA-verified `observed_flake_rate_pct` or static E2E/retry heuristic, `G5`)
    let has_live_observed_flake = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_flake_rate_pct)
        .is_some();
    let estimated_flake_rate_pct = if jobs.is_empty() {
        0.0
    } else if let Some(obs_f) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_flake_rate_pct)
    {
        (obs_f.clamp(0.0, 85.0) * 10.0).round() / 10.0
    } else if hygiene.has_browser_or_e2e_tests || hygiene.has_job_level_auto_retry {
        10.0
    } else {
        5.0
    };
    // In Static mode, `p/(1-p)` models extra reruns on top of baseline runs; in Live mode, `gross` ALREADY includes
    // observed reruns, so `flake_rate_frac` is the rerun share without double-counting (`G5`)!
    let flake_rate_frac = estimated_flake_rate_pct / 100.0;
    let rerun_amplification = if flake_rate_frac > 0.0 && flake_rate_frac < 0.9 {
        if has_live_observed_flake {
            flake_rate_frac
        } else {
            flake_rate_frac / (1.0 - flake_rate_frac)
        }
    } else {
        0.0
    };
    let monthly_flaky_reruns = ((monthly_runs * flake_rate_frac) * 100.0).round() / 100.0;
    let monthly_rerun_compute_cost_usd =
        ((gross_monthly_compute_cost_usd * rerun_amplification) * 100.0).round() / 100.0;
    let avg_investigation_mins_per_flake = 8.0_f64;
    let monthly_flake_investigation_hours =
        (((monthly_flaky_reruns * avg_investigation_mins_per_flake) / 60.0) * 100.0).round()
            / 100.0;
    let monthly_flake_investigation_payroll_usd =
        ((monthly_flake_investigation_hours * usage.dev_hourly_rate_usd) * 100.0).round() / 100.0;
    let monthly_total_immediate_flake_cost_usd =
        ((monthly_rerun_compute_cost_usd + monthly_flake_investigation_payroll_usd) * 100.0)
            .round()
            / 100.0;
    let target_1pct_amp = if has_live_observed_flake {
        0.01
    } else {
        0.01 / 0.99
    };
    let compute_sav_at_1pct = if estimated_flake_rate_pct > 1.0 && rerun_amplification > 0.0 {
        monthly_rerun_compute_cost_usd
            * ((rerun_amplification - target_1pct_amp).max(0.0) / rerun_amplification)
    } else {
        0.0
    };
    let payroll_sav_at_1pct = if estimated_flake_rate_pct > 1.0 {
        monthly_flake_investigation_payroll_usd
            * ((estimated_flake_rate_pct - 1.0) / estimated_flake_rate_pct)
    } else {
        0.0
    };
    let monthly_savings_at_1pct_target_usd =
        ((compute_sav_at_1pct + payroll_sav_at_1pct) * 100.0).round() / 100.0;

    // 9. Build Cache Savings (Per-Stack Realistic Savings & Per-Workflow Cadence `C1`)
    let (stack_cache_label, realistic_saving_pct_min, realistic_saving_pct_max, cache_mid_ratio) =
        stack_cache_savings_profile(
            &detected_log_language_profile,
            hygiene.uses_pnpm,
            hygiene.uses_poetry,
        );
    let mut jobs_with_cache = 0_usize;
    let mut jobs_missing_cache = 0_usize;
    let mut uncached_monthly_cost_raw = 0.0_f64;
    for j in &jobs {
        if j.uses_cache {
            jobs_with_cache += j.matrix_combination_count;
        } else if j.est_raw_duration_mins_per_job > 2.0 {
            jobs_missing_cache += j.matrix_combination_count;
            uncached_monthly_cost_raw +=
                j.run_compute_cost_usd * j.effective_monthly_runs(monthly_runs);
        }
    }
    let estimated_cache_hit_rate_pct = if jobs_with_cache == 0 {
        0.0
    } else if hygiene.has_restore_keys_fallback {
        85.0
    } else {
        75.0
    };
    let (vendor_cache_limit, vendor_cache_eviction) = vendor_cache_mechanics(primary_provider);
    let monthly_uncached_compute_usd = (uncached_monthly_cost_raw * 100.0).round() / 100.0;
    let monthly_unrealized_cache_savings_usd =
        ((monthly_uncached_compute_usd * cache_mid_ratio) * 100.0).round() / 100.0;

    // Spot-Ineligible Jobs (Release ceremonies, >30m long integration suites, or non-idempotent deploy/apply jobs)
    let mut spot_ineligible_jobs_count = 0_usize;
    let mut spot_ineligible_mins_per_run = 0.0_f64;
    for j in &jobs {
        let jid_low = j.job_id.to_ascii_lowercase();
        let wf_low = j.workflow_file.to_ascii_lowercase();
        let is_ineligible = j.est_raw_duration_mins_per_job > 30.0
            || jid_low.contains("release")
            || jid_low.contains("deploy")
            || jid_low.contains("publish")
            || jid_low.contains("apply")
            || jid_low.contains("migrate")
            || wf_low.contains("release")
            || wf_low.contains("deploy")
            || wf_low.contains("publish")
            || (j.matrix_combination_count == 1
                && hygiene
                    .spot_ineligible_workflow_files
                    .iter()
                    .any(|wf| wf == &j.workflow_file));
        if is_ineligible {
            spot_ineligible_jobs_count += j.matrix_combination_count;
            spot_ineligible_mins_per_run += j.total_billed_mins_all_matrix;
        }
    }
    let ineligible_share = if billed_mins_per_run > 0.0 {
        (spot_ineligible_mins_per_run / billed_mins_per_run).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let on_demand_active_monthly_usd =
        ((active_runner_hours_per_month * ec2_od_hr) + infracost_ec2_surrounding_overhead_monthly_usd).max(0.0);
    let hybrid_spot_plus_on_demand_monthly_usd = if jobs.is_empty() {
        0.0
    } else {
        (((monthly_self_hosted_spot_cost_usd + spot_retry_overhead_monthly_usd)
            * (1.0 - ineligible_share)
            + on_demand_active_monthly_usd * ineligible_share)
            * 100.0)
            .round()
            / 100.0
    };

    // Matrix Build Cost Strategy (Library vs Application & Conditional PR Matrix 80% Savings, `D5`)
    let mut distinct_matrix_jobs: Vec<(&str, &str)> = Vec::new();
    for j in &jobs {
        let (bjid, os_tag) = split_base_jid(&j.job_id);
        if j.matrix_combination_count > 1 || os_tag.is_some() {
            let pair = (j.workflow_file.as_str(), bjid);
            if !distinct_matrix_jobs.contains(&pair) {
                distinct_matrix_jobs.push(pair);
            }
        }
    }
    let matrix_jobs_count = distinct_matrix_jobs.len();
    let mut pr_unconditional_matrix_cells = 0_usize;
    let mut pr_excess_matrix_monthly_usd = 0.0_f64;
    for j in &jobs {
        if j.matrix_combination_count > 1
            && hygiene
                .pr_unconditional_matrix_workflow_files
                .iter()
                .any(|wf| wf == &j.workflow_file)
        {
            pr_unconditional_matrix_cells += j.matrix_combination_count;
            let single_cell_cost = j.run_compute_cost_usd / (j.matrix_combination_count as f64);
            let excess = (j.run_compute_cost_usd - single_cell_cost).max(0.0);
            pr_excess_matrix_monthly_usd += excess * j.effective_monthly_runs(monthly_runs);
        }
    }
    // Running 1 primary cell on PR (~80% of monthly volume) and full N cells on main/nightly saves ~80% of excess matrix cells
    let conditional_pr_matrix_monthly_savings_usd =
        ((pr_excess_matrix_monthly_usd * 0.80) * 100.0).round() / 100.0;
    let is_library_or_sdk = hygiene.has_library_release_signals
        || (hygiene.pr_push_runs_multi_os_all_three && !hygiene.has_deploy_or_release_steps)
        || (pr_unconditional_matrix_cells >= 6
            && (hygiene.has_rust_or_cargo
                || hygiene.has_cpp_or_cmake
                || hygiene.has_go_or_gomod
                || hygiene.has_flutter_or_dart));
    let detected_repo_matrix_profile = if jobs.is_empty() {
        "none".to_string()
    } else if matrix_jobs_count == 0 {
        "single-target-application".to_string()
    } else if is_library_or_sdk {
        "multi-target-library-or-sdk".to_string()
    } else if pr_unconditional_matrix_cells >= 4 {
        "application-with-excess-matrix".to_string()
    } else {
        "single-target-application".to_string()
    };

    // 7-Layer Caching Strategy Ordering & Poisoning Guard (/ci-cd-caching-strategies + Primary-Source Turborepo/ECR/S3 Pricing)
    let is_compiled_stack = detected_log_language_profile.contains("Rust")
        || detected_log_language_profile.contains("Java")
        || detected_log_language_profile.contains("iOS")
        || detected_log_language_profile.contains("Android")
        || hygiene.has_cpp_or_cmake
        || hygiene.has_flutter_or_dart;
    let next_recommended_caching_layer = if jobs.is_empty() {
        "none".to_string()
    } else if jobs_missing_cache > 0 {
        "layer-1-dependency-cache".to_string()
    } else if hygiene.has_docker_build_or_push_steps && !hygiene.has_docker_buildkit_cache {
        "layer-2-docker-buildkit-registry-cache".to_string()
    } else if is_compiled_stack && !hygiene.has_remote_compile_cache {
        "layer-4-compiler-artifact-cache-sccache-ccache".to_string()
    } else if hygiene.detected_workspace_units_count >= 3 && !hygiene.has_affected_only_graph_tool {
        "layer-5-distributed-remote-cache".to_string()
    } else if hygiene.has_docker_build_or_push_steps && !hygiene.has_ecr_or_registry_mirror {
        "layer-6-ecr-pull-through-base-image-mirror".to_string()
    } else if !hygiene.has_test_sharding_or_split && total_expanded_matrix_jobs >= 5 {
        "layer-3-test-result-cache".to_string()
    } else if monthly_billed_mins >= 50000.0 && !hygiene.has_ecr_or_registry_mirror {
        "layer-7-package-registry-mirror".to_string()
    } else {
        "all-applicable-layers-active".to_string()
    };
    // Primary-source verified distributed remote cache infra cost:
    // Vercel Turborepo Remote Cache is $0/mo (Free up to 100 GB Hobby / 1 TB Pro with $0 egress);
    // S3-backed sccache/ccache/Gradle remote cache is ~$15/mo; Bazel/dedicated monorepo cluster is ~$125/mo ($50-$200/mo midpoint).
    let distributed_remote_cache_monthly_infra_usd = if jobs.is_empty() {
        0.0
    } else if hygiene.detected_monorepo_tool == "turborepo"
        || (hygiene.uses_pnpm && !is_compiled_stack)
    {
        0.00
    } else if hygiene.detected_monorepo_tool == "bazel"
        || hygiene.detected_monorepo_tool == "nx"
        || hygiene.detected_workspace_units_count >= 10
    {
        125.00
    } else if is_compiled_stack {
        15.00
    } else {
        0.00
    };
    let container_base_image_mirror_monthly_savings_usd =
        if hygiene.has_docker_build_or_push_steps && !hygiene.has_ecr_or_registry_mirror {
            // 15% reduction on Docker phase (~18% of compute) + same-region $0/GB ECR pull-through transfer savings
            let docker_compute_sav = gross_monthly_compute_cost_usd * 0.18 * 0.15;
            let egress_sav = if true_self_hosted_jobs_per_run > 0 {
                (monthly_infracost_private_subnet_nat_worst_case_usd
                    - monthly_infracost_vpc_endpoint_cache_optimized_usd)
                    .max(0.0)
                    * 0.25
            } else {
                0.0
            };
            ((docker_compute_sav + egress_sav) * 100.0).round() / 100.0
        } else {
            0.0
        };
    let pr_cache_poisoning_risk_jobs = if hygiene.has_pr_cache_write_guard {
        0
    } else {
        jobs_with_cache
    };

    // 10. Flaky Test CI/CD Tax (Immediate + 2x Conservative Deferred Velocity Drag & Deflake ROI)
    let deferred_velocity_drag_multiplier = 2.0_f64;
    let monthly_deferred_velocity_drag_usd =
        ((monthly_total_immediate_flake_cost_usd * deferred_velocity_drag_multiplier) * 100.0)
            .round()
            / 100.0;
    let monthly_combined_flake_tax_usd =
        ((monthly_total_immediate_flake_cost_usd + monthly_deferred_velocity_drag_usd) * 100.0)
            .round()
            / 100.0;
    let deflake_project_est_cost_usd = if jobs.is_empty() { 0.0 } else { 4000.0 };
    let deflake_payback_weeks = if monthly_savings_at_1pct_target_usd > 0.0 {
        (((deflake_project_est_cost_usd / (monthly_savings_at_1pct_target_usd / 4.333)) * 100.0)
            .round()
            / 100.0)
            .min(52.0)
    } else {
        0.0
    };

    // 11. Branch Protection Cost (Required-Check Multiplication, Concurrency Cancel & Path Filters)
    let required_checks_count = total_expanded_matrix_jobs;
    let monthly_required_check_executions = jobs
        .iter()
        .map(|j| (j.matrix_combination_count as f64) * j.effective_monthly_runs(monthly_runs))
        .sum::<f64>()
        .round() as usize;
    let slow_blocking_checks_over_10m = jobs
        .iter()
        .filter(|j| j.est_raw_duration_mins_per_job > 10.0)
        .map(|j| j.matrix_combination_count)
        .sum();
    let mut uncancelled_monthly_usd = 0.0_f64;
    let mut unfiltered_monthly_usd = 0.0_f64;
    for j in &jobs {
        let jr = j.effective_monthly_runs(monthly_runs);
        if hygiene
            .uncancelled_workflow_files
            .iter()
            .any(|wf| wf == &j.workflow_file)
        {
            uncancelled_monthly_usd += j.run_compute_cost_usd * jr;
        }
        if hygiene
            .unfiltered_workflow_files
            .iter()
            .any(|wf| wf == &j.workflow_file)
        {
            unfiltered_monthly_usd += j.run_compute_cost_usd * jr;
        }
    }
    // Cancelling superseded runs saves 10-25% (15% midpoint) on workflows without cancel-in-progress,
    // or uses real `observed_cancelled_wasted_mins` when live GitHub Actions telemetry is calibrated!
    let monthly_superseded_run_waste_usd = if let Some(wasted_mins) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_cancelled_wasted_mins)
        .filter(|&m| m > 0.0)
    {
        let win_days = active_telemetry
            .as_ref()
            .map(|t| t.window_days.max(1) as f64)
            .unwrap_or(30.0);
        let eff_rate = if billed_mins_per_run > 0.0 {
            gross_compute_cost_per_run_usd / billed_mins_per_run
        } else {
            0.006
        };
        let monthly_wasted = wasted_mins * (30.0 / win_days) * eff_rate;
        ((monthly_wasted) * 100.0).round() / 100.0
    } else {
        ((uncancelled_monthly_usd * 0.15) * 100.0).round() / 100.0
    };
    // Path filtering saves 30-50% (40% midpoint) on multi-workflow PR checks missing path filters
    let monthly_unfiltered_check_savings_usd =
        if workflow_count > 1 || total_expanded_matrix_jobs > 3 {
            ((unfiltered_monthly_usd * 0.40) * 100.0).round() / 100.0
        } else {
            0.0
        };

    // 12. Startup Stage Cost (1-15 Devs, Free-Tier Burn, Upgrade Triggers & Early Traps)
    let free_tier_burn_pct = free_tier_quota_burn_pct;
    let upgrade_trigger_80pct_reached = free_tier_burn_pct >= 80.0;
    // Implied active contributors derived dynamically from monthly build minutes (~1,500-2,000 mins/dev/mo) & run frequency (~5 pushes/dev/day)
    let runs_per_day = monthly_runs / 21.65;
    let devs_from_runs = (runs_per_day / 5.0).ceil().max(1.0) as usize;
    let devs_from_mins = (monthly_billed_mins / 1600.0).ceil().max(1.0) as usize;
    let implied_active_devs = if jobs.is_empty() {
        0
    } else if let Some(obs_devs) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_active_devs_30d)
        .filter(|&d| d > 0)
    {
        obs_devs
    } else {
        devs_from_runs.max(devs_from_mins)
    };
    let inferred_team_stage = if jobs.is_empty() {
        "none (0 workflows)".to_string()
    } else if monthly_billed_mins <= 3000.0 && gross_monthly_compute_cost_usd <= 50.0 {
        "startup-pre-seed (1-5 devs)".to_string()
    } else if monthly_billed_mins <= 25000.0 && gross_monthly_compute_cost_usd <= 500.0 {
        "startup-seed-series-a (6-15 devs)".to_string()
    } else if gross_monthly_compute_cost_usd <= 12000.0 && implied_active_devs < 200 {
        "scaleup (25-100 devs)".to_string()
    } else {
        "enterprise (250+ devs)".to_string()
    };
    let premature_self_hosting_trap = true_self_hosted_jobs_per_run > 0
        && monthly_billed_mins > 0.0
        && monthly_billed_mins < 25000.0
        && gross_monthly_compute_cost_usd < 500.0
        && macos_mins_per_run == 0.0;
    let premature_multi_os_pr_matrix_trap = hygiene.pr_push_runs_multi_os_all_three
        && linux_mins_per_run > 0.0
        && windows_mins_per_run > 0.0
        && macos_mins_per_run > 0.0;
    let mut pr_non_linux_monthly_usd = 0.0_f64;
    if premature_multi_os_pr_matrix_trap {
        for j in &jobs {
            let os_low = j.os.to_ascii_lowercase();
            let jr = j.effective_monthly_runs(monthly_runs);
            if (os_low.contains("win") || os_low.contains("mac"))
                && !j.provider.starts_with("self-hosted")
                && !j.runner_label.to_ascii_lowercase().contains("self-hosted")
                && jr > 30.0
            {
                pr_non_linux_monthly_usd += j.run_compute_cost_usd * (jr - 30.0);
            }
        }
    }
    let monthly_multi_os_pr_to_nightly_savings_usd =
        (pr_non_linux_monthly_usd * 100.0).round() / 100.0;
    let mut monthly_macos_hosted_usd = 0.0_f64;
    for j in &jobs {
        if j.os.eq_ignore_ascii_case("macos")
            && !j.provider.starts_with("self-hosted")
            && !j.runner_label.to_ascii_lowercase().contains("self-hosted")
        {
            monthly_macos_hosted_usd +=
                j.run_compute_cost_usd * j.effective_monthly_runs(monthly_runs);
        }
    }
    // $700 Mac mini + $5/mo power replaces ~90% of hosted macOS CI spend
    let mac_mini_net_monthly_saving = (monthly_macos_hosted_usd * 0.90) - 5.0;
    let mac_mini_hardware_payback_months = if mac_mini_net_monthly_saving > 10.0 {
        ((700.0 / mac_mini_net_monthly_saving) * 100.0).round() / 100.0
    } else {
        0.0
    };

    // 13. Scaleup Stage Cost (25-100 Devs, Tier-Crisis Decisions, Required-Check Audit & Platform Eng ROI)
    let self_hosted_breakeven_reached =
        gross_monthly_compute_cost_usd >= 500.0 || monthly_billed_mins >= 25000.0;
    let scaleup_seat_count = implied_active_devs.max(25) as f64;
    // Team ($4/user) -> Enterprise ($21/user) step = +$17/user/month
    let team_to_enterprise_seat_jump_monthly_usd =
        ((scaleup_seat_count * 17.0) * 100.0).round() / 100.0;
    // Quarterly required-check audit removes 30-40% (35% midpoint) of accumulated low-signal checks when >6 checks exist
    let quarterly_required_check_audit_savings_usd = if required_checks_count > 6 {
        let audit_factor = if hygiene.workflows_with_path_filters > hygiene.workflows_missing_path_filters {
            0.20
        } else {
            0.35
        };
        ((gross_monthly_compute_cost_usd * audit_factor) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let platform_engineer_monthly_fully_loaded_usd = 16500.0_f64;
    let recoverable_ci_compute_usd = if gross_monthly_compute_cost_usd > 0.0 {
        let r_cache =
            (monthly_unrealized_cache_savings_usd / gross_monthly_compute_cost_usd).clamp(0.0, 0.70);
        let r_cancel =
            (monthly_superseded_run_waste_usd / gross_monthly_compute_cost_usd).clamp(0.0, 0.50);
        let r_filter =
            (monthly_unfiltered_check_savings_usd / gross_monthly_compute_cost_usd).clamp(0.0, 0.60);
        let r_flake = (compute_sav_at_1pct / gross_monthly_compute_cost_usd).clamp(0.0, 0.40);
        let remaining_frac = (1.0 - r_cache) * (1.0 - r_cancel) * (1.0 - r_filter) * (1.0 - r_flake);
        let compound_compute_sav =
            (gross_monthly_compute_cost_usd * (1.0 - remaining_frac)).min(gross_monthly_compute_cost_usd * 0.85);
        compound_compute_sav + payroll_sav_at_1pct
    } else {
        0.0
    };
    let recoverable_ci_and_wait_monthly_usd = ((recoverable_ci_compute_usd
        + (monthly_productive_payroll_loss_usd * 0.30))
        * 100.0)
        .round()
        / 100.0;
    let platform_engineer_roi_coverage_pct = if jobs.is_empty() {
        0.0
    } else {
        (((recoverable_ci_and_wait_monthly_usd / platform_engineer_monthly_fully_loaded_usd) * 100.0)
            * 100.0)
            .round()
            / 100.0
    };

    // 14. Enterprise Stage Cost (250+ Devs, Compliance Overhead, Regional Residency & Dedicated Tenancy)
    let compliance_controls_count = hygiene.detected_compliance_controls.len();
    let region_count = hygiene.detected_cloud_regions.len();
    let regional_residency_multiplier = if region_count >= 3 {
        2.75
    } else if region_count == 2 {
        1.80
    } else {
        1.00
    };
    let enterprise_250_seat_licence_monthly_usd = 5250.0_f64; // 250 seats * $21/user
    // 1-year SIEM audit log retention (SOC 2 / ISO 27001): strictly gated to repos with compliance controls or external log shippers
    let siem_1yr_audit_log_retention_monthly_usd =
        if compliance_controls_count > 0 || hygiene.has_external_log_shipper {
            ((monthly_log_volume_gb * (0.10 + 12.0 * 0.03) * regional_residency_multiplier) * 100.0)
                .round()
                / 100.0
        } else {
            0.0
        };
    let dedicated_tenancy_premium_pct_min = 40.0_f64;
    let dedicated_tenancy_premium_pct_max = 100.0_f64;

    // 15. Monorepo CI Cost (Dynamic Workspace Detection, Unconstrained Blast Radius & Affected-Only Fix)
    let detected_workspace_units_count = hygiene.detected_workspace_units_count;
    let detected_pipeline_shape = if jobs.is_empty() {
        "empty".to_string()
    } else if hygiene.has_affected_only_graph_tool
        || (hygiene.detected_monorepo_tool != "none"
            && hygiene.detected_monorepo_tool != "path-filters-only")
        || detected_workspace_units_count >= 3
    {
        "monorepo".to_string()
    } else if workflow_count >= 4
        && (hygiene.reusable_workflow_calls_count > 0
            || hygiene.workflows_with_path_filters >= 2
            || detected_workspace_units_count >= 2)
    {
        "multi-service-microservices".to_string()
    } else {
        "single-service".to_string()
    };
    let recommended_affected_tool = if detected_workspace_units_count >= 50 {
        "bazel".to_string()
    } else if infracost_iac_files_detected
        && jobs
            .iter()
            .filter(|j| {
                j.workflow_file.contains("terragrunt") || j.workflow_file.contains("terraform")
            })
            .count()
            * 2
            >= jobs.len().max(1)
    {
        "terragrunt-run-all-or-atlantis".to_string()
    } else if detected_log_language_profile.contains("Node")
        || detected_log_language_profile.contains("TypeScript")
        || hygiene.uses_pnpm
    {
        "nx-or-turborepo".to_string()
    } else if detected_log_language_profile.contains("Rust") {
        "cargo-nextest-or-bazel".to_string()
    } else if detected_log_language_profile.contains("Python") {
        "pants-or-nx".to_string()
    } else {
        "dorny-paths-filter-or-bazel".to_string()
    };
    let affected_only_reduction_pct_min = 60.0_f64;
    let affected_only_reduction_pct_max = 85.0_f64;
    let monthly_affected_only_savings_usd = if detected_pipeline_shape == "monorepo"
        && !hygiene.has_affected_only_graph_tool
    {
        let savings_ratio = if hygiene.workflows_with_path_filters > 0 {
            0.35 // Coarse path filters already active; fine-grained dependency graph saves another ~35%
        } else {
            0.725 // 60-85% midpoint reduction on unconstrained monorepo runs
        };
        ((gross_monthly_compute_cost_usd * savings_ratio) * 100.0).round() / 100.0
    } else {
        0.0
    };

    // 16. Microservices CI Cost (Per-Service Setup Overhead, Reusable Templates, Pinning & Shared Cache)
    // Per-pipeline setup overhead (container pull 30-90s + cache restore 30-60s + framework init 15-30s = 60-180s)
    let est_setup_overhead_sec_per_job = if jobs.is_empty() {
        0.0
    } else if jobs_with_cache > jobs_missing_cache {
        75.0
    } else {
        120.0
    };
    let setup_mins_per_run =
        ((est_setup_overhead_sec_per_job / 60.0) * (total_expanded_matrix_jobs as f64))
            .min(raw_duration_mins_per_run * 0.45);
    let monthly_setup_mins: f64 = jobs
        .iter()
        .map(|j| {
            let jr = j.effective_monthly_runs(monthly_runs);
            let job_raw = j.est_raw_duration_mins_per_job * (j.matrix_combination_count as f64);
            let job_setup = ((est_setup_overhead_sec_per_job / 60.0)
                * (j.matrix_combination_count as f64))
                .min(job_raw * 0.45);
            job_setup * jr
        })
        .sum();
    let monthly_setup_overhead_hours = ((monthly_setup_mins / 60.0) * 100.0).round() / 100.0;
    let setup_overhead_share_pct = if raw_duration_mins_per_run > 0.0 {
        (((setup_mins_per_run / raw_duration_mins_per_run) * 100.0) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let monthly_setup_overhead_compute_usd =
        ((gross_monthly_compute_cost_usd * (setup_overhead_share_pct / 100.0)) * 100.0).round()
            / 100.0;
    // Shared base images + pre-warmed runners + shared cache backend cut 25-40% setup overhead down to 10-15% (~60% reduction of overhead)
    let monthly_shared_infra_setup_savings_usd =
        ((monthly_setup_overhead_compute_usd * 0.60) * 100.0).round() / 100.0;
    let effective_service_units = detected_workspace_units_count
        .max(workflow_count)
        .max(if jobs.is_empty() { 0 } else { 1 });
    let est_aggregate_cache_gb = if let Some(obs_cache) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_cache_active_gb)
    {
        (obs_cache * 100.0).round() / 100.0
    } else {
        (((effective_service_units as f64) * 0.50) * 100.0).round() / 100.0
    };
    let exceeds_10gb_repo_cache_limit = est_aggregate_cache_gb > 10.0;

    // 17. Multi-Vendor TCO & Migration Comparison Engine (Seats + Included Quota + vCPU-Scaled Overage + 5-Layer Migration Payback, `C1, G6`)
    let seats_f = implied_active_devs as f64;
    let mut monthly_win_mins = 0.0_f64;
    let mut monthly_mac_mins = 0.0_f64;
    let mut larger_runners_detected_count = 0_usize;
    let mut larger_runner_monthly_usd = 0.0_f64;
    let mut larger_x86_linux_non_gpu_monthly_usd = 0.0_f64;
    let mut monthly_std_linux_mins = 0.0_f64;
    let mut monthly_vcpu_scaled_linux_mins = 0.0_f64;
    let mut monthly_std_win_mins = 0.0_f64;
    let mut monthly_std_mac_mins = 0.0_f64;
    let mut monthly_non_arm_linux_2core_mins = 0.0_f64;
    let mut monthly_cci_linux_credits = 0.0_f64;
    let mut hosted_linux_mins_per_run = 0.0_f64;

    for j in &jobs {
        let jr = j.effective_monthly_runs(monthly_runs);
        let rl = j.runner_label.to_ascii_lowercase();
        let os_low = j.os.to_ascii_lowercase();
        if os_low.contains("mac") {
            monthly_mac_mins += j.total_billed_mins_all_matrix * jr;
        } else if os_low.contains("win") {
            monthly_win_mins += j.total_billed_mins_all_matrix * jr;
        }
        let is_self_or_3p = j.provider.starts_with("self-hosted")
            || j.provider == "blacksmith"
            || j.provider == "third-party-microvm"
            || rl.contains("blacksmith")
            || rl.contains("depot")
            || rl.contains("warpbuild")
            || rl.contains("buildjet")
            || rl.contains("namespace")
            || rl.contains("self-hosted");
        let is_gh_larger = is_github_larger_runner_job(j);

        if j.os.eq_ignore_ascii_case("linux") && !is_self_or_3p {
            hosted_linux_mins_per_run += j.total_billed_mins_all_matrix;
        }

        if is_gh_larger {
            larger_runners_detected_count += j.matrix_combination_count;
            larger_runner_monthly_usd += j.run_compute_cost_usd * jr;
            if j.os.eq_ignore_ascii_case("linux")
                && !rl.contains("arm")
                && !rl.contains("aarch64")
                && !rl.contains("gpu")
                && resolve_runner_sku(&j.runner_label).arch != "arm64"
            {
                larger_x86_linux_non_gpu_monthly_usd += j.run_compute_cost_usd * jr;
            }
        } else if j.os.eq_ignore_ascii_case("linux") && !is_self_or_3p {
            monthly_std_linux_mins += j.total_billed_mins_all_matrix * jr;
            if j.vcpus <= 2
                && !rl.contains("arm")
                && !rl.contains("aarch64")
                && resolve_runner_sku(&j.runner_label).arch != "arm64"
            {
                monthly_non_arm_linux_2core_mins += j.total_billed_mins_all_matrix * jr;
            }
        } else if j.os.eq_ignore_ascii_case("windows") && !is_self_or_3p {
            monthly_std_win_mins += j.total_billed_mins_all_matrix * jr;
        } else if j.os.eq_ignore_ascii_case("macos") && !is_self_or_3p {
            monthly_std_mac_mins += j.total_billed_mins_all_matrix * jr;
        }

        if j.os.eq_ignore_ascii_case("linux") {
            let vcpu_scale = ((j.vcpus.max(2) as f64) / 2.0).max(1.0);
            monthly_vcpu_scaled_linux_mins += j.total_billed_mins_all_matrix * vcpu_scale * jr;
            monthly_cci_linux_credits += j.total_billed_mins_all_matrix * (10.0 * vcpu_scale) * jr;
        }
    }
    let larger_runners_bypass_free_minutes_tax_usd =
        (larger_runner_monthly_usd * 100.0).round() / 100.0;

    let (
        cheapest_vendor_platform,
        cheapest_vendor_monthly_tco_usd,
        github_actions_team_monthly_tco_usd,
        gitlab_ci_premium_monthly_tco_usd,
        circleci_performance_monthly_tco_usd,
        bitbucket_premium_monthly_tco_usd,
        buildkite_pro_monthly_tco_usd,
        jenkins_self_hosted_monthly_tco_usd,
        aws_codebuild_monthly_tco_usd,
        gcp_cloud_build_monthly_tco_usd,
        azure_pipelines_monthly_tco_usd,
        migration_estimated_hours,
        migration_estimated_cost_usd,
        migration_payback_months,
        migration_worth_it_2x_annual_rule,
        migration_verdict,
    ) = if jobs.is_empty() {
        (
            "none".to_string(),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            false,
            "STAY".to_string(),
        )
    } else {
        // GitHub Actions Team: $4/seat + 3,000 included pool mins (1x Linux, 2x Win, 10x Mac) + Larger Runners (never draw free mins)
        let gh_std_pool_mins =
            monthly_std_linux_mins + monthly_std_win_mins * 2.0 + monthly_std_mac_mins * 10.0;
        let gh_std_gross = monthly_std_linux_mins * 0.006
            + monthly_std_win_mins * 0.010
            + monthly_std_mac_mins * 0.062;
        let gh_std_overage = if gh_std_pool_mins <= 3000.0 {
            0.0
        } else {
            gh_std_gross * ((gh_std_pool_mins - 3000.0) / gh_std_pool_mins)
        };
        let gh_tco = ((seats_f * 4.0 + gh_std_overage + larger_runner_monthly_usd) * 100.0).round()
            / 100.0;
        // GitLab CI Premium: $29/seat + 10,000 namespace compute mins (vCPU-scaled Linux, 1x Win, 6x macOS M1) @ $0.010/compute-min overage
        let gl_compute_mins =
            monthly_vcpu_scaled_linux_mins + monthly_win_mins + monthly_mac_mins * 6.0;
        let gl_tco = ((seats_f * 29.0 + (gl_compute_mins - 10000.0).max(0.0) * 0.010) * 100.0)
            .round()
            / 100.0;
        // CircleCI Performance: $15 base (5 users + 30k credits) + $15 per extra user + credits @ $0.0006 (vCPU-scaled Linux, 40/m Win, 150/m Mac M2 Pro)
        let cci_credits = monthly_cci_linux_credits
            + monthly_win_mins * 40.0
            + monthly_mac_mins * 150.0;
        let cci_tco = ((15.0
            + (seats_f - 5.0).max(0.0) * 15.0
            + (cci_credits - 30000.0).max(0.0) * 0.0006)
            * 100.0)
            .round()
            / 100.0;
        // Bitbucket Pipelines Premium: $6.60/seat + 3,500 workspace mins ($0.010/min overage, vCPU-scaled 1x/2x/4x/8x)
        let bb_mac_addon = if monthly_mac_mins > 0.0 {
            ((monthly_mac_mins / 8500.0).ceil().max(1.0)) * 28.0
        } else {
            0.0
        };
        let bb_tco = ((seats_f * 6.60
            + (monthly_vcpu_scaled_linux_mins + monthly_win_mins - 3500.0).max(0.0) * 0.010
            + bb_mac_addon)
            * 100.0)
            .round()
            / 100.0;
        // Buildkite Pro + BYO Agents: Free for <=5 users & <=10 concurrent jobs; Pro $30/seat + ~$30/agent
        let bk_agents = (((monthly_vcpu_scaled_linux_mins + monthly_win_mins + monthly_mac_mins)
            / 8500.0)
            .ceil()
            .max(1.0))
        .min(peak_concurrent_jobs_per_run.max(1) as f64)
        .min(16.0);
        let bk_seats_usd = if implied_active_devs <= 5 && peak_concurrent_jobs_per_run <= 10 {
            0.0
        } else {
            seats_f * 30.0
        };
        let bk_tco = ((bk_seats_usd + bk_agents * 30.0) * 100.0).round() / 100.0;
        // Jenkins Self-Hosted TCO: $0 software + $70 controller + agents * $30 + operator admin allowance
        let jenkins_admin_usd = if implied_active_devs <= 10 {
            450.0
        } else {
            800.0
        };
        let jenkins_tco =
            ((70.0 + bk_agents * 30.0 + jenkins_admin_usd) * 100.0).round() / 100.0;
        // AWS CodeBuild: $0 seats + 100 free general1.small mins + vCPU-scaled Linux ($0.005/2vCPU-min) + $0.016/min Win + $0.065/min Mac
        let cb_tco = (((monthly_vcpu_scaled_linux_mins - 100.0).max(0.0) * 0.005
            + monthly_win_mins * 0.016
            + monthly_mac_mins * 0.065)
            * 100.0)
            .round()
            / 100.0;
        // GCP Cloud Build: $0 seats + 2,500 free e2-medium build-mins ($0.006/min overage, `G6`)
        let gcp_tco = (((monthly_vcpu_scaled_linux_mins - 2500.0).max(0.0) * 0.006
            + monthly_win_mins * 0.010
            + monthly_mac_mins * 0.062)
            * 100.0)
            .round()
            / 100.0;
        // Azure Pipelines: 1 free hosted job (<=1,800 mins & 1 lane = $0); buying >=1 paid $40/mo parallel job removes the 1,800m cap on ALL lanes (`G6`)
        let az_paid_lanes = ((monthly_billed_mins / 7500.0).ceil() as usize)
            .max(peak_concurrent_jobs_per_run.min(4))
            .max(1);
        let az_tco = if monthly_billed_mins <= 1800.0 && peak_concurrent_jobs_per_run <= 1 {
            0.0
        } else {
            ((az_paid_lanes as f64) * 40.0 * 100.0).round() / 100.0
        };

        let candidates = [
            ("github-actions-team", gh_tco),
            ("gitlab-ci-premium", gl_tco),
            ("circleci-performance", cci_tco),
            ("bitbucket-premium", if monthly_mac_mins > 0.0 { f64::MAX } else { bb_tco }),
            ("buildkite-pro-byo", bk_tco),
            ("aws-codebuild", cb_tco),
            ("gcp-cloud-build", gcp_tco),
            ("azure-pipelines", az_tco),
        ];
        let mut best_vendor = "github-actions-team";
        let mut best_tco = gh_tco;
        for (name, val) in candidates {
            if val < best_tco {
                best_tco = val;
                best_vendor = name;
            }
        }
        let current_vendor_slug = if hygiene.has_jenkinsfile {
            "jenkins-self-hosted"
        } else {
            match primary_provider {
                "gitlab-ci" => "gitlab-ci-premium",
                "circleci" => "circleci-performance",
                "bitbucket" => "bitbucket-premium",
                "aws-codebuild" => "aws-codebuild",
                "gcp-cloud-build" => "gcp-cloud-build",
                "azure-pipelines" => "azure-pipelines",
                _ => "github-actions-team",
            }
        };
        let current_tco = match current_vendor_slug {
            "jenkins-self-hosted" => jenkins_tco,
            "gitlab-ci-premium" => gl_tco,
            "circleci-performance" => cci_tco,
            "bitbucket-premium" => bb_tco,
            "aws-codebuild" => cb_tco,
            "gcp-cloud-build" => gcp_tco,
            "azure-pipelines" => az_tco,
            _ => gh_tco,
        };
        // Fixes Bug #27.1 & #28.1: pass `usage.dev_hourly_rate_usd` consistently and `!hygiene.has_jenkinsfile` for AST autofix!
        let switching = compute_5layer_platform_switching_tco(
            workflow_count,
            implied_active_devs,
            current_vendor_slug,
            best_vendor,
            current_tco,
            best_tco,
            usage.dev_hourly_rate_usd,
            !hygiene.has_jenkinsfile,
        );

        (
            best_vendor.to_string(),
            best_tco,
            gh_tco,
            gl_tco,
            cci_tco,
            bb_tco,
            bk_tco,
            jenkins_tco,
            cb_tco,
            gcp_tco,
            az_tco,
            switching.engineering_hours,
            switching.total_switching_cost_usd,
            switching.payback_months,
            switching.meets_2x_annual_rule,
            switching.verdict.to_string(),
        )
    };

    // 18. 6-Phase Build-Minute Time-Share & Best-Payback Optimization Ranker
    let (
        phase_clone_checkout_share_pct,
        phase_dependency_install_share_pct,
        phase_compile_typecheck_share_pct,
        phase_test_suite_share_pct,
        phase_docker_build_push_share_pct,
        phase_deploy_cleanup_share_pct,
        monthly_total_reducible_phase_savings_usd,
        best_first_optimization_phase,
        best_first_optimization_effort_hours,
        best_first_optimization_monthly_savings_usd,
        best_first_optimization_payback_months,
    ) = if jobs.is_empty() {
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, "none".to_string(), 0.0, 0.0, 0.0)
    } else {
        let is_compiled_heavy = detected_log_language_profile.contains("Rust")
            || detected_log_language_profile.contains("Java")
            || detected_log_language_profile.contains("iOS")
            || detected_log_language_profile.contains("Android")
            || detected_log_language_profile.contains(".NET")
            || hygiene.has_cpp_or_cmake
            || hygiene.has_flutter_or_dart;
        let (p_clone, p_dep, p_comp, p_test, p_dock, p_dep_clean): (
            f64,
            f64,
            f64,
            f64,
            f64,
            f64,
        ) = if let Some([rc, rd, rco, rt, rdo, rdc]) = active_telemetry
            .as_ref()
            .and_then(|t| t.real_phase_shares)
        {
            (rc, rd, rco, rt, rdo, rdc)
        } else {
            let (pc, pd, pco, pt, pdo): (f64, f64, f64, f64, f64) =
                if hygiene.has_docker_build_or_push_steps && is_compiled_heavy {
                    (5.0, 18.0, 25.0, 28.0, 17.0)
                } else if hygiene.has_docker_build_or_push_steps {
                    (5.0, 20.0, 18.0, 32.0, 18.0)
                } else if is_compiled_heavy {
                    (5.0, 22.0, 30.0, 36.0, 0.0)
                } else if hygiene.has_deploy_or_release_steps {
                    (6.0, 25.0, 17.0, 40.0, 0.0)
                } else {
                    (6.0, 26.0, 18.0, 42.0, 0.0)
                };
            let pdc = ((100.0 - (pc + pd + pco + pt + pdo)) * 10.0).round() / 10.0;
            (pc, pd, pco, pt, pdo, pdc)
        };

        let shallow_jobs: usize = jobs
            .iter()
            .filter(|j| j.has_sparse_or_shallow_clone)
            .map(|j| j.matrix_combination_count)
            .sum();
        let shallow_ratio =
            (shallow_jobs as f64) / (total_expanded_matrix_jobs.max(1) as f64);
        let red_clone = (0.60 - 0.45 * shallow_ratio).clamp(0.15, 0.60);
        // Weight dependency-install reducibility by uncached compute `$` share (Fixes Bug #28.2)
        let uncached_ratio = if gross_monthly_compute_cost_usd > 0.0 {
            (monthly_uncached_compute_usd / gross_monthly_compute_cost_usd).clamp(0.0, 1.0)
        } else {
            let cache_eligible = (jobs_with_cache + jobs_missing_cache).max(1) as f64;
            (jobs_missing_cache as f64 / cache_eligible).clamp(0.0, 1.0)
        };
        let red_dep = (0.15 + 0.55 * uncached_ratio).clamp(0.15, 0.70);
        let red_comp = if hygiene.has_remote_compile_cache {
            0.15
        } else {
            0.50
        };
        let red_test = if hygiene.has_test_sharding_or_split && hygiene.has_affected_only_graph_tool
        {
            0.20
        } else {
            0.55
        };
        let red_dock = if p_dock == 0.0 {
            0.0
        } else if hygiene.has_docker_buildkit_cache {
            0.20
        } else {
            0.65
        };
        let red_clean = if configured_retention_days <= 14 {
            0.15
        } else {
            0.40
        };

        let phases = [
            ("clone_checkout", p_clone, red_clone, 4.0_f64),
            ("dependency_install", p_dep, red_dep, 6.0_f64),
            ("compile_typecheck", p_comp, red_comp, 24.0_f64),
            ("test_suite", p_test, red_test, 32.0_f64),
            ("docker_build_push", p_dock, red_dock, 16.0_f64),
            ("deploy_cleanup", p_dep_clean, red_clean, 6.0_f64),
        ];
        let mut total_reducible_usd = 0.0_f64;
        let mut best_phase = "dependency_install";
        let mut best_effort = 6.0_f64;
        let mut best_saving = 0.0_f64;
        let mut best_payback = f64::MAX;

        for (p_name, share_pct, red_frac, effort_h) in phases {
            let phase_usd = gross_monthly_compute_cost_usd * (share_pct / 100.0);
            let sav_usd = phase_usd * red_frac;
            total_reducible_usd += sav_usd;
            if sav_usd > 0.0 {
                let payback = (effort_h * usage.dev_hourly_rate_usd) / sav_usd;
                if payback < best_payback {
                    best_payback = payback;
                    best_phase = p_name;
                    best_effort = effort_h;
                    best_saving = sav_usd;
                }
            }
        }
        let final_payback = if best_payback < f64::MAX {
            ((best_payback * 100.0).round() / 100.0).min(120.0)
        } else {
            0.0
        };
        (
            p_clone,
            p_dep,
            p_comp,
            p_test,
            p_dock,
            p_dep_clean,
            (total_reducible_usd * 100.0).round() / 100.0,
            best_phase.to_string(),
            best_effort,
            (best_saving * 100.0).round() / 100.0,
            final_payback,
        )
    };

    // 19. Compute-Class Right-Sizing Explorer (Amdahl Speedup vs Larger-Runner Billing & Dev-Wait Net ROI)
    let (
        detected_workload_compute_profile,
        arm64_2core_monthly_compute_usd,
        arm64_migration_monthly_savings_usd,
        standard_2core_monthly_compute_usd,
        larger_4core_monthly_compute_usd,
        larger_8core_monthly_compute_usd,
        larger_16core_monthly_compute_usd,
        best_net_roi_runner_tier,
        best_net_roi_monthly_delta_usd,
    ) = if jobs.is_empty() {
        (
            "none".to_string(),
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            "standard-2-core".to_string(),
            0.0,
        )
    } else {
        // Realized Amdahl speedups across (4-core, 8-core, 16-core, 32-core, 64-core) relative to 2-core baseline
        let (profile_name, s4, s8, s16, s32, s64) = if detected_log_language_profile.contains("Rust")
            || detected_log_language_profile.contains("Java")
            || detected_log_language_profile.contains("Go")
            || detected_log_language_profile.contains("iOS")
            || detected_log_language_profile.contains("Android")
            || hygiene.has_cpp_or_cmake
            || hygiene.has_flutter_or_dart
        {
            ("compile-heavy", 1.65_f64, 2.40_f64, 3.20_f64, 3.85_f64, 4.30_f64)
        } else if hygiene.has_browser_or_e2e_tests || infracost_iac_files_detected {
            ("io-bound", 1.18_f64, 1.30_f64, 1.38_f64, 1.41_f64, 1.43_f64)
        } else if total_expanded_matrix_jobs >= 8 || hygiene.has_test_sharding_or_split {
            ("test-heavy", 1.55_f64, 2.15_f64, 2.80_f64, 3.30_f64, 3.65_f64)
        } else {
            ("balanced-web-backend", 1.33_f64, 1.56_f64, 1.75_f64, 1.84_f64, 1.89_f64)
        };
        // Isolate Linux CPU-scalable hosted jobs from fixed non-Linux (macOS / Windows / GPU / 3P) jobs,
        // respecting GitHub's 1-minute per-job billing ceiling on Larger Runners (`G7`)!
        let mut linux_2c_mo = 0.0_f64;
        let mut fixed_non_linux_mo = 0.0_f64;
        let mut l4_linux_mo = 0.0_f64;
        let mut l8_linux_mo = 0.0_f64;
        let mut l16_linux_mo = 0.0_f64;
        let mut l32_linux_mo = 0.0_f64;
        let mut l64_linux_mo = 0.0_f64;
        for j in &jobs {
            let jr = j.effective_monthly_runs(monthly_runs);
            let mat_f = j.matrix_combination_count as f64;
            let rl = j.runner_label.to_ascii_lowercase();
            let is_gpu = rl.contains("gpu");
            let is_self_or_3p = j.provider.starts_with("self-hosted")
                || j.provider == "blacksmith"
                || j.provider == "third-party-microvm"
                || rl.contains("blacksmith")
                || rl.contains("depot")
                || rl.contains("warpbuild")
                || rl.contains("buildjet")
                || rl.contains("namespace")
                || rl.contains("self-hosted");
            if j.os.eq_ignore_ascii_case("linux") && !is_gpu && !is_self_or_3p {
                let raw_2c_equiv = if j.vcpus > 2 && j.rate_per_min_usd > 0.006 {
                    let job_speedup = match j.vcpus {
                        4 => s4,
                        8 => s8,
                        16 => s16,
                        32 => s32,
                        64..=u32::MAX => s64,
                        _ => 1.0,
                    };
                    j.est_raw_duration_mins_per_job * job_speedup
                } else {
                    j.est_raw_duration_mins_per_job
                };
                let ceil_m = |d: f64| -> f64 { (d - 1e-9).ceil().max(1.0) };
                linux_2c_mo += ceil_m(raw_2c_equiv) * mat_f * 0.006 * jr;
                l4_linux_mo += ceil_m(raw_2c_equiv / s4) * mat_f * 0.012 * jr;
                l8_linux_mo += ceil_m(raw_2c_equiv / s8) * mat_f * 0.022 * jr;
                l16_linux_mo += ceil_m(raw_2c_equiv / s16) * mat_f * 0.042 * jr;
                l32_linux_mo += ceil_m(raw_2c_equiv / s32) * mat_f * 0.082 * jr;
                l64_linux_mo += ceil_m(raw_2c_equiv / s64) * mat_f * 0.162 * jr;
            } else {
                fixed_non_linux_mo += j.run_compute_cost_usd * jr;
            }
        }
        let std_2c_mo = ((linux_2c_mo + fixed_non_linux_mo) * 100.0).round() / 100.0;
        // Linux arm64 2-core ($0.005/min) vs standard x64 2-core ($0.006/min) saves 16.7%, and x86 4-core+ Linux -> arm64 saves ~36%
        let arm_sav = ((monthly_non_arm_linux_2core_mins * (0.006 - 0.005)
            + larger_x86_linux_non_gpu_monthly_usd * 0.36)
            * 100.0)
            .round()
            / 100.0;
        let arm_2c_mo = ((gross_monthly_compute_cost_usd - arm_sav).max(0.0) * 100.0).round() / 100.0;

        let l4_mo = ((fixed_non_linux_mo + l4_linux_mo) * 100.0).round() / 100.0;
        let l8_mo = ((fixed_non_linux_mo + l8_linux_mo) * 100.0).round() / 100.0;
        let l16_mo = ((fixed_non_linux_mo + l16_linux_mo) * 100.0).round() / 100.0;
        let l32_mo = ((fixed_non_linux_mo + l32_linux_mo) * 100.0).round() / 100.0;
        let l64_mo = ((fixed_non_linux_mo + l64_linux_mo) * 100.0).round() / 100.0;

        // Larger runners never draw free minutes: if within Free tier, standard 2-core costs $0 out-of-pocket
        let baseline_billed_mo = if effective_tier.starts_with("free") {
            larger_runners_bypass_free_minutes_tax_usd
        } else {
            std_2c_mo
        };
        let linux_wait_share = if billed_mins_per_run > 0.0 {
            (hosted_linux_mins_per_run / billed_mins_per_run).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let wait_loss_base = monthly_productive_payroll_loss_usd * linux_wait_share;
        let net_4c = (l4_mo - baseline_billed_mo) - wait_loss_base * (1.0 - (1.0 / s4));
        let net_8c = (l8_mo - baseline_billed_mo) - wait_loss_base * (1.0 - (1.0 / s8));
        let net_16c = (l16_mo - baseline_billed_mo) - wait_loss_base * (1.0 - (1.0 / s16));
        let net_32c = (l32_mo - baseline_billed_mo) - wait_loss_base * (1.0 - (1.0 / s32));
        let net_64c = (l64_mo - baseline_billed_mo) - wait_loss_base * (1.0 - (1.0 / s64));

        let obs_cpu = active_telemetry
            .as_ref()
            .and_then(|t| t.median_cpu_util_pct);
        let obs_ram = active_telemetry
            .as_ref()
            .and_then(|t| t.median_ram_util_pct);

        let mut best_tier = "standard-2-core";
        let mut best_delta = 0.0_f64;
        for (t_name, delta) in [
            ("larger-4-core", net_4c),
            ("larger-8-core", net_8c),
            ("larger-16-core", net_16c),
            ("larger-32-core", net_32c),
            ("larger-64-core", net_64c),
        ] {
            if delta < best_delta {
                best_delta = delta;
                best_tier = t_name;
            }
        }
        // Telemetry CPU/RAM right-sizing guardrails (minici/telemetry-step Tier 5):
        if let Some(ram_pct) = obs_ram {
            if ram_pct >= 85.0 && best_tier == "standard-2-core" && max_vcpus <= 2 {
                best_tier = "larger-4-core";
                best_delta = net_4c;
            }
        }
        if let (Some(cpu_pct), Some(ram_pct)) = (obs_cpu, obs_ram) {
            if cpu_pct < 25.0 && ram_pct < 60.0 {
                best_tier = "standard-2-core";
                best_delta = 0.0;
            }
        }
        (
            profile_name.to_string(),
            arm_2c_mo,
            arm_sav,
            std_2c_mo,
            l4_mo,
            l8_mo,
            l16_mo,
            best_tier.to_string(),
            (best_delta * 100.0).round() / 100.0,
        )
    };

    // 20. Language-Stack Deep-Dive Intelligence (iOS / Flutter / Android / JVM / C++ / Rust / Node / Python / Go / IaC)
    let (
        detected_stack_archetype,
        ios_codemagic_annual_m2_monthly_usd,
        ios_buildkite_mac_mini_fleet_monthly_tco_usd,
        android_emulator_jobs_count,
        android_kvm_or_gradle_cache_savings_usd,
        jvm_warm_cache_50pct_savings_usd,
        stack_specific_monthly_savings_usd,
        stack_specific_primary_lever,
    ) = if jobs.is_empty() {
        (
            "none".to_string(),
            0.0,
            0.0,
            0,
            0.0,
            0.0,
            0.0,
            "none".to_string(),
        )
    } else {
        // Compute macOS Codemagic M2 flat plan ($332.50/mo) & Buildkite Mac mini fleet ($25/mini + seats) whenever macOS jobs exist
        let (cm_annual_mo, bk_mac_tco, ios_mac_sav) = if macos_mins_per_run > 0.0 {
            let cm = 332.50_f64; // $3,990/yr flat unmetered Mac mini M2 (3 concurrent slots)
            let minis_needed = ((monthly_mac_mins / 9000.0).ceil().max(1.0)) as f64;
            let bk_seat_usd = if implied_active_devs <= 5 && macos_concurrent_jobs_per_run <= 10 {
                0.0
            } else {
                (implied_active_devs as f64) * 30.0
            };
            // $899 Mac mini / 36 months = $25/mo per mini (calc_ios.html lines 521, 558, 579, 600)
            let bk = ((bk_seat_usd + minis_needed * 25.0) * 100.0).round() / 100.0;
            let best_alt = cm.min(bk);
            let sav = ((monthly_macos_hosted_usd - best_alt).max(0.0) * 100.0).round() / 100.0;
            (cm, bk, sav)
        } else {
            (0.0, 0.0, 0.0)
        };

        let is_primary_ios_macos = !hygiene.has_flutter_or_dart
            && !hygiene.has_cpp_or_cmake
            && !hygiene.has_rust_or_cargo
            && !hygiene.has_go_or_gomod
            && macos_mins_per_run > 0.0
            && macos_mins_per_run >= (linux_mins_per_run + windows_mins_per_run);

        if is_primary_ios_macos {
            let lever = if monthly_mac_mins >= 4000.0 {
                "self-hosted-mac-mini-fleet-or-flat-m2-plan"
            } else {
                "xcode-deriveddata-spm-cache-and-macos-on-merge-only"
            };
            (
                "ios-macos-xcode".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                hygiene.android_emulator_jobs_count,
                0.0,
                0.0,
                ios_mac_sav,
                lever.to_string(),
            )
        } else if hygiene.has_flutter_or_dart {
            let fl_sav = ((gross_monthly_compute_cost_usd * 0.35).max(ios_mac_sav) * 100.0).round()
                / 100.0;
            (
                "flutter-dart-mobile".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                hygiene.android_emulator_jobs_count,
                0.0,
                0.0,
                fl_sav,
                "pub-cache-gradle-deriveddata-and-flat-m2-plan".to_string(),
            )
        } else if detected_log_language_profile.contains("Android")
            || hygiene.android_emulator_jobs_count > 0
        {
            let ratio = if hygiene.has_gradle_build_cache_flag && jobs_missing_cache == 0 {
                0.25
            } else {
                0.50
            };
            let and_sav = ((gross_monthly_compute_cost_usd * ratio) * 100.0).round() / 100.0;
            (
                "android-gradle-emulator".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                hygiene.android_emulator_jobs_count.max(1),
                and_sav,
                0.0,
                and_sav,
                "3-layer-gradle-cache-and-kvm-emulator-snapshot".to_string(),
            )
        } else if detected_log_language_profile.contains("Java") || hygiene.has_gradle_multi_module {
            let ratio = if hygiene.has_gradle_build_cache_flag || hygiene.has_maven_parallel_flag {
                0.20
            } else {
                0.50
            };
            let jvm_sav = ((gross_monthly_compute_cost_usd * ratio) * 100.0).round() / 100.0;
            let lever = if hygiene.has_gradle_multi_module {
                "gradle-build-and-configuration-cache-with-incremental-module-check"
            } else {
                "gradle-build-cache-or-maven-parallel-m2-cache"
            };
            (
                "java-jvm-gradle-maven".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                jvm_sav,
                jvm_sav,
                lever.to_string(),
            )
        } else if hygiene.has_cpp_or_cmake && !hygiene.has_rust_or_cargo {
            let cpp_sav = if hygiene.has_remote_compile_cache {
                ((gross_monthly_compute_cost_usd * 0.18) * 100.0).round() / 100.0
            } else {
                ((gross_monthly_compute_cost_usd * 0.45) * 100.0).round() / 100.0
            };
            (
                "cpp-cmake-ninja".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                0.0,
                cpp_sav,
                "ccache-remote-storage-and-ninja-larger-runners".to_string(),
            )
        } else if detected_log_language_profile.contains("Rust")
            || (detected_log_language_profile.contains("iOS") && hygiene.has_rust_or_cargo)
        {
            let rust_sav = if hygiene.has_remote_compile_cache {
                ((gross_monthly_compute_cost_usd * 0.15) * 100.0).round() / 100.0
            } else {
                ((gross_monthly_compute_cost_usd * 0.45) * 100.0).round() / 100.0
            };
            (
                "rust-cargo".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                0.0,
                rust_sav,
                "rust-cache-sccache-and-cargo-nextest".to_string(),
            )
        } else if infracost_iac_files_detected
            && jobs
                .iter()
                .filter(|j| {
                    j.workflow_file.contains("terragrunt") || j.workflow_file.contains("terraform")
                })
                .count()
                * 2
                >= jobs.len().max(1)
        {
            let iac_sav = ((gross_monthly_compute_cost_usd * 0.40) * 100.0).round() / 100.0;
            (
                "terraform-terragrunt-iac".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                0.0,
                iac_sav,
                "terraform-plugin-cache-and-changed-module-plan".to_string(),
            )
        } else if detected_log_language_profile.contains("Go")
            || (detected_log_language_profile.contains("iOS") && hygiene.has_go_or_gomod)
        {
            let go_sav = monthly_unrealized_cache_savings_usd
                .max(((gross_monthly_compute_cost_usd * 0.30) * 100.0).round() / 100.0);
            (
                "go-mod-build-cache".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                0.0,
                go_sav,
                "go-build-mod-cache-and-arm64-runners".to_string(),
            )
        } else if detected_log_language_profile.contains("Python")
            || (detected_log_language_profile.contains("iOS") && hygiene.has_python_or_pytest)
        {
            let py_sav = monthly_unrealized_cache_savings_usd
                .max(((gross_monthly_compute_cost_usd * 0.25) * 100.0).round() / 100.0);
            (
                "python-uv-pytest".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                0.0,
                py_sav,
                "uv-cache-and-pytest-xdist-sharding".to_string(),
            )
        } else {
            let node_sav = monthly_unrealized_cache_savings_usd
                .max(((gross_monthly_compute_cost_usd * 0.30) * 100.0).round() / 100.0);
            (
                "node-typescript-vitest".to_string(),
                cm_annual_mo,
                bk_mac_tco,
                0,
                0.0,
                0.0,
                node_sav,
                "pnpm-store-cache-and-turborepo-remote-cache".to_string(),
            )
        }
    };

    // 21. Unit Economics & Chargeback (`$/PR`, `$/Deploy`, Cadence-Weighted Spend & CODEOWNERS Attribution)
    let estimated_monthly_merged_prs = if jobs.is_empty() {
        0.0
    } else if let Some(obs_prs) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_merged_prs_30d)
    {
        (obs_prs as f64).max(1.0)
    } else {
        (usage.prs_per_week * 4.33).max(1.0)
    };
    let est_monthly_deploys = if jobs.is_empty() {
        0.0
    } else if let Some(obs_dep) = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_deploys_30d)
    {
        (obs_dep as f64).max(1.0)
    } else if hygiene.has_deploy_or_release_steps {
        (estimated_monthly_merged_prs * 0.50).max(4.0)
    } else {
        estimated_monthly_merged_prs.max(1.0)
    };
    let mut cadence_weighted_monthly_compute_usd = 0.0_f64;
    let mut wf_cost_totals: Vec<(String, f64)> = Vec::new();
    for j in &jobs {
        let wf_runs = j.effective_monthly_runs(monthly_runs);
        let j_mo = j.run_compute_cost_usd * wf_runs;
        cadence_weighted_monthly_compute_usd += j_mo;
        if let Some(entry) = wf_cost_totals.iter_mut().find(|(wf, _)| wf == &j.workflow_file) {
            entry.1 += j_mo;
        } else {
            wf_cost_totals.push((j.workflow_file.clone(), j_mo));
        }
    }
    cadence_weighted_monthly_compute_usd = if jobs.is_empty() {
        gross_monthly_compute_cost_usd
    } else {
        (cadence_weighted_monthly_compute_usd * 100.0).round() / 100.0
    };
    let unit_econ_base_usd = if cadence_weighted_monthly_compute_usd > 0.0 {
        cadence_weighted_monthly_compute_usd
    } else {
        gross_monthly_compute_cost_usd
    };
    let cost_per_merged_pr_usd = if estimated_monthly_merged_prs > 0.0 {
        ((unit_econ_base_usd / estimated_monthly_merged_prs) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let cost_per_deploy_usd = if est_monthly_deploys > 0.0 {
        ((unit_econ_base_usd / est_monthly_deploys) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let cost_per_active_dev_monthly_usd = if implied_active_devs > 0 {
        ((unit_econ_base_usd / (implied_active_devs as f64)) * 100.0).round() / 100.0
    } else {
        0.0
    };
    let top_wf_usd = wf_cost_totals
        .iter()
        .map(|(_, c)| *c)
        .fold(0.0_f64, f64::max);
    let top_workflow_cost_share_pct = if unit_econ_base_usd > 0.0 {
        (((top_wf_usd / unit_econ_base_usd) * 100.0) * 10.0).round() / 10.0
    } else {
        0.0
    };
    let codeowners_detected = hygiene.codeowners_detected;
    let top_codeowners_team = hygiene.top_codeowner_team.clone();

    // 22. Spend Anomaly (`1.50x` 7d vs prior 23d baseline), OS-Weighted Quota Exhaustion & Per-Job `timeout-minutes` Exposure
    let burn_velocity_7d_vs_30d_ratio = if jobs.is_empty() {
        0.0
    } else if let Some(r7) = active_telemetry.as_ref().and_then(|t| t.observed_runs_last_7d) {
        let daily_7d = r7 / 7.0;
        let prior_23d_daily = if let Some(p23) = active_telemetry
            .as_ref()
            .and_then(|t| t.observed_runs_prev_23d)
            .filter(|v| *v > 0.0)
        {
            (p23 / 23.0).max(0.01)
        } else if let Some(r30) = active_telemetry.as_ref().and_then(|t| t.observed_monthly_runs) {
            if r30 > r7 {
                ((r30 - r7) / 23.0).max(0.01)
            } else {
                (r30 / 30.0).max(0.01)
            }
        } else {
            daily_7d.max(0.01)
        };
        ((daily_7d / prior_23d_daily) * 100.0).round() / 100.0
    } else {
        1.0
    };
    let eff_sample_runs = active_telemetry
        .as_ref()
        .and_then(|t| {
            t.observed_runs_in_window
                .map(|n| n as f64)
                .or(t.observed_runs_last_7d)
        })
        .unwrap_or(monthly_runs);
    let spend_anomaly_alert_active =
        burn_velocity_7d_vs_30d_ratio >= 1.50 && eff_sample_runs >= 5.0;
    let eom_vel_mult = if burn_velocity_7d_vs_30d_ratio <= 0.0 && !jobs.is_empty() {
        0.0
    } else {
        let v = burn_velocity_7d_vs_30d_ratio.max(0.0);
        (30.0 * v) / (7.0 * v + 23.0)
    };
    let projected_eom_compute_usd =
        norm_zero(((gross_monthly_compute_cost_usd * eom_vel_mult) * 100.0).round() / 100.0);
    let projected_eom_pool_mins = monthly_os_weighted_free_tier_mins * eom_vel_mult.max(1.0);
    let projected_quota_exhaustion_day = if free_tier_included_mins <= 0.0
        || free_tier_included_mins.is_infinite()
        || projected_eom_pool_mins <= free_tier_included_mins
    {
        0_u32
    } else {
        let frac_of_month = (free_tier_included_mins / projected_eom_pool_mins).clamp(0.03, 1.0);
        ((frac_of_month * 30.0).ceil() as u32).clamp(1, 30)
    };
    // Exact per-job `timeout-minutes` scan & per-job runner rate × matrix count (Fixes Bug #30.1 & #30.2)
    let mut distinct_missing_timeout: Vec<(&str, &str)> = Vec::new();
    for j in &jobs {
        if !j.has_timeout_minutes {
            let pair = (j.workflow_file.as_str(), split_base_jid(&j.job_id).0);
            if !distinct_missing_timeout.contains(&pair) {
                distinct_missing_timeout.push(pair);
            }
        }
    }
    let jobs_missing_timeout_minutes_count = distinct_missing_timeout.len();
    let hung_job_360m_worst_case_exposure_usd = norm_zero(
        ((jobs
            .iter()
            .filter(|j| !j.has_timeout_minutes)
            .map(|j| {
                let default_timeout_m = match j.provider.as_str() {
                    "gcp-cloud-build" => 10.0,
                    "gitlab-ci" | "azure-pipelines" => 60.0,
                    "bitbucket" => 120.0,
                    "circleci" => 300.0,
                    _ => 360.0,
                };
                default_timeout_m * j.rate_per_min_usd * (j.matrix_combination_count as f64)
            })
            .sum::<f64>())
            * 100.0)
            .round()
            / 100.0,
    );

    // 23. Container Registry Economics (Live `observed_ghcr_storage_gb` or GHCR / ECR / GCR Image Layer Delta)
    let container_images_built_count = if hygiene.container_images_built_count > 0 {
        hygiene.container_images_built_count
    } else if hygiene.has_docker_build_or_push_steps {
        1
    } else {
        0
    };
    let observed_reg_gb = active_telemetry
        .as_ref()
        .and_then(|t| t.observed_ghcr_storage_gb);
    let (
        est_registry_storage_gb,
        monthly_registry_storage_and_pull_usd,
        untagged_image_cleanup_monthly_savings_usd,
    ) = if container_images_built_count == 0 && observed_reg_gb.is_none() {
        (0.0, 0.0, 0.0)
    } else {
        let retention_factor = if hygiene.has_registry_retention_cleanup {
            0.35
        } else {
            1.0
        };
        let reg_gb = if let Some(obs_g) = observed_reg_gb {
            (obs_g * 100.0).round() / 100.0
        } else {
            (((container_images_built_count as f64)
                * 0.45
                * (monthly_runs * 0.35).max(4.0)
                * retention_factor)
                * 100.0)
                .round()
                / 100.0
        };
        let billable_reg_gb = (reg_gb - 1.0).max(0.0);
        let reg_usd = ((billable_reg_gb * 0.10) * 100.0).round() / 100.0;
        let cleanup_sav = if hygiene.has_registry_retention_cleanup {
            0.0
        } else {
            ((reg_usd * 0.65) * 100.0).round() / 100.0
        };
        (reg_gb, reg_usd, cleanup_sav)
    };

    // 24. Self-Hosted Fleet Off-Hours Idle Waste & Queue SLA Breach Risk (Fixes Bug #31)
    let (
        off_hours_idle_capacity_pct,
        monthly_always_on_idle_waste_usd,
        est_p95_queue_wait_sec,
        sla_queue_breach_risk,
    ) = if jobs.is_empty() {
        (0.0, 0.0, 0.0, false)
    } else {
        let max_idle_possible = (100.0 - always_on_runner_utilization_pct).clamp(0.0, 99.5);
        let idle_pct = if let Some(obs_off_share) = active_telemetry
            .as_ref()
            .and_then(|t| t.observed_off_hours_run_share_pct)
        {
            // Nights + weekends represent 128/168 = 76.2% of wall-clock hours, strictly bounded by `100% - active%` (`G3`)!
            let off_hours_idle_floor = (76.2 * (1.0 - (obs_off_share / 100.0).clamp(0.0, 1.0)))
                .clamp(0.0, max_idle_possible);
            (max_idle_possible.max(off_hours_idle_floor) * 10.0).round() / 10.0
        } else {
            (max_idle_possible * 10.0).round() / 10.0
        };
        let idle_waste_usd = if true_self_hosted_jobs_per_run > 0 {
            (((fleet_nodes as f64) * ec2_od_mo * (idle_pct / 100.0)) * 100.0).round() / 100.0
        } else {
            0.0
        };
        let p95_q_sec = if let Some(obs_q) = active_telemetry
            .as_ref()
            .and_then(|t| t.observed_p95_queue_wait_sec)
        {
            (obs_q * 10.0).round() / 10.0
        } else if hosted_concurrent_jobs_per_run > vendor_free_concurrent_cap {
            let over_ratio =
                (hosted_concurrent_jobs_per_run as f64) / (vendor_free_concurrent_cap as f64);
            ((over_ratio - 1.0) * 120.0 + 25.0).round()
        } else if true_self_hosted_jobs_per_run > 0 {
            35.0
        } else {
            0.0
        };
        let breach = p95_q_sec > 60.0;
        (idle_pct, idle_waste_usd, p95_q_sec, breach)
    };

    let provenance_mode = active_telemetry
        .as_ref()
        .filter(|t| !t.provenance_mode.is_empty())
        .map(|t| t.provenance_mode.clone())
        .unwrap_or_else(|| "static_heuristic".to_string());

    let cost_components = CostComponentsBreakdown {
        rounding_tax_mins_per_run,
        monthly_rounding_tax_usd,
        linux_mins_per_run: (linux_mins_per_run * 100.0).round() / 100.0,
        windows_mins_per_run: (windows_mins_per_run * 100.0).round() / 100.0,
        macos_mins_per_run: (macos_mins_per_run * 100.0).round() / 100.0,
        os_weighted_free_tier_mins_per_run: (os_weighted_free_tier_mins_per_run * 100.0).round()
            / 100.0,
        monthly_os_weighted_free_tier_mins,
        monthly_flake_retry_10pct_overhead_usd,
        infracost_iac_files_detected,
        hosted_effective_hourly_rate_usd,
        infracost_recommended_ec2_instance: ec2_label.to_string(),
        infracost_ec2_on_demand_hourly_usd: ec2_od_hr,
        infracost_ec2_on_demand_monthly_usd: ec2_od_mo,
        infracost_ec2_spot_hourly_usd: ec2_spot_hr,
        infracost_ec2_surrounding_overhead_monthly_usd,
        macos_self_hosted_mac_mini_monthly_usd,
        self_hosted_breakeven_mins_t3_medium: SELF_HOSTED_BREAKEVEN_MINS_PER_MONTH,
        self_hosted_breakeven_mins_c5_4xlarge_spot: SELF_HOSTED_C5_4XLARGE_SPOT_BREAKEVEN_MINS,
        always_on_runner_utilization_pct,
        ephemeral_runner_recommended_mode,
        ephemeral_cold_start_wait_tax_monthly_usd,
        k8s_eks_control_plane_monthly_usd,
        k8s_gke_aks_free_tier_control_plane_monthly_usd,
        spot_interruption_band_pct,
        spot_retry_overhead_monthly_usd,
        spot_ineligible_jobs_count,
        hybrid_spot_plus_on_demand_monthly_usd,
        jobs_uploading_artifacts,
        default_retention_days,
        configured_retention_days,
        steady_state_stored_gb_configured,
        monthly_vendor_storage_configured_usd,
        net_billed_vendor_storage_configured_usd,
        monthly_infracost_s3_configured_usd,
        steady_state_stored_gb_90d,
        steady_state_stored_gb_14d,
        monthly_vendor_storage_90d_usd,
        monthly_vendor_storage_14d_usd,
        monthly_infracost_s3_standard_90d_usd,
        monthly_infracost_s3_intelligent_14d_usd,
        peak_concurrent_jobs_per_run,
        hosted_concurrent_jobs_per_run,
        macos_concurrent_jobs_per_run,
        self_hosted_or_microvm_jobs_per_run,
        vendor_free_concurrent_cap,
        vendor_team_concurrent_cap,
        parallelism_tier_upgrade_tax_monthly_usd,
        detected_repo_matrix_profile,
        matrix_jobs_count,
        pr_unconditional_matrix_cells,
        conditional_pr_matrix_monthly_savings_usd,
        detected_log_language_profile,
        est_log_mb_per_run,
        monthly_log_volume_gb,
        vendor_default_log_retention_days,
        monthly_vendor_default_log_cost_usd: 0.0,
        monthly_datadog_30d_log_shipping_usd,
        monthly_datadog_90d_log_shipping_usd,
        monthly_splunk_30d_log_shipping_usd,
        monthly_infracost_loki_s3_usd,
        infracost_cloud_region,
        est_transfer_gb_per_run,
        monthly_transfer_gb,
        infracost_aws_internet_egress_per_gb_usd: AWS_INTERNET_EGRESS_USD_PER_GB,
        infracost_aws_nat_gateway_per_gb_usd,
        infracost_aws_cross_az_per_gb_usd: AWS_CROSS_AZ_USD_PER_GB,
        infracost_aws_cross_region_per_gb_usd: AWS_CROSS_REGION_USD_PER_GB,
        monthly_infracost_private_subnet_nat_worst_case_usd,
        monthly_infracost_cross_region_registry_usd,
        monthly_infracost_vpc_endpoint_cache_optimized_usd,
        productivity_loss_factor,
        blended_fully_loaded_hourly_usd,
        monthly_wait_hours,
        monthly_productive_payroll_loss_usd,
        monthly_blended_100hr_productive_loss_usd,
        wait_to_compute_cost_ratio,
        lead_time_feedback_tier,
        estimated_flake_rate_pct,
        has_job_level_auto_retry: hygiene.has_job_level_auto_retry,
        monthly_flaky_reruns,
        monthly_rerun_compute_cost_usd,
        avg_investigation_mins_per_flake,
        monthly_flake_investigation_hours,
        monthly_flake_investigation_payroll_usd,
        monthly_total_immediate_flake_cost_usd,
        monthly_savings_at_1pct_target_usd,
        stack_cache_profile: stack_cache_label.to_string(),
        realistic_saving_pct_min,
        realistic_saving_pct_max,
        jobs_with_cache,
        jobs_missing_cache,
        has_restore_keys_fallback: hygiene.has_restore_keys_fallback,
        has_docker_buildkit_cache: hygiene.has_docker_buildkit_cache,
        estimated_cache_hit_rate_pct,
        vendor_cache_size_limit: vendor_cache_limit.to_string(),
        vendor_cache_eviction_policy: vendor_cache_eviction.to_string(),
        monthly_uncached_compute_usd,
        monthly_unrealized_cache_savings_usd,
        next_recommended_caching_layer,
        distributed_remote_cache_monthly_infra_usd,
        container_base_image_mirror_monthly_savings_usd,
        pr_cache_poisoning_risk_jobs,
        deferred_velocity_drag_multiplier,
        monthly_deferred_velocity_drag_usd,
        monthly_combined_flake_tax_usd,
        deflake_project_est_cost_usd,
        deflake_payback_weeks,
        required_checks_count,
        monthly_required_check_executions,
        workflows_with_concurrency_cancel: hygiene.workflows_with_concurrency_cancel,
        workflows_missing_concurrency_cancel: hygiene.workflows_missing_concurrency_cancel,
        workflows_with_path_filters: hygiene.workflows_with_path_filters,
        workflows_missing_path_filters: hygiene.workflows_missing_path_filters,
        has_affected_only_graph_tool: hygiene.has_affected_only_graph_tool,
        slow_blocking_checks_over_10m,
        monthly_superseded_run_waste_usd,
        monthly_unfiltered_check_savings_usd,
        inferred_team_stage,
        implied_active_devs,
        free_tier_burn_pct,
        upgrade_trigger_80pct_reached,
        premature_self_hosting_trap,
        premature_multi_os_pr_matrix_trap,
        monthly_multi_os_pr_to_nightly_savings_usd,
        mac_mini_hardware_payback_months,
        self_hosted_breakeven_reached,
        team_to_enterprise_seat_jump_monthly_usd,
        quarterly_required_check_audit_savings_usd,
        platform_engineer_monthly_fully_loaded_usd,
        recoverable_ci_and_wait_monthly_usd,
        platform_engineer_roi_coverage_pct,
        detected_compliance_controls: hygiene.detected_compliance_controls,
        compliance_controls_count,
        detected_cloud_regions: hygiene.detected_cloud_regions,
        regional_residency_multiplier,
        enterprise_250_seat_licence_monthly_usd,
        siem_1yr_audit_log_retention_monthly_usd,
        dedicated_tenancy_premium_pct_min,
        dedicated_tenancy_premium_pct_max,
        detected_pipeline_shape,
        detected_workspace_units_count,
        detected_monorepo_tool: hygiene.detected_monorepo_tool,
        recommended_affected_tool,
        has_nightly_full_suite_safety_net: hygiene.has_nightly_schedule_workflow,
        affected_only_reduction_pct_min,
        affected_only_reduction_pct_max,
        monthly_affected_only_savings_usd,
        est_setup_overhead_sec_per_job,
        monthly_setup_overhead_hours,
        setup_overhead_share_pct,
        monthly_setup_overhead_compute_usd,
        monthly_shared_infra_setup_savings_usd,
        reusable_workflow_calls_count: hygiene.reusable_workflow_calls_count,
        unpinned_template_refs_count: hygiene.unpinned_template_refs_count,
        cross_repo_triggers_count: hygiene.cross_repo_triggers_count,
        est_aggregate_cache_gb,
        exceeds_10gb_repo_cache_limit,
        cheapest_vendor_platform,
        cheapest_vendor_monthly_tco_usd,
        github_actions_team_monthly_tco_usd,
        gitlab_ci_premium_monthly_tco_usd,
        circleci_performance_monthly_tco_usd,
        bitbucket_premium_monthly_tco_usd,
        buildkite_pro_monthly_tco_usd,
        jenkins_self_hosted_monthly_tco_usd,
        aws_codebuild_monthly_tco_usd,
        gcp_cloud_build_monthly_tco_usd,
        azure_pipelines_monthly_tco_usd,
        migration_estimated_hours,
        migration_estimated_cost_usd,
        migration_payback_months,
        migration_worth_it_2x_annual_rule,
        migration_verdict,
        phase_clone_checkout_share_pct,
        phase_dependency_install_share_pct,
        phase_compile_typecheck_share_pct,
        phase_test_suite_share_pct,
        phase_docker_build_push_share_pct,
        phase_deploy_cleanup_share_pct,
        monthly_total_reducible_phase_savings_usd,
        best_first_optimization_phase,
        best_first_optimization_effort_hours,
        best_first_optimization_monthly_savings_usd,
        best_first_optimization_payback_months,
        detected_workload_compute_profile,
        larger_runners_detected_count,
        larger_runners_bypass_free_minutes_tax_usd,
        arm64_2core_monthly_compute_usd,
        arm64_migration_monthly_savings_usd,
        standard_2core_monthly_compute_usd,
        larger_4core_monthly_compute_usd,
        larger_8core_monthly_compute_usd,
        larger_16core_monthly_compute_usd,
        best_net_roi_runner_tier,
        best_net_roi_monthly_delta_usd,
        detected_stack_archetype,
        ios_codemagic_annual_m2_monthly_usd,
        ios_buildkite_mac_mini_fleet_monthly_tco_usd,
        android_emulator_jobs_count,
        android_kvm_or_gradle_cache_savings_usd,
        jvm_warm_cache_50pct_savings_usd,
        stack_specific_monthly_savings_usd,
        stack_specific_primary_lever,
        cost_per_merged_pr_usd,
        cost_per_deploy_usd,
        cost_per_active_dev_monthly_usd,
        cadence_weighted_monthly_compute_usd,
        top_workflow_cost_share_pct,
        codeowners_detected,
        top_codeowners_team,
        burn_velocity_7d_vs_30d_ratio,
        spend_anomaly_alert_active,
        projected_eom_compute_usd,
        projected_quota_exhaustion_day,
        jobs_missing_timeout_minutes_count,
        hung_job_360m_worst_case_exposure_usd,
        container_images_built_count,
        est_registry_storage_gb,
        monthly_registry_storage_and_pull_usd,
        untagged_image_cleanup_monthly_savings_usd,
        off_hours_idle_capacity_pct,
        monthly_always_on_idle_waste_usd,
        est_p95_queue_wait_sec,
        sla_queue_breach_risk,
    };

    let elapsed_us = t0.elapsed().as_micros().max(1);

    BreakdownReport {
        target_path: target.display().to_string(),
        provenance_mode,
        execution_time_us: elapsed_us,
        execution_time_ms: (elapsed_us as f64) / 1000.0,
        detected_providers: providers,
        usage,
        total_workflows: workflow_count,
        total_defined_jobs,
        total_expanded_matrix_jobs,
        raw_duration_mins_per_run: (raw_duration_mins_per_run * 100.0).round() / 100.0,
        critical_path_wait_mins_per_run: (critical_path_wait_mins_per_run * 100.0).round() / 100.0,
        billed_mins_per_run: (billed_mins_per_run * 100.0).round() / 100.0,
        monthly_runs,
        monthly_billed_mins,
        free_tier_included_mins,
        free_tier_quota_burn_pct,
        effective_tier,
        gross_compute_cost_per_run_usd,
        gross_monthly_compute_cost_usd,
        discount_allowance_monthly_usd,
        net_billed_monthly_compute_cost_usd,
        third_party_runner_monthly_cost_usd,
        shard_setup_duplication_mins_per_run,
        clones_per_ci_run,
        dev_wait_cost_per_run_usd,
        monthly_dev_wait_cost_usd,
        monthly_artifact_storage_vendor_usd,
        monthly_artifact_storage_s3_usd,
        monthly_blacksmith_cost_usd,
        monthly_self_hosted_spot_cost_usd,
        exceeds_5000m_self_hosted_breakeven: monthly_billed_mins
            >= SELF_HOSTED_BREAKEVEN_MINS_PER_MONTH,
        cost_components,
        jobs,
    }
}

pub fn compute_diff(base: &BreakdownReport, head: &BreakdownReport) -> DiffReport {
    let t0 = Instant::now();
    let delta_jobs =
        (head.total_expanded_matrix_jobs as isize) - (base.total_expanded_matrix_jobs as isize);
    let delta_billed_mins = norm_zero(
        ((head.billed_mins_per_run - base.billed_mins_per_run) * 100.0).round() / 100.0,
    );
    let delta_compute = norm_zero(
        ((head.gross_monthly_compute_cost_usd - base.gross_monthly_compute_cost_usd) * 100.0)
            .round()
            / 100.0,
    );
    let delta_pct = norm_zero(if base.gross_monthly_compute_cost_usd > 0.0 {
        ((delta_compute / base.gross_monthly_compute_cost_usd) * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    });
    let delta_wait = norm_zero(
        ((head.monthly_dev_wait_cost_usd - base.monthly_dev_wait_cost_usd) * 100.0).round() / 100.0,
    );

    // When diffing two standalone single-workflow files (`minici diff /tmp/a.yml /tmp/b.yml`),
    // match jobs by `job_id` even when filenames differ (`H4`)!
    let single_wf_diff = base.total_workflows == 1 && head.total_workflows == 1;
    let jobs_match = |a: &ExpandedJobCost, b: &ExpandedJobCost| -> bool {
        if a.job_id != b.job_id {
            return false;
        }
        if a.workflow_file == b.workflow_file || single_wf_diff {
            return true;
        }
        let ab = a.workflow_file.rsplit('/').next().unwrap_or(&a.workflow_file);
        let bb = b.workflow_file.rsplit('/').next().unwrap_or(&b.workflow_file);
        ab == bb
    };

    let mut changes: Vec<String> = Vec::new();
    for hj in &head.jobs {
        if let Some(bj) = base.jobs.iter().find(|x| jobs_match(x, hj)) {
            let bj_mo = bj.monthly_compute_cost(base.monthly_runs);
            let hj_mo = hj.monthly_compute_cost(head.monthly_runs);
            if bj.matrix_combination_count != hj.matrix_combination_count
                || bj.runner_label != hj.runner_label
                || (bj.billed_mins_per_job - hj.billed_mins_per_job).abs() > 0.01
                || (bj.run_compute_cost_usd - hj.run_compute_cost_usd).abs() > 0.0001
                || (bj_mo - hj_mo).abs() > 0.01
            {
                changes.push(format!(
                    "Modified `{}` (`{}`): `{}x {}` ({:.1}m • ${:.3}/run) -> `{}x {}` ({:.1}m • ${:.3}/run)",
                    hj.job_id,
                    hj.workflow_file,
                    bj.matrix_combination_count,
                    bj.runner_label,
                    bj.total_billed_mins_all_matrix,
                    bj.run_compute_cost_usd,
                    hj.matrix_combination_count,
                    hj.runner_label,
                    hj.total_billed_mins_all_matrix,
                    hj.run_compute_cost_usd
                ));
            } else if bj.artifact_retention_days != hj.artifact_retention_days
                || bj.has_timeout_minutes != hj.has_timeout_minutes
                || bj.has_concurrency_cancel != hj.has_concurrency_cancel
            {
                changes.push(format!(
                    "Updated guardrails on `{}` (`{}`): retention {:?}d -> {:?}d, timeout {} -> {}, cancel-in-progress {} -> {}",
                    hj.job_id,
                    hj.workflow_file,
                    bj.artifact_retention_days,
                    hj.artifact_retention_days,
                    bj.has_timeout_minutes,
                    hj.has_timeout_minutes,
                    bj.has_concurrency_cancel,
                    hj.has_concurrency_cancel
                ));
            }
        } else {
            changes.push(format!(
                "Added job `{}` (`{}`): `{}x {}` (+${:.3}/run)",
                hj.job_id,
                hj.workflow_file,
                hj.matrix_combination_count,
                hj.runner_label,
                hj.run_compute_cost_usd
            ));
        }
    }
    for bj in &base.jobs {
        if !head.jobs.iter().any(|x| jobs_match(bj, x)) {
            changes.push(format!(
                "Removed job `{}` (`{}`): `{}x {}` (-${:.3}/run)",
                bj.job_id,
                bj.workflow_file,
                bj.matrix_combination_count,
                bj.runner_label,
                bj.run_compute_cost_usd
            ));
        }
    }
    let delta_net = norm_zero(
        ((head.net_billed_monthly_compute_cost_usd - base.net_billed_monthly_compute_cost_usd)
            * 100.0)
            .round()
            / 100.0,
    );
    let delta_storage = norm_zero(
        ((head.monthly_artifact_storage_vendor_usd - base.monthly_artifact_storage_vendor_usd)
            * 100.0)
            .round()
            / 100.0,
    );
    if delta_net.abs() >= 0.01 && (delta_net - delta_compute).abs() >= 0.01 {
        changes.push(format!(
            "Net billed compute delta: ${:.2}/mo -> ${:.2}/mo ({}{:.2}/mo)",
            base.net_billed_monthly_compute_cost_usd,
            head.net_billed_monthly_compute_cost_usd,
            if delta_net >= 0.0 { "+" } else { "" },
            delta_net
        ));
    }
    if delta_storage.abs() >= 0.01 {
        changes.push(format!(
            "Artifact storage delta: ${:.2}/mo -> ${:.2}/mo ({}{:.2}/mo)",
            base.monthly_artifact_storage_vendor_usd,
            head.monthly_artifact_storage_vendor_usd,
            if delta_storage >= 0.0 { "+" } else { "" },
            delta_storage
        ));
    }

    DiffReport {
        base_path: base.target_path.clone(),
        head_path: head.target_path.clone(),
        execution_time_us: t0.elapsed().as_micros() + head.execution_time_us,
        base_expanded_jobs: base.total_expanded_matrix_jobs,
        head_expanded_jobs: head.total_expanded_matrix_jobs,
        delta_expanded_jobs: delta_jobs,
        base_billed_mins_per_run: base.billed_mins_per_run,
        head_billed_mins_per_run: head.billed_mins_per_run,
        delta_billed_mins_per_run: delta_billed_mins,
        base_monthly_compute_usd: base.gross_monthly_compute_cost_usd,
        head_monthly_compute_usd: head.gross_monthly_compute_cost_usd,
        delta_monthly_compute_usd: delta_compute,
        delta_monthly_compute_pct: delta_pct,
        base_monthly_dev_wait_usd: base.monthly_dev_wait_cost_usd,
        head_monthly_dev_wait_usd: head.monthly_dev_wait_cost_usd,
        delta_monthly_dev_wait_usd: delta_wait,
        base_free_quota_burn_pct: base.free_tier_quota_burn_pct,
        head_free_quota_burn_pct: head.free_tier_quota_burn_pct,
        job_changes: changes,
        head_report: head.clone(),
    }
}
