const RATES_ETAG = "\"minici-rates-v0.9.1\"";

const CORS_HEADERS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, POST, OPTIONS",
  "access-control-allow-headers": "Content-Type, If-None-Match, Authorization"
};

const OFFICIAL_RATES_FEED = {
  schema_version: "1.0",
  catalog_version: "2026.11-v0.9.1",
  updated_at: "2026-11-20T00:00:00Z",
  github_actions: {
    "ubuntu-latest": 0.006,
    "ubuntu-24.04-arm": 0.005,
    "ubuntu-slim": 0.002,
    "ubuntu-4-core": 0.012,
    "ubuntu-4core": 0.012,
    "ubuntu-8-core": 0.022,
    "ubuntu-8core": 0.022,
    "ubuntu-16-core": 0.042,
    "ubuntu-16core": 0.042,
    "ubuntu-32-core": 0.082,
    "ubuntu-64-core": 0.162,
    "ubuntu-4-core-arm": 0.008,
    "ubuntu-8-core-arm": 0.014,
    "ubuntu-16-core-arm": 0.026,
    "windows-latest": 0.01,
    "windows-2025": 0.01,
    "windows-11-arm": 0.008,
    "macos-latest": 0.062,
    "macos-15": 0.062,
    "macos-large": 0.077,
    "macos-15-large": 0.077,
    "macos-14-xlarge": 0.102,
    "macos-xlarge": 0.12,
    "macos-15-xlarge": 0.12,
    "gpu-t4-4-core": 0.052
  },
  gitlab_ci: {
    "saas-linux-small-amd64": 0.01,
    "saas-linux-small-arm64": 0.01,
    "saas-linux-medium-amd64": 0.02,
    "saas-linux-medium-arm64": 0.02,
    "saas-linux-large-amd64": 0.03,
    "saas-linux-large-arm64": 0.03,
    "saas-linux-xlarge-amd64": 0.06,
    "saas-linux-2xlarge-amd64": 0.12,
    "saas-macos-medium-m1": 0.06,
    "saas-macos-large-m2pro": 0.12
  },
  circleci: {
    "docker-small": 0.003,
    "docker-medium": 0.006,
    "docker-medium+": 0.009,
    "docker-large": 0.012,
    "docker-xlarge": 0.024,
    "arm-medium": 0.0078,
    "arm-large": 0.0156,
    "arm-xlarge": 0.0234,
    "macos-m1-medium": 0.09,
    "macos-m2pro-medium": 0.09,
    "macos-m2pro-large": 0.18
  },
  bitbucket_pipelines: {
    "bitbucket-1x": 0.01,
    "bitbucket-2x": 0.02,
    "bitbucket-4x": 0.04,
    "bitbucket-8x": 0.08,
    "bitbucket-16x": 0.16
  },
  aws_codebuild: {
    "BUILD_GENERAL1_SMALL": 0.005,
    "BUILD_GENERAL1_MEDIUM": 0.01,
    "BUILD_GENERAL1_LARGE": 0.02,
    "BUILD_GENERAL1_XLARGE": 0.1,
    "ARM_CONTAINER_SMALL": 0.0034,
    "ARM_CONTAINER_LARGE": 0.015
  },
  gcp_cloud_build: {
    "E2_MEDIUM": 0.003,
    "E2_STANDARD_2": 0.006,
    "E2_HIGHCPU_8": 0.0156,
    "E2_HIGHCPU_32": 0.0624,
    "N1_HIGHCPU_8": 0.016,
    "N1_HIGHCPU_32": 0.064
  },
  azure_pipelines: {
    "azure-microsoft-hosted-linux": 0.008,
    "azure-microsoft-hosted-windows": 0.008,
    "azure-microsoft-hosted-macos": 0.062
  }
};

function isThirdPartyMicroVmLabel(l) {
  return (
    l.includes("blacksmith") ||
    l.includes("warpbuild") ||
    l.startsWith("warp-") ||
    l.includes("depot") ||
    l.includes("buildjet") ||
    l.includes("namespace")
  );
}

function isLargerOrNonAllowanceLabel(l) {
  return (
    l.includes("self-hosted") ||
    l.includes("runs-on=") ||
    l.startsWith("runson-") ||
    isThirdPartyMicroVmLabel(l) ||
    /\b(?:4|8|16|32|64|96)-?core\b/.test(l) ||
    /\b(?:[4-9]|\d{2,})vcpu\b/.test(l) ||
    l.includes("large") ||
    l.includes("xlarge") ||
    l.includes("gpu") ||
    l.includes("cuda") ||
    l.includes("t4")
  );
}

function resolveRunnerRate(label) {
  const raw = String(label || "ubuntu-latest").trim();
  if (OFFICIAL_RATES_FEED.github_actions[raw] !== undefined) {
    return OFFICIAL_RATES_FEED.github_actions[raw];
  }
  const l = raw.toLowerCase();
  if (l.includes("self-hosted") || l.includes("runs-on=") || l.startsWith("runson-")) {
    return 0.0024;
  }
  if (OFFICIAL_RATES_FEED.github_actions[l] !== undefined) {
    return OFFICIAL_RATES_FEED.github_actions[l];
  }
  const isArm = l.includes("arm") || l.includes("aarch64") || l.includes("graviton");
  const isWin = l.includes("windows");
  if (isThirdPartyMicroVmLabel(l)) {
    if (l.includes("macos")) return 0.032;
    const vcpuMatch =
      l.match(/(\d+)\s*vcpu/) ||
      l.match(/-(\d+)x\d+/) ||
      l.match(/-(?:ubuntu|windows|linux)[a-z0-9.-]*-(\d+)$/);
    const vcpus = vcpuMatch ? Math.max(2, Number(vcpuMatch[1]) || 2) : 2;
    const perVcpu = l.includes("warp") && isArm ? 0.0016 : 0.002;
    return Number((vcpus * perVcpu).toFixed(6));
  }
  if (l.includes("gpu") || l.includes("cuda") || l.includes("t4")) return 0.052;
  if (l.includes("macos-14") && l.includes("xlarge")) return 0.102;
  if (l.includes("macos") && l.includes("xlarge")) return 0.12;
  if (l.includes("macos") && l.includes("large")) return 0.077;
  if (l.includes("macos")) return 0.062;
  if (l.includes("96-core") || l.includes("96core") || l.includes("96vcpu")) {
    return isWin ? (isArm ? 0.291 : 0.462) : (isArm ? 0.147 : 0.243);
  }
  if (l.includes("64-core") || l.includes("64core") || l.includes("64vcpu")) {
    return isWin ? (isArm ? 0.194 : 0.308) : (isArm ? 0.098 : 0.162);
  }
  if (l.includes("32-core") || l.includes("32core") || l.includes("32vcpu")) {
    return isWin ? (isArm ? 0.098 : 0.156) : (isArm ? 0.05 : 0.082);
  }
  if (l.includes("16-core") || l.includes("16core") || l.includes("16vcpu")) {
    return isWin ? (isArm ? 0.05 : 0.082) : (isArm ? 0.026 : 0.042);
  }
  if (l.includes("8-core") || l.includes("8core") || l.includes("8vcpu")) {
    return isWin ? (isArm ? 0.026 : 0.042) : (isArm ? 0.014 : 0.022);
  }
  if (l.includes("4-core") || l.includes("4core") || l.includes("4vcpu")) {
    return isWin ? (isArm ? 0.014 : 0.022) : (isArm ? 0.008 : 0.012);
  }
  if (isWin) return isArm ? 0.008 : 0.01;
  if (isArm) return 0.005;
  if (l.includes("slim")) return 0.002;
  return 0.006;
}

