# MiniCI in GitHub Actions

Offline cost breakdown and optimize preview on every pull request. Fixture: `tests/fixtures/ci/github-actions/`.

## Snippet

`.github/workflows/minici.yml`:

```yaml
name: MiniCI cost
on: pull_request

permissions:
  contents: read

jobs:
  minici:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
      - name: Install MiniCI
        run: curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
      - name: Add MiniCI to PATH
        run: echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - name: Cost breakdown
        run: minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
      - name: Optimize preview
        run: minici shrink --path .
      - uses: actions/upload-artifact@v4
        with:
          name: minici-cost
          path: minici-cost.md
```

## Live-run env vars (blocked until a token exists)

- `GITHUB_TOKEN` (or `GH_TOKEN`): needed only for live GitHub data pulls. The snippet above is offline and needs none.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS (see `results/T05.md`).
