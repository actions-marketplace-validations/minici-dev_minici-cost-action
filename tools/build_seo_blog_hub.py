#!/usr/bin/env python3
"""
Builds MiniCI's SEO + AI-Search-Optimized (GEO / AIO) Engineering Blog Hub:
  1. Copies verified Markdown sources into `docs/blog/`
  2. Renders clean static HTML pages under `web/blog/<slug>/index.html` + `web/blog/index.html`
  3. Generates `web/sitemap.xml` & `web/robots.txt` for search & AI crawlers
"""
from __future__ import annotations

import html
import json
import os
import re
import shutil
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DOCS_BLOG = REPO_ROOT / "docs" / "blog"
WEB_BLOG = REPO_ROOT / "web" / "blog"
# Same single config value as the CLI/action: MINICI_API_URL overrides the default origin.
SITE_ORIGIN = os.environ.get("MINICI_API_URL", "https://api.minici.dev").rstrip("/")

ARTICLES = [
    {
        "src_name": "article_1_10_world_scale_repos.md",
        "doc_file": "01-github-actions-cost-benchmark-10-largest-open-source-repos.md",
        "slug": "github-actions-cost-benchmark-10-largest-open-source-repos",
        "tag": "Empirical Benchmark • 10 World-Scale Repos",
        "date": "2026-11-21",
        "read_time": "6 min read",
    },
    {
        "src_name": "article_2_infracost_and_minici_full_visibility.md",
        "doc_file": "02-infracost-and-minici-full-stack-finops-pull-request-costs.md",
        "slug": "infracost-and-minici-full-stack-finops-pull-request-costs",
        "tag": "Full-Stack FinOps • Cloud IaC + CI/CD",
        "date": "2026-11-21",
        "read_time": "7 min read",
    },
    {
        "src_name": "article_3_github_actions_runner_providers_benchmark.md",
        "doc_file": "03-github-actions-runners-benchmark-blacksmith-depot-namespace-warpbuild-buildjet-runson.md",
        "slug": "github-actions-runners-benchmark-blacksmith-depot-namespace-warpbuild-buildjet-runson",
        "tag": "Runner Economics • 6-Provider vCPU+RAM Parity",
        "date": "2026-11-21",
        "read_time": "8 min read",
    },
]


def inline_md(text: str) -> str:
    # Escape HTML first, then restore inline code, bold, italic, and links
    code_spans: list[str] = []

    def stash_code(m: re.Match[str]) -> str:
        idx = len(code_spans)
        code_spans.append(f"<code>{html.escape(m.group(1))}</code>")
        return f"\x00CODE{idx}\x00"

    text = re.sub(r"`([^`]+)`", stash_code, text)
    text = html.escape(text)
    text = re.sub(
        r"\[([^\]]+)\]\(([^)]+)\)",
        lambda m: f'<a href="{m.group(2)}">{m.group(1)}</a>',
        text,
    )
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"\*([^*]+)\*", r"<em>\1</em>", text)
    for idx, span in enumerate(code_spans):
        text = text.replace(f"\x00CODE{idx}\x00", span)
    return text