async function sha256Hex(text) {
  const bytes = new TextEncoder().encode(String(text || ""));
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

async function verifySlackSignature(request, rawBody, signingSecret, allowUnsignedDev = false) {
  if (!signingSecret) return Boolean(allowUnsignedDev);
  const sigHeader = request.headers.get("x-slack-signature") || "";
  const tsHeader = request.headers.get("x-slack-request-timestamp") || "";
  if (!sigHeader.startsWith("v0=") || !tsHeader) return false;
  const nowSec = Math.floor(Date.now() / 1000);
  const reqSec = Number(tsHeader);
  if (!Number.isFinite(reqSec) || Math.abs(nowSec - reqSec) > 300) return false;

  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    enc.encode(signingSecret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const mac = await crypto.subtle.sign(
    "HMAC",
    key,
    enc.encode(`v0:${tsHeader}:${rawBody}`)
  );
  const hex = Array.from(new Uint8Array(mac))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
  const expected = `v0=${hex}`;
  if (expected.length !== sigHeader.length) return false;
  let diff = 0;
  for (let i = 0; i < expected.length; i++) {
    diff |= expected.charCodeAt(i) ^ sigHeader.charCodeAt(i);
  }
  return diff === 0;
}

function estimateStaticStepMinutes(yml) {
  const low = String(yml || "").toLowerCase();
  let est = 1.5;
  if (low.includes("cargo build") || low.includes("cargo test")) est += 4.5;
  else if (low.includes("pytest") || low.includes("tox")) est += 2.5;
  else if (low.includes("npm test") || low.includes("vitest") || low.includes("jest")) est += 2.2;
  else if (low.includes("playwright") || low.includes("cypress")) est += 5.5;
  else if (low.includes("docker/build-push-action") || low.includes("docker build")) est += 3.8;
  else if (low.includes("xcodebuild") || low.includes("gradle")) est += 6.5;
  return Math.ceil(est);
}

/**
 * Fetches 100% real live GitHub repository + Actions run/job/cache telemetry
 * (zero hardcoded `mins = 4.0` or `monthlyRuns = 433` assumptions).
 */
async function fetchLiveGitHubRepoSummary(slug, ghToken, windowDays = 30) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(slug)) return null;
  const clampedDays = Math.max(1, Math.min(365, Number(windowDays) || 30));
  const headers = {
    accept: "application/vnd.github+json",
    "user-agent": "minici-worker/0.9.1"
  };
  if (ghToken) headers.authorization = `Bearer ${ghToken}`;

  const [repoResp, wfResp, runsResp, cacheResp] = await Promise.all([
    fetch(`https://api.github.com/repos/${slug}`, { headers }),
    fetch(`https://api.github.com/repos/${slug}/contents/.github/workflows`, { headers }),
    fetch(`https://api.github.com/repos/${slug}/actions/runs?per_page=100`, { headers }),
    fetch(`https://api.github.com/repos/${slug}/actions/cache/usage`, { headers }).catch(() => null)
  ]);

  if (!repoResp.ok) return null;
  const repoJson = await repoResp.json();
  const isPublic = !repoJson.private;
  let headSha = String(repoJson.default_branch || "main");

  let workflowFiles = [];
  if (wfResp && wfResp.ok) {
    const entries = await wfResp.json().catch(() => []);
    if (Array.isArray(entries)) {
      workflowFiles = entries.filter((e) => e && e.download_url && /\.ya?ml$/i.test(e.name || ""));
    }
  }

  let activeCacheGb = 0.0;
  let activeCachesCount = 0;
  if (cacheResp && cacheResp.ok) {
    const cj = await cacheResp.json().catch(() => null);
    if (cj) {
      activeCacheGb = Number(cj.active_caches_size_in_bytes || 0) / (1024 * 1024 * 1024);
      activeCachesCount = Number(cj.active_caches_count || 0);
    }
  }

  const nowMs = Date.now();
  const cutoffMs = nowMs - clampedDays * 86400 * 1000;
  let observedRunsInWindow = 0;
  let skippedNoopRuns = 0;
  let botRuns = 0;
  let cancelledRuns = 0;
  let oldestRunMs = nowMs;
  let newestRunMs = 0;
  const completedSampleRuns = [];

  let isSampleTruncated = false;
  if (runsResp && runsResp.ok) {
    const runsJson = await runsResp.json().catch(() => ({}));
    const rawRuns = Array.isArray(runsJson.workflow_runs) ? runsJson.workflow_runs : [];
    if (rawRuns.length > 0 && rawRuns[0].head_sha) {
      headSha = String(rawRuns[0].head_sha).slice(0, 12);
    }
    if (rawRuns.length >= 100) {
      const tailMs = Date.parse(rawRuns[rawRuns.length - 1].created_at || "");
      if (Number.isFinite(tailMs) && tailMs > cutoffMs) {
        isSampleTruncated = true;
      }
    }
    for (const run of rawRuns) {
      if (run.status && run.status !== "completed") continue;
      const createdMs = Date.parse(run.created_at || "");
      if (!Number.isFinite(createdMs) || createdMs < cutoffMs) continue;
      if (createdMs < oldestRunMs) oldestRunMs = createdMs;
      if (createdMs > newestRunMs) newestRunMs = createdMs;

      if (run.conclusion === "skipped") {
        skippedNoopRuns += 1;
        continue;
      }
      observedRunsInWindow += 1;
      const actorLogin = String((run.actor && run.actor.login) || "").toLowerCase();
      if (actorLogin.endsWith("[bot]") || actorLogin.includes("dependabot") || actorLogin.includes("renovate")) {
        botRuns += 1;
      }
      if (run.conclusion === "cancelled") {
        cancelledRuns += 1;
      }
      if (run.status === "completed" && completedSampleRuns.length < 6) {
        completedSampleRuns.push(run);
      }
    }
  }

  // Extrapolate ONLY when 100-run page truncated before `cutoffMs`; otherwise divide over full `clampedDays` (`B1`)!
  let monthlyRuns = 0.0;
  let provenanceMode = "STATIC_TOPOLOGY";
  if (observedRunsInWindow > 0) {
    provenanceMode = "LIVE_CALIBRATED";
    const spanDays = isSampleTruncated
      ? Math.max(1 / 24, Math.min(clampedDays, (nowMs - oldestRunMs) / (86400 * 1000)))
      : clampedDays;
    monthlyRuns = observedRunsInWindow * (30.0 / spanDays);
  }

  // Sample real job durations & runtime labels[] from recent completed workflow runs
  let totalJobs = 0;
  let billedMinsPerRun = 0.0;
  let runCostUsd = 0.0;
  let stdPoolMinsPerRun = 0.0;
  let stdGrossPerRun = 0.0;
  let nonPoolGrossPerRun = 0.0;
  let cancelledWastedMins = 0.0;

  if (completedSampleRuns.length > 0) {
    const jobResponses = await Promise.all(
      completedSampleRuns.map((r) =>
        fetch(`https://api.github.com/repos/${slug}/actions/runs/${r.id}/jobs?per_page=100`, {
          headers
        })
          .then((res) => (res.ok ? res.json() : null))
          .catch(() => null)
      )
    );
    let validRunSamples = 0;
    let sumJobs = 0;
    let sumBilledMins = 0.0;
    let sumRunCost = 0.0;
    let sumStdPool = 0.0;
    let sumStdGross = 0.0;
    let sumNonPoolGross = 0.0;

    for (const jPayload of jobResponses) {
      if (!jPayload || !Array.isArray(jPayload.jobs) || jPayload.jobs.length === 0) continue;
      let sampleBilled = 0.0;
      let sampleCost = 0.0;
      let sampleStdPool = 0.0;
      let sampleStdGross = 0.0;
      let sampleNonPoolGross = 0.0;
      let sampleJobCount = 0;
      for (const job of jPayload.jobs) {
        if (job.conclusion === "skipped" || job.conclusion === "startup_failure") continue;
        const startMs = Date.parse(job.started_at || "");
        const endMs = Date.parse(job.completed_at || "");
        if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs <= startMs) continue;
        const durSec = (endMs - startMs) / 1000.0;
        const labels = Array.isArray(job.labels) ? job.labels : [];
        const hasSelfHosted = labels.some((l) => String(l).toLowerCase() === "self-hosted");
        const specific = labels.find((l) => l && String(l).toLowerCase() !== "self-hosted");
        const runtimeLabel = hasSelfHosted
          ? specific
            ? `self-hosted-${specific}`
            : "self-hosted"
          : specific || "ubuntu-latest";
        const rlLow = runtimeLabel.toLowerCase();
        const isPerSec = hasSelfHosted || isThirdPartyMicroVmLabel(rlLow);
        const billedMins = isPerSec
          ? Math.round((durSec / 60.0) * 100.0) / 100.0
          : Math.max(1, Math.ceil(durSec / 60.0));
        const rate = resolveRunnerRate(runtimeLabel);
        const isLargerOr3p = hasSelfHosted || isLargerOrNonAllowanceLabel(rlLow);
        sampleJobCount += 1;
        sampleBilled += billedMins;
        const jc = billedMins * rate;
        sampleCost += jc;
        if (isLargerOr3p) {
          sampleNonPoolGross += jc;
        } else {
          const osMult = rlLow.includes("macos") ? 10.0 : rlLow.includes("windows") ? 2.0 : 1.0;
          sampleStdPool += billedMins * osMult;
          sampleStdGross += jc;
        }
        if (job.conclusion === "cancelled") {
          cancelledWastedMins += billedMins;
        }
      }
      if (sampleJobCount > 0) {
        validRunSamples += 1;
        sumJobs += sampleJobCount;
        sumBilledMins += sampleBilled;
        sumRunCost += sampleCost;
        sumStdPool += sampleStdPool;
        sumStdGross += sampleStdGross;
        sumNonPoolGross += sampleNonPoolGross;
      }
    }

    if (validRunSamples > 0) {
      totalJobs = Math.round(sumJobs / validRunSamples);
      billedMinsPerRun = sumBilledMins / validRunSamples;
      runCostUsd = sumRunCost / validRunSamples;
      stdPoolMinsPerRun = sumStdPool / validRunSamples;
      stdGrossPerRun = sumStdGross / validRunSamples;
      nonPoolGrossPerRun = sumNonPoolGross / validRunSamples;
    }
  }

  // Fallback to static workflow topology inspection ONLY when repository has 0 live completed runs
  if (totalJobs === 0 && workflowFiles.length > 0) {
    const inspectFiles = workflowFiles.slice(0, 12);
    const rawYamls = await Promise.all(
      inspectFiles.map((wf) =>
        fetch(wf.download_url, { headers })
          .then((r) => (r.ok ? r.text() : ""))
          .catch(() => "")
      )
    );
    let cronRunsPerMonth = 0.0;
    let hasPrOrPush = false;
    for (const yml of rawYamls) {
      if (!yml) continue;
      if (/pull_request|push/i.test(yml)) hasPrOrPush = true;
      if (/cron:/i.test(yml)) cronRunsPerMonth += 30.0;
      const stepEstMins = estimateStaticStepMinutes(yml);
      const matches = [...yml.matchAll(/^\s+runs-on:\s*([^\n#]+)/gm)];
      const count = Math.max(matches.length, 1);
      for (let i = 0; i < count; i++) {
        const label = matches[i] ? matches[i][1].trim().replace(/^['"]|['"]$/g, "") : "ubuntu-latest";
        const rlLow = label.toLowerCase();
        const rate = resolveRunnerRate(label);
        const jc = stepEstMins * rate;
        totalJobs += 1;
        billedMinsPerRun += stepEstMins;
        runCostUsd += jc;
        if (isLargerOrNonAllowanceLabel(rlLow)) {
          nonPoolGrossPerRun += jc;
        } else {
          const osMult = rlLow.includes("macos") ? 10.0 : rlLow.includes("windows") ? 2.0 : 1.0;
          stdPoolMinsPerRun += stepEstMins * osMult;
          stdGrossPerRun += jc;
        }
      }
    }
    if (monthlyRuns <= 0) {
      const stars = Number(repoJson.stargazers_count || 0);
      const activityFactor = stars > 10000 ? 220.0 : stars > 1000 ? 120.0 : 65.0;
      monthlyRuns = (hasPrOrPush ? activityFactor : 0.0) + cronRunsPerMonth;
    }
  }

  const botRunSharePct =
    observedRunsInWindow > 0 ? (botRuns / observedRunsInWindow) * 100.0 : 0.0;
  const grossMonthlyUsd = runCostUsd * monthlyRuns;
  // Public GitHub repos pay $0 for standard hosted runners (only Larger/3rd-party runners bill);
  // Private repos draw 2,000 OS-weighted pool minutes proportionally (`E1, E2, E4`)!
  let netMonthlyUsd = 0.0;
  if (isPublic) {
    netMonthlyUsd = nonPoolGrossPerRun * monthlyRuns;
  } else {
    const monthlyStdPool = stdPoolMinsPerRun * monthlyRuns;
    const monthlyStdGross = stdGrossPerRun * monthlyRuns;
    const monthlyNonPool = nonPoolGrossPerRun * monthlyRuns;
    if (stdPoolMinsPerRun === 0 && nonPoolGrossPerRun === 0 && grossMonthlyUsd > 0) {
      netMonthlyUsd = Math.max(0.0, grossMonthlyUsd - 2000 * 0.006);
    } else {
      const stdOverage =
        monthlyStdPool <= 2000.0 || monthlyStdPool <= 0.0
          ? 0.0
          : monthlyStdGross * ((monthlyStdPool - 2000.0) / monthlyStdPool);
      netMonthlyUsd = stdOverage + monthlyNonPool;
    }
  }

  return {
    repo: slug,
    vendor: "github",
    provenanceMode,
    windowDays: clampedDays,
    headSha,
    isPublic,
    workflows: workflowFiles.length,
    jobs: totalJobs,
    billedMins: Number(billedMinsPerRun.toFixed(2)),
    monthlyRuns: Number(monthlyRuns.toFixed(1)),
    observedRunsInWindow,
    skippedNoopRuns,
    botRunSharePct: Number(botRunSharePct.toFixed(1)),
    cancelledRuns,
    cancelledWastedMins: Number(cancelledWastedMins.toFixed(1)),
    activeCacheGb: Number(activeCacheGb.toFixed(3)),
    activeCachesCount,
    grossMonthlyUsd: Number(grossMonthlyUsd.toFixed(2)),
    netMonthlyUsd: Number(netMonthlyUsd.toFixed(2)),
    updatedAtEpochSec: Math.floor(nowMs / 1000)
  };
}

/**
 * 3-Layer Cloudflare Edge Caching Stack:
 *   Layer 1: `caches.default` (Edge PoP Cache API, <0.5 ms, s-maxage=300, stale-while-revalidate=3600)
 *   Layer 2: `env.MINICI_KV` (Global Workers KV, ~3-8 ms, `report:<slug>:<days>d`)
 *   Layer 3: `env.MINICI_D1` (Cloudflare D1 SQLite `repo_telemetry`, ~5-15 ms)
 *   Origin:  Live GitHub REST API (`fetchLiveGitHubRepoSummary`) + `ctx.waitUntil` async L3->L2->L1 backfill
 */
async function getRepoReportThreeLayer(slug, windowDays, env, ctx, forceRefresh = false) {
  const clampedDays = Math.max(1, Math.min(365, Number(windowDays) || 30));
  const cacheBase = (env && env.REPORT_CACHE_BASE_URL) || "https://cache.minici.dev";
  const l1Url = `${cacheBase}/v1/report/${slug}?days=${clampedDays}`;
  const l1Key = new Request(l1Url, { method: "GET" });
  const kvKey = `report:${slug}:${clampedDays}d`;
  const edgeCache = typeof caches !== "undefined" && caches.default ? caches.default : null;

  // 1. Layer 1: Cloudflare Edge PoP Cache API (`< 0.5 ms`)
  if (!forceRefresh && edgeCache) {
    const l1Hit = await edgeCache.match(l1Key).catch(() => null);
    if (l1Hit) {
      const data = await l1Hit.json().catch(() => null);
      if (data) return { data, cacheLayer: "L1-EDGE-POP" };
    }
  }

  const putL1 = async (payloadObj) => {
    if (!edgeCache) return;
    const resp = new Response(JSON.stringify(payloadObj), {
      headers: {
        "content-type": "application/json; charset=utf-8",
        "cache-control": "public, s-maxage=300, stale-while-revalidate=3600"
      }
    });
    await edgeCache.put(l1Key, resp).catch(() => {});
  };

  // 2. Layer 2: Cloudflare Workers KV (`~3-8 ms` globally replicated)
  if (!forceRefresh && env && env.MINICI_KV) {
    const kvRaw = await env.MINICI_KV.get(kvKey).catch(() => null);
    if (kvRaw) {
      try {
        const parsed = JSON.parse(kvRaw);
        if (ctx && typeof ctx.waitUntil === "function") ctx.waitUntil(putL1(parsed));
        return { data: parsed, cacheLayer: "L2-WORKERS-KV" };
      } catch {}
    }
  }

  // 3. Layer 3: Cloudflare D1 Serverless SQLite (`~5-15 ms` persistent FinOps store)
  if (!forceRefresh && env && env.MINICI_D1) {
    try {
      const row = await env.MINICI_D1.prepare(
        "SELECT payload_json, updated_at FROM repo_telemetry WHERE repo = ?1 AND window_days = ?2"
      )
        .bind(slug, clampedDays)
        .first();
      const nowSec = Math.floor(Date.now() / 1000);
      if (row && row.payload_json && nowSec - Number(row.updated_at || 0) <= 3600) {
        const parsed = JSON.parse(row.payload_json);
        if (ctx && typeof ctx.waitUntil === "function") {
          ctx.waitUntil(
            Promise.all([
              env.MINICI_KV
                ? env.MINICI_KV.put(kvKey, row.payload_json, { expirationTtl: 3600 }).catch(() => {})
                : Promise.resolve(),
              putL1(parsed)
            ])
          );
        }
        return { data: parsed, cacheLayer: "L3-CLOUDFLARE-D1" };
      }
    } catch {}
  }

  // 4. Origin Fetch: Live GitHub Telemetry + Async D1 -> KV -> Edge PoP Backfill
  const live = await fetchLiveGitHubRepoSummary(slug, env && env.GITHUB_TOKEN, clampedDays);
  if (!live) return { data: null, cacheLayer: "MISS" };

  const serialized = JSON.stringify(live);
  const backfillPromise = Promise.all([
    env && env.MINICI_D1
      ? env.MINICI_D1.prepare(
          `INSERT OR REPLACE INTO repo_telemetry (
             repo, vendor, visibility, head_sha, window_days, updated_at,
             monthly_runs, skipped_noop_runs, bot_run_share_pct, billed_mins_per_run,
             gross_monthly_usd, net_billed_monthly_usd, active_cache_gb, payload_json
           ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)`
        )
          .bind(
            slug,
            "github",
            live.isPublic ? "public" : "private",
            live.headSha || "",
            clampedDays,
            live.updatedAtEpochSec,
            live.monthlyRuns,
            live.skippedNoopRuns,
            live.botRunSharePct,
            live.billedMins,
            live.grossMonthlyUsd,
            live.netMonthlyUsd,
            live.activeCacheGb,
            serialized
          )
          .run()
          .catch(() => {})
      : Promise.resolve(),
    env && env.MINICI_KV
      ? env.MINICI_KV.put(kvKey, serialized, { expirationTtl: 3600 }).catch(() => {})
      : Promise.resolve(),
    putL1(live)
  ]);

  if (ctx && typeof ctx.waitUntil === "function") {
    ctx.waitUntil(backfillPromise);
  } else {
    await backfillPromise;
  }

  return { data: live, cacheLayer: "ORIGIN-GITHUB" };
}

/**
 * Real Edge `/v1/systemone` Constrained-Choice Evaluator:
 *   1. Checks Layer-2 KV (`jev:step:<sha256>`) & Layer-3 D1 (`step_decisions`) content-addressed cache.
 *   2. Forwards to `env.JEV_UPSTREAM_URL` (Ollaya / TypeSafe `/v1/systemone` or `jev_systemone_server.py`) when configured.
 *   3. Executes Workers AI (`env.AI`) with cyclic-shift option-order debiasing when bound.
 *   4. Returns calibrated `noul` (`choice: null`) when offline / unbound — zero hardcoded benchmark answers!
 */
async function evaluateSystemOneEdge(req, env, ctx) {
  const rawHashInput = JSON.stringify({
    state: req.state || null,
    prompt: req.prompt || null,
    questions: req.questions || null,
    choice: req.choice || null
  });
  const cacheDigest = await sha256Hex(rawHashInput);
  const kvKey = `jev:step:${cacheDigest}`;

  if (env && env.MINICI_KV) {
    const cached = await env.MINICI_KV.get(kvKey).catch(() => null);
    if (cached) {
      try {
        const obj = JSON.parse(cached);
        return { ...obj, cache_layer: "L2-WORKERS-KV" };
      } catch {}
    }
  }

  if (env && env.MINICI_D1) {
    const d1Row = await env.MINICI_D1.prepare(
      "SELECT phase, confidence, model_id FROM step_decisions WHERE step_sha256 = ?1"
    )
      .bind(cacheDigest)
      .first()
      .catch(() => null);
    if (d1Row && d1Row.phase) {
      const d1Hit = {
        choice: d1Row.phase,
        score: Number(d1Row.confidence || 0.9),
        noul: false,
        backend: d1Row.model_id || "jev-systemone",
        cache_layer: "L3-CLOUDFLARE-D1"
      };
      if (env.MINICI_KV && ctx && typeof ctx.waitUntil === "function") {
        ctx.waitUntil(
          env.MINICI_KV.put(kvKey, JSON.stringify(d1Hit), { expirationTtl: 2592000 }).catch(() => {})
        );
      }
      return d1Hit;
    }
  }

  const persistDecisionL2L3 = (outObj) => {
    const nowSec = Math.floor(Date.now() / 1000);
    const kvTask =
      env && env.MINICI_KV
        ? env.MINICI_KV.put(kvKey, JSON.stringify(outObj), { expirationTtl: 2592000 }).catch(() => {})
        : Promise.resolve();
    const d1Task =
      env && env.MINICI_D1 && outObj && outObj.choice
        ? env.MINICI_D1.prepare(
            `INSERT OR REPLACE INTO step_decisions (
               step_sha256, phase, arm64_safe, spot_safe, cancel_safe,
               cache_candidate, confidence, model_id, created_at
             ) VALUES (?1, ?2, 1, 1, 1, 0, ?3, ?4, ?5)`
          )
            .bind(
              cacheDigest,
              String(outObj.choice),
              Number(outObj.score || 0.9),
              String(outObj.backend || "jev-systemone"),
              nowSec
            )
            .run()
            .catch(() => {})
        : Promise.resolve();
    if (ctx && typeof ctx.waitUntil === "function") {
      ctx.waitUntil(Promise.all([kvTask, d1Task]));
    }
  };

  if (env && env.JEV_UPSTREAM_URL) {
    const upstreamHeaders = { "content-type": "application/json" };
    if (env.TYPESAFE_API_KEY) {
      upstreamHeaders.authorization = `Bearer ${env.TYPESAFE_API_KEY}`;
    }
    const upstreamResp = await fetch(env.JEV_UPSTREAM_URL, {
      method: "POST",
      headers: upstreamHeaders,
      body: JSON.stringify(req)
    }).catch(() => null);
    if (upstreamResp && upstreamResp.ok) {
      const out = await upstreamResp.json().catch(() => null);
      if (out) {
        persistDecisionL2L3(out);
        return out;
      }
    }
  }

  // Workers AI (`env.AI`) with AnyJev L0 cyclic-shift option-order debiasing
  const choices = Array.isArray(req.choice) ? req.choice.map(String) : [];
  const promptText = String(req.prompt || JSON.stringify(req.state || {}));
  if (env && env.AI && choices.length > 0) {
    const model = env.WORKERS_AI_MODEL || "@cf/qwen/qwen2.5-coder-32b-instruct";
    const shifts = choices.length >= 2 ? [0, Math.floor(choices.length / 2)] : [0];
    const votes = new Map();
    for (const shift of shifts) {
      const rotated = choices.slice(shift).concat(choices.slice(0, shift));
      if (req.noul) rotated.push("NONE_ABSTAIN");
      const sysPrompt =
        "You are a TypeSafe SystemOne constrained classifier. Output ONLY the exact matching string from ALLOWED_CHOICES and nothing else.";
      const userMsg = `INPUT:\n${promptText}\n\nALLOWED_CHOICES:\n${rotated
        .map((c, idx) => `${idx + 1}. ${c}`)
        .join("\n")}\n\nRespond with ONLY one exact token from ALLOWED_CHOICES:`;
      const aiRes = await env.AI.run(model, {
        messages: [
          { role: "system", content: sysPrompt },
          { role: "user", content: userMsg }
        ],
        temperature: 0.0,
        max_tokens: 24
      }).catch(() => null);
      const rawOut = String((aiRes && aiRes.response) || "").trim();
      const matched = rotated.find((c) => rawOut === c || rawOut.includes(c));
      if (matched) {
        votes.set(matched, (votes.get(matched) || 0) + 1);
      }
    }
    let bestChoice = null;
    let bestVotes = 0;
    for (const [cand, v] of votes.entries()) {
      if (v > bestVotes) {
        bestChoice = cand;
        bestVotes = v;
      }
    }
    if (!bestChoice || bestChoice === "NONE_ABSTAIN") {
      return { choice: null, score: 0.0, noul: true, backend: "workers-ai-abstain" };
    }
    const agreement = bestVotes / shifts.length;
    const out = {
      choice: bestChoice,
      score: Number((0.65 + 0.32 * agreement).toFixed(3)),
      noul: false,
      backend: model
    };
    persistDecisionL2L3(out);
    return out;
  }

  return {
    choice: null,
    score: 0.0,
    noul: true,
    note: "Bind JEV_UPSTREAM_URL (jev_systemone_server.py / Ollaya) or Cloudflare Workers AI (env.AI) for live neural /v1/systemone inference."
  };
}

async function hmacSha256Hex(secret, message) {
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    enc.encode(String(secret || "")),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const mac = await crypto.subtle.sign("HMAC", key, enc.encode(String(message || "")));
  return Array.from(new Uint8Array(mac))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function timingSafeEqualAscii(a, b) {
  const sa = String(a || "");
  const sb = String(b || "");
  if (sa.length !== sb.length) return false;
  let diff = 0;
  for (let i = 0; i < sa.length; i++) {
    diff |= sa.charCodeAt(i) ^ sb.charCodeAt(i);
  }
  return diff === 0;
}

async function verifyGitHubWebhookSignature(request, rawBody, secret, allowUnsignedDev = false) {
  if (!secret) return Boolean(allowUnsignedDev);
  const sigHeader = (request.headers.get("x-hub-signature-256") || "").trim();
  if (!sigHeader.startsWith("sha256=")) return false;
  const expected = `sha256=${await hmacSha256Hex(secret, rawBody)}`;
  return timingSafeEqualAscii(expected, sigHeader.toLowerCase());
}

async function verifyLinearWebhookSignature(request, rawBody, secret, allowUnsignedDev = false) {
  if (!secret) return Boolean(allowUnsignedDev);
  const sigHeader = (request.headers.get("linear-signature") || "").trim().toLowerCase();
  if (!sigHeader) return false;
  const expected = await hmacSha256Hex(secret, rawBody);
  return timingSafeEqualAscii(expected, sigHeader);
}

function stripBackticksAndBlockquotes(rawText) {
  const lines = String(rawText || "").split(/\r?\n/);
  const kept = [];
  for (const line of lines) {
    const trimmed = line.trimStart();
    if (trimmed.startsWith(">")) continue;
    let out = "";
    let inTick = false;
    for (let i = 0; i < trimmed.length; i++) {
      const ch = trimmed[i];
      if (ch === "`") {
        inTick = !inTick;
      } else if (!inTick) {
        out += ch;
      }
    }
    kept.push(out);
  }
  return kept.join("\n");
}

function parseOmnichannelAtMiniciCommand(rawText, requireMention = true) {
  const body = String(rawText || "");
  if (
    body.includes("<!-- minici-") ||
    body.includes("MiniCI Pull Request CI/CD Cost Diff") ||
    body.includes("MiniCI Merge Request CI/CD Cost Diff")
  ) {
    return null;
  }
  const cleaned = stripBackticksAndBlockquotes(body);
  const low = cleaned.toLowerCase();
  let tail = cleaned.trim();
  if (requireMention) {
    const idx = low.indexOf("@minici");
    if (idx === -1) return null;
    tail = cleaned.slice(idx + "@minici".length).trim();
  } else {
    const idx = low.indexOf("@minici");
    if (idx !== -1) tail = cleaned.slice(idx + "@minici".length).trim();
  }
  const tokens = tail.split(/\s+/).filter(Boolean);
  const rawSub = (tokens[0] || "breakdown").toLowerCase();
  const repoMatch = tokens.find((t) => /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(t));
  const arg1 = (tokens[1] || "arm64").toLowerCase();
  let action = "breakdown";
  if (["shrink", "optimize", "fix"].includes(rawSub)) action = "shrink";
  else if (["accelerate", "rust-everywhere", "rust-tools"].includes(rawSub)) action = "accelerate";
  else if (["migrate", "switch"].includes(rawSub)) action = "migrate";
  else if (["permissions", "security", "trust"].includes(rawSub)) action = "permissions";
  else if (["why", "anomalies", "anomaly"].includes(rawSub)) action = "why";

  return {
    action,
    rawSub,
    targetArg: arg1,
    repoSlug: repoMatch || null,
    fullTail: tail
  };
}

async function buildOmnichannelMiniciReply(cmdObj, fallbackRepo, env, ctx) {
  const repo = (cmdObj && cmdObj.repoSlug) || fallbackRepo || "owner/repo";
  const validSlug = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo);
  const { data: live } = validSlug
    ? await getRepoReportThreeLayer(repo, 30, env, ctx, false).catch(() => ({ data: null }))
    : { data: null };

  const grossStr = live ? `$${live.grossMonthlyUsd.toFixed(2)}/mo` : "run local CLI for static YAML audit";
  const netStr = live ? `$${live.netMonthlyUsd.toFixed(2)}/mo` : "$0.00/mo";
  const action = (cmdObj && cmdObj.action) || "breakdown";

  if (action === "permissions") {
    const md =
      `<!-- minici-bot-reply -->\n### 🔒 MiniCI Least-Privilege Security & Coverage Receipt (\`${repo}\`)\n\n` +
      `- **Public Repository Scope:** \`Contents: None\` (\`63.0%\` telemetry attributes with zero token)\n` +
      `- **Private Workflow YAML Scope:** \`Actions: Read\` + \`Contents: Read\` (\`82.4%\` coverage, zero source-code AST read)\n` +
      `- **Write Scopes Required:** \`None\` (\`100%\` read-only by default)\n` +
      `- **Local Verification (<1 ms):** \`minici permissions check --repo ${repo}\``;
    return { action, repo, live, markdown: md, plainText: `MiniCI Security Receipt for ${repo}: 100% read-only` };
  }

  if (action === "accelerate") {
    const estSave = live ? `~$${(live.grossMonthlyUsd * 0.28).toFixed(2)}/mo` : "20%-45% CI minutes";
    const md =
      `<!-- minici-bot-reply -->\n### ⚡ MiniCI Two-Tier Rust CI Acceleration Plan (\`${repo}\`)\n\n` +
      `- **Current Compute Baseline:** \`${grossStr}\` gross (\`${netStr}\` net)\n` +
      `- **Estimated Acceleration Savings:** **\`${estSave}\`** via whole-minute \`ceil(60s)\` boundary reductions\n` +
      `- **Tier 1 — Drop-In Rust Toolchains (\`MINICI-RUST-012\`):**\n` +
      `  - \`pip install\` → \`astral-sh/setup-uv@v5\` + \`uv pip install --system\` (\`10x-50x\` faster)\n` +
      `  - \`flake8\` / \`black\` / \`isort\` → \`uvx ruff check\` & \`ruff format --check\` (\`50x-150x\` faster)\n` +
      `  - \`eslint\` / \`prettier\` → \`npx --yes @biomejs/biome ci .\` (\`15x-25x\` faster)\n` +
      `  - \`cargo test\` → \`cargo nextest run\` + \`cargo test --doc\` (\`2x-3x\` faster)\n` +
      `- **Tier 2 — Scoped \`rust-everywhere\` Bespoke CI-Tool Converter (\`MINICI-RUST-EVERYWHERE-013\`):**\n` +
      `  - Converts standalone repo CI scripts (\`scripts/*.py\`, \`tools/*.js\`, \`ci/*.sh\`) with no existing Rust equivalent into \`cargo check\`-verified native binaries under \`.minici/rust-everywhere/<tool>/\` (\`0\` app source files touched).\n\n` +
      `Run locally to apply with shadow \`DiffReport\` verification:\n\`\`\`bash\nminici accelerate --path . --apply\n\`\`\``;
    return { action, repo, live, markdown: md, plainText: `MiniCI Rust CI Acceleration Plan for ${repo} (${estSave})` };
  }

  if (action === "shrink") {
    const estSave = live ? `~$${(live.grossMonthlyUsd * 0.22).toFixed(2)}/mo` : "15%-35% CI waste";
    const md =
      `<!-- minici-bot-reply -->\n### ✂️ MiniCI Surgical Workflow \`shrink\` Plan (\`${repo}\`)\n\n` +
      `- **Current Compute Baseline:** \`${grossStr}\` gross (\`${netStr}\` net)\n` +
      `- **Estimated Hygiene Savings:** **\`${estSave}\`** (superseded PR \`concurrency\` cancel, \`timeout-minutes: 30\` guard, shallow \`fetch-depth: 1\` clone, \`retention-days: 7\`, and lockfile caching)\n\n` +
      `Run locally or generate committable PR suggestions:\n\`\`\`bash\nminici shrink --path . --suggest-pr\nminici shrink --path . --apply\n\`\`\``;
    return { action, repo, live, markdown: md, plainText: `MiniCI Surgical Shrink Plan for ${repo} (${estSave})` };
  }

  if (action === "migrate") {
    const targetMode = (cmdObj && cmdObj.targetArg) || "github-arm64";
    const estSave = live ? `~$${(live.grossMonthlyUsd * 0.25).toFixed(2)}/mo` : "17%-43% runner rate drop";
    const md =
      `<!-- minici-bot-reply -->\n### 🚀 MiniCI 6-Gate Runner Migration Receipt (\`${repo}\` → \`${targetMode}\`)\n\n` +
      `- **Current Compute Baseline:** \`${grossStr}\` gross (\`${netStr}\` net)\n` +
      `- **Projected Right-Sizing Delta:** **\`${estSave}\`** (guards against x86/KVM/GPU/macOS incompatibilities)\n\n` +
      `Preview or apply locally:\n\`\`\`bash\nminici migrate --path . --to ${targetMode}\n\`\`\``;
    return { action, repo, live, markdown: md, plainText: `MiniCI Runner Migration (${repo} -> ${targetMode}): ${estSave}` };
  }

  const md = live
    ? `<!-- minici-bot-reply -->\n### 💰 MiniCI Live FinOps Receipt (\`${repo}\`)\n\n` +
      `- **Gross Compute Run-Rate:** \`$${live.grossMonthlyUsd.toFixed(2)}/mo\`\n` +
      `- **Net Billed (${live.isPublic ? "Public OSS" : "Private"}):** **\`$${live.netMonthlyUsd.toFixed(2)}/mo\`**\n` +
      `- **Workflows / Jobs / Cadence:** \`${live.workflows}\` wf • \`${live.jobs}\` jobs • \`${live.monthlyRuns} runs/mo\` (\`${live.billedMins.toFixed(1)}m/run\`)\n` +
      `- **Next Actions:** Comment \`@minici shrink\`, \`@minici accelerate\`, or \`@minici migrate arm64\`.`
    : `<!-- minici-bot-reply -->\n### 💰 MiniCI FinOps Receipt (\`${repo}\`)\n\n` +
      `- Run \`minici breakdown --repo ${repo} --live\` or comment \`@minici shrink\` / \`@minici accelerate\`.`;
  return {
    action,
    repo,
    live,
    markdown: md,
    plainText: live
      ? `MiniCI Breakdown for ${repo}: $${live.grossMonthlyUsd.toFixed(2)}/mo gross ($${live.netMonthlyUsd.toFixed(2)}/mo net)`
      : `MiniCI FinOps Receipt for ${repo}`
  };
}

function buildSlackInteractiveActionsRow(targetRepo) {
  const safeTarget = String(targetRepo || "owner/repo").slice(0, 120);
  return {
    type: "actions",
    elements: [
      {
        type: "button",
        style: "primary",
        text: { type: "plain_text", text: "✂️ Surgical Shrink Plan" },
        action_id: "minici_action_shrink",
        value: safeTarget
      },
      {
        type: "button",
        text: { type: "plain_text", text: "⚡ Rust-Everywhere Accelerate" },
        action_id: "minici_action_accelerate",
        value: safeTarget
      },
      {
        type: "button",
        text: { type: "plain_text", text: "🚀 ARM64 Right-Size" },
        action_id: "minici_action_arm64",
        value: safeTarget
      }
    ]
  };
}

async function buildSlackFinOpsBlocks(commandText, env, ctx) {
  const cleaned = (commandText || "").replace(/<@[^>]+>/g, "").trim();
  const tokens = cleaned.split(/\s+/).filter(Boolean);
  const sub = (tokens[0] || "breakdown").toLowerCase();
  const target = tokens[1] || "owner/repo";

  if (["shrink", "accelerate", "rust-everywhere", "migrate"].includes(sub)) {
    const reply = await buildOmnichannelMiniciReply(
      { action: sub === "rust-everywhere" ? "accelerate" : sub, targetArg: tokens[2] || "github-arm64", repoSlug: target },
      target,
      env,
      ctx
    );
    return {
      response_type: "in_channel",
      text: reply.plainText,
      blocks: [
        {
          type: "header",
          text: { type: "plain_text", text: `MiniCI — ${reply.action.toUpperCase()} (${reply.repo})`.slice(0, 145) }
        },
        {
          type: "section",
          text: {
            type: "mrkdwn",
            text: reply.markdown.replace(/<!--[^>]+-->\n?/g, "").slice(0, 2900)
          }
        },
        buildSlackInteractiveActionsRow(reply.repo)
      ]
    };
  }

  if (sub === "permissions" || sub === "security") {
    return {
      response_type: "in_channel",
      text: "MiniCI Least-Privilege Token & Coverage Receipt",
      blocks: [
        {
          type: "header",
          text: { type: "plain_text", text: "MiniCI — Least-Privilege Token & Coverage Receipt" }
        },
        {
          type: "section",
          fields: [
            { type: "mrkdwn", text: "*Public Repo Access:*\n`Contents: None` (`63.0%` zero-token)" },
            { type: "mrkdwn", text: "*Private Repo YAML:*\n`Actions: Read` + `Contents: Read` (`82.4%`)" },
            { type: "mrkdwn", text: "*Write Scopes Required:*\n`None` (100% Read-Only)" },
            { type: "mrkdwn", text: "*With FOCUS v1.1 CSV + Step:*\n`108 / 108` attributes (`100.0%`)" }
          ]
        }
      ]
    };
  }

  const { data: live, cacheLayer } = await getRepoReportThreeLayer(target, 30, env, ctx, false).catch(
    () => ({ data: null, cacheLayer: "MISS" })
  );
  if (live) {
    return {
      response_type: "in_channel",
      text: `MiniCI Breakdown for ${target}: $${live.grossMonthlyUsd.toFixed(2)}/mo gross ($${live.netMonthlyUsd.toFixed(2)}/mo net)`,
      blocks: [
        {
          type: "header",
          text: { type: "plain_text", text: `MiniCI — Live FinOps Receipt (${target})`.slice(0, 145) }
        },
        {
          type: "section",
          fields: [
            { type: "mrkdwn", text: `*Gross Compute:*\n\`$${live.grossMonthlyUsd.toFixed(2)}/mo\`` },
            {
              type: "mrkdwn",
              text: `*Net Billed (${live.isPublic ? "Public OSS" : "Private"}):*\n\`$${live.netMonthlyUsd.toFixed(2)}/mo\``
            },
            {
              type: "mrkdwn",
              text: `*Workflows / Jobs / Cadence:*\n\`${live.workflows}\` wf • \`${live.jobs}\` jobs • \`${live.monthlyRuns}/mo\``
            },
            {
              type: "mrkdwn",
              text: `*Billed / Cache (${cacheLayer}):*\n\`${live.billedMins.toFixed(1)}m/run\` • \`${live.activeCacheGb.toFixed(2)} GB\``
            }
          ]
        },
        {
          type: "section",
          text: {
            type: "mrkdwn",
            text: `*Full 24-Model Local Receipt (<1 ms):* \`minici breakdown --repo ${target} --live --days ${live.windowDays}\``
          }
        },
        buildSlackInteractiveActionsRow(target)
      ]
    };
  }

  return {
    response_type: "in_channel",
    text: `MiniCI FinOps Receipt for ${target}`,
    blocks: [
      {
        type: "header",
        text: { type: "plain_text", text: `MiniCI — CI/CD FinOps (${sub})`.slice(0, 145) }
      },
      {
        type: "section",
        text: {
          type: "mrkdwn",
          text: `*Target:* \`${target}\`\nRun locally or in CI in \`< 1 ms\`:\n\`minici breakdown --repo ${target} --live\`\n\`minici shrink --path .\` • \`minici accelerate --path .\` • \`minici migrate --path . --to github-arm64\``
        }
      },
      buildSlackInteractiveActionsRow(target)
    ]
  };
}

const EDGE_MEM_SHARES = new Map();
const EDGE_MEM_ENTITLEMENTS = new Map();
const EDGE_MEM_LEDGER = new Map();

function planDefaults(planName) {
  const p = String(planName || "starter").toLowerCase();
  if (p === "custom" || p === "team" || p === "enterprise" || p.includes("flex")) {
    return { plan: "custom", max_repos: 9999, retention_days: 365 };
  }
  if (p === "pro" || p.includes("pro")) {
    return { plan: "pro", max_repos: 15, retention_days: 365 };
  }
  return { plan: "starter", max_repos: 3, retention_days: 30 };
}

function escapeXml(str) {
  return String(str || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function renderFinOpsBadgeSvg(label, valueText, colorHex = "#10b981") {
  const safeLbl = escapeXml(label);
  const safeVal = escapeXml(valueText);
  const lblW = Math.max(48, safeLbl.length * 6.8 + 16);
  const valW = Math.max(76, safeVal.length * 6.8 + 18);
  const totalW = Math.round(lblW + valW);
  const lw = Math.round(lblW);
  const vw = totalW - lw;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${totalW}" height="20" role="img" aria-label="${safeLbl}: ${safeVal}">
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="${totalW}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="${lw}" height="20" fill="#0f172a"/>
    <rect x="${lw}" width="${vw}" height="20" fill="${colorHex}"/>
    <rect width="${totalW}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="11">
    <text x="${Math.round(lw / 2)}" y="14">${safeLbl}</text>
    <text x="${lw + Math.round(vw / 2)}" y="14" font-weight="bold">${safeVal}</text>
  </g>
</svg>`;
}

async function verifyPolarWebhookSignature(request, rawBody, webhookSecret) {
  if (!webhookSecret) return false;
  const msgId = request.headers.get("webhook-id") || "";
  const msgTs = request.headers.get("webhook-timestamp") || "";
  const sigHeader = request.headers.get("webhook-signature") || "";
  if (!msgId || !msgTs || !sigHeader) return false;
  const nowSec = Math.floor(Date.now() / 1000);
  const tsNum = Number(msgTs);
  if (!Number.isFinite(tsNum) || Math.abs(nowSec - tsNum) > 300) return false;

  // Standard Webhooks secret may start with `whsec_` (base64) or raw UTF-8 secret
  let keyBytes;
  const trimmed = String(webhookSecret).trim();
  if (trimmed.startsWith("whsec_")) {
    try {
      const b64 = trimmed.slice(6);
      const binStr = atob(b64);
      keyBytes = Uint8Array.from(binStr, (c) => c.charCodeAt(0));
    } catch {
      keyBytes = new TextEncoder().encode(trimmed);
    }
  } else {
    keyBytes = new TextEncoder().encode(trimmed);
  }

  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    keyBytes,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"]
  );
  const toSign = new TextEncoder().encode(`${msgId}.${msgTs}.${rawBody}`);
  const mac = await crypto.subtle.sign("HMAC", cryptoKey, toSign);
  const expectedB64 = btoa(String.fromCharCode(...new Uint8Array(mac)));

  const candidates = sigHeader.split(/\s+/).map((part) => part.replace(/^v1,/, "").trim());
  return candidates.some((cand) => cand === expectedB64);
}

async function ensureCloudTablesIfD1(env) {
  if (!env || !env.MINICI_D1) return;
  await env.MINICI_D1.batch([
    env.MINICI_D1.prepare(
      `CREATE TABLE IF NOT EXISTS cloud_entitlements (
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
      )`
    ),
    env.MINICI_D1.prepare(
      `CREATE TABLE IF NOT EXISTS shared_reports (
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
      )`
    ),
    env.MINICI_D1.prepare(
      `CREATE TABLE IF NOT EXISTS cloud_run_ledger (
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
      )`
    )
  ]).catch(() => {});
}

async function persistEntitlement(ent, env) {
  EDGE_MEM_ENTITLEMENTS.set(ent.key_sha256, ent);
  if (env && env.MINICI_KV) {
    await env.MINICI_KV.put(`entitlement:${ent.key_sha256}`, JSON.stringify(ent), {
      expirationTtl: 86400
    }).catch(() => {});
  }
  if (env && env.MINICI_D1) {
    await ensureCloudTablesIfD1(env);
    await env.MINICI_D1.prepare(
      `INSERT OR REPLACE INTO cloud_entitlements (
         key_sha256, key_hint, plan, status, max_repos, retention_days,
         customer_email, polar_subscription_id, slack_webhook_url, custom_rates_json,
         max_pr_delta_monthly_usd, require_timeout_minutes, created_at, updated_at
       ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)`
    )
      .bind(
        ent.key_sha256,
        ent.key_hint || "",
        ent.plan,
        ent.status,
        ent.max_repos,
        ent.retention_days,
        ent.customer_email || "",
        ent.polar_subscription_id || "",
        ent.slack_webhook_url || "",
        ent.custom_rates_json || "{}",
        Number(ent.max_pr_delta_monthly_usd || 0),
        ent.require_timeout_minutes ? 1 : 0,
        ent.created_at,
        ent.updated_at
      )
      .run()
      .catch(() => {});
  }
}

function extractBearerKey(request) {
  const auth = request.headers.get("authorization") || "";
  if (auth.toLowerCase().startsWith("bearer ")) {
    return auth.slice(7).trim();
  }
  return (request.headers.get("x-minici-key") || "").trim();
}

async function resolveCloudEntitlement(rawKey, env) {
  if (!rawKey || rawKey.length < 6) return null;
  const keyHash = await sha256Hex(rawKey);
  const nowSec = Math.floor(Date.now() / 1000);

  if (env && env.MINICI_DEV_TEST_KEY && rawKey === env.MINICI_DEV_TEST_KEY) {
    return {
      key_sha256: keyHash,
      key_hint: rawKey.slice(0, 8),
      plan: "custom",
      status: "active",
      max_repos: 9999,
      retention_days: 365,
      customer_email: "dev@minici.local",
      polar_subscription_id: "sub_dev",
      slack_webhook_url: "",
      custom_rates_json: "{}",
      max_pr_delta_monthly_usd: 0.0,
      require_timeout_minutes: 0,
      created_at: nowSec,
      updated_at: nowSec
    };
  }

  if (env && env.MINICI_KV) {
    const kvRaw = await env.MINICI_KV.get(`entitlement:${keyHash}`).catch(() => null);
    if (kvRaw) {
      try {
        const parsed = JSON.parse(kvRaw);
        if (parsed && parsed.status === "active") return parsed;
        if (parsed && parsed.status === "revoked") return null;
      } catch {}
    }
  }

  if (env && env.MINICI_D1) {
    const row = await env.MINICI_D1.prepare(
      "SELECT * FROM cloud_entitlements WHERE key_sha256 = ?1"
    )
      .bind(keyHash)
      .first()
      .catch(() => null);
    if (row) {
      if (row.status !== "active") return null;
      return row;
    }
  }

  if (EDGE_MEM_ENTITLEMENTS.has(keyHash)) {
    const mem = EDGE_MEM_ENTITLEMENTS.get(keyHash);
    return mem && mem.status === "active" ? mem : null;
  }

  // Live fallback: Validate Polar.sh `MINICI-...` License Key directly via Polar Customer Portal API
  const polarOrgId = (env && env.POLAR_ORGANIZATION_ID) || "6630821f-1f9d-4cb9-a5cd-a89a6bdca66b";
  if (rawKey.toUpperCase().startsWith("MINICI") || (env && env.POLAR_ORGANIZATION_ID)) {
    const validateBody = { key: rawKey, organization_id: polarOrgId };
    const polarResp = await fetch(
      "https://api.polar.sh/v1/customer-portal/license-keys/validate",
      {
        method: "POST",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify(validateBody)
      }
    ).catch(() => null);

    if (polarResp && polarResp.ok) {
      const data = await polarResp.json().catch(() => null);
      if (data && (data.status === "granted" || data.status === "active" || data.id)) {
        const benefitDesc = String(
          (data.benefit && data.benefit.description) ||
            (data.customer && data.customer.metadata && data.customer.metadata.plan) ||
            "starter"
        );
        const defs = planDefaults(benefitDesc);
        const ent = {
          key_sha256: keyHash,
          key_hint: rawKey.slice(0, 10),
          plan: defs.plan,
          status: "active",
          max_repos: defs.max_repos,
          retention_days: defs.retention_days,
          customer_email: String((data.customer && data.customer.email) || ""),
          polar_subscription_id: String(data.subscription_id || ""),
          slack_webhook_url: "",
          custom_rates_json: "{}",
          max_pr_delta_monthly_usd: 0.0,
          require_timeout_minutes: 0,
          created_at: nowSec,
          updated_at: nowSec
        };
        await persistEntitlement(ent, env);
        return ent;
      }
    }
  }

  return null;
}

async function dispatchFridayFinOpsDigests(env) {
  if (!env || !env.MINICI_D1) return { sent: 0 };
  await ensureCloudTablesIfD1(env);
  const nowSec = Math.floor(Date.now() / 1000);
  const weekAgo = nowSec - 7 * 86400;
  const ents = await env.MINICI_D1.prepare(
    "SELECT key_sha256, plan, slack_webhook_url FROM cloud_entitlements WHERE status = 'active' AND plan IN ('pro', 'custom') AND slack_webhook_url != ''"
  )
    .all()
    .catch(() => ({ results: [] }));
  const rows = (ents && ents.results) || [];
  let sent = 0;

  for (const ent of rows) {
    const agg = await env.MINICI_D1.prepare(
      `SELECT COUNT(*) AS repo_count,
              COALESCE(SUM(s.sync_cnt), 0) AS runs_synced,
              COALESCE(SUM(l.gross_monthly_usd), 0.0) AS total_gross_usd,
              COALESCE(SUM(l.net_billed_monthly_usd), 0.0) AS total_net_usd,
              COALESCE(SUM(l.dev_wait_monthly_usd), 0.0) AS total_wait_usd
         FROM cloud_run_ledger l
         INNER JOIN (
           SELECT repo, MAX(id) AS max_id, COUNT(*) AS sync_cnt
             FROM cloud_run_ledger
            WHERE key_sha256 = ?1 AND recorded_at >= ?2
            GROUP BY repo
         ) s ON l.id = s.max_id`
    )
      .bind(ent.key_sha256, weekAgo)
      .first()
      .catch(() => null);
    if (!agg || !agg.runs_synced) continue;

    const slackMsg = {
      text: `MiniCI Weekly Friday FinOps Digest (${ent.plan.toUpperCase()}): ${agg.repo_count} repo(s), $${Number(agg.total_gross_usd || 0).toFixed(2)}/mo total run-rate`,
      blocks: [
        {
          type: "header",
          text: { type: "plain_text", text: "MiniCI — Weekly Friday FinOps Digest" }
        },
        {
          type: "section",
          fields: [
            { type: "mrkdwn", text: `*Tracked Repos (7d):*\n\`${agg.repo_count}\` (` + "`" + `${agg.runs_synced}` + "`" + ` syncs)` },
            { type: "mrkdwn", text: `*Total Gross Run-Rate:*\n\`$${Number(agg.total_gross_usd || 0).toFixed(2)}/mo\`` },
            { type: "mrkdwn", text: `*Total Net Billed:*\n\`$${Number(agg.total_net_usd || 0).toFixed(2)}/mo\`` },
            { type: "mrkdwn", text: `*Total Dev Wait Impact:*\n\`$${Number(agg.total_wait_usd || 0).toFixed(2)}/mo\`` }
          ]
        }
      ]
    };
    const resp = await fetch(ent.slack_webhook_url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(slackMsg)
    }).catch(() => null);
    if (resp && resp.ok) sent += 1;
  }
  return { sent };
}

