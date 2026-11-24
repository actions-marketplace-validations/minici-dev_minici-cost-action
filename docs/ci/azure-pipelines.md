# MiniCI in Azure Pipelines

Offline cost breakdown and optimize preview as a pipeline job. Fixture: `tests/fixtures/ci/azure-pipelines/`.

## Snippet

Add to `azure-pipelines.yml` as a job or stage:

```yaml
pool:
  vmImage: ubuntu-latest

steps:
  - checkout: self
  - bash: |
      curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
      echo "##vso[task.prependpath]$HOME/.local/bin"
    displayName: Install MiniCI
  - bash: |
      minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
      minici shrink --path .
    displayName: MiniCI cost and optimize preview
  - publish: minici-cost.md
    artifact: minici-cost
```

## Live-run env vars (blocked until a token exists)

- The CLI reads no Azure DevOps token for this flow. Live cost data for this platform comes from a billing export passed with `--billing-csv <FILE>`. The export is not yet available.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS with a caveat. The Azure optimize savings include artifact retention and are not actionable (see `results/T05.md`).
