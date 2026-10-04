# Runner log: Zelan to Dezolis (C5, H28)

Lane c5-dezolis (2026-10-04): the route from Zelan through Wren, the Canceller chest,
the sabotage, the Chaos Sorcerer and the crash landing to first control on Dezolis, and
the halt it found past that (H28). The Zelan arc before it is
[RUNNER_LOG_ZELAN.md](RUNNER_LOG_ZELAN.md); the index and current state are in the
[runner log](RUNNER_LOG.md).

## C5 runs

Base `f6e84f9` (main: destination menu, map updates, the crash landing's live BG write
and `World_Index = 1`), release builds, `CARGO_BUILD_JOBS=2`, one heavy command at a
time. The pack is the owner's rebuilt local `runtime-pack` (manifest SHA-256
`b6d9dd8c…83bc`), read through a symlink and never written. Evidence is under the ignored
`build/c1/` of the lane's worktree; the commands regenerate it. No runner or port source
changed: the three new chapters ran on the committed runner.

| Run | Command (from the worktree) | Result |
| --- | --- | --- |
| C5-0 | `psiv-campaign run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-0 --tape build/c1/run-0/run.tape --report build/c1/run-0/report.json` (the 30 chapters of the base) | exit 0, 2,617,668 frames, digest `0bd25017e4696c02`: the same as M23-1, so the rebuilt pack moved nothing on that prefix; the `29-mota-spaceport` snapshot is `038afc2a…0ab556` |
| C5-1 | scratch chapter `zelan-wren-canceller`, `--from-chapter` the C5-0 snapshot | exit 0, 6,944 frames, 0 battles: Wren L20 210/210 joins (`$70`), chest 5 opens and `$72` is set by the `expect` 433 frames after the chest objective began (the C4-4 halt H25 is gone) |
| C5-2 | scratch chapter `zelan-sabotage`, `--from-chapter` C5-1's snapshot | exit 0, 13,834 frames: the sabotage menu lists Kuran alone, Speak confirms, the Chaos Sorcerer falls in one round, the crash landing ends at Raja Temple `$14C` (94,27) with Raja in the party. The board's wait ran the whole arc, so `fight_scripted` was added to assert the battle (13,836 frames) |
| C5-3 | scratch chapter that exits the temple: `go_to_map 1 via_warp 0`, `--from-chapter` C5-2's snapshot | **halt `scene_fault` `SceneMissing { event: 67 }`**, frame 99, Dezolis `$001` (36,95): H28; report kept at `build/c1/h28/halt-report.json` |
| C5-4 | the chapter ended at the exit's foot, `go_to 332 (95,37)` | exit 0, 98 frames, digest `aaa42305057f83f6` |
| C5-5, 6 | `run rust/psiv-campaign/routes/main.json --save-dir build/c1/run-{a,b} --tape …/run.tape --report …/report.json` (33 chapters, twice from New Game) | both exit 0 on the final route file (SHA-256 `8d5a1e87…d657`), **2,638,546 frames**, digest `6d365219c15bc775`, tape SHA-256 `4af507fa…15af`, `cmp` says the two tapes and the two `32-dezolis-first-control` snapshots are byte-identical; the pad SAVE `route/slot_1.sram` is `4aeff0b1…ec62`, unchanged |
| C5-7 | `replay build/c1/run-a/run.tape` | digest `6d365219c15bc775`, 2,638,546 frames, exit 0 |

Chapter snapshots of the full run (SHA-256): `29-mota-spaceport` `038afc2a…0ab556`,
`30-zelan-wren-canceller` `fc662725…f736`, `31-zelan-sabotage` `168c209a…8adc`,
`32-dezolis-first-control` `c244f6f5…c25e`. Chapter frames: 6,944, 13,836 and 98; the
`--from-chapter` iterations (C5-1, C5-2, C5-4) counted the same frames per chapter, though
their digests differ from the full run's because a loaded snapshot restarts the frame clock.

### The new chapters

