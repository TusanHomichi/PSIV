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

**Bake-off result (2026-09-23):** six runs across five lanes (routine
refactor, RE inventory, harness tooling, two retail-parity implementations),
all accepted after independent verification, for USD 0.64 in total.
Orchestrator corrections were integration-level only (a merge overlap, index
links, counts) plus one harness bug found in review. Retail-parity enemy
ability work routes to DeepSeek by default, one ability per lane, from
[the inventory](ENEMY_ABILITIES.md). Keep independent verification of every
disassembly claim that a change rests on.

`tools/ds-lane` is 956 lines. The next change to it splits it into a package
(for example, supervisor, receipts, CLI) under the 1,000-line rule.

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