def extract_meta_and_render_md(md_text: str) -> tuple[str, str, list[str], str]:
    lines = md_text.splitlines()
    h1_title = "MiniCI Engineering Blog"
    meta_title = ""
    meta_desc = ""
    keywords: list[str] = []

    for ln in lines[:20]:
        if ln.startswith("# "):
            h1_title = ln[2:].strip()
        if "Meta Title:" in ln:
            m = re.search(r"`([^`]+)`", ln)
            if m:
                meta_title = m.group(1)
        if "Meta Description:" in ln:
            m = re.search(r"`([^`]+)`", ln)
            if m:
                meta_desc = m.group(1)
        if "Target Keywords:" in ln:
            keywords = re.findall(r"`([^`]+)`", ln)

    out: list[str] = []
    i = 0
    n = len(lines)
    # Skip initial H1 and SEO blockquote metadata block
    while i < n and (lines[i].startswith("# ") or lines[i].strip() == ""):
        i += 1
    if i < n and lines[i].startswith(">") and "SEO" in lines[i]:
        while i < n and (lines[i].startswith(">") or lines[i].strip() in ("", "---")):
            i += 1

    while i < n:
        ln = lines[i]
        stripped = ln.strip()
        if not stripped:
            i += 1
            continue
        if stripped == "---":
            out.append("<hr />")
            i += 1
            continue
        if stripped.startswith("```"):
            lang = stripped[3:].strip()
            i += 1
            code_lines = []
            while i < n and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            i += 1
            cls = f' class="lang-{html.escape(lang)}"' if lang else ""
            out.append(f"<pre><code{cls}>{html.escape(chr(10).join(code_lines))}</code></pre>")
            continue
        if stripped.startswith("|") and i + 1 < n and re.match(r"^\s*\|[\s:\-\|]+\|\s*$", lines[i + 1]):
            headers = [c.strip() for c in stripped.strip("|").split("|")]
            aligns_raw = [c.strip() for c in lines[i + 1].strip().strip("|").split("|")]
            aligns = []
            for a in aligns_raw:
                if a.startswith(":") and a.endswith(":"):
                    aligns.append("center")
                elif a.endswith(":"):
                    aligns.append("right")
                else:
                    aligns.append("left")
            i += 2
            rows = []
            while i < n and lines[i].strip().startswith("|"):
                rows.append([c.strip() for c in lines[i].strip().strip("|").split("|")])
                i += 1
            th_html = "".join(
                f'<th style="text-align:{aligns[idx] if idx < len(aligns) else "left"}">{inline_md(h)}</th>'
                for idx, h in enumerate(headers)
            )
            tr_html = []
            for r in rows:
                tds = "".join(
                    f'<td style="text-align:{aligns[idx] if idx < len(aligns) else "left"}">{inline_md(c)}</td>'
                    for idx, c in enumerate(r)
                )
                tr_html.append(f"<tr>{tds}</tr>")
            out.append(f'<div class="table-wrap"><table><thead><tr>{th_html}</tr></thead><tbody>{"".join(tr_html)}</tbody></table></div>')
            continue
        if ln.startswith("#### "):
            out.append(f"<h4>{inline_md(ln[5:].strip())}</h4>")
            i += 1
            continue
        if ln.startswith("### "):
            out.append(f"<h3>{inline_md(ln[4:].strip())}</h3>")
            i += 1
            continue
        if ln.startswith("## "):
            out.append(f"<h2>{inline_md(ln[3:].strip())}</h2>")
            i += 1
            continue
        if stripped.startswith(">"):
            bq = []
            while i < n and lines[i].strip().startswith(">"):
                bq.append(lines[i].strip().lstrip(">").strip())
                i += 1
            out.append(f"<blockquote><p>{inline_md(' '.join(bq))}</p></blockquote>")
            continue
        if re.match(r"^(\*|-)\s+", stripped):
            items = []
            while i < n and re.match(r"^\s*(\*|-)\s+", lines[i]):
                item_txt = re.sub(r"^\s*(\*|-)\s+", "", lines[i]).strip()
                items.append(f"<li>{inline_md(item_txt)}</li>")
                i += 1
            out.append(f"<ul>{''.join(items)}</ul>")
            continue
        if re.match(r"^\d+\.\s+", stripped):
            items = []
            while i < n and re.match(r"^\s*\d+\.\s+", lines[i]):
                item_txt = re.sub(r"^\s*\d+\.\s+", "", lines[i]).strip()
                items.append(f"<li>{inline_md(item_txt)}</li>")
                i += 1
            out.append(f"<ol>{''.join(items)}</ol>")
            continue
        para = [stripped]
        i += 1
        while (
            i < n
            and lines[i].strip()
            and not lines[i].strip().startswith(("#", "```", "|", ">", "---", "* ", "- "))
            and not re.match(r"^\d+\.\s+", lines[i].strip())
        ):
            para.append(lines[i].strip())
            i += 1
        out.append(f"<p>{inline_md(' '.join(para))}</p>")

    return meta_title or h1_title, meta_desc, keywords, h1_title + "\x00SPLIT\x00" + "\n".join(out)


