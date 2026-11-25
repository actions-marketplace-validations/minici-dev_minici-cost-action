# MiniCI in CircleCI

Offline cost breakdown and optimize preview as a job in your workflow. Fixture: `tests/fixtures/ci/circleci/`.

## Snippet

`.circleci/config.yml` (add the job and reference it in your workflow):

```yaml
version: 2.1

jobs:
  minici-cost:
    docker:
      - image: cimg/base:stable
    steps:
      - checkout
      - run:
          name: Install MiniCI
          command: |
            curl -fsSL https://raw.githubusercontent.com/minici-dev/minici-cost-action/main/scripts/install.sh | MINICI_VERSION=v0.9.2 bash
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$BASH_ENV"
      - run:
          name: Cost breakdown
          command: minici breakdown --path . --format markdown --quiet --out-file minici-cost.md
      - run:
          name: Optimize preview
          command: minici shrink --path .
      - store_artifacts:
          path: minici-cost.md

workflows:
  ci:
    jobs:
      - minici-cost
```

## Live-run env vars (blocked until a token exists)

- `CIRCLECI_TOKEN` (or `CIRCLE_TOKEN`): needed only for `--circleci` live pulls. The snippet above is offline.

## Status

- Offline `breakdown` and `shrink` on the fixture: PASS (see `results/T05.md`).
