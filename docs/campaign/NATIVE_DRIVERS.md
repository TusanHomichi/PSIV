# Native drivers: what the tape retires and what stays

The generic replay driver (`tools/native/native_tape.gd`, entered through
`tools/verify_native_tape.py` and, for a whole run,
`tools/verify_native_route.py`) replays the campaign runner's own pad tape and
compares every chapter save byte for byte
([native tape replay](CAMPAIGN_RUNNER.md#native-tape-replay)). It replaced the
bespoke per-hop Godot drivers that once covered the campaign. This document
lists, driver by driver, what each one checks, which chapter of the tape covers
it, and whether it stays in `tools/native/`.

## The rule behind the decisions

The retired drivers were removed in commit 138da9f; `git show
138da9f^:tools/native/<name>.gd` prints one.

A driver is **retired** when it plays a stretch of the story route the tape
plays: the tape replays the same pads from New Game and its chapter saves
(party, stats, equipment, inventory, purse, flags, map, cell) match the native
game's bytes, which is a stronger state check than the driver's own
assertions. Ten of the twelve retired drivers are named as sources in
`rust/psiv-campaign/routes/main.json`, which was transcribed from them; the other
two are variants of a named one (`native_alshline_retreat` of
`native_alshline`, `native_bioplant_recovery` of `native_bioplant`).

A driver is **kept** when it starts from an isolated fixture or a debug
selector and exercises a branch the route never presses (a cancel, an undo, a
defeat, a full pack, a slot other than 1, an ability the policies do not
choose), or when its product is a capture or a comparison with the original
game. The tape carries no such branch: its pads come from the runner's
controllers and policies, and its only capture is one optional endpoint image.

What the tape proves and does not prove follows
[AGENTS.md](../../AGENTS.md#verify-the-claim-you-intend-to-make). The continuous replay never loads a save, so persistence is checked by its own
entry point: `tools/verify_native_route.py --continue-probes` loads each chapter
save in a fresh process through the title's CONTINUE and requires the
snapshot to re-encode the same bytes.

## Inventory

Chapters are the 50 of `routes/main.json` at digest `40c7f505ecc5c357`
(index in parentheses).

### Retired: they replay a stretch of the route

| Driver | What it proved | Covering chapter | Decision |
| --- | --- | --- | --- |
| `native_route` | One START, Piata and the Academy on foot, conversations, Igglanova's battle through the menu, the way out to Motavia | `academy` (0): 32,308 frames, 4 battles, the scripted Igglanova fight | Retired. It also took a camp SAVE and captured PNGs at fixed legs; the route SAVEs from chapter 1 on and the replay takes no per-leg capture |
| `native_motavia` | CONTINUE from the Academy save, camp RES recovery, the way to Holt and Holt's conversation | `holt` (1) | Retired. Its CONTINUE half is the per-chapter probe |
| `native_tonoe` | Holt to Rune and Dorin, the paid inn, the Dorin YES/NO prompts, Gryz joining | `rune-dorin` (2) | Retired |
| `native_alshline` | Gryz's door, the basement, the Alshline chest, an ordinary SAVE back in town | `alshline` (4); the chest is the route's first `open_chest`. The grinding of `basement-training` (3) is the route's own addition | Retired |
| `native_alshline_retreat` | The same quest using RUN on random encounters | The `run_unless_boss` policy of `rune-dorin` (2), `alshline` (4) and `zema-rescue` (5) presses RUN from the battle menu | Retired |
| `native_zema_return` | Rest, back through the pass, Zema's rescue boss, the post-battle conversation, SAVE | `zema-rescue` (5) | Retired. Not carried: the resident-palette, 50-page and Igglanova-entrance observations (see the gaps below) |
| `native_zema_outfit` | Purchases at two counters with exact prices, equipping through the camp, a paid rest, SAVE | `zema-outfit` (6): 6 buys, 7 equips, one `rest_inn` | Retired. Not carried: the hand-selector cancel check (see the gaps below) |
| `native_zema_training` | Birth Valley battles and paid Zema rests to a target level | `zema-training` (7), policy `train_with_inn` | Retired |
| `native_zema_armour` | Carbon suits bought and equipped for Alys and Chaz | `zema-armour` (8) | Retired |
| `native_bioplant` | The BioPlant doors, elevators, encounters, poison cures and dialogue to Rika, with the alarm palette captures | `bioplant-elevators` (9), `bioplant-rika` (11), policy `bioplant_survival` | Retired. Not carried: the alarm captures `verify_native_alarm.py` reads |
| `native_bioplant_recovery` | One BioPlant encounter, normal recovery, SAVE | `bioplant-elevators` (9): 4 battles, each followed by the camp recovery | Retired |
| `native_post_rika` | Zema's inn, then the opened northern crossing to the bank | `north-bank` (12) | Retired |

### Kept: isolated fixtures, branches the route never presses, captures

| Driver | What it proves | What the tape does not carry | Decision |
| --- | --- | --- | --- |
| `native_continue.gd` + `verify_native_continue.py` | Fresh-process title CONTINUE from slot N, one field step, an ordinary SAVE into slot 2, and the exact bytes that may change between the two files | The tape saves slot 1 only and its CONTINUE probe reads slot 1. Its route-receipt input is now optional (the loaded slot is the reference) because the route drivers that wrote it are retired | Kept |
| `native_party_order.gd` + `verify_native_order.py` | STATE, ORDER pick, undo and cancel through ordinary input, the commit, a SAVE, and the comparison of the saved bytes and captures with the original game's RAM and frames | Chapter `bioplant-order` (10) proves the committed order and its save; no tape presses undo or cancel, and none compares with the original | Kept, and made self-contained: it no longer extends the retired route skeleton |
| `native_camp_abilities.gd` | Field RES and RECOVER from the camp: target cancel leaves HP and TP unchanged, exact TP and use costs | The route casts RES in recovery but never cancels, and never uses RECOVER | Kept (also the base of eight fixtures) |
| `native_chests.gd` | Meseta, item and full-pack chests: the discard, compact, return and necessary-item protections, the "already open" message | The route opens three chests with room in the pack | Kept |
| `native_pipes.gd` | ESCAPIPE and TELEPIPE from ITEM: a blocked cast is not consumed, browsing and cancel are free, the right duplicate is spent | No route objective uses a pipe | Kept |
| `native_travel.gd` | HINAS and RYUKA from TECH: destination, cancel, save | No route objective casts a travel spell | Kept |
| `native_status.gd` | A reordered five-member party in battle, three-digit summaries, every STATUS page, SAVE into slot 2 | The route never opens STATUS | Kept |
| `native_field_status.gd` | Poison steps, the defeat notice, the game-over title and CONTINUE again | A lost battle halts the runner; no tape contains a defeat | Kept |
| `native_progression.gd` | Learned-ability messages (TSU, WAT), slots and spent uses after normal attacks, SAVE into slot 2 | The saves carry the learned state; the messages and slot 2 are not asserted | Kept |
| `native_death_abilities.gd` | CRASH and BROSE through the command windows, spent uses and the presentation | The policies pick by estimated damage, never to test an instant-death rule | Kept |
| `native_rimit.gd` | RIMIT on a group, sleep and paralysis, paid TP | No policy casts it | Kept |
| `native_battle_recovery.gd` | ANTI, RIMPA, REVER and REGEN cast in battle by an injured original-record party | Policies cure with the camp after a fight and with the healing technique they choose | Kept |
| `native_enemy_attacks.gd` | DEFEND through COMD until Acid Breath runs, then a win | No policy defends | Kept |
| `native_combat.gd` | A real battle from the debug selector: RES, FOI on a second enemy, GELUN, with captures of the technique list and target page | Policy choices only; no capture | Kept (capture observer) |
| `native_skills.gd` | EARTH, VORTEX and VISION through the skill window, uses re-read after the round, with captures | As above | Kept (capture observer) |
| `native_items.gd` | Dynamite on an enemy, Monomate on an ally, poison cure, item reservation, with captures | As above | Kept (capture observer) |
| `native_fission.gd` | The Academy boss: Defend twice for both births, Dynamite on one creature, the replacement, with captures | `academy` (0) fights the boss with the policy, not this order of commands | Kept (capture observer) |
| `native_choices.gd` | YES, NO and Cancel at Chaz's house under retail-paced text, with captures | `aiedo-chaz-house` (14) answers the same prompt; it runs at its own pace and captures nothing | Kept (capture observer) |
| `native_dialogue.gd` | The post-Igglanova scene from the isolated `dialogue_resume` save, captured page by page without acknowledgement | State only | Kept (capture observer) |
| `native_rika_fixture.gd` | Rika's join scene from `PSIV_DEBUG_EVENT=8007`, every page and the Holt exit effect, no input | `bioplant-rika` (11) proves the join's state | Kept (capture observer) |

### Kept until the tape takes frame-addressed captures

| Driver | What it proved | Covering chapter | Why it is still here |
| --- | --- | --- | --- |
| `native_opening.gd` | One Up press after the auto-acknowledged opening has returned control | `academy` (0) covers the state: it starts from START and the first pad is the first gameplay frame | It also takes the rendered first-control capture at frame 3360, which the tape replay cannot take until it captures at chosen frames ([#93](https://github.com/TusanHomichi/PSIV/issues/93)) |
| `verify_native_alarm.py` | The BioPlant alarm palette ramp and its four-frame cadence in captured PNGs | None. Its only producer, `native_bioplant`, is retired and the tape takes no mid-run capture | Its comparison is the check [#93](https://github.com/TusanHomichi/PSIV/issues/93) feeds from frame-addressed tape captures. Until then it has no producer, and the alarm check is suspended |

### Running the opening capture

`native_opening.gd` needs the local pack, Godot and Xvfb, and runs with every
other `PSIV_DEBUG_*` selector and `PSIV_LOAD_SLOT` unset. `tools/godot.py`
resolves the Godot executable ([development guide](../DEVELOPMENT.md#launch-the-native-game)):

```sh
cargo build -p psiv-godot --manifest-path rust/Cargo.toml
mkdir -p build/native-opening
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 \
  PSIV_SAVE_DIR="$PWD/build/native-opening/saves" \
  PSIV_DEBUG_TITLE_SHOT=1 PSIV_DEBUG_TITLE_AUTOSTART=1 \
  PSIV_DEBUG_AUTOCLOSE_SCENE=1 PSIV_DEBUG_SCENE_TICKS=1 PSIV_DEBUG_INPUT=1 \
  PSIV_DEBUG_STATE="$PWD/build/native-opening/state.json" \
  PSIV_DEBUG_SHOT="$PWD/build/native-opening/field.png" \
  PSIV_DEBUG_SHOT_FRAME=3360 \
  python3 tools/godot.py run -- \
  --script "$PWD/tools/native/native_opening.gd" --display-driver x11 \
  --rendering-method gl_compatibility --audio-driver Dummy \
  --fixed-fps 60 --quit-after 3370 \
  --log-file "$PWD/build/native-opening/godot.log"
```

The expected state at tick 3360 is map `$13`, cell `(48,18)`, Chaz alone, no
active scene and 500 meseta; a process that exits successfully is not by itself
a pass. Dialogue is auto-acknowledged and audio is the dummy driver, so the run
says nothing about dialogue pacing, sound or controller hardware.

## What the retirement leaves uncovered

These were proved by a retired driver and are carried by nothing today:

- **Mid-scene captures.** The route drivers saved PNGs at fixed legs and
  scenes; the replay's one capture is the endpoint. The alarm cadence check
  above is the casualty. Capturing at named frames in `native_tape.gd` would
  restore all of them ([#93](https://github.com/TusanHomichi/PSIV/issues/93)).
- **Resident palette, 50 dialogue pages and Igglanova's visible entrance**
  (`native_zema_return`): presentation observations of the Zema rescue
  ([#93](https://github.com/TusanHomichi/PSIV/issues/93)).
- **The equipment hand-selector cancel** (`native_zema_outfit`): cancelling
  the hand choice mutates neither equipment nor inventory. `suites::camp_equipment`
  covers the accepted path only ([#121](https://github.com/TusanHomichi/PSIV/issues/121)).
- **Per-chapter replay.** A chapter cannot be replayed alone, because a save
  holds no RNG state ([why](CAMPAIGN_RUNNER.md#native-tape-replay)).