CSS = """
:root {
  --bg: #07090e; --card: #0d121b; --card-alt: #121926; --border: #1e293b;
  --text: #f8fafc; --muted: #94a3b8; --emerald: #10b981; --sky: #38bdf8;
  --mono: "JetBrains Mono", "SF Mono", Menlo, Consolas, monospace;
  --sans: -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", Roboto, sans-serif;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body { background: var(--bg); color: var(--text); font-family: var(--sans); line-height: 1.68; -webkit-font-smoothing: antialiased; }
.container { max-width: 920px; margin: 0 auto; padding: 0 24px; }
header { position: sticky; top: 0; z-index: 50; background: rgba(7,9,14,0.9); backdrop-filter: blur(12px); border-bottom: 1px solid var(--border); }
.nav { display: flex; justify-content: space-between; align-items: center; height: 60px; max-width: 1080px; margin: 0 auto; padding: 0 24px; }
.brand { display: flex; align-items: center; gap: 10px; font-weight: 700; color: var(--text); text-decoration: none; }
.pill { display: inline-block; font-size: 0.75rem; font-family: var(--mono); color: var(--emerald); background: rgba(16,185,129,0.12); border: 1px solid rgba(16,185,129,0.3); padding: 3px 10px; border-radius: 999px; }
.links a { color: var(--muted); text-decoration: none; margin-left: 18px; font-size: 0.9rem; }
.links a:hover { color: var(--text); }
article { padding: 48px 0 64px; }
h1 { font-size: 2.1rem; line-height: 1.22; letter-spacing: -0.025em; margin: 14px 0 18px; }
h2 { font-size: 1.45rem; margin: 38px 0 14px; color: #f1f5f9; border-bottom: 1px solid var(--border); padding-bottom: 8px; }
h3 { font-size: 1.15rem; margin: 26px 0 10px; color: var(--sky); }
h4 { font-size: 1.02rem; margin: 20px 0 8px; }
p, ul, ol { margin-bottom: 16px; color: #cbd5e1; }
ul, ol { padding-left: 24px; }
li { margin-bottom: 8px; }
a { color: var(--sky); text-decoration: none; }
a:hover { text-decoration: underline; }
code { font-family: var(--mono); font-size: 0.86em; background: var(--card-alt); border: 1px solid var(--border); padding: 2px 6px; border-radius: 5px; color: #e2e8f0; }
pre { background: #05070b; border: 1px solid var(--border); border-radius: 10px; padding: 16px; overflow-x: auto; margin: 18px 0 24px; }
pre code { background: none; border: 0; padding: 0; font-size: 0.82rem; line-height: 1.5; color: #e2e8f0; }
.table-wrap { overflow-x: auto; margin: 20px 0 26px; border: 1px solid var(--border); border-radius: 10px; background: var(--card); }
table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
th, td { padding: 10px 13px; border-bottom: 1px solid var(--border); }
th { background: var(--card-alt); color: var(--muted); font-size: 0.76rem; text-transform: uppercase; letter-spacing: 0.03em; }
blockquote { background: var(--card); border-left: 3px solid var(--emerald); padding: 14px 18px; border-radius: 0 8px 8px 0; margin: 18px 0; }
hr { border: 0; border-top: 1px solid var(--border); margin: 34px 0; }
.cta-box { background: linear-gradient(135deg, #0d1b18 0%, #0d121b 100%); border: 1px solid rgba(16,185,129,0.4); border-radius: 12px; padding: 24px; margin-top: 42px; display: flex; justify-content: space-between; align-items: center; gap: 18px; flex-wrap: wrap; }
.btn { background: var(--emerald); color: #04130d; font-weight: 700; padding: 10px 18px; border-radius: 8px; text-decoration: none; display: inline-block; }
.card-post { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 24px; margin-bottom: 20px; display: block; text-decoration: none; transition: border-color 0.15s; }
.card-post:hover { border-color: var(--emerald); text-decoration: none; }
.card-post h2 { border: 0; margin: 10px 0 8px; padding: 0; font-size: 1.32rem; color: var(--text); }
footer { border-top: 1px solid var(--border); padding: 32px 0; font-size: 0.84rem; color: var(--muted); display: flex; justify-content: space-between; flex-wrap: wrap; gap: 12px; }
"""


