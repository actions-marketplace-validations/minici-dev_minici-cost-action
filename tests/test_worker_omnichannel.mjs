import assert from "node:assert/strict";
import crypto from "node:crypto";
import worker from "../worker.js";

function hmacHex(secret, payload) {
  return crypto.createHmac("sha256", secret).update(payload).digest("hex");
}

// Mock outbound fetch for GitHub REST API, Slack response_url, and Linear GraphQL
const outboundCalls = [];
globalThis.fetch = async (url, init = {}) => {
  const urlStr = String(url);
  outboundCalls.push({ url: urlStr, method: init.method || "GET", body: init.body || null });

  if (urlStr.includes("api.github.com/repos/demo/app/issues/42/comments")) {
    return new Response(JSON.stringify({ id: 987654321 }), { status: 201 });
  }
  if (urlStr === "https://api.github.com/repos/demo/app") {
    return new Response(JSON.stringify({ private: false, default_branch: "main" }), { status: 200 });
  }
  if (urlStr === "https://api.github.com/repos/demo/app/contents/.github/workflows") {
    return new Response(
      JSON.stringify([{ name: "ci.yml", download_url: "https://raw.githubusercontent.com/demo/app/main/ci.yml" }]),
      { status: 200 }
    );
  }
  if (urlStr.includes("raw.githubusercontent.com/demo/app/main/ci.yml")) {
    return new Response("name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-8-core\n    steps:\n      - run: pytest\n", {
      status: 200
    });
  }
  if (urlStr.includes("/actions/runs")) {
    return new Response(
      JSON.stringify({
        workflow_runs: [
          {
            id: 1,
            head_sha: "abcdef123456",
            status: "completed",
            conclusion: "success",
            created_at: new Date().toISOString(),
            actor: { login: "alice" }
          }
        ]
      }),
      { status: 200 }
    );
  }
  if (urlStr.includes("/actions/cache/usage")) {
    return new Response(JSON.stringify({ active_caches_size_in_bytes: 536870912, active_caches_count: 3 }), {
      status: 200
    });
  }
  if (urlStr === "https://api.linear.app/graphql") {
    return new Response(JSON.stringify({ data: { commentCreate: { success: true } } }), { status: 200 });
  }
  if (urlStr.startsWith("https://hooks.slack.com/")) {
    return new Response("ok", { status: 200 });
  }
  return new Response(JSON.stringify({}), { status: 200 });
};

const env = {
  GITHUB_WEBHOOK_SECRET: "gh_test_secret_123",
  GITHUB_APP_TOKEN: "ghs_mock_installation_token",
  SLACK_SIGNING_SECRET: "slack_test_secret_456",
  LINEAR_WEBHOOK_SECRET: "linear_test_secret_789",
  LINEAR_API_KEY: "lin_api_mock_key"
};
const ctx = { waitUntil: () => {} };

