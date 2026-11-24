# Verified GitHub Organization, `$0` DNS Domain (`minici.is-a.dev`) & Polar.sh Storefront Setup

This guide provides the exact zero-cost (`$0.00/mo`) workflow to activate:
1. **GitHub Free Organization (`minici-dev`)** + repository transfer (`301` auto-redirect on GitHub)
2. **Green "Verified" Creator Badge on GitHub Marketplace** using the free developer domain `minici.is-a.dev` (`TXT` DNS verification)
3. **Public Polar.sh Storefront (`https://polar.sh/minici`)** connected to `.github/FUNDING.yml`

---

## 1. Activate Polar.sh Public Storefront (`2 Minutes` in Open Chrome Tab)

`.github/FUNDING.yml` (`polar: minici`) and all web/README Sponsor links are already wired in `v0.9.0`. Currently `https://polar.sh/minici` returns `404` until Storefront is toggled ON in your open Chrome tab (`https://polar.sh/dashboard/minici`):

1. Run `POLAR_OAT="polar_oat_..." python3 tools/setup_polar_storefront.py` (or create in **`https://polar.sh/dashboard/minici/products`**) to provision the 3 commercial Cloud tiers with automatic `MINICI-...` License Key delivery:
   - **MiniCI Starter** (`$1/month` — `minici share` unlisted cloud links, 30-day trend ledger for up to 3 repos, live SVG badge)
   - **MiniCI Pro** (`$10/month` — Automated Friday Slack FinOps digest, real-time 1.5x spend velocity alerts, 15-repo dashboard, 365d retention)
   - **MiniCI Team Flex** (`Custom/month` pay-what-you-want — Unlimited repos, central org PR budget gate policy, synced enterprise custom runner rates)
2. In **`https://polar.sh/dashboard/minici/settings/webhooks`**, add endpoint **`https://minici.dev/v1/webhook/polar`** (events: `subscription.*`, `benefit_grant.*`) and bind `POLAR_WEBHOOK_SECRET` in Cloudflare Worker secrets.
3. Go to **`https://polar.sh/dashboard/minici/storefront`** (or **Settings -> Storefront**) and toggle **Enable Public Storefront** -> **Save**.
3. Verify `curl -sI https://polar.sh/minici` returns `HTTP/2 200`. Every PR and repo visitor clicking **Sponsor** on GitHub now lands directly on your Polar checkout page.

---

## 2. Create `$0` GitHub Organization (`minici-dev`) & Generate DNS `TXT` Challenge

> Verified live: `minici-dev`, `minici-sh`, `minici-hq`, and `getminici` are **available** on GitHub (`$0` Free Organization plan). GitHub requires solving an interactive browser CAPTCHA to create an organization.

1. Open **`https://github.com/account/organizations/new?plan=free`**.
2. Set:
   - **Organization account name:** `minici-dev`
   - **Contact:** maintainer contact (see SUPPORT.md)
   - **This organization belongs to:** *My personal account* -> Solve CAPTCHA -> **Next / Complete setup**.
3. Open **`https://github.com/organizations/minici-dev/settings/domains`** -> click **Add a domain** -> enter:
   ```text
   minici.is-a.dev
   ```
4. Copy the exact `TXT` record key and token shown by GitHub (for example `_github-pages-challenge-minici-dev.minici` -> `"<HEX_TOKEN>"`).

---

## 3. Claim Free `minici.is-a.dev` Subdomain & Verify Badge (`$0.00`)

`minici.is-a.dev` is **available** on [`github.com/is-a-dev/register`](https://github.com/is-a-dev/register). Create `domains/minici.json` in a fork of `is-a-dev/register` with both your Cloudflare Worker `CNAME` (or URL redirect) and the GitHub Organization verification `TXT` value:

```json
{
  "description": "MiniCI — Open-Source CI/CD Cost Breakdown, PR Diff & Quota Receipt Engine",
  "repo": "https://github.com/<current-owner>/minici-cost-action",
  "owner": {
    "username": "<current-owner>",
    "email": "<owner-email>"
  },
  "records": {
    "CNAME": "minici.dev",
    "TXT": [
      "PASTE_GITHUB_ORG_CHALLENGE_TOKEN_HERE"
    ]
  }
}
```

> Note: If GitHub specifies a nested host label `_github-pages-challenge-minici-dev.minici.is-a.dev`, create `domains/_github-pages-challenge-minici-dev.minici.json` with `"records": { "TXT": ["<HEX_TOKEN>"] }` alongside `domains/minici.json` in the same Pull Request to `is-a-dev/register`.

Once `is-a-dev/register` merges the PR (~15 minutes to a few hours):
1. Return to **`https://github.com/organizations/minici-dev/settings/domains`** and click **Verify**.
2. Set `https://minici.is-a.dev` as the Organization Profile Website -> green **Verified** pill goes live on `github.com/minici-dev`!
3. Open **`https://github.com/organizations/minici-dev/settings/actions_marketplace`** and accept the GitHub Marketplace Verified Creator agreement.

---

## 4. Transfer `minici-cost-action` & Update Repo Slug in 1 Command

1. Open **`https://github.com/<current-owner>/minici-cost-action/settings`** -> scroll to **Danger Zone -> Transfer ownership** -> select **`minici-dev`**.
   - GitHub automatically preserves all stars, releases, tags (`v1`, `v0.9.0`), Marketplace listing URL, and `301` Git/HTTPS redirects from `<current-owner>/minici-cost-action@v1`.
2. Run the built-in slug migration script locally and push a release tag:
   ```bash
   python3 tools/migrate_repo_slug_to_org.py --org minici-dev --apply
   git remote set-url origin https://github.com/minici-dev/minici-cost-action.git
   ```
