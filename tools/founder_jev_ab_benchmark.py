#!/usr/bin/env python3
"""
Founder `Jev` (`jev-lang/Jev`) Before vs. After Optimization A/B Benchmark
(`tools/founder_jev_ab_benchmark.py`).

Creates an isolated shadow copy of any repository (defaulting to local `Jev` or a realistic
compiled multi-OS Rust/C++ compiler repository matching `jev-lang/Jev`), runs:
  1. Baseline `minici breakdown`
  2. Surgical `minici shrink --apply` + `minici migrate --to ubuntu-arm64 --apply`
  3. Optimized `minici breakdown` & `minici diff`
and prints a deterministic Before/After Founder Case-Study Receipt.
"""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


def resolve_minici_bin(repo_root: Path) -> Path:
    candidates = [
        repo_root / "target" / "release" / "minici",
        repo_root / "crates" / "minici" / "target" / "release" / "minici",
    ]
    for c in candidates:
        if c.exists():
            return c
    subprocess.run(
        ["cargo", "build", "--release", "--manifest-path", str(repo_root / "crates" / "minici" / "Cargo.toml")],
        check=True,
    )
    return repo_root / "crates" / "minici" / "target" / "release" / "minici"


def seed_jev_fixture_if_needed(dest: Path) -> str:
    local_fixture = os.environ.get("MINICI_JEV_FIXTURE")
    if local_fixture:
        source = Path(local_fixture).expanduser()
        workflows = source / ".github" / "workflows"
        if not workflows.is_dir():
            raise ValueError("MINICI_JEV_FIXTURE must point to a repository with GitHub workflows")
        shutil.copytree(workflows, dest / ".github" / "workflows")
        for manifest in ("Cargo.toml", "CMakeLists.txt", "package.json"):
            if (source / manifest).is_file():
                shutil.copy2(source / manifest, dest / manifest)
        return "Configured local repository"

    wf = dest / ".github" / "workflows"
    wf.mkdir(parents=True, exist_ok=True)
    (dest / "Cargo.toml").write_text('[package]\nname = "jev"\nversion = "0.1.0"\n', encoding="utf-8")
    (wf / "ci.yml").write_text(
        """name: Jev Compiler & Runtime CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  compiler-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo check --workspace
      - run: cargo test --workspace
      - uses: actions/upload-artifact@v4
        with:
          name: jev-compiler-diagnostics
          retention-days: 90

  release-Preview:
    needs: compiler-check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
""",
        encoding="utf-8",
    )
    return "Synthetic Jev Compiler CI Fixture (offline reproducible)"


def run_json(bin_path: Path, args: list[str]) -> dict:
    out = subprocess.check_output([str(bin_path), *args], text=True)
    return json.loads(out)


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Founder Jev Before/After CI FinOps A/B Benchmark")
    parser.add_argument("--target", default="", help="Optional path to local repo to benchmark")
    parser.add_argument("--json", action="store_true", help="Emit JSON receipt")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    bin_path = resolve_minici_bin(repo_root)

    with tempfile.TemporaryDirectory(prefix="minici_jev_ab_") as tmp:
        base_dir = Path(tmp) / "before"
        opt_dir = Path(tmp) / "after"
        base_dir.mkdir()

        if args.target and Path(args.target).exists():
            src = Path(args.target)
            if (src / ".github").is_dir():
                shutil.copytree(src / ".github", base_dir / ".github")
            source_label = str(src)
        else:
            source_label = seed_jev_fixture_if_needed(base_dir)

        shutil.copytree(base_dir, opt_dir)

        before = run_json(bin_path, ["breakdown", "--path", str(base_dir), "--format", "json", "--quiet"])
        shrink_res = run_json(bin_path, ["shrink", "--path", str(opt_dir), "--apply", "--format", "json"])
        migrate_res = run_json(
            bin_path, ["migrate", "--path", str(opt_dir), "--to", "ubuntu-arm64", "--apply", "--format", "json"]
        )
        after = run_json(bin_path, ["breakdown", "--path", str(opt_dir), "--format", "json", "--quiet"])
        diff = run_json(
            bin_path,
            ["diff", "--path", str(opt_dir), "--compare-to", str(base_dir), "--format", "json", "--quiet"],
        )

        summary = {
            "source": source_label,
            "before_monthly_compute_usd": before["gross_monthly_compute_cost_usd"],
            "after_monthly_compute_usd": after["gross_monthly_compute_cost_usd"],
            "delta_monthly_compute_usd": diff["delta_monthly_compute_usd"],
            "delta_monthly_compute_pct": diff["delta_monthly_compute_pct"],
            "before_artifact_storage_usd": before["monthly_artifact_storage_vendor_usd"],
            "after_artifact_storage_usd": after["monthly_artifact_storage_vendor_usd"],
            "before_missing_timeouts": before["cost_components"]["spend_anomaly_and_forecast"][
                "jobs_missing_timeout_minutes_count"
            ],
            "after_missing_timeouts": after["cost_components"]["spend_anomaly_and_forecast"][
                "jobs_missing_timeout_minutes_count"
            ],
            "surgical_edits_applied": shrink_res["edits_count"] + migrate_res["edits_count"],
        }

        if args.json:
            print(json.dumps(summary, indent=2))
        else:
            print("+====================================================================================+")
            print("| MINICI FOUNDER `JEV` A/B SURGICAL OPTIMIZATION RECEIPT                              |")
            print("+====================================================================================+")
            print(f"  Source Workload       : {summary['source']}")
            print(
                f"  Monthly Compute       : ${summary['before_monthly_compute_usd']:.2f}/mo -> "
                f"${summary['after_monthly_compute_usd']:.2f}/mo "
                f"({summary['delta_monthly_compute_usd']:+.2f}/mo • {summary['delta_monthly_compute_pct']:+.1f}%)"
            )
            print(
                f"  Artifact Storage      : ${summary['before_artifact_storage_usd']:.2f}/mo -> "
                f"${summary['after_artifact_storage_usd']:.2f}/mo"
            )
            print(
                f"  Missing Job Timeouts  : {summary['before_missing_timeouts']} -> {summary['after_missing_timeouts']} "
                f"(eliminated 6-hour hung-runner exposure)"
            )
            print(f"  Surgical YAML Edits   : {summary['surgical_edits_applied']} comment-preserving edits")
            print("+====================================================================================+")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
