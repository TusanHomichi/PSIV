# Claude Code lane ledger

Adoption record and reliability log for the Claude Code routing: Opus
orchestrates, and DeepSeek `deepseek-flash` at max reasoning delivers through
`tools/ds-lane` ([reference](../tools/ds_lane/README.md)).

## Adoption

Owner instruction, 2026-09-23. The owner asked for the route to stay
confined to Claude Code sessions. It first landed in the shared
`AGENTS.md`/workflow docs (swept into PR #2 while uncommitted) and was moved
here and to `CLAUDE.md` on the same day. On 2026-09-25 tracked guidance
became model- and vendor-neutral: the routing moved to host-local, untracked
configuration, the `ds-lane` manual to its tool README, and `CLAUDE.md` to a
bare import of `AGENTS.md`.

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
| 2026-09-24 | or-O1 | Oracle: capture every `UpdateRNGSeed2` roll (tracked core patch, `--rng-trace`, seed-chain check) | Accepted; trace reproduced byte-identically | Missed in review: the roll column subtracted the seed's low word, and the checker agreed because it recomputed with the same convention. O2's end-to-end replay caught it (fixed in P1) | USD 0.221 |
| 2026-09-24 | or-O2 | Replay tape 07's battle in `psiv-core` with the captured rolls | Accepted. Every action matched with the modelled rolls; found two draw-count defects and the roll-column bug; all three read from `ps4.asm` | None | USD 0.473 |
| 2026-09-24 | or-N | `ds-lane`: skip ignored link exclusions; record finalize failures | Accepted; 54/54 | None. Its fix lived on the branch, so P1 and P2b still hit the pre-fix crash (committed by hand) | USD 0.078 |
| 2026-09-24 | or-P1 | Correct the trace's roll column; path-independent captures; extractor cross-check | Accepted; its checker rejects the old capture | Applied its verified two-number patch to a test outside its write set | USD 0.191 |
| 2026-09-24 | or-P2a | Alys and Kyra's second hit pass (and the `$EE49` critical demotion) | Accepted; dispatch and demotion guard read from `ps4.asm` | None | USD 0.244 |
| 2026-09-24 | or-P2b | `$FFFFEEA8` ability re-roll word: lifetime and rule | Accepted on run 2. It found the battle-load clear (9992-9994) that disproved the brief's per-session premise | Sent back to remove the session plumbing its own evidence made dead weight (owner decision) | USD 0.449 |
| 2026-09-24 | or-S | Split `SOURCE_NOTES.md` (1,463 lines) into topic files plus an index | Accepted; every original line present bar one declared link-depth fix | Retargeted oracle references outside its write set | USD 0.082 |
| 2026-09-24 | or-R | Regenerate tape 09 with the fixed tracer; one coherent replay ledger | Accepted | Re-extracted tape 07's fixture provenance; dated correction in `battle-party.md` | USD 0.196 |
| 2026-09-24 | o2-T | `ds-lane`: numbering from directories, crashed-run records, data-file size exemption | Accepted; 62/62 with the trap binary unused | None | USD 0.139 |
| 2026-09-24 | o2-F2a | Forced formation entry (group cells plus a seed patch so the cartridge's own roll picks the formation) and a scripted policy | Accepted; the `$37` capture re-run reproduced the trace hash | None (log path header noted, fixed in F2b) | USD 0.339 |
| 2026-09-24 | o2-F2b | Extractor and replay generalized (enemy skills, vehicle battles); one data-driven test; divergence manifest | Accepted. Helex FLAME BOLT and Fanbite SPIRAL BLD exact on the cartridge; found the vehicle-attack bug | None | USD 0.295 |
| 2026-09-24 | o2-V | Vehicles attack as the cartridge does | Accepted on run 2 | Run 1 relaxed the replay comparator for multi-frame swings; sent back to fix the evidence instead (sample the decisive pass) and keep the checker strict. Rewrote two stale references at merge | USD 0.437 |
| 2026-09-24 | o2-W | `oracle/force/` package, `--vehicle`, a second vehicle capture | Accepted; found the Ice Digger's two-pass swing, a timer race (object `$1C` start) read and confirmed from `ps4.asm` | None | USD 0.257 |
| 2026-09-24 | o2-Y | Per-vehicle hit-pass count from each attack object's timer | Accepted; manifest empty again | None | USD 0.206 |
| 2026-09-24 | x-redshirt-exit | Remove Redshirt from PSIV (owner decision); archived summary | Accepted and merged as PR #8; shared-doc removals were Redshirt-only and the roadmap campaign queue byte-identical | None | USD 0.090 |
| 2026-09-24 | sw-Z-prompt-stdin | Pass the worker prompt on stdin, not argv | Accepted. Independent: 64/64 via `verify` with the trap `reasonix` never invoked; a probe confirmed the real `reasonix` reads its task from stdin | None. Root cause came from sw-S1 run 1, whose `pkill -f` pattern sat in its argv-borne brief and killed its own worker | USD 0.136, 6.7 min |
| 2026-09-24 | sw-S1-motavia | Sweep all 83 Motavia overworld formations; `--durable`/`--max-rounds`; resumable batch runner; clustered worklist | Accepted after run 2 (run 1 killed its own worker, fixed by sw-Z). 81 fixtures replay as recorded; 2 capture failures (0x27, 0x28) recorded as data; 52 first divergences in 8 clusters. Independent: psiv-core suite green (659 + 1 ignored); re-sweeping 0x0A/0x3B/0x53 reproduced identical fixture data | Provenance still path-dependent (tape path, log hash); handed to sw-T2 | USD 0.640, 73.6 min |
| 2026-09-24 | sw-H2-compaction | Deduplicate (hardlink) and xz-compress lane evidence; `ds-lane compact` | Accepted. Independent: 73/73 via `verify`, trap binary unused. Demo: 16.2 MiB of duplicated captures became 267 KiB | None | USD 0.474, 28.2 min |
| 2026-09-25 | sw-T2-triage | Classify the sweep's 8 divergence clusters (port rule vs harness artifact); fix the harness; path-free provenance | Accepted after run 3. Run 1 died at 4,030 s when the provider became unreachable (a network captive portal answered TLS for `api.deepseek.com`); run 2 resumed and finished: 5 harness artifacts fixed, 81 fixtures re-extracted (34 now exact), 18 real findings left in 4 port rules (filed as #21, #22, #23). Independent: two `ps4.asm` citations spot-read, including the critical bonus's `moveq #0,d4` precondition | Review found a comparator gap: the ability branch walked only damaged slots, so a port resolution on a slot the log left clean went unchecked, behind a comment claiming otherwise. Sent back as run 3, which added the scan, three tests and a negative control; no hidden findings surfaced. It also caught cargo reporting fresh on a stale build after the outage and rebuilt clean | USD 1.38 over three runs |
| 2026-09-25 | g2-gate | `tools/gate.py`: one gate entry point with receipts, a lock, and a refusal while the debug extension is mapped (#19) | Accepted. Independent: 16/16 via `verify`; `--list` matches the doc | Reflowed a wrapped doc line that rendered as a bullet; added the doc check as the fifth gate command at integration | USD 0.077, 7.8 min |
| 2026-09-25 | g1-doccheck | `tools/check_docs.py`: links, anchors and shell command paths across tracked Markdown (#10) | Accepted. Independent: 17/17 via `verify`; the tree passes (160 files, 483 links) | Merged its gate wiring with g2's `DEVELOPMENT.md` rewrite | USD 0.103 |

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

### Scoping result: captured-roll replay (2026-09-24)

Retail battle rolls come from `UpdateRNGSeed2` (`ps4.asm:86097`):
`HV counter + Main_Frame_Count - RNG_Seed.w`, then `ror`. The VDP HV counter is
beam timing that no headless reimplementation reproduces, so the port
deliberately substitutes a surrogate stream, and the existing oracle battle
tests can only assert that a value is *reachable*. A lockstep comparison would
diverge on its first roll.

The oracle therefore **captures** the cartridge's rolls instead of reproducing
them. A tracked patch to the pinned Genesis Plus GX core logs each HV read made
by `UpdateRNGSeed2`, and the host reconstructs every returned roll. The port
replays those exact values through its `Rolls` trait (`SliceRolls`), so every
number and decision must match *exactly*. Any divergence is a rules defect in
draw count, order, mask or formula. The first proof reuses tape 07 (the first
basement battle), so forced formation entry and a scripted driver come after
the method is proven.

Steps: O1 capture and consistency check on tape 07; O2 replay tape 07's battle
in `psiv-core` with the captured rolls and compare every action; then forced
formation entry, a scripted driver, and the formation sweep.

**Proof result (2026-09-24):** method proven. Tapes 07 and 09 replay on the
verbatim captured stream with every roll consumed and every action exact. The
replay found three defects no "reachable value" check could see: the trace's
own roll convention (low versus high seed word), Alys and Kyra's second hit
pass, and the `$FFFFEEA8` re-roll word zeroed at each battle load. All are
fixed. Details: [the replay ledger](BATTLE_ORACLE_REPLAY.md).

Review lesson: a consistency checker that recomputes with the same assumption
as the producer proves nothing about that assumption. Checks must derive from
an independent source; the fixture extractor now does.

**Forced entry and generic replay (2026-09-24):** any formation can now be
captured on demand (`oracle/force/`, `docs/BATTLE_ORACLE_FORCED.md`), and one
data-driven test replays every fixture with a checked-in divergence manifest
that fails when stale. Captures confirmed FLAME BOLT and the all-party slot
order on the cartridge, and found two vehicle rules: vehicles attack (they were
skipped as unarmed), and a vehicle's hit-pass count comes from its attack
object's timer (2 for the Ice Digger, 3 for the others). Seven fixtures replay
exactly and the manifest is empty.

Review lesson: capture more than one instance of a rule. The first vehicle
capture produced a plausible universal constant that the second disproved.

**Next action:** the formation sweep. Capture every formation reachable from
the current campaign region (starting with Motavia's encounter groups) under the
attack policy, extract, and let the manifest collect the worklist; then fix by
cluster.