- **`zelan-wren-canceller`** ([40_MeetingWren](../scenes/40_MeetingWren.md)). C4's probe
  facts held on re-verification: the elevator door `interact` from (30,18) up, F1 warp 0,
  `talk` npc 0 from (31,14), chest 5 at (49,49). Chest before the elevator, so
  `Event_CancellerReminder` (H26, #56) is never reached. The closing asserts `$70` and
  `$72`.
- **`zelan-sabotage`** ([42](../scenes/42_SpaceshipSabotage.md), [43](../scenes/43_CrashLanding.md)).
  Back down the F1 elevator, to Zelan (30,47), then `board` down to Kuran (`to: 4`): the
  step onto row 48 fires `RunEvent_SpaceshipSabotage` (`$28`) ahead of
  `RunEvent_KuranEnterSpaceship` (`$23`) because Zelan's event list is `$0D,$28,$23`
  (`runtime-pack/maps/18D_Zelan.json`) and `$28` needs `$72`. After the Speak press the
  takeoff, the dialogue and event battle 8 (`CHAOSSORCR`, 460 HP; Chaz TECH `$08`, Rika
  attack, Rune TECH `$06`, Wren attack, dead in round 1 against a party at L27, L29, L27
  and L20) run inside the board's own wait, then `RunEvent_CrashLanding` (`$29`) plays
  to Raja Temple. `fight_scripted` then asserts a scripted battle began; the closing
  asserts the five-member party, `$70 $71 $72 $85 $88` and the cell (94,27).
- **`dezolis-first-control`**. The party's first walk on Dezolis: through the temple
  (the live BG write of #67 opened the lower chunks, so the walk from the scene's end to
  the exit passes) to (95,37), the cell above the exit's warp 0. The route stops there on
  purpose (H28); the closing keeps `$80` clear.

`World_Index` is 3 after the flight to Zelan, 4 after the sabotage's confirm and 1 after the
crash landing (`ps4.asm:155847`); the whole-route test reads the last.

### The 237-frame flight gap (#77)

It shifts the field RNG after a flight, but nothing in this arc consumed it: the Wren
chapter and the walk in the temple fought no random battle, and the Chaos Sorcerer fight
is scripted and was won in one round at these levels. No halt here
depends on #77. It still holds for the next arc's random battles on Dezolis.

## H28: leaving Raja Temple fires `Event_OutsideRajaTemple`, which has no scene

**What it is.** Warp 0 of Raja Temple (`runtime-pack/maps/14C_RajaTemple.json`, cells
(95..96,38)) lands on Dezolis `$001` at (36,95) (`001_Dezolis.json` warp 12 is its mirror).
The map's event list is `[$31, $35, $36]`, and the first, `RunEvent_OutsideRajaTemple`
(`ps4.asm:115936-115942`), has no position test: with Snowstorm `$80` clear it writes
`Event_Index = $43` on the first field frame there. `Event_OutsideRajaTemple`
(`ps4.asm:149239-149244`, event table `:120641`) points the dialogue system at
`DialogueTree14`, runs entry 6 (`Event_GetAndRunDialogue`) and sets `$80`. The census lists
it as `$0043` "(later)" ([12_ArcTriggerCensus.md:239](../scenes/12_ArcTriggerCensus.md)),
and no scene is registered for it: the trigger fires
(`rust/psiv-core/src/trigger_table.rs:243-244`), `scene_for(EventIndex(0x43))` finds none
(`rust/psiv-core/src/scenes/mod.rs:218-220`), `install_scene` returns false
(`rust/psiv-runtime/src/scene_control.rs:20-22`) and the frame reports
`SceneMissing { event: 67 }` (`rust/psiv-runtime/src/field_triggers.rs:64-68`); the runner
halts `scene_fault` (`rust/psiv-campaign/src/driver.rs:33`). It is a #56-class gap, a
trigger-fired scene with no transcription; whether #56's own list of four includes it is
for the orchestrator to say, this lane did not read the issue.

**Evidence.** `build/c1/h28/halt-report.json`: halt `scene_fault`, `SceneMissing { event: 67 }`,
frame 99, map 1, cell (36,95), `f98: MapChanged { map: MapId(1) }`, `f99: SceneMissing`.

**Disposition: stopped at the foot of the exit, no legitimate alternative.** Raja Temple has
one warp, so a player can only leave by it, and `RunEvent_OutsideRajaTemple` fires on every
arrival until the scene sets `$80`. The first control on Dezolis is the temple's, and the
route reaches it; the first step onto the overworld is the next lane's.

**Smallest change** (an orchestrator lane, `rust/psiv-core/src/scenes/`, outside the C5
write set): transcribe `Event_OutsideRajaTemple` as `$0043` beside
`CARNIVOROUS_TREES` (`dezo_campaign.rs:183-216` is the pattern): `SetDialogueTree`
for `DialogueTree14`, the standard dialogue op for entry 6 (its `PauseMusic` and
`ResumeMusic` actions are listed in
[DIALOGUE_ACTIONS.md](../scenes/DIALOGUE_ACTIONS.md)), `SetFlag` `$80`, return; register
it in `SCENES`, add its op count to the table test in `scenes/mod.rs`, and change the
census row. Then the route's last chapter walks on through warp 0 (`go_to_map 1 via_warp 0`,
which is what C5-3 ran), and the next stop is whatever the Dezolis arc finds. Dialogue
entry selection by `World_Index` (#79) is not in play: the scene names its tree.

## Negative controls

| Break | Guard | Result |
| --- | --- | --- |
| the chest objective opens chest 4 (a scratch copy of the route) | route run `--from-chapter zelan-wren-canceller` | halts `expect_failed`: `$10B` and `$72` clear (exit 2) |
| no Canceller: the same step with `$70` alone | `tests/zelan_arc.rs` `the_sabotage_needs_the_canceller_flag` | the ordinary boarding opens and lists `[0]` only, so Kuran is a `menu_entry_missing` halt, and `$71` stays clear |
| no chest opened | `tests/zelan_arc.rs` `wren_joins_and_the_canceller_chest_…` | asserts `$72` clear before the chest: it is, so the `$72` that follows is the chest's |
| the sabotage's `to: 9`, F1 `talk` npc 7 | `tests/validator.rs` `the_zelan_chapters_reject_…` | rejected with chapter `zelan-sabotage` and `zelan-wren-canceller` and their reasons |

## Checks

`cargo test -p psiv-campaign -- --test-threads=1` (debug): 14, 1, 5, 8, 10, 16, 4, 12 and 3
passed across the lib, bin and seven integration targets, 2 ignored (the whole route and
the arrival-prompt test, both run in release below); `cargo test --release -p psiv-campaign
--test runner -- --ignored --test-threads=1`: 2 passed (the whole route to Raja Temple,
its replay digest and the pad SAVE, 41.5 s). Clippy `-D warnings` and `fmt --check` are clean.
