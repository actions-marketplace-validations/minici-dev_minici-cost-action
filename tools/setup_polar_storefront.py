#!/usr/bin/env python3
"""
Automates Polar.sh Storefront, License-Key Benefit, Checkout Links & Recurring Tier setup
for `minici` ($1 Starter / $10 Pro / Custom Team Flex) via Polar's official REST API (`https://api.polar.sh/v1`).

Keeps 100% of existing local CLI, MCP & GitHub Action features $0 Free Forever (Apache-2.0),
while issuing automatic `MINICI-...` license keys for hosted Cloud Memory, `/r/:id` sharing,
30d/365d trend badges, Friday Slack FinOps digests, and org governance policies.

Usage:
  POLAR_OAT="polar_oat_..." python3 tools/setup_polar_storefront.py
"""
from __future__ import annotations

import json
import os
import sys
import urllib.error
import urllib.request

API_BASE = "https://api.polar.sh/v1"
ORG_SLUG = os.environ.get("POLAR_ORG_SLUG", "minici")


def api_call(method: str, path: str, token: str, payload: dict | None = None) -> dict:
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    req = urllib.request.Request(
        f"{API_BASE}{path}",
        data=data,
        method=method,
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "minici-polar-setup/0.9.0",
        },
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        return json.loads(resp.read().decode("utf-8"))


def ensure_license_key_benefit(org_id: str, token: str) -> str:
    benefits = api_call("GET", f"/benefits/?organization_id={org_id}", token).get("items") or []
    for b in benefits:
        if b.get("type") == "license_keys":
            print(f"Reusing existing Polar License Key benefit: {b.get('description')} (id={b.get('id')})")
            return str(b["id"])

    created = api_call(
        "POST",
        "/benefits/",
        token,
        {
            "type": "license_keys",
            "organization_id": org_id,
            "description": "MiniCI Cloud License Key (minici login / MINICI_KEY)",
            "properties": {
                "prefix": "MINICI",
                "activations": {"limit": 25, "enable_customer_admin": True},
            },
        },
    )
    bid = str(created["id"])
    print(f"Created Polar License Key benefit (id={bid})")
    return bid


def main() -> None:
    token = os.environ.get("POLAR_OAT") or os.environ.get("POLAR_ACCESS_TOKEN") or ""
    if not token.strip():
        print(
            "Error: Set POLAR_OAT='polar_oat_...' (create in https://polar.sh/dashboard/minici/settings -> Developers).",
            file=sys.stderr,
        )
        sys.exit(1)

    orgs = api_call("GET", f"/organizations/?slug={ORG_SLUG}", token)
    items = orgs.get("items") or []
    if not items:
        print(f"Error: Organization '{ORG_SLUG}' not found for this token.", file=sys.stderr)
        sys.exit(1)
    org_id = items[0]["id"]
    print(f"Found Polar organization '{ORG_SLUG}' (id={org_id})")

    # 1. Enable public storefront (`https://polar.sh/minici`)
    api_call(
        "PATCH",
        f"/organizations/{org_id}",
        token,
        {
            "profile_settings": {
                "enabled": True,
                "description": "Sub-millisecond CI/CD Cost Breakdown, PR Cost Diff Action & 8-Tool MCP FinOps Engine.",
            }
        },
    )
    print("Enabled Polar public storefront -> https://polar.sh/minici")

    # 2. Ensure automatic `MINICI-...` License Key benefit exists
    benefit_id = ensure_license_key_benefit(org_id, token)

    # 3. Ensure $1 Starter, $10 Pro, and Custom Team Flex recurring products exist
    existing = api_call("GET", f"/products/?organization_id={org_id}", token).get("items") or []
    by_name = {p.get("name"): p for p in existing}

    tiers = [
        {
            "slug_env": "POLAR_CHECKOUT_STARTER_URL",
            "plan_tag": "starter",
            "payload": {
                "name": "MiniCI Starter",
                "description": (
                    "Cloud Sync for up to 3 repos: unlisted `minici share` permalink reports (/r/<id>), "
                    "30-day cross-run CI cost trend ledger, and live README/PR SVG badges."
                ),
                "recurring_interval": "month",
                "prices": [{"amount_type": "fixed", "price_amount": 100, "price_currency": "usd"}],
                "metadata": {"minici_plan": "starter", "max_repos": 3, "retention_days": 30},
            },
        },
        {
            "slug_env": "POLAR_CHECKOUT_PRO_URL",
            "plan_tag": "pro",
            "payload": {
                "name": "MiniCI Pro",
                "description": (
                    "Automated Team FinOps for up to 15 private repos: scheduled Friday Slack FinOps digest, "
                    "real-time 1.5x burn-velocity anomaly alerts, and 365-day multi-repo team dashboard."
                ),
                "recurring_interval": "month",
                "prices": [{"amount_type": "fixed", "price_amount": 1000, "price_currency": "usd"}],
                "metadata": {"minici_plan": "pro", "max_repos": 15, "retention_days": 365},
            },
        },
        {
            "slug_env": "POLAR_CHECKOUT_CUSTOM_URL",
            "plan_tag": "custom",
            "payload": {
                "name": "MiniCI Team Flex",
                "description": (
                    "Pay-what-you-want Org Governance (unlimited repos): central PR budget gate policy, "
                    "synced custom enterprise runner rate cards across CLI/MCP, and priority support."
                ),
                "recurring_interval": "month",
                "prices": [
                    {
                        "amount_type": "custom",
                        "price_currency": "usd",
                        "minimum_amount": 2500,
                        "preset_amount": 4900,
                    }
                ],
                "metadata": {"minici_plan": "custom", "max_repos": 9999, "retention_days": 365},
            },
        },
    ]

    checkout_urls: dict[str, str] = {}
    for tier in tiers:
        body = tier["payload"]
        prod = by_name.get(body["name"])
        if prod is None:
            prod = api_call("POST", "/products/", token, {**body, "organization_id": org_id})
            print(f"Created Polar product: {prod.get('name')} (id={prod.get('id')})")
        else:
            print(f"Product already exists: {prod.get('name')} (id={prod.get('id')})")

        prod_id = prod["id"]
        api_call("POST", f"/products/{prod_id}/benefits", token, {"benefits": [benefit_id]})

        try:
            cl = api_call(
                "POST",
                "/checkout-links/",
                token,
                {"product_id": prod_id, "label": f"minici-{tier['plan_tag']}"},
            )
            if cl.get("url"):
                checkout_urls[tier["slug_env"]] = str(cl["url"])
        except urllib.error.HTTPError:
            pass

    print("\nBind these checkout URLs in Cloudflare Worker secrets (or visit https://polar.sh/minici):")
    for k, url in checkout_urls.items():
        print(f"  {k}={url}")
    print("Done! Verify live storefront: curl -sI https://polar.sh/minici")


if __name__ == "__main__":
    try:
        main()
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        print(f"Polar API HTTP {exc.code}: {body}", file=sys.stderr)
        sys.exit(1)
