# MiniCI in GitLab CI

Offline cost breakdown and optimize preview as a merge request job. Fixture: `tests/fixtures/ci/gitlab-ci/`.

## Snippet

`.gitlab-ci.yml` (add to your existing file):

```yaml
minici-cost:
  stage: test
  image: ubuntu:24.04
  before_script:
    - apt-get update && apt-get install -y curl ca-certificates
    - curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
    - export PATH="$HOME/.local/bin:$PATH"
  script:
    - minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
    - minici shrink --path .
  artifacts:
    paths:
      - minici-cost.md
  rules:
    - if: '$CI_PIPELINE_SOURCE == "merge_request_event"'
```

## Live-run env vars (blocked until a token exists)

- `GITLAB_TOKEN` (or `CI_JOB_TOKEN`, per the CLI's error message): needed only for live GitLab pulls or posting MR notes. The snippet above is offline.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS (see `results/T05.md`).
