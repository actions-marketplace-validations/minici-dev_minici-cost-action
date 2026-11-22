#!/usr/bin/env python3
"""
Declarative Cloud Runner Catalog Seed & Live Infracost Sync (`tools/sync_infracost_cloud_catalog.py`)

Generates `pricing/cloud_instances_seed.json` containing 150+ self-hosted CI runner VM flavors
across AWS EC2 (x86_64, Graviton3/4 ARM64, GPU G5/G4dn, Dedicated Mac M1/M2), GCP Compute Engine
(E2, N2, C3, T2A Tau ARM64), and Azure Virtual Machines (Dv5, Dpsv5 Ampere ARM64, Fsv2) with
regional multipliers across 12 major cloud regions.

When `--live` and `INFRACOST_API_KEY` (or `~/.config/infracost/credentials.yml`) are available,
queries `https://pricing.api.infracost.io/graphql` to refresh live On-Demand and Spot hourly prices.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
OUT_SEED_JSON = REPO_ROOT / "pricing" / "cloud_instances_seed.json"

REGIONAL_MULTIPLIERS: dict[str, dict[str, float]] = {
    "us-east-1": {"compute_mult": 1.000, "nat_per_gb_usd": 0.045, "egress_per_gb_usd": 0.090},
    "us-east-2": {"compute_mult": 1.000, "nat_per_gb_usd": 0.045, "egress_per_gb_usd": 0.090},
    "us-west-2": {"compute_mult": 1.000, "nat_per_gb_usd": 0.045, "egress_per_gb_usd": 0.090},
    "us-west-1": {"compute_mult": 1.118, "nat_per_gb_usd": 0.048, "egress_per_gb_usd": 0.090},
    "eu-west-1": {"compute_mult": 1.072, "nat_per_gb_usd": 0.048, "egress_per_gb_usd": 0.090},
    "eu-west-2": {"compute_mult": 1.115, "nat_per_gb_usd": 0.050, "egress_per_gb_usd": 0.090},
    "eu-central-1": {"compute_mult": 1.148, "nat_per_gb_usd": 0.052, "egress_per_gb_usd": 0.090},
    "ap-south-1": {"compute_mult": 0.945, "nat_per_gb_usd": 0.056, "egress_per_gb_usd": 0.109},
    "ap-southeast-1": {"compute_mult": 1.160, "nat_per_gb_usd": 0.059, "egress_per_gb_usd": 0.120},
    "ap-northeast-1": {"compute_mult": 1.242, "nat_per_gb_usd": 0.062, "egress_per_gb_usd": 0.114},
    "ca-central-1": {"compute_mult": 1.065, "nat_per_gb_usd": 0.048, "egress_per_gb_usd": 0.090},
    "sa-east-1": {"compute_mult": 1.540, "nat_per_gb_usd": 0.093, "egress_per_gb_usd": 0.150},
}

# Family base rates per vCPU-hour in us-east-1 (Linux Shared On-Demand & Spot)
AWS_FAMILIES = [
    ("t3", "x86_64", 0.0208, 0.0062, 2.0, [("small", 2), ("medium", 2), ("large", 2), ("xlarge", 4), ("2xlarge", 8)]),
    ("t3a", "x86_64", 0.0188, 0.0056, 2.0, [("medium", 2), ("large", 2), ("xlarge", 4), ("2xlarge", 8)]),
    ("t4g", "arm64", 0.0168, 0.0050, 2.0, [("small", 2), ("medium", 2), ("large", 2), ("xlarge", 4), ("2xlarge", 8)]),
    ("c6i", "x86_64", 0.0425, 0.0145, 2.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("12xlarge", 48), ("16xlarge", 64), ("24xlarge", 96)]),
    ("c6a", "x86_64", 0.03825, 0.0130, 2.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("16xlarge", 64)]),
    ("c7i", "x86_64", 0.0446, 0.0152, 2.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("16xlarge", 64)]),
    ("c7a", "x86_64", 0.0513, 0.0170, 2.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("16xlarge", 64)]),
    ("c7g", "arm64", 0.0361, 0.0125, 2.0, [("medium", 1), ("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("12xlarge", 48), ("16xlarge", 64)]),
    ("c8g", "arm64", 0.0397, 0.0138, 2.0, [("medium", 1), ("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("16xlarge", 64)]),
    ("m6i", "x86_64", 0.0480, 0.0165, 4.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32), ("16xlarge", 64)]),
    ("m7i", "x86_64", 0.0504, 0.0175, 4.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32)]),
    ("m7g", "arm64", 0.0408, 0.0140, 4.0, [("medium", 1), ("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16), ("8xlarge", 32)]),
    ("r7g", "arm64", 0.0536, 0.0182, 8.0, [("large", 2), ("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16)]),
    ("g4dn", "x86_64", 0.1315, 0.0395, 4.0, [("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16)]),
    ("g5", "x86_64", 0.2515, 0.0880, 4.0, [("xlarge", 4), ("2xlarge", 8), ("4xlarge", 16)]),
]

GCP_FAMILIES = [
    ("e2-standard", "x86_64", 0.0335, 0.0100, 4.0, [2, 4, 8, 16, 32]),
    ("e2-highcpu", "x86_64", 0.0247, 0.0074, 1.0, [2, 4, 8, 16, 32]),
    ("n2-standard", "x86_64", 0.0485, 0.0146, 4.0, [2, 4, 8, 16, 32, 48, 64]),
    ("n2d-standard", "x86_64", 0.0422, 0.0127, 4.0, [2, 4, 8, 16, 32, 64]),
    ("c3-standard", "x86_64", 0.0528, 0.0158, 4.0, [4, 8, 22, 44]),
    ("t2a-standard", "arm64", 0.0385, 0.0115, 4.0, [1, 2, 4, 8, 16, 32, 48]),
    ("c4a-standard", "arm64", 0.0410, 0.0123, 4.0, [2, 4, 8, 16, 32]),
]

AZURE_FAMILIES = [
    ("Standard_D{n}s_v5", "x86_64", 0.0480, 0.0144, 4.0, [2, 4, 8, 16, 32, 48, 64]),
    ("Standard_D{n}as_v5", "x86_64", 0.0432, 0.0130, 4.0, [2, 4, 8, 16, 32, 64]),
    ("Standard_D{n}ps_v5", "arm64", 0.0384, 0.0115, 4.0, [2, 4, 8, 16, 32, 64]),
    ("Standard_F{n}s_v2", "x86_64", 0.0423, 0.0127, 2.0, [2, 4, 8, 16, 32, 64]),
]


import shutil
import subprocess
import tempfile


def discover_infracost_api_key() -> str | None:
    if os.environ.get("INFRACOST_API_KEY"):
        return os.environ["INFRACOST_API_KEY"].strip()
    for cred in (
        Path.home() / ".config" / "infracost" / "credentials.yml",
        Path.home() / "Library" / "Application Support" / "infracost" / "credentials.yml",
    ):
        if cred.is_file():
            m = re.search(r"api_key:\s*([^\s#]+)", cred.read_text())
            if m:
                return m.group(1).strip().strip('"').strip("'")
    return None


def batch_fetch_via_infracost_v2_cli(ec2_types: list[str]) -> dict[str, float]:
    bin_path = shutil.which("infracost") or ("/opt/homebrew/bin/infracost" if Path("/opt/homebrew/bin/infracost").is_file() else None)
    if not bin_path or not ec2_types:
        return {}
    ver_out = subprocess.run([bin_path, "--version"], capture_output=True, text=True)
    ver_txt = f"{ver_out.stdout} {ver_out.stderr}".lower()
    is_legacy_v1 = any(tag in ver_txt for tag in ("v0.", " 0.", "v1.", " 1."))
    out_map: dict[str, float] = {}
    with tempfile.TemporaryDirectory() as td:
        hcl = ['provider "aws" {\n  region = "us-east-1"\n}\n']
        for i, itype in enumerate(ec2_types):
            hcl.append(f'resource "aws_instance" "vm_{i}" {{\n  ami           = "ami-0c55b159cbfafe1f0"\n  instance_type = "{itype}"\n}}\n')
        Path(td, "main.tf").write_text("\n".join(hcl))
        cmd = (
            [bin_path, "breakdown", "--path", td, "--format", "json", "--no-color"]
            if is_legacy_v1
            else [bin_path, "scan", td, "--json", "--no-color"]
        )
        res = subprocess.run(cmd, capture_output=True, text=True)
        raw = res.stdout
        idx = raw.find("{")
        if idx >= 0:
            end = raw.rfind("}")
            if end > idx:
                try:
                    data = json.loads(raw[idx : end + 1])
                    for proj in data.get("projects", []):
                        resources = proj.get("resources") or (proj.get("breakdown") or {}).get("resources") or []
                        for r in resources:
                            name = r.get("name", "")
                            if name.startswith("aws_instance.vm_"):
                                r_idx = int(name.split("vm_", 1)[1])
                                comps = r.get("cost_components") or r.get("costComponents") or []
                                for comp in comps:
                                    if comp.get("unit") == "hours" and float(comp.get("price") or 0) > 0:
                                        out_map[ec2_types[r_idx]] = round(float(comp["price"]), 5)
                                        break
                except Exception:
                    pass
    return out_map


def fetch_live_infracost_ec2_quote(api_key: str, instance_type: str, region: str = "us-east-1") -> tuple[float, float] | None:
    query = {
        "query": (
            f'{{ products(filter: {{ vendorName: "aws", service: "AmazonEC2", region: "{region}", '
            f'attributeFilters: [{{ key: "instanceType", value: "{instance_type}" }}, '
            f'{{ key: "operatingSystem", value: "Linux" }}, {{ key: "tenancy", value: "Shared" }}, '
            f'{{ key: "capacitystatus", value: "Used" }}] }}) {{ prices {{ USD purchaseOption }} }} }}'
        )
    }
    req = urllib.request.Request(
        "https://pricing.api.infracost.io/graphql",
        data=json.dumps(query).encode("utf-8"),
        headers={"Content-Type": "application/json", "X-Api-Key": api_key},
    )
    try:
        with urllib.request.urlopen(req, timeout=4) as resp:
            data = json.loads(resp.read().decode("utf-8"))
        od = None
        sp = None
        for prod in data.get("data", {}).get("products", []):
            for pr in prod.get("prices", []):
                val = float(pr.get("USD") or 0.0)
                if val <= 0.0:
                    continue
                po = str(pr.get("purchaseOption", "")).lower()
                if "on_demand" in po or "ondemand" in po:
                    od = val if od is None else min(od, val)
                elif "spot" in po:
                    sp = val if sp is None else min(sp, val)
        if od:
            return (round(od, 5), round(sp or (od * 0.32), 5))
    except Exception:
        return None
    return None


def build_seed_catalog(live: bool = False) -> dict:
    api_key = discover_infracost_api_key() if live else None
    v2_cli_quotes: dict[str, float] = {}
    if live and not api_key:
        all_ec2 = [f"{fam}.{sz_name}" for fam, _, _, _, _, sizes in AWS_FAMILIES for sz_name, _ in sizes]
        v2_cli_quotes = batch_fetch_via_infracost_v2_cli(all_ec2)
    instances: list[dict] = []
    live_hits = 0

    for fam, arch, od_vcpu_hr, sp_vcpu_hr, ram_per_vcpu, sizes in AWS_FAMILIES:
        for sz_name, vcpus in sizes:
            itype = f"{fam}.{sz_name}"
            ram = 2.0 if itype.endswith(".small") else (4.0 if itype.endswith(".medium") and fam.startswith("t") else vcpus * ram_per_vcpu)
            od_hr = round(vcpus * od_vcpu_hr, 5)
            sp_hr = round(vcpus * sp_vcpu_hr, 5)
            if itype == "t3.small":
                od_hr, sp_hr = 0.0208, 0.0062
            elif itype == "t3.medium":
                od_hr, sp_hr = 0.0416, 0.0125
            if api_key:
                live_q = fetch_live_infracost_ec2_quote(api_key, itype, "us-east-1")
                if live_q:
                    od_hr, sp_hr = live_q
                    live_hits += 1
            elif itype in v2_cli_quotes:
                spot_ratio = (sp_hr / od_hr) if od_hr > 0 else 0.32
                od_hr = v2_cli_quotes[itype]
                sp_hr = round(od_hr * spot_ratio, 5)
                live_hits += 1
            instances.append({
                "provider": "self-hosted-aws",
                "instance_type": itype,
                "os": "linux",
                "arch": arch,
                "vcpus": vcpus,
                "ram_gb": ram,
                "on_demand_hr_usd": od_hr,
                "spot_hr_usd": sp_hr,
                "on_demand_per_min_usd": round(od_hr / 60.0, 5),
                "spot_per_min_usd": round(sp_hr / 60.0, 5),
            })

    # Dedicated AWS EC2 Mac hosts (24-hour minimum allocation floor!)
    for mac_it, arch, vcpus, ram, od_hr in [
        ("mac1.metal", "x86_64", 12, 32.0, 1.083),
        ("mac2.metal", "arm64", 8, 16.0, 0.650),
        ("mac2-m2.metal", "arm64", 8, 24.0, 0.798),
        ("mac2-m2pro.metal", "arm64", 12, 32.0, 0.9125),
    ]:
        instances.append({
            "provider": "self-hosted-aws",
            "instance_type": mac_it,
            "os": "macos",
            "arch": arch,
            "vcpus": vcpus,
            "ram_gb": ram,
            "on_demand_hr_usd": od_hr,
            "spot_hr_usd": od_hr,
            "on_demand_per_min_usd": round(od_hr / 60.0, 5),
            "spot_per_min_usd": round(od_hr / 60.0, 5),
            "min_allocation_hours": 24,
        })

    for fam, arch, od_vcpu_hr, sp_vcpu_hr, ram_per_vcpu, core_list in GCP_FAMILIES:
        for vcpus in core_list:
            itype = f"{fam}-{vcpus}"
            od_hr = round(vcpus * od_vcpu_hr, 5)
            sp_hr = round(vcpus * sp_vcpu_hr, 5)
            instances.append({
                "provider": "self-hosted-gcp",
                "instance_type": itype,
                "os": "linux",
                "arch": arch,
                "vcpus": vcpus,
                "ram_gb": vcpus * ram_per_vcpu,
                "on_demand_hr_usd": od_hr,
                "spot_hr_usd": sp_hr,
                "on_demand_per_min_usd": round(od_hr / 60.0, 5),
                "spot_per_min_usd": round(sp_hr / 60.0, 5),
            })

    for pat, arch, od_vcpu_hr, sp_vcpu_hr, ram_per_vcpu, core_list in AZURE_FAMILIES:
        for vcpus in core_list:
            itype = pat.format(n=vcpus)
            od_hr = round(vcpus * od_vcpu_hr, 5)
            sp_hr = round(vcpus * sp_vcpu_hr, 5)
            instances.append({
                "provider": "self-hosted-azure",
                "instance_type": itype,
                "os": "linux",
                "arch": arch,
                "vcpus": vcpus,
                "ram_gb": vcpus * ram_per_vcpu,
                "on_demand_hr_usd": od_hr,
                "spot_hr_usd": sp_hr,
                "on_demand_per_min_usd": round(od_hr / 60.0, 5),
                "spot_per_min_usd": round(sp_hr / 60.0, 5),
            })

    return {
        "schema_version": "1.0",
        "source": "live_cloud_pricing_sync" if live_hits > 0 else "seeded_cloud_runner_catalog",
        "live_api_hits": live_hits,
        "total_instances": len(instances),
        "total_regional_price_points": len(instances) * len(REGIONAL_MULTIPLIERS),
        "regions": REGIONAL_MULTIPLIERS,
        "instances": instances,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Sync 150+ Cloud Runner VM & Spot SKUs across 12 regions")
    parser.add_argument("--live", action="store_true", help="Query upstream GraphQL API if credentials present")
    parser.add_argument("--out", default=str(OUT_SEED_JSON))
    args = parser.parse_args()

    payload = build_seed_catalog(live=args.live)
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, indent=2) + "\n")
    print(
        f"✅ Synced {payload['total_instances']} Cloud VM flavors x {len(payload['regions'])} regions "
        f"({payload['total_regional_price_points']} regional price points) -> {out_path}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