async function runTests() {
  // 1. GitHub Webhook: Reject invalid HMAC signature (401)
  {
    const body = JSON.stringify({ action: "created" });
    const req = new Request("https://minici.dev/webhooks/github", {
      method: "POST",
      headers: {
        "x-github-event": "issue_comment",
        "x-hub-signature-256": "sha256=deadbeef"
      },
      body
    });
    const res = await worker.fetch(req, env, ctx);
    assert.equal(res.status, 401);
  }

  // 2. GitHub Webhook: D-34 Bot-Loop Guard & Backtick Prose Guard
  {
    const botPayload = JSON.stringify({
      action: "created",
      sender: { login: "minici[bot]", type: "Bot" },
      repository: { full_name: "demo/app" },
      issue: { number: 42 },
      comment: { body: "@minici shrink" }
    });
    const sig = `sha256=${hmacHex(env.GITHUB_WEBHOOK_SECRET, botPayload)}`;
    const res = await worker.fetch(
      new Request("https://minici.dev/webhooks/github", {
        method: "POST",
        headers: { "x-github-event": "issue_comment", "x-hub-signature-256": sig },
        body: botPayload
      }),
      env,
      ctx
    );
    const json = await res.json();
    assert.equal(json.ignored, true);
    assert.equal(json.reason, "bot_author_loop_guard");

    const prosePayload = JSON.stringify({
      action: "created",
      sender: { login: "alice", type: "User" },
      repository: { full_name: "demo/app" },
      issue: { number: 42 },
      comment: { body: "> @minici shrink\nYou can also run `@minici accelerate` later, LGTM!" }
    });
    const sig2 = `sha256=${hmacHex(env.GITHUB_WEBHOOK_SECRET, prosePayload)}`;
    const res2 = await worker.fetch(
      new Request("https://minici.dev/webhooks/github", {
        method: "POST",
        headers: { "x-github-event": "issue_comment", "x-hub-signature-256": sig2 },
        body: prosePayload
      }),
      env,
      ctx
    );
    const json2 = await res2.json();
    assert.equal(json2.ignored, true);
    assert.equal(json2.reason, "no_direct_at_minici_mention");
  }

  // 3. GitHub Webhook: Direct `@minici accelerate` posts comment & returns Two-Tier Rust + rust-everywhere plan
  {
    const accPayload = JSON.stringify({
      action: "created",
      sender: { login: "alice", type: "User" },
      repository: { full_name: "demo/app" },
      issue: { number: 42 },
      comment: { body: "@minici accelerate" }
    });
    const sig = `sha256=${hmacHex(env.GITHUB_WEBHOOK_SECRET, accPayload)}`;
    const res = await worker.fetch(
      new Request("https://minici.dev/webhooks/github", {
        method: "POST",
        headers: { "x-github-event": "issue_comment", "x-hub-signature-256": sig },
        body: accPayload
      }),
      env,
      ctx
    );
    assert.equal(res.status, 200);
    const json = await res.json();
    assert.equal(json.ok, true);
    assert.equal(json.action, "accelerate");
    assert.equal(json.posted_comment_id, 987654321);
    assert.match(json.reply_markdown, /MINICI-RUST-012/);
    assert.match(json.reply_markdown, /MINICI-RUST-EVERYWHERE-013/);
    assert.doesNotMatch(json.reply_markdown, /infracost|datadog|splunk|cicdcost\.com/i);
  }

  // 4. Slack Interactive Buttons (`POST /slack/interactions`) with HMAC-SHA256 verification
  {
    const ts = String(Math.floor(Date.now() / 1000));
    const formBody = new URLSearchParams({
      payload: JSON.stringify({
        type: "block_actions",
        user: { username: "alice" },
        response_url: "https://hooks.slack.com/actions/T000/B000/XXXX",
        actions: [{ action_id: "minici_action_accelerate", value: "demo/app" }]
      })
    }).toString();
    const slackSig = `v0=${hmacHex(env.SLACK_SIGNING_SECRET, `v0:${ts}:${formBody}`)}`;
    const res = await worker.fetch(
      new Request("https://minici.dev/slack/interactions", {
        method: "POST",
        headers: {
          "content-type": "application/x-www-form-urlencoded",
          "x-slack-request-timestamp": ts,
          "x-slack-signature": slackSig
        },
        body: formBody
      }),
      env,
      ctx
    );
    assert.equal(res.status, 200);
    const json = await res.json();
    assert.equal(json.ok, true);
    assert.equal(json.mapped_command, "accelerate");
    assert.ok(Array.isArray(json.reply.blocks));
  }

  // 5. Linear Native Agent API (`POST /webhooks/linear`) with Linear-Signature + GraphQL commentCreate
  {
    const linBody = JSON.stringify({
      type: "AgentSessionEvent",
      action: "created",
      data: {
        issueId: "ISS-101",
        prompt: "shrink demo/app"
      }
    });
    const linSig = hmacHex(env.LINEAR_WEBHOOK_SECRET, linBody);
    const res = await worker.fetch(
      new Request("https://minici.dev/webhooks/linear", {
        method: "POST",
        headers: { "linear-signature": linSig },
        body: linBody
      }),
      env,
      ctx
    );
    assert.equal(res.status, 200);
    const json = await res.json();
    assert.equal(json.ok, true);
    assert.equal(json.channel, "linear");
    assert.equal(json.action, "shrink");
    assert.equal(json.comment_posted, true);
  }

  // 6. Jira Cloud ADF + Google Workspace Chat cardsV2
  {
    const jiraRes = await worker.fetch(
      new Request("https://minici.dev/webhooks/jira", {
        method: "POST",
        body: JSON.stringify({ repo: "demo/app", comment: { body: "@minici migrate github-arm64" } })
      }),
      env,
      ctx
    );
    const jiraJson = await jiraRes.json();
    assert.equal(jiraJson.ok, true);
    assert.equal(jiraJson.adf.type, "doc");
    assert.equal(jiraJson.action, "migrate");

    const gchatRes = await worker.fetch(
      new Request("https://minici.dev/webhooks/gchat", {
        method: "POST",
        body: JSON.stringify({ type: "MESSAGE", message: { argumentText: "accelerate demo/app" } })
      }),
      env,
      ctx
    );
    const gchatJson = await gchatRes.json();
    assert.ok(Array.isArray(gchatJson.cardsV2));
    assert.equal(gchatJson.cardsV2[0].cardId, "minici-accelerate");
  }

  // 7. Cloud Share Security Gate: Stored-XSS CSP `sandbox` on `GET /r/:id` + 512 KB Payload Cap (`413`)
  {
    const secEnv = { ...env, MINICI_DEV_TEST_KEY: "MINICI-SEC-TEST-KEY" };
    const shareRes = await worker.fetch(
      new Request("https://minici.dev/v1/cloud/share", {
        method: "POST",
        headers: {
          authorization: "Bearer MINICI-SEC-TEST-KEY",
          "content-type": "application/json"
        },
        body: JSON.stringify({
          repo: "demo/app",
          gross_monthly_usd: 42.5,
          net_monthly_usd: 30.5,
          report_html: "<html><body><h1>Report</h1><script>alert(1)</script></body></html>",
          report_json: { repo: "demo/app" }
        })
      }),
      secEnv,
      ctx
    );
    assert.equal(shareRes.status, 200);
    const shareJson = await shareRes.json();
    const rRes = await worker.fetch(new Request(`https://minici.dev/r/${shareJson.share_id}`), secEnv, ctx);
    assert.equal(rRes.status, 200);
    const csp = rRes.headers.get("content-security-policy") || "";
    assert.match(csp, /^sandbox;/);
    assert.equal(rRes.headers.get("x-content-type-options"), "nosniff");

    const hugeRes = await worker.fetch(
      new Request("https://minici.dev/v1/cloud/share", {
        method: "POST",
        headers: {
          authorization: "Bearer MINICI-SEC-TEST-KEY",
          "content-type": "application/json"
        },
        body: JSON.stringify({ repo: "demo/app", report_html: "x".repeat(530000) })
      }),
      secEnv,
      ctx
    );
    assert.equal(hugeRes.status, 413);
  }

  console.log("✅ ALL 7 OMNICHANNEL & SECURITY WORKER SUITES PASSED (GitHub App, D-34 Guard, Slack, Linear, Jira, GChat, Stored-XSS CSP Sandbox)");
}

await runTests();
