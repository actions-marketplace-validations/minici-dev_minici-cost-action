# Empirical 220-Case 3-Arm Benchmark: Deterministic Rust vs. Direct Generative LLM vs. Real classifier `/v1/systemone`

- **Model Evaluated Live:** `qwen2.5-coder:14b` via local Ollama (`http://127.0.0.1:11434`) — **Zero Canned Dictionaries / Zero Mocked Answers**
- **Corpus Size:** `220` ground-truth cases (`27` Golden Repos + `100` Runner SKUs + `53` Billing/ERP CSVs + `20` Optimization Safety Gates + `20` Cross-Vendor Step Translations, including OOV `noul` abstention traps)

| Metric | Arm 0: Pure Rust (`minici`) | Arm A: Hybrid + Unconstrained LLM (`qwen2.5-coder:14b`) | Arm B: `minici` + Real classifier `/v1/systemone` |
| :--- | :---: | :---: | :---: |
| **Full 220-Case Corpus Accuracy** | `89.55%` (`197/220`) | `82.27%` (`181/220`) | **`95.91%` (`211/220`)** |
| **Unseen 58-Case Tier-3 Semantic Accuracy** | `65.52%` (`38/58`) | `37.93%` (`22/58`) | **`89.66%` (`52/58`)** |
| **Schema / OOV Hallucination Rate** | **`0.00%`** | `11.82%` | **`0.00%`** |
| **Harmful Misprice / Unsafe Gate Rate** | `0.00%` | `3.18%` | **`1.82%`** |
| **Cold Latency (`p50` / `p95`)** | **`6.64 ms`** / `7.56 ms` | `990.3 ms` / `4089.4 ms` | `6.78 ms` / `3044.5 ms` |
| **Warm SHA-256 Cache Latency (`p50` / `p95`)** | **`6.64 ms`** | N/A (unconstrained text) | **`0.0187 ms` / `0.0318 ms`** |

## Key Empirical Findings

1. **Zero Schema or OOV Hallucinations Under Real classifier (`0.00%` vs `11.82%`):** Unconstrained generative JSON (`Arm A`) frequently invents non-existent runner SKUs, outdated action tags (`@v2`/`@v3`), or maps non-billing columns onto financial fields. `ClassifierEngine` (`tools/jev_systemone_server.py`) constrains logits via JSON-Schema `enum` + Classifier L0 cyclic-shift debiasing + `noul` abstention, eliminating out-of-vocabulary hallucinations (`0.00%`).
2. **Semantic Generalization Beyond Regex/Lookup Tables (`95.91%` vs `89.55%`):** Deterministic Rust (`Arm 0`) achieves `100%` on all canonical/cloud SKUs and 5-vendor CSVs in `~6.6 ms`, while Real classifier unlocks semantic comprehension on unseen corporate runner pools, foreign German/French/Spanish/SAP ERP billing columns, and cross-vendor CI step translations.
3. **Sub-50-Microsecond Warm Amortized Speed (`0.0187 ms`):** Because every `/v1/systemone` decision is content-addressed by `(model, prompt, choices, noul)` SHA-256 hash and persisted in `~/.minici/jev_decisions_cache.json` + `~/.minici/learned_sku_registry.json`, warm CI runs pay zero neural latency penalty.
