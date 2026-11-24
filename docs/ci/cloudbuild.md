# MiniCI in Google Cloud Build

Offline cost breakdown and optimize preview as a build step. Fixture: `tests/fixtures/ci/cloudbuild/`.

Note: run this as a separate trigger or as an extra step in `cloudbuild.yaml`. Cloud Build mounts the source at `/workspace`, so `--path .` analyses the repo.

## Snippet

`cloudbuild-minici.yaml`:

```yaml
steps:
  - name: ubuntu:24.04
    id: minici
    entrypoint: bash
    args:
      - -c
      - |
        apt-get update && apt-get install -y curl ca-certificates
        curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
        export PATH="$HOME/.local/bin:$PATH"
        minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
        minici shrink --path .

timeout: 600s
```

## Live-run env vars (blocked until a token exists)

- The CLI reads no Cloud Build token. Live cost data for this platform comes from a billing export passed with `--billing-csv <FILE>`. The export is not yet available.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS (see `results/T05.md`).
