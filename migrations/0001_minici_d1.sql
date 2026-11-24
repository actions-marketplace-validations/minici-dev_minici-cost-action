-- MiniCI Cloudflare D1 Schema (`migrations/0001_minici_d1.sql`)
-- Layer 3 persistent serverless SQLite backing store behind:
--   Layer 1: Cloudflare Edge Cache API (`caches.default`, s-maxage=300, stale-while-revalidate=3600)
--   Layer 2: Cloudflare Workers KV (`MINICI_KV`, TTL 3600s for reports, 30d for cross-repo classifier step hashes)

CREATE TABLE IF NOT EXISTS repo_telemetry (
  repo TEXT NOT NULL,
  vendor TEXT NOT NULL DEFAULT 'github',
  visibility TEXT NOT NULL DEFAULT 'public',
  head_sha TEXT NOT NULL DEFAULT '',
  window_days INTEGER NOT NULL DEFAULT 30,
  updated_at INTEGER NOT NULL,
  monthly_runs REAL NOT NULL DEFAULT 0.0,
  skipped_noop_runs INTEGER NOT NULL DEFAULT 0,
  bot_run_share_pct REAL NOT NULL DEFAULT 0.0,
  billed_mins_per_run REAL NOT NULL DEFAULT 0.0,
  gross_monthly_usd REAL NOT NULL DEFAULT 0.0,
  net_billed_monthly_usd REAL NOT NULL DEFAULT 0.0,
  active_cache_gb REAL NOT NULL DEFAULT 0.0,
  payload_json TEXT NOT NULL,
  PRIMARY KEY (repo, window_days)
);

CREATE INDEX IF NOT EXISTS idx_repo_telemetry_updated
  ON repo_telemetry(updated_at DESC);

CREATE TABLE IF NOT EXISTS step_decisions (
  step_sha256 TEXT PRIMARY KEY,
  phase TEXT NOT NULL,
  arm64_safe INTEGER NOT NULL DEFAULT 1,
  spot_safe INTEGER NOT NULL DEFAULT 1,
  cancel_safe INTEGER NOT NULL DEFAULT 1,
  cache_candidate INTEGER NOT NULL DEFAULT 0,
  confidence REAL NOT NULL DEFAULT 0.90,
  model_id TEXT NOT NULL DEFAULT 'jev-systemone',
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS org_billing_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  org TEXT NOT NULL,
  vendor TEXT NOT NULL,
  period_start TEXT NOT NULL,
  period_end TEXT NOT NULL,
  gross_usd REAL NOT NULL,
  discount_usd REAL NOT NULL DEFAULT 0.0,
  net_billed_usd REAL NOT NULL,
  focus_summary_json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_org_billing_lookup
  ON org_billing_snapshots(org, vendor, period_start DESC);

CREATE TABLE IF NOT EXISTS learned_skus (
  raw_label TEXT PRIMARY KEY,
  canonical_sku TEXT NOT NULL,
  confidence REAL NOT NULL,
  source TEXT NOT NULL DEFAULT 'jev-systemone',
  updated_at INTEGER NOT NULL
);
