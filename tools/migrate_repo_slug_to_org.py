#!/usr/bin/env python3
"""
One-command repository slug migration tool for transferring `minici-cost-action`
from a personal GitHub user account to the Verified GitHub Organization
(e.g., `minici-dev`).

Usage:
  python3 tools/migrate_repo_slug_to_org.py --org minici-dev --dry-run
  python3 tools/migrate_repo_slug_to_org.py --org minici-dev --apply
"""
from __future__ import annotations

import argparse
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

TARGET_FILES = [
    ".github/workflows/minici-cost.yml",
    ".github/workflows/minici-pr-bot.yml",
    "README.md",
    "SECURITY.md",
    "action.yml",
    "crates/minici/src/main.rs",
    "crates/minici/src/permissions.rs",
    "docs/INSTALL_AND_USAGE.md",
    "docs/blog/01-github-actions-cost-benchmark-10-largest-open-source-repos.md",
    "docs/blog/02-infracost-and-minici-full-stack-finops-pull-request-costs.md",
    "docs/blog/03-github-actions-runners-benchmark-blacksmith-depot-namespace-warpbuild-buildjet-runson.md",
    "scripts/install.sh",
    "tools/build_seo_blog_hub.py",
    "web/index.html",
    "web/social-preview.svg",
]


def main() -> None:
    parser = argparse.ArgumentParser(description="Migrate repo slug to GitHub Organization")
    parser.add_argument("--from-owner", required=True, help="Current GitHub owner (e.g. your personal user)")
    parser.add_argument("--org", default="minici-dev", help="New GitHub Organization handle")
    parser.add_argument("--repo", default="minici-cost-action", help="Repository name")
    parser.add_argument("--apply", action="store_true", help="Write changes to disk")
    parser.add_argument("--dry-run", action="store_true", help="Preview changes without writing")
    args = parser.parse_args()

    old_slug = f"{args.from_owner}/{args.repo}"
    new_slug = f"{args.org}/{args.repo}"
    write_mode = args.apply and not args.dry_run

    total_replacements = 0
    touched_files = 0

    for rel in TARGET_FILES:
        fpath = REPO_ROOT / rel
        if not fpath.exists():
            continue
        raw = fpath.read_text(encoding="utf-8")
        count = raw.count(old_slug)
        if count == 0:
            continue
        touched_files += 1
        total_replacements += count
        mode_tag = "UPDATED" if write_mode else "DRY-RUN"
        print(f"  [{mode_tag}] {rel}: {count} occurrence(s) ({old_slug} -> {new_slug})")
        if write_mode:
            fpath.write_text(raw.replace(old_slug, new_slug), encoding="utf-8")

    if write_mode and (REPO_ROOT / "tools" / "build_seo_blog_hub.py").exists():
        subprocess.run(["python3", str(REPO_ROOT / "tools" / "build_seo_blog_hub.py")], check=True)

    print(
        f"\nSummary: {total_replacements} occurrence(s) across {touched_files} file(s)."
        + ("" if write_mode else " Re-run with --apply after transferring the repo on GitHub.")
    )


if __name__ == "__main__":
    main()