def render_page(title: str, desc: str, slug_path: str, keywords: list[str], body_html: str, is_article: bool = True, date: str = "2026-11-21") -> str:
    url = f"{SITE_ORIGIN}{slug_path}"
    ld = {
        "@context": "https://schema.org",
        "@type": "TechArticle" if is_article else "Blog",
        "headline": title,
        "description": desc,
        "datePublished": date,
        "author": {"@type": "Organization", "name": "MiniCI", "url": "https://github.com/minici-dev"},
        "publisher": {"@type": "Organization", "name": "MiniCI", "url": SITE_ORIGIN},
        "url": url,
        "keywords": ", ".join(keywords),
    }
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{html.escape(title)} | MiniCI Engineering</title>
  <meta name="description" content="{html.escape(desc)}" />
  <meta name="keywords" content="{html.escape(', '.join(keywords))}" />
  <link rel="canonical" href="{url}" />
  <link rel="icon" type="image/svg+xml" href="/minici-icon.svg" />
  <meta property="og:type" content="{'article' if is_article else 'website'}" />
  <meta property="og:title" content="{html.escape(title)}" />
  <meta property="og:description" content="{html.escape(desc)}" />
  <meta property="og:url" content="{url}" />
  <meta property="og:image" content="https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/web/social-preview-1280x640.png" />
  <script type="application/ld+json">{json.dumps(ld)}</script>
  <style>{CSS}</style>
</head>
<body>
  <header>
    <div class="nav">
      <a href="/" class="brand">
        <img src="/minici-icon.svg" width="28" height="28" alt="MiniCI" />
        <span>MiniCI</span>
        <span class="pill">Engineering &amp; FinOps Benchmarks</span>
      </a>
      <nav class="links">
        <a href="/">Calculator</a>
        <a href="/blog/">All Benchmarks</a>
        <a href="https://github.com/marketplace/actions/minici-ci-cd-cost-breakdown-pr-diff-quota-receipt">Marketplace</a>
        <a href="/#pricing" style="color:var(--emerald);font-weight:600;">Plans</a>
        <a href="https://github.com/minici-dev/minici-cost-action">GitHub</a>
      </nav>
    </div>
  </header>
  <main class="container">
    {body_html}
  </main>
  <footer class="container">
    <span>Built in pure Rust by the <a href="https://github.com/minici-dev">MiniCI maintainers</a> • Apache-2.0</span>
    <span><a href="/">Home</a> • <a href="/blog/">Blog</a> • <a href="https://polar.sh/minici/portal">Customer Portal</a> • <a href="https://github.com/minici-dev/minici-cost-action">GitHub</a></span>
  </footer>
