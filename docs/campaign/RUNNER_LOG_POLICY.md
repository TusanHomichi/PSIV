# Runner log: the party policy and the training sweep (P88b)

Lane P88b (2026-10-07): one party policy that chooses from every technique, skill and item each
member can use, with the engine's own formulas, and the training chapters cut down to what that
policy needs. The planner is described in [CAMPAIGN_RUNNER.md](CAMPAIGN_RUNNER.md#the-runner)
(**Battle policy**); this log holds the evidence. The arc logs it changes are
[RUNNER_LOG.md](RUNNER_LOG.md) (Aiedo and Krup training), [RUNNER_LOG_KURAN.md](RUNNER_LOG_KURAN.md)
(Dezolis training) and [RUNNER_LOG_AIRCASTLE.md](RUNNER_LOG_AIRCASTLE.md) (Air Castle training).

## Runs

Base: `claude/campaign-30` (`cd0c8d1`, the A6 engine with every remaining enemy ability) merged into
the lane at `22ac050`. Release builds, `CARGO_BUILD_JOBS=2`, one heavy command at a time. Every run
uses the accepted campaign-30 pack, `build/accepted-c30-pack-8327c39`, read only. Evidence is under
the ignored `build/` of the lane's worktree; the commands regenerate it. A full run takes about 40 s.

| Run | What | Result |
| --- | --- | --- |
| `build/p88b-a6base` | `cd0c8d1` itself: the old policy and the old route on the A6 engine | exit 2: Lashiec (event battle 16) defeats the party, 4,443,882 frames |
| `build/p88b-a6base-55` | the same, `--until-chapter air-castle-inner` | exit 0, 56 chapters, 4,435,079 frames, digest `4d972bde42c3e846`: the prefix-check base |
| `build/f-old` | the new policy on the old route (training levels 6, 6, 17, 24, 38, 37) | exit 0, 62 chapters, 4,209,488 frames, digest `b70d3d4b5223cdca`; Lashiec and Dark Force 2 won |
| `build/f-*`, `build/s-*`, `build/e-*` | the sweep below | |
| `build/p88b-final-a`, `-b`, `-c` | the final route, twice, and once more after the battle-end record | [the final runs](#the-final-runs) |

The old policy (one per-actor rule set, `policy.rs` and `policy_boss.rs`) loses Lashiec on the A6
engine; the new one wins it on the old route and on the trimmed one.

## The training rule

A training chapter patrols until `party_level_at_least` (the lowest member's level) holds. Its level
is **the lowest at which the next fixed (scripted) fight ends with every member standing, on both
clocks**, and the route completes on both. The cartridge's random draws are one stream, so the same
route on a different stream is a different trial:

- **the full clock**: the route from New Game in one process;
- **the snapshot clock**: the route resumed in a fresh process from the save of the chapter before
  the training (`--from-chapter`), which starts a different stream from that chapter on.

**Who stands at a fight's end** is the runner's own line under each chapter,
`event battle N: frames S..E; party ...; every member standing` or `down: names`
(`BattleRecord::end_line`, `rust/psiv-campaign/src/driver.rs`), not the chapter's closing party.
The sweep first read the closing party, and that misread two fights: the Xe-A-Thoul chapter ends on
the recovery tile, which revives the fallen, and Demi leaves after Zio, so a member down at the end
of either fight never showed. Every decisive trial was rerun with the line (`build/e-*`), and the
tables below are read from it. The rerun reproduced each earlier run's digest: the record changes
no play.

The next fixed fight of each training chapter, and the other fixed fights before the next training:

| Training | Next fixed fight | Later fixed fights before the next training |
| --- | --- | --- |
| `aiedo-training` | Juza (event battle 3, `zio-fort-juza`) | Demi (4) |
| `krup-training` | Gy-Laguiah (5, `ladea-tower-psycho-wand`) | Zio (6, `nurvus-zio`), the Zelan sabotage (8) |
| `dezolis-training` | Dark Force 1 (9, `kuran-dark-force-1`) | the trees (10), Xe-A-Thoul (14, `air-castle-xe-athoul`) |
| `air-castle-training` | the Spector and Lashiec (15, 16, `air-castle-lashiec`) | Dark Force 2 (17) |

## Before and after

Frames and battles of each training chapter. "Old policy" is `build/p88b-a6base` (it stops at
Lashiec, after the last training); "new policy, old levels" is `build/f-old`; "after" is the final
route.

| Chapter | Old level | Old policy | New policy, old levels | New level | After | Saved |
| --- | --- | ---: | ---: | --- | ---: | ---: |
| `basement-training` | 6 | 47,578 f / 26 b | 37,516 f / 25 b | 6 | 37,516 f / 25 b | 0 |
| `zema-training` | 6, 1,180 meseta | 3,161 f / 1 b | 4,139 f / 3 b | 6, 1,180 meseta | 4,139 f / 3 b | 0 |
| `aiedo-training` | 17 | 910,584 f / 688 b | 765,211 f / 657 b | 15, 22,000 meseta | 524,546 f / 428 b | 240,665 f (31%) |
| `krup-training` | 24 | 1,562,490 f / 1,916 b | 1,568,089 f / 1,913 b | 23 | 1,359,506 f / 1,632 b | 208,583 f (13%) |
| `dezolis-training` | 38 | 1,055,707 f / 736 b | 946,251 f / 728 b | 34 | 626,379 f / 478 b | 319,872 f (34%) |
| `air-castle-training` | 37 | 538,511 f / 273 b | 550,111 f / 289 b | 34 | 347,304 f / 172 b | 202,807 f (37%) |
| all training | | 4,118,031 f / 3,640 b | 3,871,317 f / 3,615 b | | 2,899,390 f / 2,738 b | 971,927 f (25%) |
| the route | | lost at Lashiec, 4,443,882 f | 4,209,488 f | | 3,244,777 f | 964,711 f (23%) |

"Saved" is against the new policy on the old levels. The Aiedo patrol also waits for 22,000 meseta:
the next chapter, `aiedo-shopping`, spends 21,500 on the Aiedo gear, and a level-15 party reaches
the shop without it.

## The sweep

Every level tried, frames and battles of the training chapter on the full clock, and the outcome on
each clock. "Standing" means every member stood at the end of the fight named, read from the
battle-end line; "down" names who did not; "lost" is a halt. Rows from runs made before the line
existed read the closing party, which proves a member down (nothing on the route revives one before
the chapter ends, except the Xe-A-Thoul chapter's tile) but not that everyone stood: there a won
fight is "won".

### Basement and Zema

On the final planner (`build/e-f-bas-*`, `e-s-bas-*`, to `north-bank`); the next fixed fight is the
Zema rescue (event battle 1). `zema-training` must reach level 6 and 1,180 meseta whatever the
basement leaves, so a lower basement level moves the training to Zema rather than saving it:

| `basement-training` | Full: basement + Zema frames (battles) | Full: Zema rescue (1) | Snapshot |
| --- | ---: | --- | --- |
| dropped | - | `alshline` halts at once: it starts where the patrol ends (`map 0x43 has no warp with record index 5`) | |
| 3 | 972 (0) | `zema-outfit` halts: the counter refuses, too little money | the same |
| 4 | 6,232 + 78,655 = 84,887 (89) | standing | standing; 8,552 + 79,878 |
| 5 | 23,515 + 37,150 = 60,665 (60) | standing | standing; 20,936 + 46,280 |
| **6** | 37,516 + 4,139 = 41,655 (28) | standing | standing; 40,237 + 6,071 |

Level 6, the old one, is the cheapest pair and is kept, with `zema-training` as it was. The earlier
planner (`57ba30c`) lost `bioplant-rika` at basement 5 on the full clock; the final one does not.

### Aiedo (Krup at 24)

| Level | Full: frames / battles | Full: Juza (3), Demi (4) | Snapshot: Juza, Demi |
| --- | ---: | --- | --- |
| 10 + 22,000 meseta (reaches 12 to 13) | 284,727 / 220 | lost at Juza | lost at Juza |
| 12 + 22,000 (the `57ba30c` planner) | 278,904 / 226 | Juza: down Gryz, Alys, Chaz, Hahn | |
| 13 (the `57ba30c` planner) | 327,398 / 272 | Juza: down Alys, Hahn | |
| 14 + 22,000 | 421,319 / 332 | Juza: down Hahn; Demi standing | standing, standing |
| **15 + 22,000** | 524,546 / 428 | standing, standing | standing, standing |
| 16 + 22,000 | 638,522 / 527 | standing, standing | standing, standing |

### Krup (Aiedo at 15)

| Level | Full: frames / battles | Full: Gy-Laguiah (5); Zio (6); Zelan (8) | Snapshot: 5; 6; 8 |
| --- | ---: | --- | --- |
| 18 | 536,099 / 639 | won; lost | won; lost |
| 20 | 820,041 / 983 | won; lost | won; lost |
| 22 | 1,156,986 / 1,392 | standing; lost | down Rune; lost |
| **23** | 1,359,506 / 1,632 | standing; down Demi; standing | standing; down Demi; standing |
| 24 (old) | 1,579,357 / 1,891 | standing; down Demi; standing | standing; standing; standing |
| 25 | 1,817,391 / 2,185 | standing; standing; standing | standing; standing (Demi 1 HP); standing |
| 26 | 2,073,084 / 2,491 | standing; down Demi; standing | standing; down Demi; standing |

### Dezolis (Aiedo 15, Krup 23)

Air Castle training at 37 for the first sweep (`build/f-dz-*`, `s-dz-*`), at 34 for the reruns
(`build/e-*-dz-*`); the fights before the Air Castle training do not depend on it.

| Level | Full: frames / battles | Full: DF1 (9); trees (10); Xe-A-Thoul (14) | Snapshot: 9; 10; 14 |
| --- | ---: | --- | --- |
| 26 | 188,487 / 141 | won; won; lost | |
| 28 | 286,369 / 207 | won; won; lost | |
| 30 | 387,731 / 288 | won; won; lost | |
| 32 | 502,246 / 378 | won; won; lost | |
| 33 | 568,004 / 427 | won; won; lost | won; won; lost |
| **34** | 626,379 / 478 | standing; standing; down Chaz | standing; standing; down Wren |
| 35 | 706,409 / 536 | standing; standing; standing | standing; standing; lost |
| 36 | 784,086 / 598 | standing; standing; down Wren | standing; standing; down Rika |
| 37 | 866,530 / 663 | standing; standing; standing | standing; standing; standing |
| 38 (old) | 948,245 / 731 | standing; standing; standing | standing; standing; standing |

Dark Force 1, the next fixed fight, ends with every member standing at every level read from the
line (34 to 38), on both clocks, and is won at every level tried. Xe-A-Thoul sets the level: 33 loses it on both clocks, 34 wins it on both. With the Air
Castle training at 34 after it, 36 then loses Dark Force 2 and 37 loses Lashiec on the full clock
(`build/e-f-dz-36`, `e-f-dz-37`): levels above 34 are not safer on these streams.

### Air Castle (Aiedo 15, Krup 23, Dezolis 34)

| Level | Full: frames / battles | Full: Spector (15), Lashiec (16); DF2 (17) | Snapshot: 15, 16; 17 |
| --- | ---: | --- | --- |
| 28 | 32,209 / 15 | lost at Lashiec | lost at Lashiec |
| 29 | 79,584 / 38 | lost at Lashiec | lost at Lashiec |
| 30 | 129,349 / 61 | down Rune, Wren, Kyra; won | won; down Chaz |
| 31 | 183,779 / 87 | down Wren; won | lost at Lashiec |
| 32 | 244,043 / 117 | down Wren; down Wren | down Wren; lost |
| 33 | 290,798 / 142 | standing; standing | down Wren; standing |
| **34** | 347,304 / 172 | standing; standing | standing; standing |
| 35 | 402,627 / 205 | down Wren; down Chaz | down Wren; standing |
| 36 | 465,881 / 241 | standing; down Rune | down Rune; standing |
| 37 (old) | 528,777 / 275 | standing; standing | standing; standing |

Levels 28 to 32 come from the closing-party summaries (`build/f-ac-*`, `s-ac-*`) and 33 to 37 from
the rerun with the battle-end line (`build/e-f-ac-*`, `e-s-ac-*`); where both exist they agree on
every member down.

### What the sweep says

The chosen levels are 15 (and 22,000 meseta), 23, 34 and 34. Each is the lowest at which the next
fixed fight ends with every member standing on both clocks and the route completes on both. Two of
the later fights end with a member down at those levels: Demi at Zio (Krup 23, both clocks) and
Xe-A-Thoul (Dezolis 34: Chaz on the full clock, Wren on the snapshot). Demi, an android, is down at
the end of Zio at Krup 22 to 26 except 25 (and 24 on one clock): no healing technique reaches an
android, and the party's items are the only cure she has. Requiring every fixed fight before the next
training to end with every member standing would give Krup 25 and Dezolis 37, and neither is safer
on the next fights: the outcome of the hard fights (Xe-A-Thoul, Lashiec, Dark Force 2) moves with
the stream at every level tried, up and down, not with the level alone. On other streams the trimmed
route fails about half the time ([below](#robustness-other-streams)).

## Robustness: other streams

The sweep's two clocks are two streams. Resuming a route in a fresh process from any chapter's save
starts another one, so `build/p88b-evidence/scratch/robust.py ROUTE RUN PREFIX` resumes the route
from each of the 61 chapter saves of a completed run and plays it to the end: 61 more streams per
route, the last chapters' with little left to lose. Logs: `build/p88b-evidence/robust-*.log`, runs
`build/rb-*-NN`.

| Route (training levels) | Resumes that complete | Where the others stop |
| --- | ---: | --- |
| the trimmed route (6, 6, 15 + 22,000, 23, 34, 34) | **28 of 61** | Xe-A-Thoul 15, Zio 8, Lashiec 6, Dark Force 2 3, Meese 1 (not a fight) |
| the old levels (6, 6, 17, 24, 38, 37) | **52 of 61** | Xe-A-Thoul 6, Gyuna 2 and Eclipse Torch 1 (not fixed fights) |
| the Aiedo cut alone (6, 6, 15 + 22,000, 24, 38, 37) | **52 of 61** | Xe-A-Thoul 6, Eclipse Torch 2 (random battles, not fixed fights), Dark Force 2 1 |

**The trimmed levels are tuned to their two streams.** On the full and snapshot clocks every fixed
fight is won; on other streams the trimmed route loses a fixed fight about half the time, where the
old levels lose one about one time in ten. Xe-A-Thoul is the weakest fight of both. The rule, the
lowest level that wins on two clocks, chooses the level at which those two clocks are lucky: the
non-monotonic sweep tables above show the same thing level by level. The Aiedo cut alone keeps the
old levels' robustness (`build/h-aie`, 3,981,053 frames, digest `fffae068b2863f27`): its 240,665
frames are saved at no measured cost, and the Krup, Dezolis and Air Castle cuts are the ones that
halve it. A training level chosen this
way is not a route in the sense `policy.rs` sets ("a route that survives on only one stream is not
a route"). The trimmed levels are what the lane's acceptance asked for and are in
`routes/main.json`; the choice between them and a robust rule belongs to integration
([the lane's finding](#findings)).

The resumes also found three halts that are not about training: the Meese clinic
talk (`build/rb-new-39`: the party stands on (34,29) beside the doctor's recorded cell (35,29),
facing left, and `Speak reached nothing`; not diagnosed), the Gyuna chapter's walk (`build/rb-old-13`, `rb-old-17`: `no walk
from (39,36) reaches warp 9 of map 0x144`) and a random battle lost on the Eclipse Torch drive
(`build/rb-old-34`, `rb-aie-13`, `rb-aie-54`).

## Negative controls

`build/p88b-evidence/scratch/negctl.py` makes one break in the source, runs the crate's library tests
(36), `tests/runner.rs` (15 run, the 2 whole-route cases ignored) and `tests/party_policy.rs` (1),
checks that exactly the named test fails, and restores the file (byte-identical, checked with
`diff -r`). Each break ran 52 tests: one failed, 51 passed. Logs:
`build/p88b-evidence/negctl-<name>.log`, all six in `negctl-all.log` (exit 0, 252 s).

| Break | Where | The one test that fails |
| --- | --- | --- |
| every element factor read as neutral | `policy_value.rs` `on_enemy` | `the_policy_picks_the_element_the_enemy_is_weak_to` |
| the threat ignored | `policy_value.rs` `threat` | `the_policy_heals_before_a_member_falls` |
| a technique in stock whatever the TP (and no TP kept for a cure) | `policy_plan.rs` `in_stock`, `policy_value.rs` `best_pick` | `the_policy_never_orders_a_technique_it_cannot_afford` |
| CROSSCUT counted as two damage passes | `policy_value.rs` `on_enemy` | `the_policy_counts_crosscut_once` |
| a single cure reaching anyone, not only its menu's targets | `policy_plan.rs` `single_offers` | `mixed_party_cures_use_the_menus_eligible_targets` |
| a member standing whatever their status bits | `driver.rs` `MemberAtEnd::standing` | `a_battle_end_names_the_members_who_are_down` |

The threat break first failed a second test, `the_cure_goes_to_the_member_whose_action_is_worth_least`,
whose patient stood at 60% and so was at risk only through the threat. That test is about who cures,
not when; its patient now stands below half, and the threat break fails only its own test.

## The final runs

From the worktree:

```
CARGO_BUILD_JOBS=2 cargo build --release --manifest-path rust/Cargo.toml -p psiv-campaign
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --pack /home/peter/PSIV/build/accepted-c30-pack-8327c39 --save-dir build/p88b-final-a --tape build/p88b-final-a/run.tape --report build/p88b-final-a/report.json
./rust/target/release/psiv-campaign run rust/psiv-campaign/routes/main.json --pack /home/peter/PSIV/build/accepted-c30-pack-8327c39 --save-dir build/p88b-final-b --tape build/p88b-final-b/run.tape --report build/p88b-final-b/report.json
./rust/target/release/psiv-campaign replay build/p88b-final-a/run.tape --pack /home/peter/PSIV/build/accepted-c30-pack-8327c39
./rust/target/release/psiv-campaign prefix-check build/p88b-a6base-55/run.tape build/p88b-a6base-55/report.json build/p88b-final-a/run.tape build/p88b-final-a
```

| Run | Result |
| --- | --- |
| `build/p88b-final-a` | exit 0 in 40 s, **62 chapters, 3,244,777 frames, digest `f16cf8961b9d915b`**, tape SHA-256 `53faf9122b45bce8f12d5ec66a63af3e19ed1879977c46d4304db45e9940ecc8` |
| `build/p88b-final-b` | exit 0 in 38 s, the same frames, digest and tape SHA-256; all 63 save files (62 chapter saves and the route slot) byte-identical (`final-a-saves.sha256`, `final-b-saves.sha256`); the reports identical but for the directory name |
| replay of `p88b-final-a/run.tape` | exit 0 in 3 s, 3,244,777 frames, digest `f16cf8961b9d915b` |
| `build/p88b-final-c` | after the battle-end record (binary SHA-256 `fd09645d…903a`): the same digest, tape SHA-256 and 63 saves; the report lists 13 scripted battles |
| prefix check | exit 2 at frame 14,260 in `academy` (below) |

Binary SHA-256 for `-a` and `-b`: `37713081…d94b`. Route file `afbe06e8…16c5`. The last chapter save
is SHA-256 `e97882c6…6bdf`; the pad SAVE `route/slot_1.sram` is `8c58419f…7c0c`.

**The prefix moves from the first battle.** `prefix-check` against the old policy's run on the A6
engine stops at frame 14,260 in `academy`: `the pads differ from frame 14260, in chapter 0 academy:
base Some(32), run Some(2)`, exit 2. That is the route's first random battle (formation 141, began at
frame 14,216, `PSIV_CAMPAIGN_TRACE` in `build/p88b-trace-base` and `build/p88b-trace-new`): the old
policy sends all three members at the first enemy, the new one sends Chaz at the enemy his swing is
worth most against (fighter 7), so the target cursor moves where the old run confirmed. A new policy
moves every frame after its first decision; nothing before frame 14,260 differs.

## Gate

`python3 tools/gate.py` on the lane's uncommitted tree (`build/gate/20261007T230823Z-8c7091b`, exit 1
in 2,275 s):

| Command | Result |
| --- | --- |
| `python3 tools/check_docs.py` | exit 0, 0 problems |
| the Python suite | exit 1: 1,377 run, 0 failures, 49 errors, 2 skipped; every error is a `FileNotFoundError` on an ignored oracle input (`oracle/layouts`, `oracle/states`) the worktree did not have |
| `cargo fmt --all --check` | exit 1: `rust/psiv-runtime/src/session/battle/mod.rs` wants the lane's `mod policy_view;` one line lower, after `mod player_tests;`; the file is outside the lane's write set |
| workspace tests | exit 0: 1,596 passed, 0 failed, 3 ignored over 53 suites |
| clippy `-D warnings` | exit 0 |

With `oracle/layouts`, `oracle/states` and `oracle/frames` copied in from the main checkout, the Python
suite alone: 1,449 run, OK, 1 skipped (`build/p88b-evidence/python-suite-with-oracle-inputs.log`).

## Runner tests

- `tests/party_policy.rs` `mixed_party_cures_use_the_menus_eligible_targets` (moved out of
  `tests/runner.rs`, which the change took past the 1,000-line limit) plays the party policy on a
  live battle (formation `$4E`, two enemies one round of the party cannot finish) with Chaz, Demi and
  Rika: the core's target lists keep the android out of RES, MONOMATE and GISAR; a desperate Demi
  with no cure that reaches her never becomes a cure's patient and Rika is cured once; with her
  RECOVER on offer Demi cures herself; only the android hurt orders no cure; two humans at 60% get
  GISAR from Rika, one does not. Both temperaments.
- **The shop trips raced.** `shop_trip_result` ran three tests' trips in the academy run's one save
  directory, so in parallel one trip read another's save
  (`the_first_actor_of_a_scripted_battle_opens_with_the_item` failed on the accepted pack: the
  MONOMATE it bought was gone). Every test that resumes after the academy now copies the academy save
  into its own directory first (`after_academy`).
- **The whole-route case was pinned to the 50-chapter route.**
  `the_whole_route_defeats_zio_saves_and_replays` (ignored; it plays the route) still expected
  `air-castle-arrival` last. It now expects `garuberk-dark-force-2`, checks Lashiec's flags and Dark
  Force 2's (`$9E`, `$A1`, Kyra gone), and passes:
  `cargo test --release -p psiv-campaign --test runner -- --ignored --test-threads=1`, 2 passed.

## Findings

For the orchestrator; none is fixed in this lane.

1. **The two-clock rule picks fragile levels.** The trimmed route completes from 28 of 61 resume
   streams; the old levels and the Aiedo cut alone from 52 (above). A rule that judges a level on
   many streams would choose differently: for example, the lowest level at which every fixed fight
   up to the next training is won from every resume point before the training chapter
   (`robust.py`'s method, a few minutes a level). On the evidence here the Aiedo cut (15 and
   22,000 meseta) is safe to keep and the other three are not. Changing the rule changes the
   sweep's acceptance, so it is not done here.
2. **The closing party is not a fight's result.** The paused sweep judged Zio and Xe-A-Thoul by the
   chapter's closing party and called both "every member standing"; Demi was down at Zio (Krup 23)
   and Chaz or Wren at Xe-A-Thoul (Dezolis 34). Fixed in this lane: the runner records the party at
   every battle's end (`BattleRecord::party_at_end`) and prints it under the chapter (rung 1, the
   runner's own output; `driver.rs` test `a_battle_end_names_the_members_who_are_down` and its
   negative control).
3. **Halts that are not training**, each reproducible from its run directory: the Meese clinic talk
   (`build/rb-new-39`, `Speak reached nothing` beside the doctor's recorded cell), the Gyuna chapter's walk to warp 9 of `$144`
   (`build/rb-old-13`, `rb-old-17`), a random battle lost on the Eclipse Torch drive
   (`build/rb-old-34`, `rb-aie-13`, `rb-aie-54`).
4. **The lane's helpers are untracked.** `trial.py` (route variants), `fights.py` (the battle-end
   summary), `negctl.py` and `robust.py` live in the ignored `build/p88b-evidence/scratch/`. The
   brief names `tools/` for a helper needed twice, and `tools/` is outside the lane's write set;
   `robust.py` and `trial.py` are needed again by any training sweep.

## Not claimed

- **Native play.** Headless `Session` only; no Godot replay of the new route and no capture.
- **A route that survives every stream.** The levels are chosen on two clocks; the hard fights
  move with the stream (above).
- **The prefix.** Every chapter moves; the base run is evidence of the old policy only.
