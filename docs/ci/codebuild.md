# MiniCI in AWS CodeBuild

Offline cost breakdown and optimize preview as a build step. Fixture: `tests/fixtures/ci/codebuild/`.

Note: this snippet is a separate CodeBuild project (or a pre-build step in an existing `buildspec.yml`) that runs MiniCI against the source. It does not change your `buildspec.yml` build.

## Snippet

`buildspec-minici.yml`, used by a CodeBuild project that checks out the repo:

```yaml
version: 0.2

phases:
  install:
    commands:
      - curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
  build:
    commands:
      - export PATH="$HOME/.local/bin:$PATH"
      - minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
      - minici shrink --path .

artifacts:
  files:
    - minici-cost.md
```

## Live-run env vars (blocked until a token exists)

- The CLI reads no CodeBuild token. Live cost data for this platform comes from a billing export passed with `--billing-csv <FILE>` to `breakdown`. The export and its source (AWS Cost and Usage Report) are not yet available.
- `AWS_REGION` (optional): sets the pricing region; defaults to `us-east-1`.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS (see `results/T05.md`).
