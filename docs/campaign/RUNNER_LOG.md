# Runner log

The durable record of campaign-runner runs: each run's command, base revision,
how far it got, and every halt with its diagnosis. The runner and its report
format are described in [the campaign runner plan](CAMPAIGN_RUNNER.md#the-runner);
this file holds what the runs found. Evidence files live under the git-ignored
`build/campaign/` of the checkout that ran them, so a fresh clone does not hold
them; the commands below regenerate them.

A run that halts on a port defect keeps its report here and a diagnosis with
`file:line` and the cartridge evidence. The orchestrator turns a diagnosis into
a fix lane or an issue; the runner never fixes a port defect and never adds a
shortcut, a state edit or a skip to get past one.

## Current state

`routes/main.json` completes from New Game with pads only: 18 chapters, New
Game to the post-Rika north-bank checkpoint, the Aiedo arrival and, since lane
C1, Chaz's house rest, training, the passageway and the Zio Fort's east wing:
the party stands on Zio Fort's Juza room (map `$87`, cell (32,21)), all five
alive and whole at level 12 to 13. 457,356 frames, exit 0, digest
`96d2835a8633def8` (run C1-5). The run is a traversal proof, not a balance
proof (see "Not claimed").

The route stops there because the next objective, talking to Juza, halts on a
port defect with no in-game alternative:
[H16](#h16-juzas-battle-rolls-zan-and-forceflash-which-the-engine-does-not-run).
Three more unsupported abilities sit on the way and are only passed by running
from every encounter: [H15](#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities).
The Aiedo inn defect [#39](https://github.com/TusanHomichi/PSIV/issues/39) does
not block the story: [H13](#h13-the-aiedo-inn-returns-aiedoeventpending-39).

The one port defect the first runner lane found,
[H1](#h1-the-mile-sand-worm-trigger-halts-every-visit-to-mile), is fixed
(issue #54).

## Runs

Base revision for every run: `016cdc7` (PR #53, the battle, shop and camp
Session modes), runner code of this lane on top. Pack: the local
`runtime-pack`. All runs are release builds unless said otherwise; a debug
build gives the same tape byte for byte (checked on chapter one).

| Run | Command | Result |
| --- | --- | --- |
| R1-1 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/campaign/full1` (route as R0 shipped it) | academy completed (30,822 frames, 2 battles); halted in `holt` objective 0, `TriggerUnsupported { trigger: 112 }` on arriving in Mile: [H1](#h1-the-mile-sand-worm-trigger-halts-every-visit-to-mile). Report: `build/campaign/evidence/halt-1-mile-sandworm-trigger.json` |
| R1-2 | `... --from-chapter holt` while the route and the runner were repaired | `holt` completed once Mile was left out (H1) and the `expect` cell corrected (H2); `rune-dorin` halted on the Zema inn (H3), then lost a battle in the Valley Maze (H4) until the runner cured the party between battles |
| R1-3 | `... --from-chapter alshline` | storage door object settled (H5); lost a battle in the Alshline basement until `run_unless_boss` existed (H6); then chest (H7), equipment with every slot full (H8), the BioPlant elevator doors (H9) and the Rika scene's `go_to` (H10) |
| R1-4 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/campaign/full4` | **completed**, exit 0. 175,497 frames, digest `52ccbf8d9a7de61a`; `psiv-campaign replay build/campaign/full4/run.tape` reproduces the digest. A second full run (`full3`, after H11) printed the same digest |
| R1-5 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c54-route --tape build/c54-route/run.tape --report build/c54-route/report.json` (release; issue #54: sand worm transcribed, Mile restored, `basement-training` added) | **completed**, exit 0. 168,843 frames, digest `5e80a50e50dad15c`, tape sha256 `24ad3ae31b58bc574cf76f9c2e727887c69c6575b32d6588f4f1da5f64ae67d5`; `psiv-campaign replay` reproduces the digest. Before the training chapter existed the same route halted in `alshline` objective 13 (`lost_battle`, [H12](#h12-the-alshline-basement-kills-an-under-levelled-party-after-the-walk-shifted)) |

R1-4 per chapter (frames, battles, party at the chapter's end):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| academy | 32,308 | 4 | Alys L7 50/53, Chaz L2 31/31, Hahn L2 27/27 |
| holt | 9,958 | 6 | Alys L7 41/53, Chaz L3 28/34, Hahn L3 28/30 |
| rune-dorin | 29,215 | 9 | Alys L7 53/53, Chaz L5 49/49, Hahn L4 27/33, Gryz L6 66/66 |
| alshline | 8,871 | 6 | Alys L7 53/53, Chaz L5 49/49, Hahn L4 33/33, Gryz L6 66/66 |
| zema-rescue | 14,198 | 9 | Alys L7 52/53, Chaz L5 49/49, Hahn L5 33/38, Gryz L6 66/66 |
| zema-outfit | 1,282 | 0 | Alys L7 53/53, Chaz L5 49/49, Hahn L5 38/38, Gryz L6 66/66 |
| zema-training | 53,841 | 55 | Alys L7 53/53, Chaz L6 53/53, Hahn L6 45/45, Gryz L6 66/66 |
| zema-armour | 718 | 0 | the same, all full |
| bioplant-elevators | 4,887 | 3 | Alys L7 52/53, Chaz L6 53/53, Hahn L6 44/45, Gryz L6 65/66 |
| bioplant-order | 60 | 0 | Gryz leads: Gryz, Alys, Chaz, Hahn |
| bioplant-rika | 18,763 | 2 | Gryz L7 0/76, Alys L8 0/61, Chaz L6 53/53, Hahn L6 0/45, Rika L1 39/39 |
| north-bank | 850 | 0 | Gryz L7 76/76, Alys L8 61/61, Chaz L6 53/53, Hahn L6 45/45, Rika L1 39/39 |
| aiedo | 546 | 0 | the same; standing at Aiedo (47,83) |

Evidence of R1-4 (local, under `build/campaign/`): `evidence/r1-4-full-route.tape`
(sha256 `dd5ff60f0cd2ef06ff483ecf7f55ee398cc22bed85752a7181dc9291c2e5e6c5`),
the chapter saves `full4/NN-id/slot_1.sram` (north bank
`29b717fdba5bdd1d5047025b2f6f63c7448dedf792f609a273b25512bf1897dc`, Aiedo
`866c29cd343403cb3babe77f31abdde6da7620af08fecd0272929a0141c999a2`), and the
route's own saves in `full4/route/`.

Acceptance target: after `north-bank` the party stands on Motavia `$00` at
(84,64) with Gryz, Alys, Chaz, Hahn and Rika, all alive, flag `$35` set (the
chapter's `expect` and `closing` hold), and `aiedo` arrives at map `$54`
(47,83), one warp, as R0 planned.

## C1 runs: Aiedo to the Zio Fort

Base revision: `8c19769` (main after PR #57), lane C1 runner code on top.
Release builds. The branch is `worktree-agent-a03b13906c10bc1e3`. Evidence is
under the git-ignored `build/c1/` of the lane's worktree; the commands below
regenerate it.

| Run | Command | Result |
| --- | --- | --- |
| C1-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-0 --tape build/c1/run-0/run.tape --report build/c1/run-0/report.json` (the route as C1 found it) | completed, exit 0, 168,843 frames, digest `5e80a50e50dad15c`: identical to R1-5 |
| C1-1 | the same route plus a scratch chapter at the Aiedo supermarket inn counter (50,32), `rest_inn`, `expect event:0x46` | halted at `rest_inn`: `the counter refused: "Aiedo rest event pending."` ([H13](#h13-the-aiedo-inn-returns-aiedoeventpending-39)); `build/c1/evidence/h-inn-39-report.json` |
| C1-2 | `--save-dir build/c1/s1`, scratch chapters: Chaz's house, then `go_to_map 130` with `attack_all` | halted at the passageway: `unsupported ability 18 for fighter 7` (map `$81`, formation 211, frame 171,935); re-recorded as `build/c1/evidence/h15-fusion-report.json` ([H15](#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities)) |
| C1-3 | the same with `run_unless_boss` through the passageway and the fort, no training | the passageway passed; the fort's F1 killed a level 8 to 11 party (`lost_battle`, map `$84`), and a variant halted on ability 33 (`build/c1/evidence/h14-untrained-report.json`): [H14](#h14-the-fort-needs-a-trained-party-and-a-rest-that-is-not-the-aiedo-inn) |
| C1-4 | `aiedo-training` (patrol until level 10, then 12), passage and approach chapters with `attack_all` in the fort | halted on ability 33 (FIREBREATH, `build/c1/evidence/h16a-airslash-report.json`) and, in an earlier draw, on ability 45 ([H15](#h15-passageway-and-zio-fort-encounters-roll-unsupported-abilities)); with `run_unless_boss` the same walk completes |
| C1-5 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-5 --tape build/c1/run-5/run.tape --report build/c1/run-5/report.json` | **completed**, exit 0. 457,356 frames, digest `96d2835a8633def8`, tape sha256 `53adf11f092bb99982c299aa2620b75a4078f6a4fe3ba69f0adae0a3d749bd57` |
| C1-6 | the same command with `run-6` | **completed**, exit 0, identical frames, digest and tape sha256 (and the same Zio Fort chapter save, `71e15b109ef788ed4f215fa76409d56fd43b4b4d370f32b8e85e9fbf127a20a3`); a third run (`run-7`) printed the same tape sha256. `psiv-campaign replay build/c1/run-5/run.tape` reproduces the digest |
| C1-7 | the full route plus a scratch chapter `talk npc 0` at Juza (`build/c1/run-4/`) | halted at the talk: `unsupported ability 71 for fighter 6` (Juza's ZAN), frame 458,343; `build/c1/run-4/halt-juza.json` ([H16](#h16-juzas-battle-rolls-zan-and-forceflash-which-the-engine-does-not-run)). The earlier draw halted on ability 86 |

C1-5 per new chapter (frames, battles, party at the chapter's end):

| Chapter | Frames | Battles | Party at the end |
| --- | --- | --- | --- |
| aiedo-chaz-house | 1,130 | 0 | Gryz L8 84/84, Alys L8 61/61, Chaz L7 62/62, Hahn L7 51/51, Rika L3 47/47 |
| aiedo-training | 281,447 | 171 | Gryz L13 121/121, Alys L12 92/92, Chaz L12 93/93, Hahn L12 81/81, Rika L12 105/105 |
| passage-to-zio-fort | 3,144 | 6 | the same, all full |
| zio-fort-approach | 2,792 | 5 | the same, all full; standing at map `$87` (32,21), purse 21,832 meseta |

Not exercised by these chapters: scenes 27 (blocked by H13) and 31 to 39, which
all lie past Juza. Scenes 28 and 29 (Chaz's house) are exercised and assert
their temp flag `$18`.

## Halts and their diagnoses

H1 is a port defect. H2 to H11 are route claims or runner gaps, each fixed in
this lane; they are here so the next runner author does not rediscover them.

### H1: the Mile sand worm trigger halts every visit to Mile

**Fixed (issue #54, run R1-5).** `trigger_custom.rs` now transcribes `RunEvent_MileSandWorm` with the field RNG supplied through `TriggerContext` (one draw only when the flag and box pass), and the three other formerly unsupported custom triggers (`RidingElevator`, `EnterGrbkTwDoor`, `PenguFeedStolen`); the `Unsupported` variants are gone. The route visits Mile again, in `holt` and in `rune-dorin` (the Mile inn). Arrival at pixel y `$310` is outside the worm's box, so no RNG draw and no event. Event `$71`'s scene (`Event_MileSandWormBattle`, `ps4.asm:152222`) is not registered in `rust/psiv-core/src/scenes/mod.rs`; a player who walks into the box and rolls `& $1F == 0` meets `SceneMissing`. The same holds for `$37`/`$38` (Garuberk tower door) and `$96` (Pengu Feed stolen). The diagnosis below is kept as found.

- **Halt:** R1-1, chapter `holt`, objective 0 (`go_to_map 29 via_warp 1`), on the
  frame the party arrives in Mile (map `$1D`, cell (20,49)):
  `scene_fault: TriggerUnsupported { trigger: 112 }` (`$70`).
- **Cause (port):** `rust/psiv-core/src/trigger_custom.rs:43` answers
  `TriggerResult::Unsupported(.., Unsupported::Rng)` for `MileSandWorm`
  unconditionally, and `trigger_custom.rs:434` pins it (`the_four_undecidable_routines_say_so`).
  The trigger table binds `$5C..$70` to it (`rust/psiv-core/src/trigger_table.rs:407`).
  Most of the routine is decidable without any RNG.
- **Cartridge:** `RunEvent_MileSandWorm`, `reference/ps4disasm/ps4.asm:116449`:
  it tests `EventFlag_MileSandWorm` (`$1B`, `ps4.constants.asm:1505`) and
  returns NoEvent when set; returns NoEvent unless the leader's `curr_x_pos`
  is at most `$170` and `curr_y_pos` is within `$150..$2B0`; only then calls
  `UpdateRNGSeed` and fires event `$71` when `RNG_Seed & $1F == 0`. Arrival is
  at pixel y `$310`, so the cartridge answers NoEvent, takes no RNG draw, and
  the party walks on. The port halts where the cartridge does nothing.
- **Fix lane:** decide the flag test and the position box in `trigger_custom.rs`
  as the routine does, and raise `Unsupported::Rng` only inside the box with the
  flag clear (or give `TriggerContext` the shared RNG and run the draw and event
  `$71`, which is the real fix). Add the position cases as unit tests. Until
  then every visit to Mile halts a runner, and a player who walks into the
  worm's box meets the same gap.
- **Route alternative used:** Mile is not needed. The two Mile objectives of
  `holt` (a detour before Zema, flagged `verify` by R0) are gone; the route goes
  to Zema directly and every later claim still holds. The Mile inn at the start
  of `rune-dorin` is replaced by the Piata inn: the Zema inn is shut
  (`psiv-campaign plan --from-map 36 --from-cell 31,49 --to-map 39` with the
  flags the route holds finds no walk; with `event:0x33` and `event:0x37` it
  finds the 23-step one), so `go_to_map 25`, stand at (41,32), `rest_inn`,
  `go_to_map 0`. A player can choose that inn.

### H2: the Holt scene's landing cell (route claim)

`expect` after Professor Holt claimed Zema (60,21). The scene writes
`Map_Start = ($3C,$14)` in 8-pixel units (`docs/scenes/13_ProfHolt.md:26`):
cell (30,10), and the port stands the party one row south, (30,11), as at first
control. The route now says (30,11). No port defect: the claim halved the wrong
unit.

### H3: the Zema inn is shut early (not a defect)

`rune-dorin` first replaced the Mile inn with the Zema inn and halted with
`unreachable: no walk from (31,49) fires warp 4 of map 0x24`. The inn's door is
patched shut until the Igglanova rescue (static plan evidence under H1). The
story keeps the town sealed; the route uses the Piata inn.

### H4: a lost battle in the Valley Maze (runner gap)

The default policy only attacked, never cured between battles and never
revived; Chaz and Hahn fell in the first maze fights and Rune walked alone until
the party was wiped (`lost_battle`). The runner now cures the party through the
camp after each battle (`rust/psiv-campaign/src/recovery.rs`: the same TECH menu
a player uses, a healing technique from whoever has the TP, until nobody is
below 70%), and the in-battle threshold is 50%. With the cure in place R1-4
walks the maze with nobody down.

### H5: Gryz's storage door is object 1 (route `verify`)

`alshline` objective 7 named object 0; the press reaches object 1
(`talk` halts `wrong_object` naming both). Route fixed.

### H6: a lost battle in the Alshline basement (runner gap)

The chapters `alshline` and `zema-rescue` name `run_unless_boss`; the runner
resolved every name to the default policy and fought a basement formation it
cannot beat. `run_unless_boss` now runs from random encounters (RUN on the main
options until it works) and fights scripted battles.

### H7 to H11: runner gaps and route claims, fixed

- **H7** `open_chest` halted `wrong_object`: a chest press opens a chest
  window, not a dialogue. The talk controller reads the loot window as opened.
- **H8** `equip` for a member who wears something in every slot: the EQUIP
  menu opens its list only from an empty slot and takes off what a full slot
  holds (`rust/psiv-runtime/src/session/camp/equipment.rs`). The controller
  clears the item's own hand, or a head or body piece it puts back on after.
- **H9** the BioPlant elevator: after event `$13` the door cells are walkable
  map-change ground (collision 1) while the warp record is a ground trigger, so
  the stepping rule never "fires" it and R0's flood found no walk. The
  controller walks into the warp's trigger area and lets the game's elevator
  event do the rest. Confirms the route's `opens` claims.
- **H10** `bioplant-rika` objective 8: the Rika scene fires on the way and
  carries the party off the map, so the `go_to`'s target no longer exists. A
  scene that ends the walk off the target map now ends the objective; the next
  objective's `expect` asserts what the scene left.
- **H11** north bank: the `expect` claimed at least 1103 meseta, one native run's
  purse; a New Game run had 914. The purse is what the battles paid, not a rule,
  so the clause is gone.

An earlier gap, found on the first academy run: `Igglanova: talk` presses Speak
and the engine starts interaction area 0 of map 23 (event `$6B`), not an object's
dialogue; the controller accepts an area event as what the press opened.

### H12: the Alshline basement kills an under-levelled party after the walk shifted

Restoring Mile moved the frame at which every later random battle rolls (the
RNG stream is a function of the frame count and the pad history), so R1-5's
first attempt met a different basement sequence: formation 177 on map `$48` took
Chaz to 11/39 and Hahn to 8/30 in one round, `run_unless_boss` kept pressing RUN
(each failed run is another round), and the party was wiped inside five
encounters (`lost_battle`, chapter `alshline`, objective 13, frame 74,971).
R1-4's six-battle basement walk was a lucky draw, not a margin: Chaz was L4 and
Hahn L3 entering it. Not a port defect.

Fix, in `rust/psiv-campaign`: a new chapter `basement-training` between
`rune-dorin` and `alshline` rests at the Tonoe inn and then patrols the strip of
Motavia outside Tonoe's gate (the only ground reachable from it without a warp)
until the party is level 6. `patrol` gained an optional `refuge`: objectives run
when a member has fallen or a living member is below half HP after the camp
cure (`Driver::needs_refuge`), here an inn trip out and back, after which the
patrol resumes. Without it the first patrol attempt trained until the healers
ran dry and the party died on the strip. R1-5 trains in 22 battles with one
refuge and enters the basement at Alys L8, Chaz L6, Hahn L6, Gryz L7.

### H13: the Aiedo inn returns `AiedoEventPending` (#39)

**Open port defect (issue #39), not on the critical path.**

- **Halt:** C1-1, `rest_inn` at the supermarket counter (50,32) facing up:
  `unexpected_state -- the counter refused: "Aiedo rest event pending."`
  (`build/c1/evidence/h-inn-39-report.json`).
- **Cause (port):** `rust/psiv-runtime/src/shop.rs:126-127` returns
  `InnResult::AiedoEventPending` when selector 6 is rested at with `$42` and
  `$46` clear, after `RecoverStats` and before the bill; the shop window shows
  the placeholder at `rust/psiv-runtime/src/session/shop.rs:397`; nothing runs
  `Event_GirlsSneakingOut`, whose transcription is registered
  (`docs/scenes/27_GirlsSneakingOut.md`).
- **Cartridge:** `loc_66122`, `reference/ps4disasm/ps4.asm:136380`; after
  `RecoverStats` it tests selector 6, `EventFlag_Zio` and `EventFlag_GirlsCaught`
  and calls `Event_GirlsSneakingOut` (`ps4.asm:136406`, body `:146949`), which
  sets `$46`; the bill is taken after the scene (`loc_661B6`, `ps4.asm:136411`).
- **Smallest change:** make `shop_stay` hand the scene to the session: run
  event `$23` in the shop context (the scene tests already start it on map
  `$63`) and settle the bill when the scene ends.
- **Does it block the story?** No. `$46` is read only by Aiedo's own map data
  (`MapDataMan_AiedoSupermarket`, `MapDataMan_AiedoPrison`,
  `ps4.asm:109146-109178`, which show or hide the two girls) and by this inn.
  No trigger the next arc uses reads it (`RunEvent_SavingDemi` tests `$42`
  only, `docs/scenes/31_DemiRescue.md`). The route rests at Chaz's house
  instead, which is a free rest the cartridge offers (scenes 28 and 29, passing
  in `aiedo-chaz-house`). When #39 is fixed a chapter that rests at the inn
  should assert `$46` and the prison map should show the girls.

### H14: the fort needs a trained party and a rest that is not the Aiedo inn

Route and runner, fixed in this lane.

- C1-3 walked the fort's first corridors at level 8 to 11 with Rika at level 3:
  `lost_battle` on map `$84` (Zio Fort F1), the party wiped inside three
  encounters even with `run_unless_boss` (a failed RUN is another round of
  enemy attacks).
- Fix: a new chapter `aiedo-training` patrols the open ground east of Aiedo's
  gate (Motavia (40,56) to (50,56)) until every member is level 12, with a
  `refuge` that rests at Chaz's house (a free rest) when a member falls or the
  healers run dry, and rests there once more before leaving so the party enters
  the passageway whole. 171 battles, 281,447 frames. The first draft stopped at
  level 10 and arrived at Juza with three members down (C1-5 draft).
- Runner: `walk.rs` now ends `go_to_map` at once when a scene's yes/no prompt
  opens on the target map (Chaz's house asks on arrival), leaving the prompt
  for the next `answer` objective; any other prompt still halts as before.
  `tests/runner.rs::an_arrival_prompt_is_the_next_objectives_to_answer`
  (ignored, release) passes and halts when the `answer` is removed.
- A refuge bug found on the way: a `go_to_map` with `via_warp` names a warp of
  the *current* map. The refuge's first leg runs on Motavia, where warp 3 is
  not Aiedo's gate, so it walked the party across the continent into poison
  and a defeat (one draft run). The route names no `via_warp` there now.

### H15: passageway and Zio Fort encounters roll unsupported abilities

**Open port defects.** Random battles on the only road to the Zio Fort fault
the battle presentation when an enemy rolls an ability the engine does not run
(`BattleEvent::UnsupportedAbility`, emitted at
`rust/psiv-core/src/battle/engine.rs:745` and `:847`; the session turns it into
a fault at `rust/psiv-runtime/src/session/battle/mod.rs:386-391`).

| Ability | Carrier and where | Halt | Cartridge |
| --- | --- | --- | --- |
| 18 FUSION (`$12`) | 34 ZolSlug, formation 211 (three ZolSlugs), Passageway `$81`, the condition `EnemyAI_ZolSlugs` holds once exactly two remain (`rust/psiv-core/src/battle/enemy_ai.rs:106,463-466`) | `unsupported ability 18 for fighter 7`, frame 171,935 (C1-2); `build/c1/evidence/h15-fusion-report.json` | `ps4.asm:23017` (the condition); the routine is `EnemyAttack_Blob` (`ps4.asm:19241`, label `:23043`); ledger row `docs/battle/ENEMY_ABILITIES.md:276` (class unknown) |
| 33 FIREBREATH (`$21`) | 83 Ripper (regular list `[0,0,0,0,0,33,33,33]`), formations 219, 220 and 226 in the Zio Fort (`$82`, `$84`); 25 formations in all | `unsupported ability 33` (C1-3 variant, C1-4); `build/c1/evidence/h14-untrained-report.json`, `h16a-airslash-report.json` | `EnemyAttack_Ripper` `ps4.asm:21587` to `loc_23D0A` `ps4.asm:47507`; ledger row `ENEMY_ABILITIES.md:196` (damage, †) |
| 45 DEBAN (`$2D`) | 70 ShadowSabr, conditional slots `[45,45,44,44]` under conditions `[7,7,2,2]`, formation 226 (F1) | `unsupported ability 45 for fighter 7` (C1-4, first draft) | `EnemyAttack_ShadowSabr` `ps4.asm:21933`: arm `$2C` at `loc_F85E`, arm `$2D` at `loc_F89A`; the ledger lists only `$2C` (Airslash) for these carriers and `$2D` for 116 Radhin, so its conditional-ability table omits ShadowSabr's `$2D` |

- **Disposition:** the route runs from every encounter on the way
  (`run_unless_boss`, chapters `passage-to-zio-fort` and `zio-fort-approach`),
  which a player can choose. It is not a fix: with the draws of C1-3 and C1-4
  the same walk halts, and run C1-5 passes only because every fight in it
  ended by RUN before such an ability rolled. A change to the route anywhere
  upstream shifts the draws.
- **Smallest change:** one `DamageRoute` entry each in
  `rust/psiv-core/src/battle/enemy_damage.rs` (`DAMAGE_SKILL_ROUTES`, line 421)
  for FIREBREATH on 83 and its sibling carriers and for the two ShadowSabr arms,
  transcribed from the routines above; FUSION is not a damage ability (effect
  `$1F`) and needs its own transcription. FIREBREATH is the largest class: the
  ledger counts 54 formations (+3 boss) across the Zio Fort, Ladea Tower, Island
  Cave and later maps, so every dungeon on this arc meets it.

### H16: Juza's battle rolls ZAN and FORCEFLASH, which the engine does not run

**Open port defect on the critical path, no legitimate alternative: the route
stops here.**

- **Halt:** C1-7, chapter `zio-fort-juza` (scratch), objective 0 (`talk npc 0`
  at Juza, `$87` (32,19)): the dialogue fires event `$40`, event battle 3
  begins, and Juza's first action faults: `unsupported ability 71 for fighter 6`
  (frame 458,343; `build/c1/run-4/halt-juza.json`). An earlier draw halted on
  ability 86 for the same fighter.
- **Cause (port):** event battle 3 is one JUZA (enemy 114, 1,523 HP) with the
  regular list `[64,64,68,68,71,71,86,86]`. `DAMAGE_SKILL_ROUTES` carries 114's
  WAT `$40` (`rust/psiv-core/src/battle/enemy_damage.rs:626`) and FOI `$44`
  (`:654`) and nothing for ZAN `$47` or FORCEFLASH `$56`, which are half of
  his draws. A fight of more than a couple of rounds cannot avoid them.
- **Cartridge:** `EnemyAttack_Juza` `ps4.asm:20575`; ZAN `loc_2AF18`
  (`ps4.asm:56372`); FORCEFLASH `loc_2A300` (`ps4.asm:55527`); ledger rows
  `ENEMY_ABILITIES.md:228` and `:241` (both damage, unsupported; FORCEFLASH
  is also carried by 115 Greneris and 116 Radhin, ZAN by 100 TechMaster).
- **Why no alternative:** the stairs to F3 are patched shut until `$41` and
  `$48` are set (`ZioFortJuzaRoom` map effect entry `$38`: with both clear the
  four cells at (24,18) are written to collision 0), `$41` is set by talking to
  Juza, `$48` by the trigger after his battle (`docs/scenes/50_JuzaDefeated.md`),
  and F3 and F4 (the Demi rescue) are reachable only from that room (the
  planner finds no other chain to `$8A` or `$8B`).
- **Smallest change:** two `DamageRoute` entries for enemy 114 (ZAN, FORCEFLASH),
  traced to the routines above, with the regression test that fights event
  battle 3 to its end.
- **Forecast, not evidence:** the scripted battles ahead are event battle 4
  (enemy 152, ZIO, ability 112, "implemented" in the ledger), 5 (117
  GY-LAGUIAH, abilities 33 and `[0,0,0,0]`; FIREBREATH is unsupported) and 6
  (139 ZIO, ability 84 BLACK WAVE, the ledger lists it unsupported with 0
  formations); Ladea Tower's carriers include VOICE `$34` and FIREBREATH.
  The next lane should expect those halts after H16 is fixed. The party also
  arrives rich (21,832 meseta) with Hahn holding a shield and no weapon; shopping
  at Aiedo's weapon shop (`$5B`) and supermarket is a legitimate step before
  the fight, and Chaz's house is the rest.

## `verify` items resolved by playing

R0 left 12 objectives marked `"verify": true`. All are resolved and the flags
dropped; the run that settled each is in the route's `note`.

| Chapter and objective | Question | Answer |
| --- | --- | --- |
| academy 10 | Hahn: object 0 or 1 | object 0 |
| academy 16 | Igglanova: which object | none: the press starts interaction area 0 (event `$6B`); object 0 only places the party |
| holt 0 | the Mile detour | restored once the sand worm trigger was transcribed (H1); arrival is outside the worm's box |
| holt 7 | Zema cell after the Holt scene | (30,11), not (60,21) (H2) |
| rune-dorin 20 | Dorin: object 0, 5 or 8 | object 0 |
| alshline 7 | Gryz's storage door | object 1 (H5) |
| zema-training 3 | patrol until level 6 and 1,180 meseta | holds after 55 battles in R1-4; the runner cures the party between them |
| bioplant-elevators 4 | `temp:0` after the door | set |
| bioplant-elevators 8, 10 | `opens` cells of the elevator doors | walkable (collision 1) after the scene (H9) |
| bioplant-rika 0, 4 | the same | the same |

## Not claimed

- **Balance.** Members fall in `bioplant-rika` (ten command windows open with a
  member down; Gryz, Alys and Hahn stand at 0 HP when the Rika scene ends); the
  inn restores them (`north-bank` rests at Zema before the crossing). The policy
  does not revive and does not grind for safety, and the party reaches Aiedo at
  levels 6 to 8. A route that wants a healthier party names an inn or a
  `patrol`.
- **Seeds.** Frames and battles of a run are a function of the whole pad
  history. A `--from-chapter` run restarts the frame counter and RNG, so it is
  the same game but not the same frames; only the full run is evidence.
- **Unexercised objectives.** `sell`, `use_item` and an explicit
  `use_technique` are not in `routes/main.json`; `tests/runner.rs` plays them at
  the Piata item shop. `Session` field notices (a member falling to poison) have
  no pad path, so the runner halts on one (`unexpected_state`); none occurred.
- **The boundary.** Construction (`Runtime::new_game`, `enable_battles`,
  `start_event`, `from_save`) is the one thing the runner does outside
  `Session::frame` and views, in `src/start.rs`. Node S5 will make those
  crate-private; the runner then needs a `Session` constructor taking the pack
  and the battle files.
