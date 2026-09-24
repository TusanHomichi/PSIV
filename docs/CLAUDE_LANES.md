# Claude Code lane ledger

Adoption record and reliability log for the Claude Code routing in
[CLAUDE.md](../CLAUDE.md): Opus orchestrates, and DeepSeek `deepseek-flash`
at max reasoning delivers through `tools/ds-lane`.

## Adoption

Owner instruction, 2026-09-23. The owner asked for the route to stay
confined to Claude Code sessions. It first landed in the shared
`AGENTS.md`/workflow docs (swept into PR #2 while uncommitted) and was moved
here and to `CLAUDE.md` on the same day.

Route choice: Reasonix was chosen over Claude Code pointed at DeepSeek's
Anthropic-compatible endpoint. Reasonix validates the effort level
(`disabled|low|high|max` for flash; other values are rejected), provides an
OS sandbox, and reports cost and trajectories. How that endpoint maps Claude
Code's thinking settings to DeepSeek effort was not established, and that
route was not tested.

## Harness verification

2026-09-23, Reasonix v1.38.2. Smoke lanes were removed afterward.

- Headless `reasonix run --effort max` succeeded, and reasoning tokens were
  recorded.
- Sandbox: `write_file` and bash writes outside the worktree and into a
  `--link`ed input were blocked. `git add`/`commit` inside the worktree failed
  on the read-only `.git`. `cargo check -p psiv-core` ran inside the lane.
- A lane committed only its owned file and excluded the link. `resume`
  continued the original session with its context. `--read-only` produced
  constraint blocks and no diff.
- Four simultaneous lanes: three ran and one queued, then ran once a slot
  freed. That lane's waiting foreground process was killed while queued; the
  detached supervisor still ran it and committed. With `CARGO_BUILD_JOBS=8`
  in the caller, the worker saw `2`.
- Harness defects found and fixed:
  - The preamble's negated wording tripped the Reasonix constraint ban.
  - A symlink to an ignored directory was committed; a `dir/` ignore rule
    does not match a symlink.
  - `reasonix run` omits `session_id` from its JSON result.
  - Block counting matched the worker's own prose.
- At max effort, a one-line edit used 10 tool rounds; the cost was
  USD 0.005.

## Reliability log

One row per lane. Include the orchestrator's corrections and any rejected
output.

| Date | Lane | Task | Outcome | Orchestrator corrections | Cost |
| --- | --- | --- | --- | --- | --- |
| 2026-09-23 | bake-D | Split `battle/engine_tests.rs` (1,002 lines) into topic child modules (routine refactor) | Accepted as delivered. Independent rerun: 41/41 tests before and after, identical names, fmt clean, zero original lines lost; largest file 392 lines | None to the code. Its cited logs were in the sandbox-private `/tmp` and gone; fixed in the harness (`build/lane-evidence/`). It noticed a stale test path outside its write set and correctly left it; fixed at integration | USD 0.029, 3.5 min |
| 2026-09-23 | bake-A | Inventory all enemy regular abilities with dispatch/effect citations (RE exploration + ledger) | Accepted: `docs/ENEMY_ABILITIES.md`. Independent checks: 83/83 ability ids match `enemies.json`; 8 sampled `ps4.asm` citations land on their labels; all four "port gap" claims match the code; it caught the fork's `if bugfixes` substitution for effects `$13-$16` | Added the `docs/README.md` link it flagged as outside its write set | USD 0.227, 15.6 min |
| 2026-09-23 | bake-E | Harden `ds-lane`: hermetic test suite, phrasing preflight, write sets, timeout, `verify`, `tail`, receipt in summary | Accepted. Independent: 24/24 in 20.4 s via `verify`; a trap `reasonix` on `PATH` proved no test reaches the real worker. It correctly declined a spec item that would have disarmed `--read-only` | Found a group-kill gap in its timeout path (SIGKILL skipped when the parent exits on SIGTERM); sent back with the `stop` command as run 2 | USD 0.068, 9.4 min |
| 2026-09-23 | bake-B | Extend Acid Breath `$33` to 76/85/86 from the retail routines (RE + implementation) | Accepted. Its `EnemyAttackOffs` routing, the Piercer `$33` arm and the `loc_23AB6` versus `loc_24AEC` damage-request equivalence were read directly from `ps4.asm`; its tests reran green via `verify`; its negative control fails with the old gate | None | USD 0.133, 13.4 min |
| 2026-09-23 | bake-C | Crawler POISON `$11` (RE + implementation + ledger) | Accepted. `Battle_CalculateChances` formula and the `d4` hit-chance threshold checked in `ps4.asm`; its choice to emit no event on a miss is grounded in `loc_6652` and matches the existing attack-poison path | Resolved an `enemy_skill.rs` merge overlap with bake-B; fixed the inventory counts and index link it flagged as outside its write set. Merged tree: psiv-core 598 passed; 11 combat/boss runtime tests; clippy and fmt clean | USD 0.139, 13.8 min |
| 2026-09-23 | bake-E run 2 | `ds-lane stop` plus the group-kill fix from review | Accepted. Independent: 28/28 via `verify` with the trap binary unused and no orphans left behind. It found that its own first orphan test was vacuous (it passed with the fix reverted) and replaced it with a SIGTERM-ignoring child | None | USD 0.041, 5.8 min |
| 2026-09-24 | ab-F | Record-driven damage-skill resolver with a verified route table; FLAME BOLT | Accepted. The Helex/ForcedFly route (one guarded `#$C`) was read from `ps4.asm`; 603 core tests plus four runtime targets rerun green | None (run 1 was stalled by a host suspend; resumed by hand) | USD 0.120 |
| 2026-09-24 | ab-G | Route survey of all 146 (enemy, damage-ability) pairs | Accepted. Its key finding: HP damage comes only from `#$C`, so route class, not target nibble, decides. SPIRAL BLD and SAND STORM were spot-read | None | USD 0.264 |
| 2026-09-24 | ab-I | `ds-lane` package split and stall watchdog | Accepted on run 2 | Run 1's any-CPU-gain rule could not see a hung worker (idle Node measured 0.03% of a core); sent back for a 1% rate threshold | USD 0.152 |
| 2026-09-24 | ab-J | Route-class gate and the 21 Motavia single-target routes | Accepted. TechUser WAT and Rappy ROUND EYES were read from `ps4.asm`; 614 core tests, 5/5 real-pack | Stripped `.reasonix/` host state that the lane commit swept in (fixed in the harness by ab-L) | USD 0.245 |
| 2026-09-24 | ab-H | FloatMine2 `$07` / `$17` wasted turns | Accepted on run 3. `loc_10406` read from `ps4.asm`; merged-tree gate: core 617, runtime 108, godot 75 | Sent back: test file over 1,000 lines, and a native gap (Godot timeline wildcard), fixed by making `BattleEvent` exhaustive. Resolved merge overlaps; removed a dead `ui.rs` check | USD 0.319 |
| 2026-09-24 | ab-L | Exclude `.reasonix/` from lane commits; honor follow-up write sets | Accepted on run 2; 41/41 with the trap binary unused | Run 1 left the test file at 1,034 lines, arguing the rule covered only the package; sent back to split it | USD 0.095 |
| 2026-09-24 | ab-K | All-party damage class: SPIRAL BLD and EARTHQUAKE | Accepted on run 2. Slot order read from `Battle_UpdateFighters` (`ps4.asm:987`); merged-tree gate: core 626, runtime 109, godot 75 | Sent back to split `enemy_damage_tests.rs` (1,504 lines, grown by ab-J and missed in that review, which led to ab-M); doc recount at merge | USD 0.435 |
| 2026-09-24 | ab-M | `ds-lane` flags files over 1,000 lines | Accepted; 51/51 with the trap binary unused | None | USD 0.068 |

**Bake-off result (2026-09-23):** six runs across five lanes (routine
refactor, RE inventory, harness tooling, two retail-parity implementations),
all accepted after independent verification, for USD 0.64 in total.
Orchestrator corrections were integration-level only (a merge overlap, index
links, counts) plus one harness bug found in review. Retail-parity enemy
ability work routes to DeepSeek by default, one ability per lane, from
[the inventory](ENEMY_ABILITIES.md). Keep independent verification of every
disassembly claim that a change rests on.

`tools/ds-lane` is now a shim over the `tools/ds_lane/` package (split
2026-09-24; every module is under 400 lines). Its hermetic suite is
`tests/test_ds_lane_{unit,lanes,supervisor}.py` plus `tests/ds_lane_support.py`,
each under 500 lines.

## Method decision: coverage-first, oracle-judged (2026-09-23)

Owner-approved direction for battle-rule parity. Gaps were previously found one
at a time by playing next to an emulator. Instead:

1. **Enumerate** the finite rule surface from the disassembly (for example
   [the enemy ability inventory](ENEMY_ABILITIES.md): 83 abilities, 44 effect
   handlers, 74 attack routines, 504 formations), so gaps are known before play.
2. **Build shared mechanisms** with verified route tables (for example a
   record-driven damage-skill resolver whose table rows each cite their retail
   route) instead of one resolver per ability.
3. **Judge in bulk with a differential battle oracle.** Inject the same
   formation, party and RNG seed into the headless emulator (`oracle/`,
   `--ram-patch`) and into `psiv-core`, run scripted turns, and compare
   per-action battle state (HP, status, RNG) to locate the first divergence.
   Sweep formations and seeds to generate the gap worklist, and make "zero
   divergence on formations carrying X" each ability lane's acceptance.

Side-by-side emulator play remains for presentation (timing, palettes, feel),
which needs eyes. Rule parity is a number comparison.

Existing capability: headless libretro host, tapes, per-frame named-RAM logs
(782-entry map with battle groups), `--ram-patch` fixture injection, saved
battle states and `engine_tests_oracle.rs`. Missing: forced formation entry by
patch, a scripted battle-command driver, a port replay from the injected state,
and a per-action comparator.

**Next action:** after lanes F, G and H integrate, scope the differential
battle oracle: prove forced entry and one formation's full-battle comparison
end to end before building the sweep.