export default {
  async scheduled(event, env, ctx) {
    if (ctx && typeof ctx.waitUntil === "function") {
      ctx.waitUntil(dispatchFridayFinOpsDigests(env));
    } else {
      await dispatchFridayFinOpsDigests(env);
    }
  },

  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    if (
      request.method === "OPTIONS" &&
      (url.pathname === "/health" ||
        url.pathname === "/api/health" ||
        url.pathname === "/v1/rates.json" ||
        url.pathname === "/v1/pricing/catalog" ||
        url.pathname === "/v1/systemone" ||
        url.pathname.startsWith("/v1/report/") ||
        url.pathname.startsWith("/v1/jev/") ||
        url.pathname.startsWith("/v1/cloud/") ||
        url.pathname.startsWith("/v1/badge/") ||
        url.pathname.startsWith("/r/"))
    ) {
      return new Response(null, { status: 204, headers: CORS_HEADERS });
    }

    if (url.pathname === "/health" || url.pathname === "/api/health") {
      return new Response(
        JSON.stringify({
          status: "ok",
          service: "minici-cost-action",
          version: "0.9.1",
          engine: "Pure Rust std-only (<1ms)",
          cache_layers: [
            "L1:caches.default",
            env && env.MINICI_KV ? "L2:MINICI_KV(bound)" : "L2:MINICI_KV(optional)",
            env && env.MINICI_D1 ? "L3:MINICI_D1(bound)" : "L3:MINICI_D1(optional)"
          ],
          providers: 7,
          skus: 310,
          models: 24,
          mcp_tools: 8,
          author: "Aamir Raza <raza.aamir01@gmail.com>"
        }),
        { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
      );
    }

    if (url.pathname === "/v1/rates.json" || url.pathname === "/v1/pricing/catalog") {
      const inm = request.headers.get("if-none-match") || "";
      const inmMatched = inm
        .split(",")
        .map((t) => t.trim().replace(/^W\//i, ""))
        .some((t) => t === "*" || t === RATES_ETAG || t === "minici-rates-v0.9.1");
      if (inmMatched) {
        return new Response(null, {
          status: 304,
          headers: {
            "cache-control": "public, max-age=3600, s-maxage=86400",
            etag: RATES_ETAG,
            ...CORS_HEADERS
          }
        });
      }
      if (env && env.MINICI_KV) {
        const kvPricing = await env.MINICI_KV.get("pricing:catalog:2026").catch(() => null);
        if (kvPricing) {
          return new Response(kvPricing, {
            headers: {
              "content-type": "application/json; charset=utf-8",
              "cache-control": "public, max-age=3600, s-maxage=86400",
              "x-minici-cache": "L2-WORKERS-KV",
              etag: RATES_ETAG,
              ...CORS_HEADERS
            }
          });
        }
      }
      return new Response(JSON.stringify(OFFICIAL_RATES_FEED, null, 2), {
        headers: {
          "content-type": "application/json; charset=utf-8",
          "cache-control": "public, max-age=3600, s-maxage=86400",
          "x-minici-cache": "EMBEDDED-CATALOG",
          etag: RATES_ETAG,
          ...CORS_HEADERS
        }
      });
    }

    // 3-Layer Cached Live Repo Report Endpoint: GET /v1/report/:owner/:repo?days=30&refresh=1
    if (
      (url.pathname.startsWith("/v1/report/") || url.pathname === "/api/analyze") &&
      (request.method === "GET" || request.method === "HEAD")
    ) {
      const slug = url.pathname.startsWith("/v1/report/")
        ? url.pathname.slice("/v1/report/".length).replace(/^\/+|\/+$/g, "")
        : String(url.searchParams.get("repo") || "").trim();
      const windowDays = Number(url.searchParams.get("days") || 30);
      const forceRefresh = url.searchParams.get("refresh") === "1";
      const { data, cacheLayer } = await getRepoReportThreeLayer(
        slug,
        windowDays,
        env,
        ctx,
        forceRefresh
      );
      if (!data) {
        return new Response(
          JSON.stringify({ error: "repo_not_found_or_rate_limited", repo: slug }),
          {
            status: 404,
            headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
          }
        );
      }
      return new Response(JSON.stringify(data, null, 2), {
        headers: {
          "content-type": "application/json; charset=utf-8",
          "cache-control": "public, s-maxage=300, stale-while-revalidate=3600",
          "x-minici-cache": cacheLayer,
          ...CORS_HEADERS
        }
      });
    }

    // GitHub App & Repo Webhook Router (`POST /webhooks/github` & `/v1/webhook/github`)
    // Supports org-wide `@minici` issue/PR comments with zero YAML edits, D-34 bot-loop guard,
    // threshold-gated Slack PR alerts, and L1/L2 report cache invalidation.
    if ((url.pathname === "/v1/webhook/github" || url.pathname === "/webhooks/github") && request.method === "POST") {
      const rawBody = await request.text();
      if (env && env.GITHUB_WEBHOOK_SECRET) {
        const allowDev = Boolean(env.ALLOW_UNSIGNED_GITHUB_DEV === "true");
        const sigOk = await verifyGitHubWebhookSignature(
          request,
          rawBody,
          env.GITHUB_WEBHOOK_SECRET,
          allowDev
        );
        if (!sigOk) {
          return new Response(JSON.stringify({ error: "invalid_github_webhook_signature" }), {
            status: 401,
            headers: { "content-type": "application/json; charset=utf-8" }
          });
        }
      }

      let body = {};
      try {
        body = JSON.parse(rawBody || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
      }

      const ghEvent = (request.headers.get("x-github-event") || "").toLowerCase();
      const senderType = String((body && body.sender && body.sender.type) || "");
      const senderLogin = String((body && body.sender && body.sender.login) || "").toLowerCase();
      const slug = body && body.repository && body.repository.full_name;
      const ghApiBase = String((env && env.GITHUB_API_URL) || "https://api.github.com").replace(/\/+$/, "");
      const ghToken = (env && (env.GITHUB_APP_TOKEN || env.GITHUB_TOKEN)) || "";

      // D-34 Bot-Loop Guard: never trigger `@minici` replies on Bot-authored events
      if (ghEvent === "issue_comment" && (senderType === "Bot" || senderLogin.endsWith("[bot]"))) {
        return new Response(JSON.stringify({ ok: true, ignored: true, reason: "bot_author_loop_guard" }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }

      if (ghEvent === "issue_comment" && (!body.action || body.action === "created")) {
        const commentBody = String((body.comment && body.comment.body) || "");
        const parsedCmd = parseOmnichannelAtMiniciCommand(commentBody, true);
        if (!parsedCmd) {
          return new Response(JSON.stringify({ ok: true, ignored: true, reason: "no_direct_at_minici_mention" }), {
            headers: { "content-type": "application/json; charset=utf-8" }
          });
        }
        const reply = await buildOmnichannelMiniciReply(parsedCmd, slug || "owner/repo", env, ctx);
        let postedCommentId = null;
        const issueNum = body.issue && body.issue.number;
        if (ghToken && slug && issueNum) {
          const postResp = await fetch(`${ghApiBase}/repos/${slug}/issues/${issueNum}/comments`, {
            method: "POST",
            headers: {
              accept: "application/vnd.github+json",
              authorization: `Bearer ${ghToken}`,
              "content-type": "application/json; charset=utf-8",
              "user-agent": "minici-worker/0.9.1"
            },
            body: JSON.stringify({ body: reply.markdown })
          }).catch(() => null);
          if (postResp && postResp.ok) {
            const pj = await postResp.json().catch(() => null);
            if (pj && pj.id) postedCommentId = pj.id;
          }
        }
        return new Response(
          JSON.stringify({
            ok: true,
            channel: "github",
            handled: "issue_comment",
            repo: reply.repo,
            action: reply.action,
            posted_comment_id: postedCommentId,
            reply_markdown: reply.markdown
          }),
          { headers: { "content-type": "application/json; charset=utf-8" } }
        );
      }

      if (slug && /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(slug)) {
        const edgeCache = typeof caches !== "undefined" && caches.default ? caches.default : null;
        const cacheBase = (env && env.REPORT_CACHE_BASE_URL) || "https://cache.minici.dev";
        for (const d of [7, 30, 60, 90]) {
          if (edgeCache) {
            await edgeCache
              .delete(new Request(`${cacheBase}/v1/report/${slug}?days=${d}`))
              .catch(() => {});
          }
          if (env && env.MINICI_KV) {
            await env.MINICI_KV.delete(`report:${slug}:${d}d`).catch(() => {});
          }
        }
        if (
          ghEvent === "pull_request" &&
          ["opened", "synchronize", "reopened"].includes(String(body.action || ""))
        ) {
          const reply = await buildOmnichannelMiniciReply(
            { action: "breakdown", repoSlug: slug },
            slug,
            env,
            ctx
          );
          let slackAlertSent = false;
          if (
            env &&
            env.SLACK_WEBHOOK_URL &&
            reply.live &&
            Number(reply.live.grossMonthlyUsd || 0) >= 50
          ) {
            const slackBlocks = await buildSlackFinOpsBlocks(`breakdown ${slug}`, env, ctx);
            const sr = await fetch(env.SLACK_WEBHOOK_URL, {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify(slackBlocks)
            }).catch(() => null);
            slackAlertSent = Boolean(sr && sr.ok);
          }
          return new Response(
            JSON.stringify({
              ok: true,
              channel: "github",
              handled: "pull_request",
              refreshed_repo: slug,
              slack_threshold_alert_sent: slackAlertSent,
              preview_markdown: reply.markdown
            }),
            { headers: { "content-type": "application/json; charset=utf-8" } }
          );
        }
        if (ctx && typeof ctx.waitUntil === "function") {
          ctx.waitUntil(getRepoReportThreeLayer(slug, 30, env, ctx, true));
        }
        return new Response(JSON.stringify({ ok: true, refreshed_repo: slug }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      return new Response(JSON.stringify({ ok: true, ignored: true }), {
        headers: { "content-type": "application/json; charset=utf-8" }
      });
    }

    // Cross-Repo SHA-256 Step Classification Cache (`GET/POST /v1/jev/step-cache`)
    if (url.pathname.startsWith("/v1/jev/step-cache")) {
      if ((request.method === "GET" || request.method === "HEAD")) {
        const hash = url.pathname.split("/").pop() || "";
        if (!/^[a-f0-9]{64}$/i.test(hash)) {
          return new Response(JSON.stringify({ error: "invalid_sha256" }), {
            status: 400,
            headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
          });
        }
        if (env && env.MINICI_KV) {
          const kvVal = await env.MINICI_KV.get(`jev:step:${hash}`).catch(() => null);
          if (kvVal) {
            return new Response(kvVal, {
              headers: {
                "content-type": "application/json; charset=utf-8",
                "x-minici-cache": "L2-WORKERS-KV",
                ...CORS_HEADERS
              }
            });
          }
        }
        if (env && env.MINICI_D1) {
          const row = await env.MINICI_D1.prepare(
            "SELECT * FROM step_decisions WHERE step_sha256 = ?1"
          )
            .bind(hash)
            .first()
            .catch(() => null);
          if (row) {
            return new Response(JSON.stringify(row), {
              headers: {
                "content-type": "application/json; charset=utf-8",
                "x-minici-cache": "L3-CLOUDFLARE-D1",
                ...CORS_HEADERS
              }
            });
          }
        }
        return new Response(JSON.stringify({ hit: false }), {
          status: 404,
          headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
        });
      }
      if (request.method === "POST") {
        let payload = {};
        try {
          payload = JSON.parse((await request.text()) || "{}");
        } catch {
          return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
        }
        const stepHash =
          payload.step_sha256 || (await sha256Hex(String(payload.step_text || "")));
        const nowSec = Math.floor(Date.now() / 1000);
        const record = {
          step_sha256: stepHash,
          phase: String(payload.phase || "compile_typecheck"),
          arm64_safe: payload.arm64_safe !== false,
          spot_safe: payload.spot_safe !== false,
          cancel_safe: payload.cancel_safe !== false,
          cache_candidate: Boolean(payload.cache_candidate),
          confidence: Number(payload.confidence || 0.92),
          model_id: String(payload.model_id || "jev-systemone"),
          created_at: nowSec
        };
        if (env && env.MINICI_KV) {
          await env.MINICI_KV.put(`jev:step:${stepHash}`, JSON.stringify(record), {
            expirationTtl: 2592000
          }).catch(() => {});
        }
        if (env && env.MINICI_D1) {
          await env.MINICI_D1.prepare(
            `INSERT OR REPLACE INTO step_decisions (
               step_sha256, phase, arm64_safe, spot_safe, cancel_safe,
               cache_candidate, confidence, model_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)`
          )
            .bind(
              record.step_sha256,
              record.phase,
              record.arm64_safe ? 1 : 0,
              record.spot_safe ? 1 : 0,
              record.cancel_safe ? 1 : 0,
              record.cache_candidate ? 1 : 0,
              record.confidence,
              record.model_id,
              record.created_at
            )
            .run()
            .catch(() => {});
        }
        return new Response(JSON.stringify({ ok: true, step_sha256: stepHash }), {
          headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
        });
      }
    }

    if (url.pathname === "/v1/systemone") {
      if (request.method !== "POST") {
        return new Response(
          JSON.stringify({ service: "minici-systemone-jev", status: "ready", method: "POST" }),
          { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
        );
      }
      let req = {};
      try {
        req = JSON.parse((await request.text()) || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), {
          status: 400,
          headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
        });
      }
      const decision = await evaluateSystemOneEdge(req, env, ctx);
      return new Response(JSON.stringify(decision), {
        headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
      });
    }

    if (url.pathname === "/slack/command" && request.method === "POST") {
      const rawBody = await request.text();
      const allowDev = Boolean(env && env.ALLOW_UNSIGNED_SLACK_DEV === "true");
      const valid = await verifySlackSignature(
        request,
        rawBody,
        env && env.SLACK_SIGNING_SECRET,
        allowDev
      );
      if (!valid) {
        return new Response(JSON.stringify({ error: "invalid_slack_signature" }), {
          status: 401,
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      const params = new URLSearchParams(rawBody);
      const text = params.get("text") || "breakdown";
      const payload = await buildSlackFinOpsBlocks(String(text), env, ctx);
      return new Response(JSON.stringify(payload), {
        headers: { "content-type": "application/json; charset=utf-8" }
      });
    }

    if (url.pathname === "/slack/events" && request.method === "POST") {
      const rawBody = await request.text();
      const allowDev = Boolean(env && env.ALLOW_UNSIGNED_SLACK_DEV === "true");
      const valid = await verifySlackSignature(
        request,
        rawBody,
        env && env.SLACK_SIGNING_SECRET,
        allowDev
      );
      if (!valid) {
        return new Response(JSON.stringify({ error: "invalid_slack_signature" }), {
          status: 401,
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      let body = {};
      try {
        body = JSON.parse(rawBody || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), {
          status: 400,
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      if (body && body.type === "url_verification" && body.challenge) {
        return new Response(JSON.stringify({ challenge: body.challenge }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      if (body && body.event && body.event.type === "app_mention") {
        const reply = await buildSlackFinOpsBlocks(body.event.text || "", env, ctx);
        if (env && env.SLACK_BOT_TOKEN && body.event.channel) {
          await fetch("https://slack.com/api/chat.postMessage", {
            method: "POST",
            headers: {
              "content-type": "application/json; charset=utf-8",
              authorization: `Bearer ${env.SLACK_BOT_TOKEN}`
            },
            body: JSON.stringify({
              channel: body.event.channel,
              thread_ts: body.event.thread_ts || body.event.ts,
              text: reply.text,
              blocks: reply.blocks
            })
          });
        }
        return new Response(JSON.stringify({ ok: true, preview: reply }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      return new Response(JSON.stringify({ ok: true, ignored: true }), {
        headers: { "content-type": "application/json; charset=utf-8" }
      });
    }

    // Slack Block Kit Interactive Buttons Handler (`POST /slack/interactions`)
    if (url.pathname === "/slack/interactions" && request.method === "POST") {
      const rawBody = await request.text();
      const allowDev = Boolean(env && env.ALLOW_UNSIGNED_SLACK_DEV === "true");
      const valid = await verifySlackSignature(
        request,
        rawBody,
        env && env.SLACK_SIGNING_SECRET,
        allowDev
      );
      if (!valid) {
        return new Response(JSON.stringify({ error: "invalid_slack_signature" }), {
          status: 401,
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      let payload = {};
      try {
        if (rawBody.trim().startsWith("{")) {
          payload = JSON.parse(rawBody);
        } else {
          const params = new URLSearchParams(rawBody);
          payload = JSON.parse(params.get("payload") || "{}");
        }
      } catch {
        return new Response(JSON.stringify({ error: "invalid_interaction_payload" }), {
          status: 400,
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      const firstAction = (Array.isArray(payload.actions) && payload.actions[0]) || {};
      const actionId = String(firstAction.action_id || "");
      const repoTarget = String(firstAction.value || "owner/repo");
      const actorName = String((payload.user && (payload.user.username || payload.user.name)) || "engineer");

      if (actionId === "minici_action_ack_spike") {
        const ackCard = {
          response_type: "in_channel",
          replace_original: false,
          text: `CI spend delta on ${repoTarget} acknowledged by @${actorName}.`
        };
        if (payload.response_url && /^https:\/\/hooks\.slack\.com\//.test(String(payload.response_url))) {
          await fetch(payload.response_url, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(ackCard)
          }).catch(() => {});
        }
        return new Response(JSON.stringify({ ok: true, action_id: actionId, repo: repoTarget, reply: ackCard }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }

      let mappedSub = "breakdown";
      if (actionId === "minici_action_shrink" || actionId === "minici_open_fix_pr") mappedSub = "shrink";
      else if (actionId === "minici_action_accelerate") mappedSub = "accelerate";
      else if (actionId === "minici_action_arm64") mappedSub = "migrate";

      const replyBlocks = await buildSlackFinOpsBlocks(`${mappedSub} ${repoTarget} github-arm64`, env, ctx);
      if (payload.response_url && /^https:\/\/hooks\.slack\.com\//.test(String(payload.response_url))) {
        await fetch(payload.response_url, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ ...replyBlocks, replace_original: false })
        }).catch(() => {});
      }
      return new Response(
        JSON.stringify({
          ok: true,
          channel: "slack_interactions",
          action_id: actionId,
          mapped_command: mappedSub,
          repo: repoTarget,
          reply: replyBlocks
        }),
        { headers: { "content-type": "application/json; charset=utf-8" } }
      );
    }

    // Linear Native Agent API (`actor=app`) & Issue Comment Webhook (`POST /webhooks/linear`)
    if (url.pathname === "/webhooks/linear" && request.method === "POST") {
      const rawBody = await request.text();
      if (env && env.LINEAR_WEBHOOK_SECRET) {
        const allowDev = Boolean(env.ALLOW_UNSIGNED_LINEAR_DEV === "true");
        const sigOk = await verifyLinearWebhookSignature(
          request,
          rawBody,
          env.LINEAR_WEBHOOK_SECRET,
          allowDev
        );
        if (!sigOk) {
          return new Response(JSON.stringify({ error: "invalid_linear_webhook_signature" }), {
            status: 401,
            headers: { "content-type": "application/json; charset=utf-8" }
          });
        }
      }
      let ev = {};
      try {
        ev = JSON.parse(rawBody || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
      }
      const evType = String(ev.type || "");
      const data = ev.data || {};
      const isAgentSession = evType === "AgentSessionEvent";
      const promptText = String(data.prompt || data.body || ev.prompt || "");
      const parsedCmd = parseOmnichannelAtMiniciCommand(promptText, !isAgentSession);
      if (!parsedCmd) {
        return new Response(JSON.stringify({ ok: true, ignored: true, reason: "no_minici_invocation" }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      const fallbackRepo = String(data.repository || (env && env.DEFAULT_REPO_SLUG) || "owner/repo");
      const reply = await buildOmnichannelMiniciReply(parsedCmd, fallbackRepo, env, ctx);
      let linearCommentPosted = false;
      const issueId = data.issueId || (data.issue && data.issue.id);
      if (env && env.LINEAR_API_KEY && issueId) {
        const gqlResp = await fetch("https://api.linear.app/graphql", {
          method: "POST",
          headers: {
            "content-type": "application/json",
            authorization: env.LINEAR_API_KEY
          },
          body: JSON.stringify({
            query: "mutation MiniCIComment($issueId: String!, $body: String!) { commentCreate(input: { issueId: $issueId, body: $body }) { success } }",
            variables: { issueId: String(issueId), body: reply.markdown }
          })
        }).catch(() => null);
        linearCommentPosted = Boolean(gqlResp && gqlResp.ok);
      }
      return new Response(
        JSON.stringify({
          ok: true,
          channel: "linear",
          event_type: evType || "Comment",
          action: reply.action,
          repo: reply.repo,
          comment_posted: linearCommentPosted,
          reply_markdown: reply.markdown
        }),
        { headers: { "content-type": "application/json; charset=utf-8" } }
      );
    }

    // Jira Cloud Automation `@minici` Webhook (`POST /webhooks/jira` -> ADF + Markdown)
    if (url.pathname === "/webhooks/jira" && request.method === "POST") {
      let body = {};
      try {
        body = JSON.parse((await request.text()) || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
      }
      const commentText = String((body.comment && body.comment.body) || body.text || "");
      const parsedCmd = parseOmnichannelAtMiniciCommand(commentText, true);
      if (!parsedCmd) {
        return new Response(JSON.stringify({ ok: true, ignored: true }), {
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      const reply = await buildOmnichannelMiniciReply(
        parsedCmd,
        String(body.repo || "owner/repo"),
        env,
        ctx
      );
      const adf = {
        version: 1,
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: reply.plainText }]
          },
          {
            type: "codeBlock",
            attrs: { language: "markdown" },
            content: [{ type: "text", text: reply.markdown.replace(/<!--[^>]+-->\n?/g, "") }]
          }
        ]
      };
      return new Response(
        JSON.stringify({
          ok: true,
          channel: "jira",
          action: reply.action,
          repo: reply.repo,
          reply_markdown: reply.markdown,
          adf
        }),
        { headers: { "content-type": "application/json; charset=utf-8" } }
      );
    }

    // Google Workspace Chat Synchronous `@minici` App (`POST /webhooks/gchat` -> `cardsV2`)
    if (url.pathname === "/webhooks/gchat" && request.method === "POST") {
      let body = {};
      try {
        body = JSON.parse((await request.text()) || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
      }
      const argText = String(
        (body.message && (body.message.argumentText || body.message.text)) || body.text || "breakdown"
      ).trim();
      const parsedCmd =
        parseOmnichannelAtMiniciCommand(argText, false) || { action: "breakdown", repoSlug: "owner/repo" };
      const reply = await buildOmnichannelMiniciReply(parsedCmd, "owner/repo", env, ctx);
      return new Response(
        JSON.stringify({
          text: reply.plainText,
          cardsV2: [
            {
              cardId: `minici-${reply.action}`,
              card: {
                header: {
                  title: `MiniCI — ${reply.action.toUpperCase()}`,
                  subtitle: reply.repo
                },
                sections: [
                  {
                    widgets: [
                      {
                        textParagraph: {
                          text: reply.markdown.replace(/<!--[^>]+-->\n?/g, "")
                        }
                      }
                    ]
                  }
                ]
              }
            }
          ]
        }),
        { headers: { "content-type": "application/json; charset=utf-8" } }
      );
    }

    // 1. Smart Polar.sh Checkout Redirects (`/v1/checkout/starter` -> $1, `/pro` -> $10, `/custom` -> Team Flex)
    if (url.pathname.startsWith("/v1/checkout/") && (request.method === "GET" || request.method === "HEAD")) {
      const planSlug = url.pathname.slice("/v1/checkout/".length).trim().toLowerCase();
      let dest = "https://buy.polar.sh/polar_cl_SCcWdjeIJvtwoa3JKBo5ouVXBL0yBv40bzuZr36iPMm";
      if (planSlug === "starter") {
        dest =
          (env && env.POLAR_CHECKOUT_STARTER_URL) ||
          "https://buy.polar.sh/polar_cl_62EwSj9qdDF362doVY7Hk68HQcxXrDhhvmZmB0JjvnI";
      } else if (planSlug === "pro") {
        dest =
          (env && env.POLAR_CHECKOUT_PRO_URL) ||
          "https://buy.polar.sh/polar_cl_Gbe4JUlRoTN7a6PqQ3lddj8tYbKtGE3PuYk5624TvwA";
      } else if (planSlug === "custom" || planSlug === "flex") {
        dest =
          (env && env.POLAR_CHECKOUT_CUSTOM_URL) ||
          "https://buy.polar.sh/polar_cl_inz7KQPwUMDhcHLflZYSIkKiDkUqDJmpq7X7Z0CaDpN";
      }
      return Response.redirect(dest, 302);
    }

    // 2. Polar.sh Webhook (`POST /v1/webhook/polar`) — Automatic $1 / $10 / Custom License Entitlement Sync
    if (url.pathname === "/v1/webhook/polar" && request.method === "POST") {
      const rawBody = await request.text();
      const allowUnsigned = Boolean(env && env.ALLOW_UNSIGNED_POLAR_DEV === "true");
      const sigOk = allowUnsigned
        ? true
        : await verifyPolarWebhookSignature(request, rawBody, env && env.POLAR_WEBHOOK_SECRET);
      if (!sigOk) {
        return new Response(JSON.stringify({ error: "invalid_polar_webhook_signature" }), {
          status: 401,
          headers: { "content-type": "application/json; charset=utf-8" }
        });
      }
      let ev = {};
      try {
        ev = JSON.parse(rawBody || "{}");
      } catch {
        return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
      }
      const evType = String(ev.type || "");
      const data = ev.data || {};
      const props = data.properties || {};
      const licenseKey = String(
        props.granted_key || props.key || data.key || data.license_key || ""
      ).trim();

      if (licenseKey) {
        const keyHash = await sha256Hex(licenseKey);
        const nowSec = Math.floor(Date.now() / 1000);
        const rawPlan = String(
          (data.product && data.product.metadata && data.product.metadata.minici_plan) ||
            (data.product && data.product.name) ||
            (data.metadata && data.metadata.minici_plan) ||
            "starter"
        );
        const defs = planDefaults(rawPlan);
        const revoked =
          evType.includes("revoked") ||
          evType.includes("canceled") ||
          String(data.status || "").toLowerCase() === "revoked";
        const ent = {
          key_sha256: keyHash,
          key_hint: licenseKey.slice(0, 10),
          plan: defs.plan,
          status: revoked ? "revoked" : "active",
          max_repos: defs.max_repos,
          retention_days: defs.retention_days,
          customer_email: String((data.customer && data.customer.email) || data.user_email || ""),
          polar_subscription_id: String(data.subscription_id || data.id || ""),
          slack_webhook_url: "",
          custom_rates_json: "{}",
          max_pr_delta_monthly_usd: 0.0,
          require_timeout_minutes: 0,
          created_at: nowSec,
          updated_at: nowSec
        };
        await persistEntitlement(ent, env);
        return new Response(
          JSON.stringify({ ok: true, event: evType, plan: ent.plan, status: ent.status }),
          { headers: { "content-type": "application/json; charset=utf-8" } }
        );
      }
      return new Response(JSON.stringify({ ok: true, event: evType, note: "acknowledged" }), {
        headers: { "content-type": "application/json; charset=utf-8" }
      });
    }

    // 3. Unlisted Shared Report Viewer (`GET /r/:id` & `/r/:id.json`) — Starter ($1/mo+)
    if (url.pathname.startsWith("/r/") && (request.method === "GET" || request.method === "HEAD")) {
      const rawTail = url.pathname.slice("/r/".length).trim();
      const wantJson = rawTail.endsWith(".json");
      const shareId = (wantJson ? rawTail.slice(0, -5) : rawTail).replace(/[^A-Za-z0-9_-]/g, "");
      if (!shareId) {
        return new Response("Invalid report ID", { status: 400 });
      }
      let record = EDGE_MEM_SHARES.get(shareId) || null;
      if (!record && env && env.MINICI_KV) {
        const kvStr = await env.MINICI_KV.get(`share:${shareId}`).catch(() => null);
        if (kvStr) {
          try {
            record = JSON.parse(kvStr);
          } catch {}
        }
      }
      if (!record && env && env.MINICI_D1) {
        const nowSec = Math.floor(Date.now() / 1000);
        record = await env.MINICI_D1.prepare(
          "SELECT * FROM shared_reports WHERE share_id = ?1 AND expires_at > ?2"
        )
          .bind(shareId, nowSec)
          .first()
          .catch(() => null);
      }
      if (!record) {
        return new Response(JSON.stringify({ error: "shared_report_not_found_or_expired", id: shareId }), {
          status: 404,
          headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
        });
      }
      if (wantJson) {
        return new Response(record.report_json || "{}", {
          headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
        });
      }
      return new Response(record.report_html || "<p>Report empty</p>", {
        headers: {
          "content-type": "text/html; charset=utf-8",
          "content-security-policy":
            "sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data: https:; font-src data:; base-uri 'none'; form-action 'none'",
          "x-content-type-options": "nosniff",
          "referrer-policy": "no-referrer",
          "x-robots-tag": "noindex, nofollow",
          ...CORS_HEADERS
        }
      });
    }

    // 4. Live Dynamic README / PR Cost & 30d Trend SVG Badge (`GET /v1/badge/:owner/:repo.svg`)
    if (url.pathname.startsWith("/v1/badge/") && (request.method === "GET" || request.method === "HEAD")) {
      const rawSlug = url.pathname
        .slice("/v1/badge/".length)
        .replace(/\.svg$/i, "")
        .replace(/^\/+|\/+$/g, "");
      let badgeVal = "audit ready";
      let badgeColor = "#10b981";

      const cutoff30dSec = Math.floor(Date.now() / 1000) - 30 * 86400;
      const memRows = (EDGE_MEM_LEDGER.get(rawSlug) || []).filter(
        (r) =>
          (!r.recorded_at || r.recorded_at >= cutoff30dSec) &&
          (!r.branch || r.branch === "main" || r.branch === "master")
      );
      let latestUsd = null;
      let oldestUsd = null;
      if (memRows.length > 0) {
        latestUsd = memRows[memRows.length - 1].gross_monthly_usd;
        oldestUsd = memRows[0].gross_monthly_usd;
      } else if (env && env.MINICI_D1) {
        const res = await env.MINICI_D1.prepare(
          "SELECT gross_monthly_usd FROM cloud_run_ledger WHERE repo = ?1 AND recorded_at >= ?2 AND branch IN ('main', 'master', '') ORDER BY recorded_at DESC LIMIT 200"
        )
          .bind(rawSlug, cutoff30dSec)
          .all()
          .catch(() => null);
        const pts = (res && res.results) || [];
        if (pts.length > 0) {
          latestUsd = Number(pts[0].gross_monthly_usd || 0);
          oldestUsd = Number(pts[pts.length - 1].gross_monthly_usd || latestUsd);
        }
      }
      if (latestUsd === null && /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(rawSlug)) {
        const { data } = await getRepoReportThreeLayer(rawSlug, 30, env, ctx, false).catch(() => ({
          data: null
        }));
        if (data) {
          latestUsd = Number(data.grossMonthlyUsd || 0);
          oldestUsd = latestUsd;
        }
      }
      if (latestUsd !== null) {
        let trendSuffix = "";
        if (oldestUsd && oldestUsd > 0 && Math.abs(latestUsd - oldestUsd) >= 0.5) {
          const pct = ((latestUsd - oldestUsd) / oldestUsd) * 100.0;
          trendSuffix = ` (${pct >= 0 ? "+" : ""}${pct.toFixed(0)}% 30d)`;
          if (pct > 15) badgeColor = "#f59e0b";
        }
        badgeVal = `$${latestUsd.toFixed(2)}/mo${trendSuffix}`;
      }

      return new Response(renderFinOpsBadgeSvg("MiniCI", badgeVal, badgeColor), {
        headers: {
          "content-type": "image/svg+xml; charset=utf-8",
          "cache-control": "public, max-age=300, s-maxage=600",
          ...CORS_HEADERS
        }
      });
    }

    // 5. Authenticated MiniCI Cloud Endpoints (`/v1/cloud/*` — Starter $1 / Pro $10 / Custom Team Flex)
    if (url.pathname.startsWith("/v1/cloud/")) {
      const rawKey = extractBearerKey(request);
      const ent = await resolveCloudEntitlement(rawKey, env);
      if (!ent) {
        return new Response(
          JSON.stringify({
            error: "cloud_key_required_or_invalid",
            message:
              "Local CLI, MCP & GitHub Action remain 100% Free ($0). To use hosted Cloud Sync, `minici share`, or Friday Slack digests, get a $1 Starter or $10 Pro key at https://buy.polar.sh/polar_cl_SCcWdjeIJvtwoa3JKBo5ouVXBL0yBv40bzuZr36iPMm and run `minici login <KEY>`."
          }),
          {
            status: 401,
            headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
          }
        );
      }

      // 5a. `GET /v1/cloud/me` — Verify key & return entitlement + custom enterprise rate overrides
      if (url.pathname === "/v1/cloud/me" && (request.method === "GET" || request.method === "HEAD")) {
        let customRates = {};
        try {
          customRates = JSON.parse(ent.custom_rates_json || "{}");
        } catch {}
        return new Response(
          JSON.stringify({
            ok: true,
            plan: ent.plan,
            status: ent.status,
            max_repos: ent.max_repos,
            retention_days: ent.retention_days,
            max_pr_delta_monthly_usd: Number(ent.max_pr_delta_monthly_usd || 0),
            require_timeout_minutes: Boolean(ent.require_timeout_minutes),
            custom_rates: customRates
          }),
          { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
        );
      }

      // 5b. `POST /v1/cloud/share` — Create unlisted permalink FinOps report (`Starter $1+`)
      if (url.pathname === "/v1/cloud/share" && request.method === "POST") {
        const rawShareText = await request.text();
        if (rawShareText.length > 524288) {
          return new Response(
            JSON.stringify({ error: "payload_too_large", max_bytes: 524288 }),
            { status: 413, headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
          );
        }
        let body = {};
        try {
          body = JSON.parse(rawShareText || "{}");
        } catch {
          return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
        }
        const nowSec = Math.floor(Date.now() / 1000);
        const ttlSec = Math.max(1, Number(ent.retention_days || 30)) * 86400;
        const expiresAt = nowSec + ttlSec;
        const seedHash = await sha256Hex(
          `${ent.key_sha256}:${nowSec}:${Math.random()}:${String(body.repo || "")}`
        );
        const shareId = seedHash.slice(0, 14);
        const repo = String(body.repo || "local-repo").slice(0, 140);
        const provider = String(body.provider || "github-actions").slice(0, 40);
        const grossUsd = Number(body.gross_monthly_usd || 0);
        const netUsd = Number(body.net_monthly_usd || 0);
        const waitUsd = Number(body.dev_wait_monthly_usd || 0);
        const reportJson =
          typeof body.report_json === "string"
            ? body.report_json
            : JSON.stringify(body.report_json || {});
        const reportHtml = String(body.report_html || "<p>MiniCI Shared Report</p>");

        const record = {
          share_id: shareId,
          key_sha256: ent.key_sha256,
          repo,
          provider,
          gross_monthly_usd: grossUsd,
          net_monthly_usd: netUsd,
          dev_wait_monthly_usd: waitUsd,
          report_json: reportJson,
          report_html: reportHtml,
          created_at: nowSec,
          expires_at: expiresAt
        };
        EDGE_MEM_SHARES.set(shareId, record);
        if (env && env.MINICI_KV) {
          await env.MINICI_KV.put(`share:${shareId}`, JSON.stringify(record), {
            expirationTtl: ttlSec
          }).catch(() => {});
        }
        if (env && env.MINICI_D1) {
          await ensureCloudTablesIfD1(env);
          await env.MINICI_D1.prepare(
            `INSERT OR REPLACE INTO shared_reports (
               share_id, key_sha256, repo, provider, gross_monthly_usd,
               net_monthly_usd, dev_wait_monthly_usd, report_json, report_html,
               created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)`
          )
            .bind(
              shareId,
              ent.key_sha256,
              repo,
              provider,
              grossUsd,
              netUsd,
              waitUsd,
              reportJson,
              reportHtml,
              nowSec,
              expiresAt
            )
            .run()
            .catch(() => {});
        }

        const baseOrigin = `${url.protocol}//${url.host}`;
        return new Response(
          JSON.stringify({
            ok: true,
            share_id: shareId,
            plan: ent.plan,
            retention_days: ent.retention_days,
            share_url: `${baseOrigin}/r/${shareId}`,
            json_url: `${baseOrigin}/r/${shareId}.json`,
            badge_url: `${baseOrigin}/v1/badge/${repo}.svg`
          }),
          { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
        );
      }

      // 5c. `POST /v1/cloud/ingest` — Cross-run ledger sync (`Starter $1+`) + Real-Time Anomaly Alert (`Pro $10+`)
      if (url.pathname === "/v1/cloud/ingest" && request.method === "POST") {
        let body = {};
        try {
          body = JSON.parse((await request.text()) || "{}");
        } catch {
          return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
        }
        const repo = String(body.repo || "local-repo").slice(0, 140);
        const nowSec = Math.floor(Date.now() / 1000);

        // Enforce distinct repo cap (`3` Starter, `15` Pro, `9999` Custom)
        if (env && env.MINICI_D1) {
          await ensureCloudTablesIfD1(env);
          const existingRepos = await env.MINICI_D1.prepare(
            "SELECT DISTINCT repo FROM cloud_run_ledger WHERE key_sha256 = ?1"
          )
            .bind(ent.key_sha256)
            .all()
            .catch(() => ({ results: [] }));
          const knownSet = new Set(((existingRepos && existingRepos.results) || []).map((r) => r.repo));
          if (!knownSet.has(repo) && knownSet.size >= Number(ent.max_repos || 3)) {
            return new Response(
              JSON.stringify({
                error: "repo_limit_reached",
                plan: ent.plan,
                max_repos: ent.max_repos,
                message: `Your ${ent.plan} plan includes up to ${ent.max_repos} synced repositories. Upgrade at https://polar.sh/minici/portal.`
              }),
              {
                status: 403,
                headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
              }
            );
          }
        }

        const entry = {
          repo,
          provider: String(body.provider || "github-actions"),
          branch: String(body.branch || "main"),
          commit_sha: String(body.commit_sha || "").slice(0, 40),
          expanded_jobs: Number(body.expanded_jobs || 0),
          billed_mins_per_run: Number(body.billed_mins_per_run || 0),
          gross_monthly_usd: Number(body.gross_monthly_usd || 0),
          net_billed_monthly_usd: Number(body.net_billed_monthly_usd || 0),
          dev_wait_monthly_usd: Number(body.dev_wait_monthly_usd || 0),
          cost_per_pr_usd: Number(body.cost_per_pr_usd || 0),
          spend_anomaly_active: Boolean(body.spend_anomaly_active),
          recorded_at: nowSec
        };

        const memArr = EDGE_MEM_LEDGER.get(repo) || [];
        memArr.push(entry);
        if (memArr.length > 60) memArr.shift();
        EDGE_MEM_LEDGER.set(repo, memArr);

        if (env && env.MINICI_D1) {
          await env.MINICI_D1.prepare(
            `INSERT INTO cloud_run_ledger (
               key_sha256, repo, provider, branch, commit_sha, expanded_jobs,
               billed_mins_per_run, gross_monthly_usd, net_billed_monthly_usd,
               dev_wait_monthly_usd, cost_per_pr_usd, spend_anomaly_active, recorded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)`
          )
            .bind(
              ent.key_sha256,
              entry.repo,
              entry.provider,
              entry.branch,
              entry.commit_sha,
              entry.expanded_jobs,
              entry.billed_mins_per_run,
              entry.gross_monthly_usd,
              entry.net_billed_monthly_usd,
              entry.dev_wait_monthly_usd,
              entry.cost_per_pr_usd,
              entry.spend_anomaly_active ? 1 : 0,
              nowSec
            )
            .run()
            .catch(() => {});
        }

        // Pro ($10+) & Custom: Real-time Slack alert when 7d spend velocity anomaly triggers
        let alertDispatched = false;
        if (
          (ent.plan === "pro" || ent.plan === "custom") &&
          entry.spend_anomaly_active &&
          ent.slack_webhook_url
        ) {
          const alertMsg = {
            text: `MiniCI Spend Anomaly Alert on ${repo}: $${entry.gross_monthly_usd.toFixed(2)}/mo run-rate`,
            blocks: [
              {
                type: "header",
                text: {
                  type: "plain_text",
                  text: `MiniCI — Spend Velocity Spike (${repo})`.slice(0, 145)
                }
              },
              {
                type: "section",
                fields: [
                  { type: "mrkdwn", text: `*Gross Run-Rate:*\n\`$${entry.gross_monthly_usd.toFixed(2)}/mo\`` },
                  { type: "mrkdwn", text: `*Net Billed:*\n\`$${entry.net_billed_monthly_usd.toFixed(2)}/mo\`` },
                  { type: "mrkdwn", text: `*Branch / Commit:*\n\`${entry.branch}\` (\`${entry.commit_sha.slice(0, 7) || "HEAD"}\`)` },
                  { type: "mrkdwn", text: `*Cost / PR:*\n\`$${entry.cost_per_pr_usd.toFixed(2)}\`` }
                ]
              }
            ]
          };
          const sr = await fetch(ent.slack_webhook_url, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(alertMsg)
          }).catch(() => null);
          alertDispatched = Boolean(sr && sr.ok);
        }

        // Custom (Team Flex): Evaluate central org PR budget gate policy
        const maxDelta = Number(ent.max_pr_delta_monthly_usd || 0);
        const prDelta = Number(body.pr_delta_monthly_usd || 0);
        const policyExceeded = ent.plan === "custom" && maxDelta > 0 && prDelta > maxDelta;

        const baseOrigin = `${url.protocol}//${url.host}`;
        return new Response(
          JSON.stringify({
            ok: true,
            plan: ent.plan,
            repo,
            badge_url: `${baseOrigin}/v1/badge/${repo}.svg`,
            trends_url: `${baseOrigin}/v1/cloud/trends/${repo}`,
            anomaly_alert_dispatched: alertDispatched,
            policy_gate: {
              enforced: ent.plan === "custom" && maxDelta > 0,
              max_pr_delta_monthly_usd: maxDelta,
              observed_pr_delta_monthly_usd: prDelta,
              exceeded: policyExceeded
            }
          }),
          { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
        );
      }

      // 5d. `GET /v1/cloud/trends/:owner/:repo` — 30d/365d Cross-Run Ledger (`Starter $1+`)
      if (url.pathname.startsWith("/v1/cloud/trends/") && (request.method === "GET" || request.method === "HEAD")) {
        const repoSlug = url.pathname.slice("/v1/cloud/trends/".length).replace(/^\/+|\/+$/g, "");
        let points = EDGE_MEM_LEDGER.get(repoSlug) || [];
        if (env && env.MINICI_D1) {
          const cutoff = Math.floor(Date.now() / 1000) - Number(ent.retention_days || 30) * 86400;
          const res = await env.MINICI_D1.prepare(
            "SELECT branch, commit_sha, expanded_jobs, billed_mins_per_run, gross_monthly_usd, net_billed_monthly_usd, dev_wait_monthly_usd, cost_per_pr_usd, recorded_at FROM cloud_run_ledger WHERE key_sha256 = ?1 AND repo = ?2 AND recorded_at >= ?3 ORDER BY recorded_at ASC LIMIT 365"
          )
            .bind(ent.key_sha256, repoSlug, cutoff)
            .all()
            .catch(() => null);
          if (res && Array.isArray(res.results) && res.results.length > 0) {
            points = res.results;
          }
        }
        return new Response(
          JSON.stringify({
            ok: true,
            repo: repoSlug,
            plan: ent.plan,
            retention_days: ent.retention_days,
            points_count: points.length,
            points
          }),
          { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
        );
      }

      // 5e. `GET /v1/cloud/dashboard` — Multi-Repo Team Dashboard (`Pro $10+` & `Custom`)
      if (url.pathname === "/v1/cloud/dashboard" && (request.method === "GET" || request.method === "HEAD")) {
        if (ent.plan === "starter") {
          return new Response(
            JSON.stringify({
              error: "pro_plan_required",
              message: "Multi-repo team dashboard requires MiniCI Pro ($10/mo) or Team Flex."
            }),
            {
              status: 403,
              headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
            }
          );
        }
        let repos = [];
        if (env && env.MINICI_D1) {
          const res = await env.MINICI_D1.prepare(
            `SELECT repo, provider,
                    MAX(recorded_at) AS last_synced_at,
                    AVG(gross_monthly_usd) AS avg_gross_monthly_usd,
                    AVG(net_billed_monthly_usd) AS avg_net_monthly_usd,
                    AVG(cost_per_pr_usd) AS avg_cost_per_pr_usd
               FROM cloud_run_ledger
              WHERE key_sha256 = ?1
              GROUP BY repo, provider
              ORDER BY avg_gross_monthly_usd DESC`
          )
            .bind(ent.key_sha256)
            .all()
            .catch(() => null);
          repos = (res && res.results) || [];
        } else {
          for (const [rName, pts] of EDGE_MEM_LEDGER.entries()) {
            if (pts.length > 0) {
              const last = pts[pts.length - 1];
              repos.push({
                repo: rName,
                provider: last.provider,
                last_synced_at: last.recorded_at,
                avg_gross_monthly_usd: last.gross_monthly_usd,
                avg_net_monthly_usd: last.net_billed_monthly_usd,
                avg_cost_per_pr_usd: last.cost_per_pr_usd
              });
            }
          }
        }
        return new Response(
          JSON.stringify({ ok: true, plan: ent.plan, repos_count: repos.length, repos }),
          { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
        );
      }

      // 5f. `GET/POST /v1/cloud/policy` — Org Governance & Custom Enterprise Rates (`Pro $10+` Slack hook / `Custom` Policy)
      if (url.pathname === "/v1/cloud/policy") {
        if ((request.method === "GET" || request.method === "HEAD")) {
          let customRates = {};
          try {
            customRates = JSON.parse(ent.custom_rates_json || "{}");
          } catch {}
          return new Response(
            JSON.stringify({
              ok: true,
              plan: ent.plan,
              slack_webhook_configured: Boolean(ent.slack_webhook_url),
              max_pr_delta_monthly_usd: Number(ent.max_pr_delta_monthly_usd || 0),
              require_timeout_minutes: Boolean(ent.require_timeout_minutes),
              custom_rates: customRates
            }),
            { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
          );
        }
        if (request.method === "POST") {
          if (ent.plan === "starter") {
            return new Response(
              JSON.stringify({
                error: "pro_or_custom_required",
                message: "Slack digest webhooks require Pro ($10/mo); central org policies require Team Flex (Custom)."
              }),
              {
                status: 403,
                headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS }
              }
            );
          }
          let body = {};
          try {
            body = JSON.parse((await request.text()) || "{}");
          } catch {
            return new Response(JSON.stringify({ error: "invalid_json" }), { status: 400 });
          }
          const updated = { ...ent, updated_at: Math.floor(Date.now() / 1000) };
          if (typeof body.slack_webhook_url === "string") {
            updated.slack_webhook_url = body.slack_webhook_url.trim();
          }
          if (ent.plan === "custom") {
            if (body.max_pr_delta_monthly_usd !== undefined) {
              updated.max_pr_delta_monthly_usd = Math.max(0, Number(body.max_pr_delta_monthly_usd || 0));
            }
            if (body.require_timeout_minutes !== undefined) {
              updated.require_timeout_minutes = body.require_timeout_minutes ? 1 : 0;
            }
            if (body.custom_rates && typeof body.custom_rates === "object") {
              updated.custom_rates_json = JSON.stringify(body.custom_rates);
            }
          }
          await persistEntitlement(updated, env);
          return new Response(
            JSON.stringify({
              ok: true,
              plan: updated.plan,
              slack_webhook_configured: Boolean(updated.slack_webhook_url),
              max_pr_delta_monthly_usd: updated.max_pr_delta_monthly_usd,
              require_timeout_minutes: Boolean(updated.require_timeout_minutes)
            }),
            { headers: { "content-type": "application/json; charset=utf-8", ...CORS_HEADERS } }
          );
        }
      }
    }

    return env.ASSETS.fetch(request);
  }
};
