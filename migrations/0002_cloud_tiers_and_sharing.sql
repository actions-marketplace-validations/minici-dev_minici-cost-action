-- MiniCI Cloud Tiers ($1 Starter / $10 Pro / Custom Team Flex) Schema (`migrations/0002_cloud_tiers_and_sharing.sql`)
-- Keeps 100% of stateless CLI, MCP & GitHub Action features $0 Free Forever while powering stateful Cloud Memory,
-- unlisted `/r/:id` report sharing, 30d/365d cross-run trend badges, Friday Slack digests, and org policies.

CREATE TABLE IF NOT EXISTS cloud_entitlements (
  key_sha256 TEXT PRIMARY KEY,
  key_hint TEXT NOT NULL DEFAULT '',
  plan TEXT NOT NULL DEFAULT 'starter',
  status TEXT NOT NULL DEFAULT 'active',
  max_repos INTEGER NOT NULL DEFAULT 3,
  retention_days INTEGER NOT NULL DEFAULT 30,
  customer_email TEXT NOT NULL DEFAULT '',
  polar_subscription_id TEXT NOT NULL DEFAULT '',
  slack_webhook_url TEXT NOT NULL DEFAULT '',
  custom_rates_json TEXT NOT NULL DEFAULT '{}',
  max_pr_delta_monthly_usd REAL NOT NULL DEFAULT 0.0,
  require_timeout_minutes INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_entitlements_plan_status
  ON cloud_entitlements(plan, status);

CREATE TABLE IF NOT EXISTS shared_reports (
  share_id TEXT PRIMARY KEY,
  key_sha256 TEXT NOT NULL,
  repo TEXT NOT NULL,
  provider TEXT NOT NULL DEFAULT 'github-actions',
  gross_monthly_usd REAL NOT NULL DEFAULT 0.0,
  net_monthly_usd REAL NOT NULL DEFAULT 0.0,
  dev_wait_monthly_usd REAL NOT NULL DEFAULT 0.0,
  report_json TEXT NOT NULL,
  report_html TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_shared_reports_key_created
  ON shared_reports(key_sha256, created_at DESC);

CREATE TABLE IF NOT EXISTS cloud_run_ledger (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key_sha256 TEXT NOT NULL,
  repo TEXT NOT NULL,
  provider TEXT NOT NULL DEFAULT 'github-actions',
  branch TEXT NOT NULL DEFAULT 'main',
  commit_sha TEXT NOT NULL DEFAULT '',
  expanded_jobs INTEGER NOT NULL DEFAULT 0,
  billed_mins_per_run REAL NOT NULL DEFAULT 0.0,
  gross_monthly_usd REAL NOT NULL DEFAULT 0.0,
  net_billed_monthly_usd REAL NOT NULL DEFAULT 0.0,
  dev_wait_monthly_usd REAL NOT NULL DEFAULT 0.0,
  cost_per_pr_usd REAL NOT NULL DEFAULT 0.0,
  spend_anomaly_active INTEGER NOT NULL DEFAULT 0,
  recorded_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_run_ledger_repo_time
  ON cloud_run_ledger(repo, recorded_at DESC);

CREATE INDEX IF NOT EXISTS idx_cloud_run_ledger_key_time
  ON cloud_run_ledger(key_sha256, recorded_at DESC);