</body>
</html>
"""


def main(src_dir: Path) -> None:
    DOCS_BLOG.mkdir(parents=True, exist_ok=True)
    WEB_BLOG.mkdir(parents=True, exist_ok=True)

    cards_html = []
    sitemap_urls = [f"{SITE_ORIGIN}/", f"{SITE_ORIGIN}/blog/"]

    for art in ARTICLES:
        src_path = src_dir / art["doc_file"] if (src_dir / art["doc_file"]).exists() else src_dir / art["src_name"]
        raw_md = src_path.read_text(encoding="utf-8")
        if src_path != (DOCS_BLOG / art["doc_file"]):
            (DOCS_BLOG / art["doc_file"]).write_text(raw_md, encoding="utf-8")

        meta_title, meta_desc, kw, split_payload = extract_meta_and_render_md(raw_md)
        h1_title, rendered_body = split_payload.split("\x00SPLIT\x00", 1)

        post_dir = WEB_BLOG / art["slug"]
        post_dir.mkdir(parents=True, exist_ok=True)

        article_html = f"""
    <article>
      <div style="margin-bottom:8px;">
        <a href="/blog/" style="font-size:0.86rem;">← Back to Engineering Benchmarks</a>
      </div>
      <span class="pill">{html.escape(art['tag'])} • {art['date']} • {art['read_time']}</span>
      <h1>{inline_md(h1_title)}</h1>
      {rendered_body}
      <div class="cta-box">
        <div>
          <strong style="font-size:1.1rem;display:block;margin-bottom:4px;">See What Every Pull Request Costs in &lt; 2 ms</strong>
          <span style="color:var(--muted);font-size:0.9rem;">Zero external Rust crates • Zero API keys • Sticky GitHub PR Cost Diff + 1-click YAML fixes.</span>
        </div>
        <div style="display:flex;gap:10px;flex-wrap:wrap;">
          <a class="btn" href="https://github.com/marketplace/actions/minici-ci-cd-cost-breakdown-pr-diff-quota-receipt">Install GitHub Action</a>
          <a class="btn" style="background:#1e293b;color:#f8fafc;" href="https://buy.polar.sh/polar_cl_SCcWdjeIJvtwoa3JKBo5ouVXBL0yBv40bzuZr36iPMm">View Plans</a>
        </div>
      </div>
    </article>
"""
        full_html = render_page(meta_title, meta_desc, f"/blog/{art['slug']}/", kw, article_html, True, art["date"])
        (post_dir / "index.html").write_text(full_html, encoding="utf-8")
        sitemap_urls.append(f"{SITE_ORIGIN}/blog/{art['slug']}/")

        cards_html.append(
            f"""<a class="card-post" href="/blog/{art['slug']}/">
  <span class="pill">{html.escape(art['tag'])} • {art['read_time']}</span>
  <h2>{inline_md(h1_title)}</h2>
  <p style="margin:0;color:var(--muted);font-size:0.94rem;">{html.escape(meta_desc)}</p>
</a>"""
        )

    index_body = f"""
    <article>
      <span class="pill">Empirical FinOps Research • Verified Live via MiniCI</span>
      <h1>MiniCI Engineering &amp; CI/CD FinOps Benchmarks</h1>
      <p style="font-size:1.05rem;">Reproducible CI/CD cost benchmarks across world-scale open-source repositories, Cloud IaC + CI/CD shift-left workflows, and hardware-matched runner ecosystems.</p>
      <div style="margin-top:32px;">
        {''.join(cards_html)}
      </div>
    </article>
"""
    index_html = render_page(
        "CI/CD FinOps Engineering Benchmarks & Guides",
        "Empirical CI/CD cost benchmarks across 10 world-scale GitHub repositories, Cloud IaC + MiniCI workflows, and 6 runner providers.",
        "/blog/",
        ["CI/CD FinOps", "GitHub Actions cost benchmark", "MiniCI", "reduce GitHub Actions bill"],
        index_body,
        False,
    )
    (WEB_BLOG / "index.html").write_text(index_html, encoding="utf-8")

    sitemap_xml = '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
    for u in sitemap_urls:
        sitemap_xml += f"  <url><loc>{u}</loc><lastmod>2026-11-21</lastmod></url>\n"
    sitemap_xml += "</urlset>\n"
    (REPO_ROOT / "web" / "sitemap.xml").write_text(sitemap_xml, encoding="utf-8")
    (REPO_ROOT / "web" / "robots.txt").write_text(
        f"User-agent: *\nAllow: /\nSitemap: {SITE_ORIGIN}/sitemap.xml\n", encoding="utf-8"
    )
    print(f"✅ Built {len(ARTICLES)} Markdown docs in docs/blog/ and static SEO/AIO HTML pages in web/blog/ + sitemap.xml")


if __name__ == "__main__":
    import sys

    src = Path(sys.argv[1]) if len(sys.argv) > 1 else DOCS_BLOG
    main(src)
