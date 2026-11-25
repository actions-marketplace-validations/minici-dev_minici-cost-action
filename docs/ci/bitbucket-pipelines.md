# MiniCI in Bitbucket Pipelines

Offline cost breakdown and optimize preview on pull requests. Fixture: `tests/fixtures/ci/bitbucket-pipelines/`.

## Snippet

`bitbucket-pipelines.yml` (add under `pipelines:`):

```yaml
pipelines:
  pull-requests:
    '**':
      - step:
          name: MiniCI cost
          image: ubuntu:24.04
          script:
            - apt-get update && apt-get install -y curl ca-certificates
            - curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
            - export PATH="$HOME/.local/bin:$PATH"
            - minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
            - minici shrink --path .
          artifacts:
            - minici-cost.md
```

## Live-run env vars (blocked until a token exists)

- `BITBUCKET_TOKEN`: needed only for `--bitbucket` live pulls or posting PR comments. The snippet above is offline.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS after the anchor fix (see `results/T05.md`).
