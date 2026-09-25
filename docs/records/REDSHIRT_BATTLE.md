# Redshirt battle-decision experiment (archived)

**Status: archived 2026-09-24** by owner decision. After thorough testing
elsewhere, the project moved on from Redshirt and its TypeSafe/Jev model
provider. The build-run-extend instructions that once lived here are gone from
the [workflow](../AGENT_WORKFLOW.md) and the [development setup](../DEVELOPMENT.md),
and no paid model service is authorized. PSIV keeps its model-free battle path.

## What the experiments were

A thin PSIV consumer adapter (`tools/redshirt_battle.py` over a `psiv-core`
example) let the shared Redshirt controller choose among legal actions in one
small, resettable synthetic battle driven by `psiv_core::battle::Battle`, and
compared the unchanged deterministic projected policy with the pinned
TypeSafe/Jev selector on frozen authored cases. No ROM, pack, save, native
input or campaign rule took part. Five stages ran on 2026-09-23: an eight-case
first proof, a survival-pressure trial, a sparse-versus-mechanics briefing
ablation, a fresh-case policy comparison against a new threat/skill-aware
rule, and a direct-model comparison of Jev with DeepSeek Flash.

## Headline results

Every quoted figure below is copied verbatim (Markdown emphasis included) from
this ledger's own results sections at
`be8931ad58813a3f22fbf3d7d6d6c9d3c1da71f8`.

- **First proof, eight frozen cases:** "| Victories without KO | 8/8 | 8/8 |";
  "Calibration was 4/4 victories for each arm" and "Held-out was also 4/4
  each". Jev made "**34 actual calls, no retries, refusals or unknown-usage
  calls**", "35,089 input tokens and 2,688 output tokens", and "selected
  ordinary Attack for all 34 actions". A separately labelled post-hoc
  attack-only rule "won 8/8 in 34 rounds", so "No incremental model value or
  speedup over deterministic selection was established."
- **Survival pressure:** Jev "won **3/8 in each pass**, versus **3/8** for the
  projected policy and **0/8** for attack-only", on different victory sets,
  with "**82 actual calls**". A post-hoc threat-first rule "won 1/8" and
  reproduced Jev's one extra victory without a model.
- **Mechanics briefing ablation:** "| Jev, minimal | 3/8 | 3/8 | 3/8 |";
  "| Jev, mechanics-v1 | **8/8** | **8/8** | **8/8** |"; "| Projected fixed
  policy, both information modes | 3/8 | 3/8 | 3/8 |".
- **Fresh-case policy comparison:** "| Threat/skill heuristic | 19/24 | 19/24
  | 19/24 |"; "| Informed Jev | 18/24 | 18/24 | 17/24 |"; "| Jev-only
  victories | 0 | 0 | 0 |". All four extra Jev non-wins were provider
  `operation_timeout` exits. "The old projected-policy reference won 19/24
  fresh cases once, model-free."
- **Direct-model comparison:** "| Verified victories | 47/72 (65.3%) | 50/72
  (69.4%) |" for Jev versus DeepSeek Flash over three passes; "Jev's median
  first selection was 2.52 times faster"; known charge estimates of "USD
  0.020415486, plus 12 unknown calls" versus "USD 0.033800880"; and "The fresh
  threat-policy reference won **19/24** in its one measured pass."
- **Decision:** "Keep the model-free battle path; neither provider is
  justified as a native controller by this result."

## Where the full record lives

The complete ledger, adapter, example, tests and fixtures are preserved in Git
at `be8931ad58813a3f22fbf3d7d6d6c9d3c1da71f8`:

- `docs/records/REDSHIRT_BATTLE.md` — the full experiment ledger
- `tools/redshirt_battle.py` — the PSIV consumer adapter
- `rust/psiv-core/examples/redshirt_battle.rs` — the synthetic battle example
- `tests/test_redshirt_battle.py` — the adapter tests
- `tests/fixtures/redshirt_pressure/` — survival-pressure fixtures
- `tests/fixtures/redshirt_holdout/` — fresh-case fixtures and manifests

Read any of them with `git show be8931ad:<path>`. Historical records elsewhere
in `docs/` stay as they were written.
