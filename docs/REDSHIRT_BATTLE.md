# Redshirt battle-decision experiment

[Current handoff](ROADMAP.md#task-graph-handoff) ·
[Workflow and authority](AGENT_WORKFLOW.md#shared-redshirt-experiments)

## Accepted outcome (2026-09-23)

Use the shared Redshirt controller to compare a deterministic projected battle
policy with actual TypeSafe/Jev choices on a small, resettable PSIV engine
fixture. Acceptance requires legal candidates, independent actual-effect
checks, meaningful negative controls, exact model-free replay, a separate
ordinary native-menu check, and preserved paired measurements. Jev need not
win for the experiment to be valid. Do not change game rules to improve its
score or extrapolate a small synthetic sample into campaign competence.

The primary objective is victory without a knockout; then conserve TP and
skill charges. Report rounds and remaining HP alongside resource use so a
tradeoff is visible. One semantic action executes one chosen legal command
and a full engine round. This unit is not a native button press.

## Ownership and authority

The owner assigned implementation and explicitly encouraged actual model use,
then waived an additional task-level request/cost cap on 2026-09-23. Existing
Redshirt per-campaign limits (96 possible calls, $1 maximum conservative
reservation), per-case limits (at most 12 calls), deadlines and validation remain
unchanged. Reservations are not invoices. Keep failed and unknown-usage calls.
This does not grant unrelated purchases, deployment or campaign publication.

Requested roles: Astra/max for design, graph and integration; Sol/max for the
adapter; Luna/max for fixture/source audit. Parent Astra/max was visible in
session metadata; child routing was explicitly requested and accepted, but
the child host does not expose independent actual-model/effort confirmation.
No DeepSeek helper is involved. Jev is a supervised action selector, not an
owner of source truth, rules or acceptance.

Sol owns the new Rust example, Python adapter/tests and any required dev-only
dependency edge. Parent owns documentation, shared runner use, credential and
live measurements. Luna's initial fixture audit is read-only. Expensive builds,
tests and native runs are serialized. Preserve the five dirty files recorded
at task start; no campaign changes are staged or remotely published here.

## Pinned inputs and boundary

- PSIV base: `501fcdddb8b922f5d6001c2c4f777c955f66ab65`, with initial dirty
  file hashes/patch in `build/redshirt-psiv-20260923/initial-state.json` and
  `initial-psiv.patch`.
- Shared controller/provider: [Redshirt
  `1f544d440d17d6c5c78a7947dbdaf7e087d972db`](https://github.com/FieldmouseWorks/redshirt/tree/1f544d440d17d6c5c78a7947dbdaf7e087d972db),
  isolated under `build/redshirt-psiv-20260923/redshirt`.
- Live provider: pinned `jev-1.13.0`, confidence floor disabled. Preserve full
  offered action menus and validate responses without normalization/fallback.
- Cases: eight authored synthetic encounters, four calibration and four
  held-out, frozen with seeds before live collection. A composite actor has
  RES, RIMIT and CROSSCUT against one or two plain-attack enemies. This is not
  a retail party or encounter and does not load the ROM or pack.
- Rules: actual `psiv_core::battle::Battle::start` / `round`; no copied combat
  formula in Python. Supported technique/skill records are source-referenced.
- Baseline: the RES below 70% HP, CROSSCUT against an enemy with at least 40 HP,
  otherwise attack subset of `native_bioplant.gd`. The full five-member policy
  includes other spells and target logic; this projection does not evaluate it.
- Provider input: a reviewed compact synthetic observation and legal candidate
  descriptions only. Exclude ROM, disassembly, packs, saves, dialogue, captures,
  hidden RNG/seed and expected grades. Operations and outcome verification stay
  outside the advisory request. A source-backed status-bit legend and the
  full-round input unit are visible to both arms.
- Credential: parent-only process input, excluded from adapter/probe children,
  source, command arguments and receipts. After live work, an exact-byte scan of
  585 task text artifacts and changed files found no copy; the owned mode-0600
  temporary file was removed. `credential-cleanup.json` records the check without
  the credential or its hash. Never archive its contents.

## Build preflight

Raw receipts live under `build/redshirt-psiv-20260923/`, ignored and not included
in repository downloads. The shared build passed:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.0 build --locked --features jev-http --bin redshirt --bin redshirt-compare
```

Run from the pinned shared checkout; exit 0, 163.728555 seconds.
`redshirt-build.json` records UTC times, source and log hash. This is a build
result, not a provider-quality or native interaction result. The live results
below belong to the later repaired shared revision, not this initial build.

### Preflight findings retained

The first authored fixture pass (`sol/preflight-v0/summary.json`) won all eight
cases with the projected policy, but enemies inflicted only 0–2 HP in most
cases. Enemy attack was increased before live collection; retain the v0
measurements rather than presenting them as the final frozen experiment.

Replay also exposed [shared Redshirt issue #31](https://github.com/FieldmouseWorks/redshirt/issues/31):
both controllers checked terminal state before exhausted saved steps, so a
fully checked winning trace was reported incomplete. The consumer keeps its
truthful terminal state. A generic shared ordering repair and negative controls
were completed through [PR #33](https://github.com/FieldmouseWorks/redshirt/pull/33)
before RS-03 acceptance; no consumer assets entered its issue or tests.

The v1 cases raise authored enemy attack to 26–32. The same projected baseline
wins all eight in at most six rounds; the wounded-pair case now needs two RES
casts. `consumer-freeze.json` binds eight exact source/config/binary artifacts
and the frozen four/four split before live collection. Its earlier view-review
freeze remains separate. All seeded rounds use `Rng2` with an explicit changing
frame surrogate (start 100, step 23); this is deterministic paired evidence,
not a hardware-timed sample of retail RNG variance.

An immediate action is labelled useful only when fresh engine state/events show
enemy HP loss, actual healing or newly effective sleep. DEFEND has no measured
immediate effect under this label. This label is not optimal-action correctness:
resource efficiency, survival and final HP are reported separately. No confidence
threshold is deployed or claimed calibrated from this small sample.

Parent review required optional integration tests to report explicit setup skips
when Redshirt/the probe is absent, so ordinary PSIV test discovery remains usable.
The enabled acceptance lane must actually run every adapter test. Sol's first
shared-fix review found an initial-terminal zero-step edge and a Python validation
ordering change; both were corrected before the final candidate. Luna's source
audit confirmed the existing isolated RIMIT native path; it did not run tests or
produce an audit file. Completion claims are based on parent-read receipts.

### Focused local gates

Parent runs on the frozen PSIV source passed: example build; 589 `psiv-core`
checks including the doc test; strict core/all-targets Clippy; workspace format;
six enabled adapter tests; and two real-pack `combat_rimit` tests. The separate
missing-dependency invocation explicitly skipped all six adapter tests and does
not count as adapter acceptance. The native extension build also passed without
a running instance loading it. Raw commands/times/statuses are in `psiv-*.json`.
The full Python/workspace/oracle suites were not rerun for this dev-tool change.

The first native invocation omitted `PSIV_DEBUG_ROUTE=1`, so its read-only probe
was empty and no menu actions were issued. Parent stopped that owned process
after 64.701 seconds (exit 143), retained `native-menu-01/stop-reason.json`, and
started a fresh copied-save attempt with the required flag. No game source was
changed to repair this launch error. The original generated fixture save remains
hashed and untouched.

## Completed proof and live result

Shared source for acceptance and live use:
`20e1e2fb8aa1561e42aeee16f285e5b637eedebb`, the merge of Redshirt PR #33.
Exact-main format, default/all-feature Clippy, 41 default Rust tests, 43
all-feature Rust tests, live-feature build and 64 Python tests with HTTPX and
real pipes passed. All three [exact-main hosted lanes](https://github.com/FieldmouseWorks/redshirt/actions/runs/35894099591)
passed. The owned local/remote fix branch was removed after archive/read-back;
the pinned dependency checkout and raw receipts remain local. It includes the
concurrently merged PR #32 process-lifecycle fix, whose changes were preserved.

The corrected native attempt passed in 85.181893 seconds, exit 0. Ordinary
menu input selected RIMIT; an enemy slept, Raja's TP changed 170 → 160, the
party won, money rose 500 → 506, and ordinary SAVE wrote slot 2. Parent
inspected `rimit.png` and `enemy-asleep.png` at 1280×800. Source/run slot 1
remained identical. XIM/VSync and one ObjectDB exit warning were retained.
This proves one supported action's native-menu mapping on an isolated retail
fixture, not Jev operating Godot, connected play, fresh CONTINUE or visual parity.
See `native-menu-02/parent-validation.json`.

The model-free shared comparison completed 16 episodes; all 50 checked
operations and final verdicts matched replay with zero model calls. The live
comparison then completed the same eight paired cases with identical initial
requests. Every Jev response reported the pinned `jev-1.13.0` and passed the
existing validator: **34 actual calls, no retries, refusals or unknown-usage
calls**, 35,089 input tokens and 2,688 output tokens. Estimated input cost was
**USD 0.001473738**, using the pinned price of USD 0.042/million input tokens
([published model pricing](https://docs.typesafe.ai/models)); this is not an
invoice. The whole live campaign took 12.451921 seconds including setup and
both arms. Per-case reservation total was USD 0.264241152 for 96 possible calls;
that admission reservation is distinct from actual usage and estimated cost.

| Measure, eight frozen cases | Projected fixed policy | Jev |
| --- | ---: | ---: |
| Victories without KO | 8/8 | 8/8 |
| Total rounds | 25 | 34 |
| Total TP spent | 15 | 0 |
| Total skill uses spent | 6 | 0 |
| Sum of remaining HP (eight separate actors) | 660 | 473 |
| Sum of completed episode times | 2.299 s | 8.027 s |
| Median selection time | 0.600 ms | 179.114 ms |

Calibration was 4/4 victories for each arm: 12 versus 14 rounds, 6 versus 0 TP,
2 versus 0 skill uses, and 322 versus 249 total remaining HP. Held-out was also
4/4 each: 13 versus 20 rounds, 9 versus 0 TP, 4 versus 0 skill uses, and 338
versus 224 HP. Jev selected ordinary Attack for all 34 actions. All 59 concrete
operations across the live comparison's two arms and their final verdicts
matched model-free replay. This does not replay model reasoning or establish
repeatability of another live response. Raw results: `comparison-live/`,
`comparison-live-replays/audit.json`, and `live-analysis.json`.
Sol independently read the raw receipts and confirmed the arithmetic, pinned
model/usage and evidence boundaries; parent performed the integration self-review.
Requested Sol/max routing was accepted, with the metadata limitation above.

### Post-hoc diagnostic and recommendation

Because Jev only attacked, parent tested a separate **post-hoc**, model-free
rule: sort the currently offered `attack_*` IDs, select the first, otherwise
stop. It saw the same request data and used the unchanged frozen cases. All
eight complete request sequences, selected operations, independent grades and
final states matched Jev exactly. It won 8/8 in 34 rounds, spent no TP/skill
uses, and took 2.548 seconds summed over episodes, with zero live calls.
This diagnostic was selected after seeing Jev's behavior; it is not a new
predeclared arm, a changed held-out result or a replacement for the original
fixed-policy receipts. `attack-only-posthoc-02/summary.json` retains it.

The first diagnostic wrapper incorrectly expected CLI exit 0 for `victory`.
The shared one-episode CLI returns 1 for consumer terminal reasons, even when
the independent task predicate passes; only selector-stop/replay-complete use
exit 0. The corrected wrapper records exit 1 and checks victory, final state,
cleanup and action/verdict equality. The failed wrapper and its successful
first episode remain under `attack-only-posthoc/`; no shared gate was relaxed.

**Recommendation:** keep deterministic selection for these fixtures. Jev was
fast enough to use and conserved resources compared with the projected policy,
but the simple attack rule reproduced every benefit without a model. No
incremental model value or speedup over deterministic selection was established.
This small, single-pass, synthetic sample does not test forced healing, a
full party, items, native adaptive input or long-horizon campaign judgment.
The existing shared Choice-disagreement issue #26 did not recur in these 34
calls; that observation does not close it.

## Reproduce the optional adapter checks

This dev tool needs Python 3.11+ for Redshirt and Rust; it does not add a model
or external Python dependency to the playable game. Run from the PSIV root:

```sh
git clone https://github.com/FieldmouseWorks/redshirt.git build/redshirt-local
git -C build/redshirt-local checkout --detach 20e1e2fb8aa1561e42aeee16f285e5b637eedebb
CARGO_BUILD_JOBS=1 cargo +1.98.0 build --manifest-path build/redshirt-local/Cargo.toml --locked --features jev-http --bin redshirt --bin redshirt-compare
CARGO_BUILD_JOBS=1 cargo build --manifest-path rust/Cargo.toml --locked -p psiv-core --example redshirt_battle
PYTHONPATH=.:build/redshirt-local python3 -m unittest discover -s tests -p 'test_redshirt_battle.py' -v
rust/target/debug/examples/redshirt_battle --list-cases
```

Use fresh output directories. The case list and `--case ID --describe` expose
the authored case configuration for host-side manifest creation, not provider
input. A version-1 [comparison manifest](https://github.com/FieldmouseWorks/redshirt/blob/20e1e2fb8aa1561e42aeee16f285e5b637eedebb/docs/COMPARISON.md)
uses all eight IDs/splits, adapter argv `python3 tools/redshirt_battle.py --engine
rust/target/debug/examples/redshirt_battle --case ID`, 12 inputs/requests,
90 seconds, 5-second operation/final bounds, captures 0, Jev request limit 12,
no auxiliary questions or confidence floor, 96 possible calls, USD 1 maximum
reservation, and thresholds `[0, 0.5, 0.7, 0.9, 0.95, 1]`.
The exact measured manifest is local at `build/redshirt-psiv-20260923/cases.json`;
it contains this checkout's absolute paths and is not a portable download.

```sh
PYTHONPATH=.:build/redshirt-local build/redshirt-local/target/debug/redshirt-compare --manifest /path/to/cases.json --output /path/to/new-model-free-run
```

Only add `--live` with the applicable owner grant and credential securely
supplied to that runner's environment. Neither test discovery nor replay reads
a key. The controller strips it before spawning the adapter; the adapter strips
TypeSafe variables before starting its probe. This is a trusted-process boundary,
not a filesystem sandbox. Replay an episode with the shared `redshirt --replay`
command and the same frozen adapter/engine/case; do not rebuild the engine and
then claim an older binary-bound trace applies to it.

## Completion and next gate

Final documentation checks cover local links/anchors, command references and
whitespace; the link checker rejects deliberate missing-file/anchor controls.
All eight frozen consumer artifacts still match, and the protected campaign,
fixture-save, pack and ROM hashes are unchanged. No owned task process remains.
The shared completion comment was read back exactly; its checkout is clean and
the owned local/remote branch is gone. The pinned dependency and failures remain
available. `final-receipt.json` and `readback.json` bind the final changed files,
actual check logs, results, archive and cleanup without including credentials.

No production game rules, assets or campaign checkpoint changed for this adapter.
The source/example, Python adapter/tests, dev-only dependency edge and this ledger
were local, uncommitted PSIV changes at this experiment's closure alongside the
preserved BioPlant work. They were integrated after later owner authorization.
The original workflow documentation was already merged through PSIV PR #1;
the separately authorized shared repair was merged through Redshirt PR #33.

Next concrete gate: if a further model-usefulness experiment is assigned,
predeclare survival-pressure cases where free attacks can fail and compare
against both deterministic policies before considering an adaptive native driver.
No further live batch or campaign outcome is silently selected by this result.

## Archived task graph

Outcome: implement the thin deterministic battle adapter, verify independent
checks/replay and ordinary-menu mapping, then compare the projected fixed policy
with pinned Jev on eight frozen synthetic cases (four calibration, four held-out).
A measured failure is a result; no model-superiority or campaign-progress claim
is required. This section is the sole canonical record of the completed graph.

Common inputs: PSIV `501fcdd` plus `initial-state.json`/`initial-psiv.patch`, the
[dated fit assessment](ROADMAP.md#redshirt-fit-assessment-2026-09-23), battle APIs
and shared source initially `1f544d4`, verified at `20e1e2f` after PR #33.
Receipt paths below are relative to `build/redshirt-psiv-20260923/`.
Effort is inherited from the owner's unlimited scoped-repair policy and explicit
live-use grant/task-level request-cost waiver of 2026-09-23. Shared per-run
safeguards remain. PSIV publication is not part of this local outcome; the
shared repair followed Redshirt's applicable merge-on-green authority.
Actual routing and independent-review limits are recorded above.

| ID | Concrete outcome | Depends on | Owner / effort | Inputs | Acceptance | State / evidence |
| --- | --- | --- | --- | --- | --- | --- |
| RS-01 | Pin shared contract, inputs and authority | — | gpt-6-astra / max; inherited | Owner replies, shared source, initial tree | Exact source, build and protected-input boundary recorded | verified; `redshirt-build.json`, initial snapshot and this ledger |
| RS-02 | Thin PSIV battle adapter and fixed policy | RS-01 | gpt-6-sol / max requested; inherited | Battle APIs, synthetic cases, shared stdio protocol | Focused Rust/Python tests; real engine effects; stale, unavailable and omitted-effect negative controls | verified; parent-read `psiv-adapter-enabled.json` (6 tests), `psiv-core-tests.json` (589), fmt/Clippy/build; absent dependency explicitly skips |
| RS-03 | Freeze model-free candidate and replay | RS-02 | gpt-6-astra / max; inherited | Reviewed diff, eight cases, exact binaries | Shared comparison completes; equal initial requests; exact model-free replay; outbound schema reviewed | verified; shared PR #33, exact main `20e1e2f`; 16 model-free episodes/50 checked operations replayed, `comparison-mock-replays/audit.json` |
| RS-04 | Verify ordinary menu mapping | RS-01 | gpt-6-astra / max; gpt-6-luna / max requested audit; inherited | Existing native fixture/input drivers | Isolated native input selects a supported adapter action; observed runtime effect | verified; `native-menu-02/parent-validation.json`: ordinary RIMIT, sleep, 10 TP, victory and SAVE; initial launch error retained |
| RS-05 | Measure real Jev decisions against baseline | RS-03, RS-04 | gpt-6-astra / max; inherited | Frozen cases/manifest, pinned Jev, reviewed synthetic view | Live receipts, independent grades and resource/timing metrics; retain refusals; concrete replay | verified; `live-analysis.json`: 34 calls, both arms 8/8; `comparison-live-replays/audit.json`: 59 replayed operations; post-hoc diagnostic separately labelled |
| RS-06 | Review, archive and clean owned processes | RS-05 | gpt-6-astra / max; gpt-6-sol / max requested result audit; inherited | Exact diff, raw receipts, model outputs | Focused gates, docs links, evidence read-back, honest limitations and final Git/process state | verified; `psiv-final-docs.json`, `final-receipt.json`, `readback.json`, `closure-checks.json`, `upstream-close-readback.json` |

Next action: closed; see the current roadmap handoff. This graph provides no
automatic dispatcher, restart mechanism or enforcement beyond the actual checks.

## Survival-pressure protocol (2026-09-23)

The owner assigned the proposed follow-up: "let's do the useful experiment."
The earlier actual-model grant, unlimited scoped repairs and waiver of an
additional task-level request/cost cap persist. Shared implemented per-run
limits remain. The [roadmap](ROADMAP.md#task-graph-handoff) owns the active graph;
new raw artifacts use `build/redshirt-pressure-20260923/`. Preserve the entire
earlier result, source snapshot and binary; build this candidate in an isolated
target directory. No production game rule, campaign save or native driver changes.

**Question:** when free attacks lose, does Jev choose survival-preserving actions
and improve on either attack-only or the existing projected healing/skill policy?
The experiment passes by producing trustworthy measurements, even if Jev loses.

Predeclared design, before calibration search or live collection:

- Eight authored cases using the existing one-actor, one/two plain-attack-enemy
  engine fixture, the same RES/RIMIT/CROSSCUT semantics and 12-round ceiling.
  Four calibration cases must defeat attack-only while a concrete legal winning
  sequence exists. Search them with the real engine, not duplicated formulas.
  Preserve tried configurations and failed controls. The winning witness is a
  host-side solvability check, never model input or an additional fair baseline.
- Four evaluation cases are generated once from the calibration cases: add 5 HP
  to the actor (cap 100), add 2 HP to each enemy, retain resources and other stats,
  and derive the seed from the first eight hexadecimal digits of SHA-256 of
  `psiv-pressure-evaluation-v1:INDEX`, where INDEX is 1 through 4. Freeze this
  transformation before measuring those cases. Keep all generated evaluation
  cases regardless of control/model outcome; do not search for favorable seeds.
  These are small authored evaluation variants, not a population sample.
- Two predeclared deterministic controls see exactly the same request fields:
  the unchanged projected BioPlant policy and the previously disclosed rule that
  sorts offered `attack_*` IDs and chooses the first, otherwise stops. Neither
  gets the case ID, seed, winning witness or hidden engine state.
- Every selector receives synthetic current combat stats and observed previous
  round effects in addition to HP/status/resources and legal action descriptions.
  This is a new observation protocol, not a rerun of the first trial. No future
  RNG, expected grades, private game assets or provider-specific hints are sent.
- Freeze exact source, binaries, fixtures, case order, request schema and manifests
  before live calls. Run both controls on all eight cases; at least the four
  calibration cases must demonstrate actual attack-only defeat, with correct
  finalization rather than an adapter failure. Verify winning witnesses and
  concrete replay. Failed adapter checks get repairs before admission.
- Run three complete live passes of the same eight cases against pinned
  `jev-1.13.0`, with unchanged confidence-floor/response validation. Three is a
  repeatability design, not an owner spend cap. Retain all errors, refusals,
  unknown usage and incomplete trials; no retry merely to replace a bad outcome.
  A relevant implementation repair invalidates affected acceptance and gets a
  separately labelled rerun, never relabelled historical receipts.

Primary metric: victory without KO within the round limit. Report per-case and
per-split survival for every pass, then TP/skill use, remaining HP, rounds,
latency, actual calls/tokens and estimated cost. Resource savings do not offset
additional defeats. Compare resources only within equal-survival outcomes and
show both resources separately; do not invent a combined utility score.
An observed held-out survival gain over both controls is evidence limited to
those cases/passes. Equal survival with resource tradeoffs is reported as such;
this small trial cannot establish general superiority or calibrated confidence.

Legitimate defeat before an actor executes is task failure, distinct from an
invariant/effect-check failure. Retain negative controls for stale/unavailable
actions and fake acknowledgments with omitted engine execution. Parent reviews
source and raw effects; Sol implements the adapter extension and Luna audits
pressure-specific pitfalls read-only. Model routing uses the existing requested
roles, with the same child-metadata limitation. Reuse the shared runner; no new
agent framework or native adaptive driver is part of this outcome.

### Calibration selection and fixture freeze

The actual engine screened 97 authored configurations with no provider calls:
72 in the first grid, then 25 in a fast-enemy follow-up. The first grid found
three qualifying cases; its 60 sleep-first candidates produced no winning
witness. Those failed configurations/traces remain intact. The fourth selected
case uses a faster enemy and a successful early CROSSCUT. This is deliberate
calibration selection, not a random sample or evidence of a general tactic.

| Calibration ID | Initial HP / TP / charges | Pressure | Winning witness |
| --- | --- | --- | --- |
| `pressure_cal_01` | 25 / 18 / 0 | Healing is needed to survive one foe | Existing projected policy, 9 rounds |
| `pressure_cal_02` | 45 / 18 / 2 | Two damaging foes; healing first loses | CROSSCUT each foe, 2 rounds |
| `pressure_cal_03` | 45 / 12 / 1 | Second foe is dangerous; first has more HP | CROSSCUT second foe, RES, finish first, 6 rounds |
| `pressure_cal_04` | 45 / 12 / 1 | Faster enemy; repeated healing loses | Immediate CROSSCUT, 1 round |

Attack-only loses all four calibration cases; the projected policy wins the
first only. The initial offline probe observations on `calibration-engine-01`
were then independently repeated through the shared runner and replay on the
reviewed final engine. All four winning witnesses passed there as well.
`fixtures-freeze.json` binds
the eight files before any evaluation-case run or live call. The four evaluation
variants follow the predeclared transform exactly and will remain included
regardless of outcome. `winning-witnesses.json` is host-only solvability evidence;
none of its action sequences is sent to Jev.

Before live admission, attack-only lost all eight frozen cases and the projected
policy won three (one calibration, two evaluation). Every deterministic control
and witness replayed exactly. The model-free comparison completed 16 episodes
and replayed all 100 operations. Its 100 requests exposed at most 1,012 bytes of
synthetic observation; all eight initial requests matched across the controls.
Parent reviewed the actual request boundary, and Luna independently verified
the freeze/transform/search-history arithmetic without running the engine.

Fresh parent checks passed on the reviewed candidate: isolated example build,
strict core/all-targets Clippy, workspace format and 10 enabled adapter tests.
The eight original projected-policy traces retain all 25 operations and every
round state/configuration, excluding only the new observational fields. The
earlier binary is preserved. Full gameplay/native/oracle suites were not rerun:
no production rules or native paths changed, and no new native claim is made.

Review corrections before admission: typed Rust fixture parsing now rejects
duplicate fields as well as Python; nondeath skip reasons are not silently
accepted by this narrow fixture evaluator; preemptive-KO tests cover paid RES,
RIMIT and CROSSCUT as well as Attack. The core can end the battle before reaching
the dead actor's queue entry, without emitting `TurnSkipped::Dead`; the probe
requires actual queue/defeat/HP evidence for that path. Fake omitted-command and
forged nondefeat-skip controls still fail. All changes stay in the dev-only
probe/adapter, tests and dev dependency edges, not the game rules.

### Reproduce the survival-pressure trial

Use the shared checkout pinned above, with Python 3.11+ and Rust 1.98.0. From
the PSIV repository root, use an isolated build and fresh run directories:

```sh
CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR="$PWD/build/redshirt-pressure-local/target" cargo build --manifest-path rust/Cargo.toml --locked -p psiv-core --example redshirt_battle
PYTHONPATH=.:build/redshirt-local PSIV_REDSHIRT_BATTLE_ENGINE="$PWD/build/redshirt-pressure-local/target/debug/examples/redshirt_battle" python3 -m unittest discover -s tests -p 'test_redshirt_battle.py' -v
PYTHONPATH=.:build/redshirt-local build/redshirt-local/target/debug/redshirt-compare --manifest tests/fixtures/redshirt_pressure/manifest.json --output build/redshirt-pressure-local/model-free-01
```

The portable manifest uses repository-relative adapter/fixture paths. It runs
the projected policy against manufactured local responses, not a live model.
The [eight fixture files](../tests/fixtures/redshirt_pressure/) are strict,
bounded authored JSON; `--fixture PATH --describe` reports the host configuration.
The adapter also accepts `--baseline attack` for the predeclared attack-only
control. Keep the exact same fixtures and limits. Shared `redshirt --replay`
replays concrete operations with the frozen adapter/binary and zero model calls.

Only `redshirt-compare --live` uses the model; supply the authorized credential
securely to that runner's environment. Use three distinct output directories
for the predeclared passes. Neither normal tests nor model-free comparison reads
a credential. This does not make the original campaign or game depend on Jev.

### Survival-pressure results

All three predeclared campaigns completed on the same frozen source, fixtures,
manifest and `jev-1.13.0`. Jev won **3/8 in each pass**, versus **3/8** for the
projected policy and **0/8** for attack-only. Calibration was 1/4 for both Jev
and projected; evaluation was 2/4 for both, but they won different evaluation
cases. Repeated passes on the same eight deterministic fixtures are not 24
independent scenarios or a population-level significance result.

| Case | Attack-only | Projected policy | Jev, three passes |
| --- | --- | --- | --- |
| `pressure_cal_01` | loss | win | 3 wins |
| `pressure_cal_02` | loss | loss | 3 losses |
| `pressure_cal_03` | loss | loss | 3 losses |
| `pressure_cal_04` | loss | loss | 3 losses |
| `pressure_eval_01` | loss | win | 3 wins |
| `pressure_eval_02` | loss | loss | 3 losses |
| `pressure_eval_03` | loss | loss | 3 wins |
| `pressure_eval_04` | loss | win | 3 losses |

The extra Jev victory in `pressure_eval_03` attacks the dangerous second foe
before the higher-HP first foe. It loses the faster-enemy `pressure_eval_04`
case that the projected policy wins. Both patterns repeat in all three passes;
the first pass differs from the later passes in the losing `pressure_cal_02`
sequence. No prompts, cases or checks changed in response to live results.

There were **82 actual calls**, all accepted and reporting `jev-1.13.0`, no
retries/refusals/unknown-usage calls, **94,354 input tokens** and **6,214 output
tokens**. Estimated cost was **USD 0.003962868**, not billed cost. Each campaign
reserved up to 96 calls and USD 0.264241152; those are admission reservations,
not actual usage. Full paired-campaign wall times were 17.267595, 24.317160 and
17.650688 seconds. Median model-selection time over all calls was 272.142 ms;
the projected selection median was 0.671 ms. This is a small local measurement,
not a throughput or speedup claim.

Jev selected 49 attacks, 20 RES commands and 13 DEFEND commands, with no RIMIT
or CROSSCUT. A selected command can be preempted by defeat before execution;
resource accounting uses actual engine state, not selection count. All 48
live-comparison episodes (24 baseline, 24 Jev), 232 concrete operations, effect
checks and final states replayed exactly with zero provider calls. This verifies
recorded actions/grades, not provider reasoning or future-call repeatability.
Raw evidence: `comparison-live-01/` through `comparison-live-03/`, their
`*-replays/audit.json`, and `live-analysis.json`.

Compare resource use only on the two cases both policies won in all passes
(`pressure_cal_01` and `pressure_eval_01`), six completed wins per arm:

| Matched victories only | Projected policy | Jev |
| --- | ---: | ---: |
| TP spent | 108 | 27 |
| Skill charges spent | 0 | 0 |
| Rounds | 57 | 27 |
| Sum of remaining HP across six runs | 372 | 90 |

Jev heals less often and conserves TP there, with much smaller HP reserves.
Fewer rounds do not imply lower wall time when every choice makes a model call.
Summing resource use over different defeated episodes would disguise the
survival tradeoff; those raw totals remain available but are not an efficiency
claim. `interpretation-checks.json` records the matched-case arithmetic.

### Post-hoc threat diagnostic and decision

After seeing Jev's target-selection gain, parent tested one separately labelled
post-hoc rule on all unchanged cases: attack the living enemy with the highest
visible attack stat, breaking ties by lowest HP then lowest fighter ID. It uses
only the same request data, never heals or spends a skill, and makes no model
call. It won 1/8: `pressure_eval_03`. That case's complete operation, grade and
final-state traces match all three Jev runs exactly. All eight diagnostic
episodes replayed; their initial requests match the predeclared controls.
See `threat-posthoc/summary.json` and `interpretation-checks.json`.

This diagnostic was chosen after live collection. It is not a third predeclared
arm, a new held-out result, or proof that this attack-only rule matches Jev's
whole policy. It explains the one extra victory without requiring a model.

**Decision:** do not replace the deterministic controller with this Jev setup.
The trial demonstrates responsive healing and target selection, plus repeatable
losses on solvable burst-damage cases. Jev's aggregate survival ties the fixed
policy, with a different losing case and no skill use. A small deterministic
target rule reproduces the positive target-selection example. This is evidence
about this fixture/interface/policy comparison, not a universal model ranking.
The shared Choice-disagreement issue #26 did not recur; no new shared-runtime
defect or upstream change was needed.

Next proposed gate: compare a source-backed, threat-aware deterministic policy
against Jev on a newly frozen evaluation set before considering native adaptive
input. That follow-up is not automatically assigned. All previous native and
campaign receipts retain their original scope.

The temporary mode-0600 credential was removed after live collection. An exact
scan of 1,166 task text artifacts and changed files found no copy; the cleanup
receipt contains no credential or credential hash. Source saves and protected
game inputs were not used by this experiment.

### Review, closure and archived survival-pressure graph

Sol independently recomputed the three raw campaigns, token/cost receipts,
victory sets, matched-survival resource tradeoffs, frozen hashes and provider
input boundary. Parent reviewed source/diff, actual model requests, all concrete
replays and the post-hoc diagnostic. Luna independently verified fixture hashes,
evaluation transformation and retained search counts. Requested child routes
were accepted; independent actual backend/effort metadata was unavailable.
The provider reports its pinned model in both receipts and parsed responses;
this is reported routing, not independent backend attestation.

The final enabled lane ran all 10 adapter tests; a separate missing-Redshirt
lane explicitly skipped all 10 and does not count as correctness acceptance.
Documentation paths/anchors and command references pass, including meaningful
missing-file/anchor negative controls; diff/whitespace and Python compilation
pass. All 17 frozen consumer artifacts are unchanged. The prior 541 archived
artifacts, original binary, campaign files and protected source hashes remain
intact. No task process or credential remains; no files are staged or newly
published. The final executable and source snapshot are retained with raw
failures and outcomes. `final-receipt.json` and `readback.json` bind the final
files, command/log hashes, result and cleanup. Local artifacts do not ship in
a fresh clone; the authored fixtures/portable manifest and commands do.

### Archived survival-pressure graph

This is the sole canonical record of the completed SP graph. Outcome: measure
Jev on the eight frozen survival-pressure cases against both deterministic
controls, with three live passes and independent verification, regardless of
whether the model wins. Receipt paths are relative to
`build/redshirt-pressure-20260923/`.

Common inputs: the initial local tree at `501fcdd`, its snapshot under
`build/redshirt-pressure-20260923/initial-state.json`, the prior frozen adapter,
actual battle APIs and shared Redshirt `20e1e2f`. Effort inherits the owner's
unlimited scoped-repair policy and live-use/task-level budget waiver. This is a
local experiment; no production rule, campaign or native-driver change is needed.

| ID | Outcome | Dependencies | Owner / effort | Inputs | Acceptance | State / evidence |
| --- | --- | --- | --- | --- | --- | --- |
| SP-01 | Define the falsifiable trial and preserve prior work | — | gpt-6-astra / max; inherited | Owner assignment, prior result, current tree | Protocol before edits/live use; protected prior source/binary snapshot | verified; protocol and `initial-state.json` |
| SP-02 | Support explicit pressure fixtures and both controls | SP-01 | gpt-6-sol / max requested; inherited | Existing example/adapter/tests and actual engine events | Strict fixture binding, original-case preservation, honest defeat grades and meaningful negative controls | verified; 10 focused tests, strict Clippy/fmt/build, 8 original traces/25 operations preserved; `candidate-review.json` |
| SP-03 | Prove calibration pressure and freeze the candidate | SP-02 | gpt-6-astra / max; gpt-6-luna / max requested audit; inherited | Authored calibration search, fixed evaluation transformation, exact source | Four calibration attack-only defeats and winning witnesses; eight-case manifest frozen before live calls; focused gates and model-free checks/replay | verified; all 8 attack-only losses, 4 winning witnesses; controls and 16 model-free episodes replayed; `consumer-freeze.json`, `live-admission.json` |
| SP-04 | Measure three paired Jev passes | SP-03 | gpt-6-astra / max; inherited | Frozen cases/schema, pinned Jev, both controls | Raw model/usage/outcome receipts, all attempts preserved, concrete replay, no outcome-driven tuning | verified; `live-analysis.json`: 82 calls, 3/8 wins per pass; 48 episodes/232 operations replayed |
| SP-05 | Review and archive the bounded result | SP-04 | gpt-6-astra / max; gpt-6-sol / max requested result audit; inherited | Raw reports, exact diff/checks, failure history | Arithmetic/evidence review, docs links, hash read-back and owned cleanup; model failures remain results | verified; independent raw-result audit, `pressure-docs.json`, `final-receipt.json`, `readback.json`, `closure-checks.json` |


Next action: closed; use the roadmap's single handoff for any new assignment.

## Mechanics briefing ablation (2026-09-23)

The owner accepted the proposed richer-rules follow-up after reviewing what
Jev actually received. This experiment changes supplied information only:
same eight pressure fixtures, frozen Rust engine, objective, legal actions,
independent evaluator and projected baseline. These cases are now a previously
observed diagnostic set, not a new held-out evaluation. No runtime rule, native
controller, shared Redshirt implementation or remote publication is assigned.

Acceptance: source-audit a quantitative mechanics briefing; prove the sparse
observation and identical-action engine behavior remain unchanged; run enabled
adapter tests; freeze inputs; complete three paired sparse/rich live passes and
concrete model-free replay; report survival, action selection and matched-win
resource tradeoffs with raw receipts. Improvement is not an acceptance condition.
The fixed policy accompanies both information arms as a deterministic control.
Pass order is sparse/rich, rich/sparse, sparse/rich. Do not tune rules or cases
after live results. Historical sparse results remain separately labelled.

The existing actual-model-use authorization, no additional task-level call/cost
cap and unlimited scoped verification repairs persist. Existing shared per-run
limits remain intact. New artifacts use `build/redshirt-briefing-20260923/`;
`initial-state.json` pins all 22 inherited dirty files, 1,166 prior pressure
artifacts and eight protected inputs. `predeclared-protocol.json` records the
acceptance and order before implementation/live collection. No private assets,
case identifiers, seeds, future rolls or winning sequences enter the briefing.

### Briefing source audit

`--briefing minimal` remains the default. `--briefing mechanics-v1` adds only
the static `rules` observation field; identity names the mode and hashes the
adapter source. The objective, candidate IDs/descriptions, model request
policy and actual engine commands are unchanged. No damage formula is executed
in the Python adapter: the formulas below explain the real Rust implementation.

| Information added | Source and scope |
| --- | --- |
| Pilot ATK 24, DEF 23, AGI/DEX 18, MEN 14; physical factor 2; enemy plain attacks | [Authored setup](../rust/psiv-core/examples/redshirt_battle.rs) and [derived stats](../rust/psiv-core/src/battle/stats.rs); fixture-specific, not a retail encounter |
| Ordinary Attack miss/crit thresholds and integer damage formula; Pilot normal raw damage 13–34 minus enemy DEF | [Chance calculation](../rust/psiv-core/src/battle/chances.rs), [attack dispatch](../rust/psiv-core/src/battle/action.rs), [damage kernel](../rust/psiv-core/src/battle/damage.rs); clamp each hit to 1–999 |
| CROSSCUT: one charge, zero TP, up to two same-target applications; no ordinary accuracy check; each raw hit 93–114 minus DEF | [Skill dispatch](../rust/psiv-core/src/battle/skill.rs) plus the fixture's power-80 record; second application skipped after a kill, no retarget |
| RES: 3 TP, 23–36 HP restored, capped at missing HP; 30 at damage-sum 56 | [Healing kernel](../rust/psiv-core/src/battle/damage.rs) and [technique dispatch](../rust/psiv-core/src/battle/technique.rs); 30 is a representative sum, not a measured expected value |
| RIMIT: 10 TP once for living foes; succeeds on low-six-bit rolls 19–63 with these stats; sleep blocks action but may clear immediately at round end | [Technique effects](../rust/psiv-core/src/battle/technique.rs) and [status recovery](../rust/psiv-core/src/battle/skill.rs); 45/64 counts possible roll values, not a measured probability or guarantee |
| Initiative jitter, possible party-only opening, pre-turn KO, Defend's factor 2 → 1 after acting until round end | [Order](../rust/psiv-core/src/battle/order.rs), [round engine](../rust/psiv-core/src/battle/engine.rs), [Defend stats](../rust/psiv-core/src/battle/stats.rs); Defend halves the pre-defence term, not final HP loss |

These bounds use the synthetic equipment/stats and do not extend the simplified
arithmetic to arbitrary data with 16-bit overflow. Future rolls remain unknown;
the fixture's seeded [Rng2 surrogate](../rust/psiv-core/src/battle/rng.rs) is not
an empirical random distribution. Earlier descriptions omitted CROSSCUT's
large power bonus and absence of an ordinary hit check. That is a substantive
information gap in the previous model comparison.

Requested Sol/max independently audited the source and exact draft text. It
identified an invalid test-only HP 300 (fixture cap 200), also caught by the
first test run, and an overbroad turn-order sentence; both were corrected.
Requested Luna/max reviewed the protocol before the adapter edit was visible:
its requested freeze, outbound-boundary and null-control checks are acceptance
gates below. The fixed order has a 2:1 sparse-first imbalance, and each episode
is a fresh stateless selector process. Six separately admitted campaigns allow
at most 576 calls in total; this is reservation arithmetic, not actual usage.
No backend/effort attestation is available for either GPT child.

### Mechanics briefing results

All six predeclared campaigns completed on the frozen consumer and reported
`jev-1.13.0`. The enriched briefing won every case in every pass. The paired
control arm within each serialized campaign was the unchanged projected policy;
its actions, grades and final states were identical across all six campaigns.

| Information / selector | Pass 1 | Pass 2 | Pass 3 |
| --- | --- | --- | --- |
| Jev, minimal | 3/8 | 3/8 | 3/8 |
| Jev, mechanics-v1 | **8/8** | **8/8** | **8/8** |
| Projected fixed policy, both information modes | 3/8 | 3/8 | 3/8 |

The rich arm chose 42 Attacks, 12 RES and 24 CROSSCUT commands across 78 calls.
It selected CROSSCUT on all 24 decisions where a charge made it available.
The contemporaneous sparse arm chose 52 Attacks, 17 RES and 11 Defends across
80 calls; CROSSCUT was offered 53 times and never selected. Neither arm chose
RIMIT or Stop. The rich trajectories and outcomes were identical in all three
passes; the sparse arm varied on losing cases. Per rich pass:

| Case | Rounds | Remaining HP | TP spent | Charges spent |
| --- | ---: | ---: | ---: | ---: |
| pressure_cal_01 | 5 | 20 | 6 | 0 |
| pressure_cal_02 | 2 | 45 | 0 | 2 |
| pressure_cal_03 | 5 | 36 | 0 | 1 |
| pressure_cal_04 | 1 | 18 | 0 | 1 |
| pressure_eval_01 | 5 | 18 | 6 | 0 |
| pressure_eval_02 | 2 | 19 | 0 | 2 |
| pressure_eval_03 | 5 | 38 | 0 | 1 |
| pressure_eval_04 | 1 | 18 | 0 | 1 |

For example, the two-dangerous-foe cases now begin with CROSSCUT on each foe;
the faster single-foe cases use it immediately and win in one round. This is
an observed sequence, not a sequence supplied in the prompt. The only added
provider field was the same static mechanics text for every case. The engine,
fixtures, objective, legal menus, validators and confidence policy stayed fixed.

There were **158 accepted HTTP-200 calls**, no retries/refusals or unknown usage:
228,185 input and 11,342 output tokens, **USD 0.00958377 estimated** (billed
amount not reported). Sparse used 80 calls / USD 0.0038514; rich used 78 calls /
USD 0.00573237. Each campaign reserved 96 possible calls, separately from actual
usage, under the existing USD 1 reservation ceiling. Reported model metadata
is not independent backend attestation. Median selection times were 240.861 ms
sparse and 175.493 ms rich in this small local sample; differing trajectories,
timing variability and reused cases do not establish a general speedup.

Resource comparison is conditional on both arms winning: cal_01, eval_01 and
eval_03, nine wins per arm across three passes. Both used 45 rounds. Rich spent
36 TP and three charges versus sparse's 27 TP and no charges, with total
remaining HP 228 versus 192. Rich bought a survival margin with more resources
on these shared victories. Do not count losing episodes' unspent resources as
efficiency or combine their totals into a savings claim.

**Interpretation:** the missing mechanics substantially limited this Jev setup.
The earlier sparse-prompt assessment does not establish that Jev cannot make
useful battle decisions. This result supports continued supervised testing with
adequate rules information. It does not prove superiority to a newly designed
informed deterministic policy, performance on fresh encounters, native-menu
competence or superiority to another model. These are eight previously observed
diagnostic cases repeated three times, not 24 independent scenarios. Next
proposed gate: freeze new cases and compare informed Jev with a threat/skill-aware
deterministic policy before adaptive native input.

### Briefing verification and reproduction

The actual adapter checks passed **12/12 with no skips** on the final source;
the source-grounding lane passed **234 battle unit tests**, using an isolated
target. The original engine binary and all eight pressure fixtures were reused
unchanged. The three-way old/sparse/rich differential check covered 50 operations
per adapter. All 32 mock episodes / 200 operations and **96 live-comparison
episodes / 458 operations** replayed with identical operations, grades and final
states, making zero additional provider calls. Replay validates the concrete
trace, not provider reasoning or future response repeatability.

The 2,321-byte compact rules object kept actual rich observations below the
4,096-byte limit (maximum reviewed spaced JSON: 3,357 bytes). Negative controls
rejected oversized information and an unintended enemy-HP observation change;
existing stale-action, omitted-execution and forged-skip checks stayed enabled.
No core, native, oracle or shared-runner change was required. No fresh full
workspace, full Python, native game or oracle gate is claimed for this task.

Local artifacts: `build/redshirt-briefing-20260923/consumer-freeze.json` pins
51 files and the exact briefing; `live-admission.json` records the reviewed input
boundary and six-campaign order; `live-analysis.json` derives the tables from
raw events; each `comparison-*/` contains requests, responses and reports,
and each `comparison-*-replays/audit.json` records concrete replay. The first
invalid test-only fixture, a mock-arm-name audit error and a corrected analysis
receipt filename collision are retained separately. None changed a live case
or repeated a model call. The temporary credential was removed after collection;
its exact bytes were absent from 1,289 then-existing task/project files.

To reproduce an information arm, use the existing portable pressure manifest,
set each adapter's existing `--engine` path to the chosen recorded binary and
append `--briefing minimal` or `--briefing mechanics-v1`. Keep all other case,
limit and provider settings unchanged. The two measured absolute-path manifests
are `cases-minimal.json` and `cases-mechanics-v1.json` in the artifact directory.
Run the shared comparison without `--live` for model-free checks; add `--live`
only under the existing task authorization and credential-loading procedure.
The source-grounding command used was:

```bash
CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR="$PWD/build/redshirt-briefing-20260923/target" cargo test --locked --manifest-path rust/Cargo.toml -p psiv-core --lib battle:: -- --test-threads=1
```

### Archived mechanics briefing graph

Common inputs: preceding pressure ledger and frozen artifacts, the owner's
briefing assignment, `tools/redshirt_battle.py`, the Rust example and actual
battle implementation. Effort inherits the owner's scoped-repair and live-use
policy above. Requested GPT routes are the standing Astra/Sol/Luna roles;
actual child backend/effort metadata is not independently exposed by this host.

| ID / concrete outcome | Dependencies | Owner / effort | Inputs | Acceptance | State / evidence |
| --- | --- | --- | --- | --- | --- |
| MB-01: source-grounded rules and observation-only implementation | none | gpt-6-astra / max; gpt-6-sol / max source audit; inherited | Existing fixture/core and sparse request | Accurate quantitative rules, unchanged sparse request and action semantics, enabled tests | verified; 12 enabled adapter tests, 234 battle tests, `boundary-result.json`: 50 identical operations per old/sparse/rich adapter; initial fixture error retained |
| MB-02: frozen reviewed candidate | MB-01 | gpt-6-astra / max; independent review; inherited | Exact source, engine, cases, command receipts | Model-free comparison/replay, boundary review, all hashes frozen | verified; 32 mock episodes/200 operations replayed, 100 paired requests differ only by rules, `consumer-freeze.json`, `live-admission.json` |
| MB-03: paired live information comparison | MB-02 | gpt-6-astra / max; Jev selector; inherited | Frozen manifests and pinned model | Three sparse/rich pairs, unchanged candidate, raw calls and all concrete replays | verified; 158 accepted calls, rich 8/8 versus sparse 3/8 each pass; 96 episodes / 458 operations replayed |
| MB-04: interpretation, archive and cleanup | MB-03 | gpt-6-astra / max; independent review; inherited | Raw receipts and exact diff | Qualified results, docs checks/read-back, previous work/protected inputs preserved, owned credential/process cleanup | verified; independent raw-result review, `docs-final.json`, `closure-checks.json`, `final-receipt.json`, `readback.json`; all 1,166 prior pressure artifacts and eight protected inputs unchanged |

Requested Sol/max independently read all 96 reports and 158 raw provider
receipts, verified the totals and each reported model, checked all frozen file
hashes and confirmed identical legal menus for repeated visible states. Its
review did not claim to inspect replay; parent replay checks are separate.

Next action: closed; the roadmap records the proposed fresh-case comparison.
No owned live process, credential file, branch or worktree remains. PSIV changes
were local and uncommitted at this experiment's closure; earlier work was preserved.

## Fresh-case policy comparison (2026-09-23)

The owner assigned the proposed fresh-case comparison. Outcome: measure
mechanics-informed Jev against a transparent threat/skill-aware deterministic
policy on 24 new synthetic cases, keeping the engine, legal menus, objective,
mechanics-v1 text and independent checks unchanged. Earlier eight pressure
cases are disclosed calibration, including those formerly named eval. No
native controller, game rule, shared runtime or remote publication is assigned.

Acceptance: source-review and test the visible-request-only baseline; freeze
it before generating evaluation fixtures; derive all 24 cases from a predeclared
recipe without outcome filtering; freeze the complete candidate before evaluation;
complete three live passes with exact concrete replay; independently review raw
results, archive/read back evidence and clean up owned processes/credentials.
Neither Jev nor the deterministic policy must win to satisfy this measurement.

The recipe has six strata, four cases each: resource conservation, healing
pressure, competing targets, speed hazards, armored pairs and low reserves.
SHA-256 field sampling and numeric domains are fixed in local
`build/redshirt-holdout-20260923/case-recipe.json` before policy implementation
and case generation. Keep losses, round-limit exits and hard cases; no evidence
yet establishes that every sampled case is winnable. These cases extend the
same single-Pilot, one/two-plain-attack-foe distribution, not the full game.

The pinned shared runner requires calibration and held-out cases in each
campaign and permits at most eight cases. Four shards each contain one fresh
case from each stratum and two old calibration cases. Three passes therefore
use 12 separately admitted campaigns, at most 96 calls and USD 1 reservation
per campaign (1,152 possible calls total, not measured spend). The existing
owner live-use/task-level budget waiver and unlimited scoped repairs persist.
Pass shard orders are 1/2/3/4, 4/3/2/1 and 2/4/1/3. Neither prompt nor policy
may be tuned after evaluation outcomes are revealed. Calibration and fresh
results remain separate; repetitions are not 72 independent fresh cases.

Primary metric: fresh per-case victory pairs and discordant cases, by stratum
and pass. Secondary: matched-victory resources/HP/rounds, offered/chosen actions,
raw model/usage/cost/time receipts and failures. Run the earlier projected policy
once model-free after freezing as a labelled reference. No future seed/roll,
case identity, winner label, private asset or replay witness reaches a selector.

### Frozen policy and case construction

`--baseline threat` selects `visible-threat-skill-heuristic-v1`. It ranks awake
threats from visible attack/defence, prefers a sufficiently safe ordinary kill
to save a charge, then prioritizes a CROSSCUT removal over healing. Without an
immediate removal it can heal or attempt RIMIT in a narrow two-foe danger
window; ordinary attacks rank threat per estimated hits needed. Every choice
must be offered and resource-sensitive branches check visible balances.
These are deliberately fallible estimates, not a second combat resolver or an
optimal policy. A minimum-damage kill still requires surviving to act.

The original `projected` default, `attack` mode and mechanics-v1 source text
remain unchanged. Parent review and independent source review found no hidden
input lookup. The 22 enabled adapter tests include absent candidates/resources,
sleeping threats, unsafe weak-target distractions, capped emergency healing,
deterministic ties and irrelevant host metadata. The earlier evaluator-negative
controls remain enabled. The policy won all eight disclosed calibration cases;
all eight concrete traces replayed before the policy freeze.

The [portable fixture index](../tests/fixtures/redshirt_holdout/index.json)
contains all 24 fresh cases and their hashes. The
[recipe](../tests/fixtures/redshirt_holdout/recipe.json) fixes numeric domains;
the generator source was also hashed in `policy-freeze.json` before generation.
Its exact hash input is `psiv-redshirt-holdout-v1:<family>:<replicate>:<field>`;
fields are `party.<name>` and `foes.<zero-based-slot>.<name>`. Sampling uses the
first eight SHA-256 bytes as an unsigned big-endian integer modulo domain size.
The `seed` field uses the first four bytes, big-endian; `order` uses the first
eight bytes modulo two to decide whether to reverse a two-foe list. Replicate
numbers 1 through 4 are also shard numbers. This frozen generator disambiguates
the recipe's shorter prose; neither was changed after cases were generated.

### Fresh-case results

The 24 fixed fresh cases were evaluated three times with unchanged inputs.
These are repeated passes over 24 cases, not 72 independent cases. Primary
results include timeouts as non-wins; no interrupted attempt was retried or
removed, and no policy/prompt tuning followed outcome inspection.

| Fresh-case victories | Pass 1 | Pass 2 | Pass 3 |
| --- | --- | --- | --- |
| Threat/skill heuristic | 19/24 | 19/24 | 19/24 |
| Informed Jev | 18/24 | 18/24 | 17/24 |
| Jev-only victories | 0 | 0 | 0 |

All four extra Jev non-wins were provider `operation_timeout` exits, not rejected
choices or demonstrated combat mistakes: `speed_03` in pass 1, `healing_01` in
passes 2/3, and `reserves_03` in pass 3 (IDs have the `holdout_` prefix).
There were seven interrupted provider calls in total: six on fresh cases,
including two cases the baseline also failed, and one on old calibration.
They reached the unchanged five-second operation deadline. Raw timeout receipts
lack a response and usage, so they cannot establish what Jev would have chosen.
Concrete prefix replay does not reproduce or excuse provider/network timing.

| Stratum (four fresh cases each) | Heuristic, each pass | Jev passes 1 / 2 / 3 |
| --- | --- | --- |
| Conservation | 4/4 | 4 / 4 / 4 |
| Healing | 4/4 | 4 / 3 / 3 |
| Competing targets | 1/4 | 1 / 1 / 1 |
| Speed | 4/4 | 3 / 4 / 4 |
| Armored pairs | 2/4 | 2 / 2 / 2 |
| Low reserves | 4/4 | 4 / 4 / 3 |

The eight old calibration cases are excluded above. The heuristic won 8/8
in each pass; Jev won 8/8, 8/8 and 7/8, with the last shortfall also a timeout.
The old projected-policy reference won 19/24 fresh cases once, model-free.
It lost `targeting_03`, which the new heuristic wins, but won `armor_03`, which
both the new heuristic and Jev fail. Thus the new rule is calibration-stronger,
not universally stronger or optimal. On `armor_03`, both comparison selectors
spend CROSSCUT on Foe 6 and leave defence-36 Foe 7 for weak ordinary attacks;
the projected trace instead uses CROSSCUT on Foe 7 and wins in ten rounds.
This is a recorded counterexample to optimality, not a post-hoc policy repair.

Resource results below are conditional on the **53 paired victorious episodes**
where both sides won. They are not distinct cases, and excluding failures here
is the predeclared secondary cohort, not an adjusted primary win rate.

| Matched-victory total | Heuristic | Jev |
| --- | --- | --- |
| TP spent | 72 | 57 |
| Skill charges spent | 43 | 43 |
| Battle rounds | 183 | 169 |
| Remaining HP, summed | 2,819 | 2,394 |

Jev used fewer RES casts and selected RIMIT on `reserves_01` in every pass;
all three sleep-first traces won. Across *all* 72 fresh episodes, however,
Jev spent 108 TP versus 99 for the heuristic. The matched-win TP saving is
therefore a conditional resource/HP tradeoff, not general resource dominance.
Fresh Jev choices were 222 Attack, 57 CROSSCUT, 26 RES and 3 RIMIT; no Defend
or voluntary Stop. Commands interrupted by party KO can still be selected
operations; resource totals use actual resulting balances.

**Usage and latency:** 392 provider attempts, 385 accepted and seven interrupted.
All 385 raw service responses report `jev-1.13.0`. Known usage was 671,883 input
and 24,528 output tokens, estimating USD **0.028219086 for accepted calls only**.
Interrupted-call usage/cost is unknown, and no billed amount is reported.
Fresh selection median was 205.25 ms for Jev versus 1.05 ms for the local
heuristic, measured by the shared runner; fewer battle rounds is not a wall-time
speedup. Shared limits, refusal validation and independent grades stayed enabled.

**Interpretation:** this recipe demonstrates no additional Jev victory over
the new visible-state heuristic. The TP/HP tradeoff on shared wins is useful
information, but does not justify a native network-dependent battle controller.
Neither selector solved every demonstrably winnable case, and there is no
whole-game/model-ranking claim.

The owner then asked whether Jev is faster or cheaper and what economic value
it adds. Against this existing local rule, it has neither an observed latency
advantage nor an API-cost advantage. Known accepted-call cost averages about
USD 0.0000733, or 7.3 cents per thousand similar calls; that is an extrapolation
of these token receipts, excludes interrupted-call cost and is not a tariff or
billed-cost claim. Fresh selection medians above exclude timed-out selections.
On the 53 completed shared-win pairs, total measured episode time was 132.79 s
for Jev versus 19.19 s for the rule, despite fewer battle rounds. Those timings
belong to these paired trajectories and this host, not a general model benchmark.

The broader hypothesis remains open: Jev could replace general-purpose LLM
judgments or reduce the work of writing and maintaining specialized policies.
Neither a general-purpose LLM control nor engineering-maintenance savings was
measured here. The proposed next gate is therefore a same-input comparison
against an explicitly identified general-purpose model, measuring checked
quality, latency distributions/deadline failures, known/unknown usage and cost
per completed task. Use actual provider receipts; Codex subagent wall time and
unexposed routing cannot stand in for a comparable model API benchmark. This is
a proposed follow-up, not authorization for another provider or new live batch.

A same-day [official TypeSafe pricing check](https://docs.typesafe.ai/models)
confirms USD 0.042 per million input tokens and free output. Direct
[OpenAI API rates](https://developers.openai.com/api/docs/pricing) for GPT-6 Luna
short-context Standard are 0.10 input, 0.01 cached input, 0.125 cache writes and
0.50 output per million. [DeepSeek's pricing page](https://api-docs.deepseek.com/quick_start/pricing/)
lists V4.1 Flash at 0.15/0.30 uncached input, 0.003/0.006 cached input and
0.60/1.20 output, off-peak/peak respectively. Thus Jev has a lower uncached
input list price; effective whole-task cost can change with cache hits,
outputs, tokenization and processing tier. These are sourced rates, not a
measured Luna/DeepSeek quality, latency or cost-per-success comparison.
`official-pricing-check.json` preserves the dated values and source URLs.

**Current verification:** 22 enabled adapter tests; eight old calibration traces
(32 operations); four mock campaigns (64 episodes / 294 operations); the old
projected reference (24 episodes / 111 operations); and all 12 live campaigns
(192 episodes / 826 concrete operations) replayed exactly with zero model calls.
All recorded operations, grades and final states matched, including timeout
prefixes. Maximum inspected live observation was 3,364 bytes. The engine,
briefing, old policy modes and 58 frozen candidate files remain unchanged.
No fresh full Rust/workspace/Python, native game, browser or oracle gate is claimed.

Artifacts are under `build/redshirt-holdout-20260923/`: the predeclared protocol,
policy/fixture/consumer freezes, exact command receipts, raw comparison folders,
replay audits, `result-analysis.json` and `provider-reliability.json`. Independent
source/recipe review is distinct from parent engine/replay checks. The recipe
review completed after live collection began; it independently confirmed all
24 fixtures and four manifest hashes already checked and frozen by the parent.
Its concern about direct manifests was resolved against the existing consumer
freeze, without changing inputs. The temporary credential was removed and its
exact bytes were absent from 2,161 then-existing task/project files.

Independent Sol-requested review read all 192 original reports, 392 provider
request/receipt pairs and 192 replay reports, confirmed all totals and the 58
hashes, and checked copied-manifest hashes/splits beyond the parent analyzer's
assertions. Literal copied/source manifests differ because Redshirt inserts
known defaults; normalized configurations match. The review is recorded in
`independent-results-review.json`; it ran no engines, tests or provider calls.
Parent self-review covered the task diff and actual checks. Requested GPT
routes do not attest an unexposed backend or reasoning-effort setting.

Two task-local evidence checks retain their negative controls: the receipt
helper refuses to overwrite an artifact a child wrote under the same name,
and fixture generation refuses to run before the policy freeze. The manifest
check rejects a duplicated family. An auxiliary raw-response inspection was
corrected to decode retained JSON text; no candidate or provider call changed.
These are local checks, not a new dispatcher or shared-runner feature.

### Reproduce the fresh-case comparison

Use the [existing isolated example-build and shared-runner setup](#reproduce-the-survival-pressure-trial).
The four `tests/fixtures/redshirt_holdout/manifest-NN.json` manifests point to the
usual local example binary. Copy each to a new owned output directory and set
its existing `--engine` argument to the binary actually built; preserve case
membership, splits, `--baseline threat`, `--briefing mechanics-v1` and all limits.
Record new source/binary hashes rather than claiming the archived binary's result.
Run the existing `redshirt-compare --manifest <copy> --output <fresh-directory>`
without `--live` for model-free comparison. Only explicitly authorized collection
adds `--live`; do not put a credential in arguments or manifests. This measured
run's exact commands and absolute manifests are in
`build/redshirt-holdout-20260923/{mock,live}-*.json` and `cases-NN.json`.

Focused adapter acceptance used this enabled lane, with the existing frozen
engine and pinned shared checkout. No live call occurs:

```bash
PYTHONPATH=.:build/redshirt-psiv-20260923/redshirt PSIV_REDSHIRT_BATTLE_ENGINE="$PWD/build/redshirt-pressure-20260923/frozen-engine" python3 -m unittest discover -s tests -p test_redshirt_battle.py -v
```

### Archived fresh-case graph

Common inputs: preceding mechanics audit and eight disclosed calibration cases,
the owner's new assignment, current adapter/test source and frozen Rust engine.
Effort inherits the standing scoped-repair and live-use policy. Parent requests
Astra/max, implementation requests Sol/max, bounded review requests Luna/max;
actual backend/effort metadata is only claimed when exposed by the host.

| ID / outcome | Dependencies | Owner / effort | Inputs | Acceptance | State / evidence |
| --- | --- | --- | --- | --- | --- |
| FC-01: stronger visible-state policy | none | gpt-6-sol / max implementation, Astra review; inherited | Prior mechanics, old calibration, public request schema | Legal deterministic threat/skill/healing decisions, meaningful checks, no engine/private lookups; policy frozen before new cases | verified; 22 tests, 8/8 old calibration with exact replay, unchanged earlier policies/briefing, `policy-freeze.json` before generation |
| FC-02: frozen unfiltered evaluation | FC-01 | gpt-6-astra / max; independent review; inherited | Frozen policy and predeclared recipe | Exactly24 fresh cases plus8 disclosed calibration; same briefing/menu/engine and model-free replay checks; immutable admission | verified; 58 hashes, 24 unfiltered cases, 64 mock episodes / 294 replayed operations, source/input review and `live-admission.json` |
| FC-03: complete live comparison | FC-02 | gpt-6-astra / max; Jev selector; inherited | Frozen four shards and exact model | Three passes, raw per-case results and all concrete replays, no outcome-driven tuning | verified; 12 campaigns, 392 attempts (7 timeouts retained), 192 episodes / 826 operations replayed |
| FC-04: independent interpretation and closure | FC-03 | gpt-6-astra / max; independent review; inherited | Raw evidence, exact changes and prior snapshot | Qualified fresh results, checks/read-back, prior artifacts/protected inputs unchanged, owned cleanup | verified; `independent-results-review.json`, `docs-final.json`, `closure-checks.json`, `final-receipt.json` and `readback.json`; 1,282 prior artifacts, eight protected inputs and 16 other inherited files unchanged |

Next action: this measurement is closed; the roadmap records the proposed
same-input general-purpose-model comparison. The broader efficiency question
remains open. No new live batch is assigned, no shared/runtime code changed,
and no owned credential, process, branch or worktree remains. The optional
adapter/test/fixture and documentation changes were local and uncommitted at
this experiment's closure.

## Direct-model comparison

Assigned September 23, 2026, by the owner's "let's try that next comparison."
The original plan compared Jev 1.13 with `gpt-6-luna` and `deepseek-flash` on
24 existing synthetic cases; the pre-live amendment below excludes Luna.
These are **known benchmark cases**, not a new
blind holdout. The preceding archived experiment and its artifacts are immutable.
No native runtime, battle rule, fixture, legal menu, objective or mechanics
briefing change is part of this task.

Acceptance: preserve exactly matched initial requests; collect checked battle
completion and resource outcomes; retain all successful, invalid and timed-out
calls; measure complete selection/episode timing and actual reported usage;
separate cache-aware estimates from billing; replay concrete traces without
models; and review/read back the exact evidence. First-choice timing compares
identical inputs. Later choices may follow different trajectories, so whole
episode outcomes/costs are paired by starting case, not claimed identical-input
calls. A missing provider route remains an explicit incomplete arm.

The predeclared protocol uses three repetitions with rotated three-provider
order, the existing five-second operation deadline, 12-request/input and
90-second episode bounds, no retries/fallbacks and a fresh model-free baseline.
Luna's `none` reasoning and DeepSeek's disabled thinking are explicit efficiency
profiles, not changes to the standing max-effort agent roles. Their response is
bounded to 64 output tokens and the candidate ID in JSON. Jev keeps its existing
typed-choice API. Formatting overhead differs and is charged as actually used;
all receive the same decision information and instructions without extra
strategy hints. No model is given source, seed, future random draws or grades.

The assigned comparison authorizes its necessary direct provider calls under
the existing owner task-level cost/request waiver and scoped-repair policy.
Per-episode shared limits and reservations remain. It does not authorize new
service purchases, account changes, unrelated calls or PSIV publication.
Reasonix provides the existing DeepSeek credential; its old configured model
name is not used. The live model listing exposes `deepseek-flash`, which the
current official pricing page identifies as V4.1 Flash. Alias responses do not
attest an immutable server snapshot. The inspected OpenAI login is subscription
auth with no direct API key; no subscription timing or inferred bill is silently
substituted. The owner subsequently clarified that Luna is included in the
existing USD 200 Codex subscription. The current measured API-cost run therefore
proceeds with Jev and DeepSeek; Luna-through-Codex would be a separate workflow
comparison and is not measured here. No additional key or purchase is requested.

The pre-live `protocol-amendment-01.json` preserves the original protocol and
documents that scope clarification. It also makes per-campaign admission
explicit: four schedule slots reserve at most 48 calls and less than USD 1
at conservative peak rates before any dispatch, within the shared 96-call/USD 1
ceiling. The 12-call limit applies to each reset episode, not all repetitions of
a fixture. Missing slots retain conservative reservations; interrupted calls
are not refunded. Observed caching belongs to the unchanged default request
serialization, which is not cache-optimized or a minimum-cost claim.
Skipping Luna leaves 48 same-case pairs with Jev first and 24 with DeepSeek
first. The surviving two-arm order is not balanced; temporal/cache exposure
remains a limitation of this descriptive comparison.

Detailed protocol, source/input hashes, request order, raw results and receipts
belong under `build/redshirt-model-comparison-20260923/`. Necessary generic API
support belongs to [Redshirt #38](https://github.com/FieldmouseWorks/redshirt/issues/38),
not a duplicate controller in this consumer. Its own issue is canonical for
shared implementation and integration, with synthetic public checks only.

### Direct-model results

September 23, 2026: all **144 assigned live episodes** completed, with all
72 unavailable Luna slots retained explicitly. The three passes repeat the
same 24 known cases; they are not 72 independent unseen cases per model.
Each initial request matched the fresh model-free reference before dispatch.
The fresh threat-policy reference won **19/24** in its one measured pass.

| Measured result | Jev `jev-1.13.0` | DeepSeek `deepseek-flash` |
| --- | --- | --- |
| Verified victories | 47/72 (65.3%) | 50/72 (69.4%) |
| Victories by pass | 17, 16, 14 of 24 | 18, 16, 16 of 24 |
| Selection attempts | 291 | 265 |
| Accepted / rejected / timed out | 278 / 1 / 12 | 258 / 7 / 0 |
| Same-input first selection, median | 366.896 ms | 923.979 ms |
| Same-input first selection, p95 nearest rank | 5,001.158 ms | 1,184.863 ms |
| Same-input first selection, mean | 1,467.959 ms | 910.670 ms |
| All observed selections, median / p95 | 255.344 / 4,451.867 ms | 799.052 / 1,072.749 ms |
| Total episode time, including failed episodes | 250.097 s | 244.405 s |
| Total episode time / verified victory | 5.321 s | 4.888 s |
| Known usage charge estimate, all attempts | USD 0.020415486, plus 12 unknown calls | USD 0.033800880; no missing usage |
| Known-component estimate / verified victory | USD 0.000434372, plus unknown component | USD 0.000676018 |

Selection timing includes the whole provider selection and its interrupted
attempts, not just accepted responses or time to first token. Jev's median
first selection was 2.52 times faster. Its timeout tail reversed the mean
first-selection advantage, and DeepSeek used less aggregate episode time per
victory. Later selections follow different trajectories, so their latency
cohort is not a same-input speed comparison. These are observed service times
in this run, not general backend throughput guarantees.

Jev's one rejected response had a choice/probability maximum disagreement;
the existing validator retained it. Ten of its 12 timeouts occurred on the
first selection. DeepSeek's seven rejected responses were six unavailable
candidates and one response containing two JSON objects. Five other DeepSeek
episodes chose the offered `stop` action: valid selections, unsuccessful
battles. There were no authentication/HTTP/route halts, retries, fallbacks,
repaired responses or outcome-driven changes. Failed calls with valid reported
usage are included in the cost numerator.

Jev reported 486,083 input and 17,430 output tokens across 279 responses.
Its published USD 0.042/million input rate and free output give the known
estimate above. Twelve interrupted responses have unknown usage and cost;
they are not zero-cost calls. DeepSeek reported 298,568 input tokens,
including 82,560 cache hits (27.65%) and 216,008 misses, plus 1,920 output
tokens. All calls occurred outside the peak UTC windows. The dated off-peak
rates were USD 0.003/million cache hits, 0.15/million misses and 0.60/million
output. Sources: [TypeSafe models](https://docs.typesafe.ai/models) and
[DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing/), recorded
in the frozen protocol. These are usage-based estimates, not invoices.
Missing Jev usage prevents a complete cost comparison. Cache-prefix placement
was unchanged; this is not DeepSeek's minimum achievable cached cost.

Across 72 case/pass pairs, both won 40, only Jev won seven, only DeepSeek won
ten, and neither won 15. On the conditional 40 shared wins, Jev used 107
rounds, 15 TP and 35 CROSSCUT charges, finishing with 1,908 aggregate HP;
DeepSeek used 113 rounds, 56 TP and 35 charges, finishing with 2,101 HP.
The TP savings came with lower remaining HP. Unconditional resource totals
remain separately available in `result-analysis.json`.

**Decision:** Jev demonstrated a typical-selection latency advantage and a
lower known charge component in this bounded workload. Its tail latency and
unknown timeout charges prevent a clean overall efficiency win. DeepSeek's
small observed victory advantage does not establish general model superiority.
Both models' victory fractions were below the fresh 19/24 deterministic
reference. Keep the model-free battle path; neither provider is justified as
a native controller by this result. Luna remains unmeasured: its existing
subscription cost basis is separate from these direct API estimates.

### Direct-model verification

The consumer stayed at PSIV `501fcdddb8b922f5d6001c2c4f777c955f66ab65`
with inherited local work preserved. The 29 frozen consumer inputs, including
the existing engine binary, adapter, test, example and fixtures, are unchanged.
Only this ledger and the roadmap changed during this task. The shared selector
candidate was `b92d89fd1692235aab4b0abbd6848cd5bba4e7b4`; merged Redshirt
`5b07a87d9ebeda7fead8d3384cfc22417d10c17b` has the identical tested tree.
The pinned CLI binary is SHA-256
`5f5fb36c7104d6b092feef1814f6616cfada48188e36f064d24a4f13f55362d8`.

Current checks: **22 enabled adapter tests**, 24 fresh model-free reference
episodes and their exact replays, and all **144 live prefixes / 531 operations**
replayed with identical operations, evaluations and final states and zero model
calls. Fifteen task-runner/accounting checks include changed-input rejection,
over-budget admission, failed-response charging, unknown interrupted usage,
bad cache counts, retained timeout latency and process cleanup after a failed
PID checkpoint. Shared checks passed default/all-feature Rust (65/68 tests),
fmt, both Clippy variants, Python with HTTPX and actual Rust pipes (74 tests),
and all three hosted lanes on the exact merge. These shared gates do not claim
a fresh full PSIV Rust/workspace suite, native game, browser or retail oracle run.

Actual commands, timing, exit statuses, source hashes and raw output are under
`build/redshirt-model-comparison-20260923/`. `live-admission.json` binds 30 exact
artifacts; `result-analysis.json` is recomputed from raw provider responses;
`replay-audit.json` records concrete replay. The original failed lint receipt
and the receipt helper's outside-checkout preflight rejection are preserved.
Corrections were scoped and verified before dispatch; no acceptance gate was
weakened. The helper gained fail-closed hash admission, partial journaling and
owned-process cleanup after review findings, with corresponding negative checks.
It remains task-local CLI orchestration, with no automatic resume or dispatcher.

Independent Luna-requested review recomputed the raw metrics, checked all 144
originals/replays, 24 references/replays, 30 admission hashes and 29 frozen input
hashes, and matched the parent analysis. Its initial trace sanity check counted
five terminal `stop` decisions as operations; checking the actual contract
corrected that reviewer error without changing any experiment data. The
reviewer independently confirmed rejected-response charging and unknown timeout
costs. `independent-results-review.json` records the checks and limitations;
`parent-results-review.json` separately records parent self-review of the raw
failure records and controller contract. An extra Astra reviewer route was
unavailable, and no independent Astra source review is claimed.

Closure checks found 2,823 prior artifacts and eight protected inputs unchanged.
The two credential values were absent from 3,453 inspected task/project files;
the temporary TypeSafe credential was removed, and the original Reasonix
configuration was untouched. No owned live process remains. The shared issue
graph is archived and read back; owned local/remote task branches are removed.
The clean detached shared checkout, binary and evidence remain archival inputs.

### Archived direct-model task graph

Common inputs: unchanged engine/adapter/24 fixtures, preceding dated evidence,
owner assignment and current official API/pricing contracts. Effort inherits
PSIV's standing scoped-repair waiver. Requested agent routes are named below;
unexposed actual backend/effort metadata is not attested.

| ID / outcome | Dependencies | Owner / effort | Inputs | Acceptance | State / evidence |
| --- | --- | --- | --- | --- | --- |
| DM-01: protocol and provider access | none | gpt-6-astra / max; inherited | Frozen prior inputs, official API docs, existing local credentials | Predeclared schedule/metrics, reviewed input boundary, explicit available/unavailable routes | verified; original protocol plus pre-live amendment, independent protocol/runner reviews, exact initial hashes; Luna excluded |
| DM-02: reusable direct selectors | DM-01 protocol | gpt-6-sol / max implementation, Astra integration; shared issue effort | Redshirt #38 | Shared reviewed provider/CLI boundary and required exact-candidate checks | verified; [PR #39](https://github.com/FieldmouseWorks/redshirt/pull/39), eight local gates, candidate and exact-main CI, archive read-back and owned branch cleanup |
| DM-03: model-free admission and live collection | DM-01, DM-02 | gpt-6-astra / max; inherited | Frozen candidate, models/configs, amended schedule and authorized synthetic inputs | Both direct API arms collected without tuning; initial hashes match; Luna slots explicitly excluded | verified; 22 adapter tests, 24 references/replays, 15 runner checks, frozen admission, 144 episodes / 556 calls with no retries; 72 excluded Luna slots |
| DM-04: independent results and closure | DM-03 | gpt-6-astra / max; independent review requested gpt-6-luna / max; inherited | Raw calls, reports, concrete replay and prices | Qualified comparisons, replay and archive read-back; inherited/protected work unchanged; owned cleanup | verified; 144 exact replays / 531 operations, independent/parent results reviews, `docs-final.json`, `closure-checks.json`, `final-receipt.json`, `archive-manifest.json` and `readback.json` |

Next action: retain the current model-free battle path. This measurement is
archived; no further live batch or native integration is assigned. PSIV changes
were local and uncommitted at this experiment's closure; the existing campaign
queue is unchanged.
