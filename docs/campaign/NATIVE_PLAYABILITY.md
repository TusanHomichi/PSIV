# Native playability — evidence ledger

## Current checkpoint — 2026-09-15

Connected routes now reach the Zema aftermath and a verified BioPlant `$A7`
save. STATE/ORDER, CROSSCUT gameplay and bounded ANTI-before-healing recovery
are implemented and verified. The onward six-fight attempt reaches `$A9`
but loses Hahn; the connected Rika join/escape remains unfinished.

The matching source passed 898 Rust tests. Code checkpoint `82de4a3` also
passed 927 Python tests, post-commit camp checks, formatting and strict Clippy.
See the [BioPlant ledger](BIOPLANT_NATIVE.md) for current save hashes and proof
limits, and the [roadmap](../ROADMAP.md) for the next milestones. Earlier dated
results below retain their original counts and scope.

## Owner objective

Play Phantasy Star IV on a modern computer using Godot, Rust, and the
ROM-derived JSON/assets. The installed game must run without an emulator.
The emulator remains a development reference. Completing the playable game
comes first; a modding frontend is a later goal.

The eventual mod interface may be a local browser editor with an API usable
by agents. Preserve the extracted retail pack, store mods separately, and
validate changes before launching Godot. No editor implementation is part
of this repair. Keep the default unmodified game usable without any editor
or agent service.

## Audit context

The earlier audit established that selected matching reference screens did
not prove complete gameplay. Its obsolete coverage table and task ordering
have been removed; use [the roadmap](../ROADMAP.md) for current gaps. The dated
implementation and failure records below retain their original evidence.

The audit's extracted enemy data contains 100 of 153 enemies with nonzero
regular abilities and 46 with nonzero condition IDs. Complete animation
coverage does not implement those gameplay effects.

Code anchors:

- `rust/psiv-core/src/battle/engine.rs`: `Command`, `roll_enemy_ability`.
- `rust/psiv-godot/src/battle/ui_input.rs`: `take_command`;
  `commands.rs`: pure menu state and target selection.
- `rust/psiv-godot/src/camp/mod.rs`: `confirm_root`, STATE/ORDER.
- `rust/psiv-runtime/src/scene_runtime.rs`: `DialogueResume`, `ChoiceRequested`.
- `rust/psiv-runtime/src/camp.rs` and `camp/abilities.rs`: field recovery and confirmation costs.
- `rust/psiv-runtime/tests/next_arc.rs`: the campaign test explicitly
  relocates between maps and directly starts scenes. It is useful script
  coverage, but does not prove a player can traverse the whole campaign.

## Repairs in this slice

START previously reused `debug_scene_runtime`: an empty `GameState` with
two characters and a location. That discarded the retail initial 500
meseta, eleven preloaded extended flags, and three town flags. The
first-control constructor also omitted the starting money.

`psiv-data` now reads the existing `game_start.json` initializer separately
from the manifest's post-opening summary. `Runtime::new_game` supplies the
pre-opening party, money, settings and flags; START uses that constructor.
The first-control constructor shares the initializer and then applies the
post-opening party/flags. Loaded saves remain a separate path.

The uninterrupted opening test exposed a second bug: triggers were only
checked after a player step. Returning from the opening therefore left
Chaz's first line waiting for movement. The runtime now checks map events
when control returns from a scene, before accepting movement. Retail's
`FieldRoutine_Controls` runs `RunEvents` before `FieldControls_GetInput`;
see `reference/ps4disasm/ps4.asm:116755`.

New regression coverage:

- The exact constructor called by Godot START retains the initial state,
  with the opening's two progression flags still clear.
- A single runtime plays the opening with real map loads, receives Chaz's
  first line without movement, and reaches the first-control state. Dialogue
  is acknowledged automatically in this test; text pacing is a separate
  presentation check.
- The resulting flag banks, party, money, roster, settings and inventory
  match the retail starting state; directional input then moves Chaz.
- Saving and continuing that state preserves the snapshot and position,
  including when battle data is enabled again.

`PSIV_DEBUG_STATE=<path.json>` records the Godot-owned runtime at
`PSIV_DEBUG_SHOT_FRAME` (default 180). It includes map, cell, scene-active
state, party, money and flags. This makes a native launch inspectable
without inferring state from a screenshot or a separate fixture.

## Evidence

Local receipts for this audit are under `/tmp/psiv-audit-20260912/`; this
directory is temporary and contains derived game data, so it is not tracked.

- `boot-before.txt`: regression fails with 0 meseta, expected 500.
- `boot-after.txt`: the same constructor regression passes after repair.
- `opening-before.txt`: missing Chaz dialogue before movement.
- `opening-after.txt`: uninterrupted opening regression passes.
- `retail-newgame.csv`: fresh power-on oracle tape 01; frame 6456 has
  map `$13`, character pixels `(768,288)`, Chaz alone, 500 meseta,
  and town flags `80008040`.
- `opening.png`: inspected pre-repair native field after the opening.
- `rust-after.txt`: 743 Rust tests passed; `python-tests.txt`: 918 Python
  tests passed. `clippy.txt`: all workspace targets pass with warnings denied.
  `runtime-final.txt` repeats the affected runtime tests after final ordering
  cleanup.
- `native-after.log`, `native-state.json`, `native-after.png`: rendered
  Godot execution at tick 3360. START played the opening, automatically
  dispatched trigger 124 / Chaz's `$6D` line, and accepted an eight-frame Up
  input. State is map `$13`, cell `(48,18)`, Chaz alone, no active scene,
  500 meseta, and the exact expected event/town banks. The screenshot was
  inspected. This uses auto-acknowledged dialogue and the dummy audio driver;
  it does not certify dialogue pacing, sound or hardware controller input.

The saved/native control tests establish this opening slice, not completion
of the game. Missing battle mechanics remain the largest known obstacle.

Reproduce the rendered opening smoke test from the repository root (requires
the local pack, Godot, and Xvfb):

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
  "$HOME/.local/bin/psiv-godot-4.7.1" --path godot \
  --script "$PWD/tools/native/native_opening.gd" --display-driver x11 \
  --rendering-method gl_compatibility --audio-driver Dummy \
  --fixed-fps 60 --quit-after 3370 \
  --log-file "$PWD/build/native-opening/godot.log"
```

Run with other `PSIV_DEBUG_*` selectors and `PSIV_LOAD_SLOT` unset. The
expected state is specified above; a process exiting successfully alone is
not a passing receipt. Headless opening/save coverage is
`cargo test -p psiv-runtime --test new_game` from `rust/`.

## Native combat continuation

Ordinary COMD previously submitted `attack_all()` immediately. It now opens
individual orders, with attack-target selection, TECH, SKILL, DEFEND, and backtracking
through targets, technique lists, and previous characters. Up/down or left/right
move the cursor; accept selects; cancel goes back. Unavailable or unaffordable
techniques are marked with a leading dash and cannot be submitted. Technique
lists use retail's newest-learned-first order and page after four rows.

The engine reads normalized technique definitions from `battle/abilities.json`.
Supported effects are elemental damage, healing, attack/agility reductions,
and attack/defense/mental-defense/agility boosts. This enables 27 retail
techniques, including the early party's RES, FOI, SHIFT, SANER and GELUN.
All 40 records' effect, TP, range, power, resistance and element bytes were
checked directly against the verified US ROM on 2026-09-12.

Damage and healing use the existing retail integer formulas and sixteen
draws per target. Damaging techniques do not take physical hit/critical rolls.
Support changes use the caster's mental stat and replace the previous buff;
they are cleared on battle exit. TP is charged when the actor executes.
Invalid/unlearned/unimplemented/unaffordable selections do not consume TP or
RNG. Retail's late-seal exception is preserved: a technique sealed after
selection costs TP but produces no effect. A dead healing recipient is not
replaced by another party member. Human-only techniques exclude androids.

Validation: nine focused core tests, four menu tests, and a real-pack runtime
test that casts RES/FOI/GELUN, wins, returns to the field, saves and continues
with the same state. The workspace passes **757 Rust tests** and Clippy with
warnings denied. The previous 918-test Python extraction run remains the
extractor baseline; this continuation changes no Python extraction code.

The Godot smoke fixture loads an isolated native save with Chaz/Alys/Hahn
and Chaz at 20/25 HP, then starts real formation `$8A` in Academy Basement B3
(map `$17`). `tools/native/native_combat.gd` supplies actual menu input: RES on Chaz,
FOI on enemy slot 7, GELUN on the enemy group. This is a constructed combat
fixture, not a claim that the route from START was traversed. The test can
create its save without touching the player's normal saves:

```sh
mkdir -p build/native-combat/saves
PSIV_COMBAT_SMOKE_SAVE_DIR="$PWD/build/native-combat/saves" \
  cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --test combat_techniques
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 \
  PSIV_SAVE_DIR="$PWD/build/native-combat/saves" PSIV_LOAD_SLOT=1 \
  PSIV_DEBUG_BATTLE=0x8A PSIV_DEBUG_INPUT=1 \
  PSIV_COMBAT_SHOTS="$PWD/build/native-combat" \
  PSIV_DEBUG_STATE="$PWD/build/native-combat/state.json" \
  PSIV_DEBUG_SHOT="$PWD/build/native-combat/round.png" \
  PSIV_DEBUG_SHOT_FRAME=1000 \
  "$HOME/.local/bin/psiv-godot-4.7.1" --path godot \
  --script "$PWD/tools/native/native_combat.gd" --display-driver x11 \
  --rendering-method gl_compatibility --audio-driver Dummy \
  --fixed-fps 60 --quit-after 1010 \
  --log-file "$PWD/build/native-combat/godot.log"
```

Run with other debug selectors unset. Expected evidence: logged commands
name technique IDs 24, 1 and 20, with FOI targeting fighter 7; live TP becomes
7, 37 and 20 respectively; RES emits a positive healing event. Enemy turns
can subsequently reduce HP. Inspect `techniques.png`, `target.png`,
`round.png`, `state.json`, and the event log together. `$88` is the older
presentation-only oracle fixture and must not be used as combat proof.

The new menu uses the extracted font/window art but a compact native layout.
Spell effects currently show names, HP/TP updates and damage/support results;
retail spell animations and casting sounds are not implemented. The smoke
run uses software rendering and dummy audio, so it does not certify hardware
pacing, controller behavior, audio, or pixel parity. Existing status-strip
command icons also remain tied to the old idle presentation.
Forced exits still report the previously documented single
`AudioStreamGeneratorPlayback` teardown warning; the verbose probe confirmed
the same object described in `docs/sound/SOUND_INTEGRATION.md`.

## Opening skills and resistance repair

COMD now includes a SKILL page. EARTH targets one enemy and can put it to
sleep, VORTEX applies one weapon-based damage hit, and VISION adds 8 dexterity
to the living human party. The menu displays current and maximum uses, lists
newest learned first, supports cancellation and target backtracking, and
disables exhausted, unarmed or unimplemented selections. Skills spend one
use in the matching learned slot when the actor executes; they spend no TP
and work while techniques are sealed. Victory, field return, save/continue,
and re-entering combat preserve the remaining uses.

Only these **3 of 54 ordinary skills** are enabled. Several other skills
reuse a familiar effect byte but perform additional gameplay inside their
animation routine; matching that byte alone would silently get them wrong.
The runtime normalizes every skill's own JSON fields and fails closed for
unsupported dispatchers. The six gameplay bytes of all 54 records were checked
against the verified US ROM; see `build/native-skills/rom-records.json`.

Earth short-circuits without a roll on an already sleeping or paralyzed
target, while still spending one use. On a successful roll it sets sleep
without changing agility. At round end, each occupied sleeping fighter gets
one wake-up roll in fighter-slot order: odd clears both sleep flags and
restores modified agility. Enemy paralysis then clears without a roll.
Vision replaces its previous buff and clears on battle exit. The native
implementation fixes the cartridge's dependence on Hahn's first encoded
name byte while retaining his normal +8 effect. The controlled retail
comparisons and exact source routines are recorded in `docs/source-notes/battle-party.md`,
including an oracle run showing a renamed Hahn produces +26 in retail.

The real-pack skill test exposed a pre-existing resistance mapping error:
`enemies.properties` is an alphabetical census, but the bridge treated it as
cartridge element order for enemies and newly initialized party members.
Earth consequently checked mechanical immunity instead of psychic resistance.
The loader now uses the canonical cartridge element names to place each value
and validates the complete name set. A regression checks all 153 enemies and
all eleven initialized characters, including equipment-derived resistances.
The JSON format and extractor are unchanged.

Native development saves created before this repair can retain incorrectly
ordered character resistances: their stored stats are intentionally preserved
on load, and the retail slot format does not identify which native version
wrote them. The smoke fixtures below are regenerated from corrected initial
records. This change does not silently rewrite existing player saves.

Evidence is under `build/native-skills/` (ignored, local derived data).
Ten core skill tests cover draw counts, damage, status timing, turn skipping,
weapon requirements, target filtering, resource rejection and rewards. Three
new menu tests cover skill selection, disabled entries and cancellation. The
real-pack integration test spends all three skills, wins, returns to the
field, saves, continues and re-enters combat with identical remaining uses.
The resistance bridge regression checks the complete loaded roster.

Reproduce the native Godot smoke test from the repository root:

```sh
mkdir -p build/native-skills/saves
PSIV_SKILL_SMOKE_SAVE_DIR="$PWD/build/native-skills/saves" \
  cargo test --manifest-path rust/Cargo.toml -p psiv-runtime --test combat_skills
cargo build --manifest-path rust/Cargo.toml -p psiv-godot
xvfb-run -a env LIBGL_ALWAYS_SOFTWARE=1 \
  PSIV_SAVE_DIR="$PWD/build/native-skills/saves" PSIV_LOAD_SLOT=1 \
  PSIV_DEBUG_BATTLE=0x8A PSIV_DEBUG_INPUT=1 \
  PSIV_SKILL_SHOTS="$PWD/build/native-skills" \
  PSIV_DEBUG_STATE="$PWD/build/native-skills/state.json" \
  PSIV_DEBUG_SHOT="$PWD/build/native-skills/remaining.png" \
  PSIV_DEBUG_SHOT_FRAME=1100 \
  "$HOME/.local/bin/psiv-godot-4.7.1" --path godot \
  --script "$PWD/tools/native/native_skills.gd" --display-driver x11 \
  --rendering-method gl_compatibility --audio-driver Dummy \
  --fixed-fps 60 --quit-after 1110 \
  --log-file "$PWD/build/native-skills/godot.log"
```

The constructed fixture starts formation `$8A` in map `$17` with Chaz/Alys/Hahn.
Actual Godot input selects EARTH on enemy 6, VORTEX on enemy 7, and VISION on
the party, then reopens Chaz's skill list after the round. Expected logged
skill IDs are 31, 6, 47; remaining uses become 2, 4, 4, TP remains 10, 40, 25,
and dexterity becomes 13, 21, 13. Earth can miss according to its roll; the
core and real-pack integration tests separately exercise its successful path.
Inspect `earth.png`, `target.png`, `vortex.png`, `vision.png`, `remaining.png`,
`state.json` and the event log together. No second round should be submitted.
The uses display spells out `OF` because the extracted menu font has no slash.
Skill narration uses a larger existing window when a line exceeds 16 glyphs.

This is native battle/menu proof on a constructed save, with the same software
rendering, dummy audio and known forced-exit audio warning as the technique
smoke. Skill animation art and casting sounds are still pending. It does not
establish a naturally traversed opening route or a full campaign.

## Battle items

COMD's ITEM page now activates concrete equipment or inventory copies. It
lists equipment first, preserves raw inventory slots across holes, and marks
a shared copy unavailable after an earlier character reserves it. Cancelling
and replacing an order releases that reservation. Consumption happens when
the fighter acts, directly in the runtime's inventory, so escape, defeat,
victory and saving cannot accidentally restore a spent item.

All 160 item definitions are normalized separately from equipment bonuses.
The 26 usable battle-item records cover single/group healing, poison and
paralysis cures, human/android revival, elemental damage, sleep, self buffs,
and Psyco-Wand's removal of enemy stat/resistance boosts. Equipment activation
is reusable; disposable type-8 items, including Repair-Kit, are consumed.
Unsupported/no-effect items such as Telepipe stay disabled in battle.
Healing uses the item's literal power and sixteen retail random draws,
including when the recipient already has full HP. Moon-Dew restores a dead
human to one-quarter maximum HP and preserves technique seal. Sol-Dew heals
living targets without clearing their ailments; on dead humans it also
removes the death/ailment flags except seal. Cure effects reset AGI/DEX to
modified values, as retail does. TP and skill uses are unaffected.

Validation: twelve focused core tests, three new menu tests, a real-pack
win/save/continue test, **788 passing workspace tests**, and Clippy with
warnings denied. All 160 records' eight activation bytes and disposal type
were checked against the verified US ROM. A fresh controlled retail command
probe revived Hahn from 0/21 HP with status `$1D` to 5 HP and `$90`: seal plus
the cartridge's transient sprite flag. The native engine expresses the
transient part as a revival event instead of persisting it in saves.

`build/native-items/receipt.json` records the native smoke and artifact
hashes. `tools/native/native_items.gd` selects Chaz's Dynamite on enemy 7, Alys's
Monomate on Hahn, and Hahn's Antidote on himself. The inspected screens show
all five actions, reserved copies, and the inventory after consumption.
Hahn ends at 21 HP without poison, enemy 7 is dead, TP remains 10/40/25,
and only the unused Moon-Dew and Repair-Kit remain in inventory slots 3/4.
Generate the isolated save with `PSIV_ITEM_SMOKE_SAVE_DIR` and the
`combat_items` runtime test. Run the Godot smoke like the skill recipe, with
`PSIV_ITEM_SHOTS`, `tools/native/native_items.gd`, shot frame 1320 and quit frame
1330. The usual software-rendering/dummy-audio and constructed-fixture
limitations apply; retail item animation and casting sounds remain pending.

## First-boss Fission

The Academy boss now begins alone. Its formation stores the two Xanafalgues'
stats from the start, but retail's enemy initializer removes their objects.
The native roster now distinguishes a dormant fighter from a living active
one: it remains available for Fission without being drawn, targeted, queued,
or counted for victory. Formation position bit 7 selects a palette line;
it is not the dormant flag.

Igglanova/Guilgenova check their immediate formation neighbors after the
ordinary ability roll. One empty neighbor takes no additional random draw;
two take one parity draw to choose a side. Fission consumes the turn and
refills the chosen slot from that slot's cached enemy record. This matters
for Guilgenova, whose neighbors use Gicefalgue rather than the Fission2
record's nominal target byte. Killed creatures can be replaced repeatedly,
and each kill contributes its own reward. Revived fighters cannot execute
an old queued turn later in the same round, matching retail's transient
status-bit-7 suppression without putting that bit in saves.

Seven core regressions and a real-pack boss win/story/save/continue test
pass. All 112 enemy skill records were checked against the verified ROM;
only the two Fission definitions are enabled in this slice. The full
workspace passes **796 Rust tests** and Clippy with warnings denied.

`build/native-fission/receipt.json` records actual Godot input on an isolated
save: two all-defend rounds grow the neighbors; Alys then uses Dynamite on
the left creature and the boss replaces it. The inspected native captures
show the initial solitary boss, its first creature, and both full-health
creatures after replacement. `tools/native/native_fission.gd` drives the commands;
the `combat_fission` test creates its save with `PSIV_FISSION_SMOKE_SAVE_DIR`.
Run it with `PSIV_DEBUG_EVENT=0x6B`, `PSIV_DEBUG_AUTOCLOSE_SCENE=1`,
`PSIV_FISSION_SHOTS`, screenshot frame 2400 and quit frame 2410.

`oracle-receipt.json` beside it records a fresh input-only retail replay
using the Academy prefix of natural tape 31. Inspected cartridge captures
at frames 40650, 41200 and 41850 show the same solitary/right/both sequence;
the RAM trace also records a killed neighbor returning to 16 HP. This is
mechanics and visibility proof, not matched-frame animation parity. The
native fixture still uses software rendering and dummy audio; Fission art,
casting sound and the complete multi-group enemy-name layout remain pending.

## Dialogue, choices, and recovery

Scene text now returns at `$F7` so actor movement runs between dialogue
chunks. The window retains the entry cursor and its original tree binding;
`SceneDialogueResume` reopens that cursor, using updated event flags. An
entry-ending `$FF` retains the following entry as its continuation. Ordinary
page waits remain inside the current dialogue call. The post-Igglanova
regression requires exactly three chunks (2, 4 and 17 pages), with no missing
or repeated lines; the runtime waits for both resumed chunks before setting
the completion flag. Headless scene drivers now explicitly acknowledge
resumes instead of relying on the production runtime to skip them.

The `$F5` yes/no window now waits for actual input. Up/down changes the
selection, accept answers, and Cancel answers NO. Operand zero continues
after the choice inside the same entry; a positive operand counts following
`$FF` delimiters. A taken branch preserves the current portrait and window.
The scene receives the answer independently of its dialogue-close signal,
so choosing NO at Chaz's house cannot accidentally take the rest path.
Even the debug retail-paced text driver waits at choices. A scene with no
recorded answer waits for one instead of manufacturing YES.

The real-pack recovery tests exposed missing skill refills in both story
rests and paid inns. Both now call the same core `RecoverStats` operation:
current-party HP/TP/skill uses refill, ailments clear, and all three saved
vehicle skill-use banks refill. Off-party characters and saved vehicle HP
remain unchanged, as in the retail routine. Both house answers set its
temporary visit flag; only YES performs recovery.

`tools/native/native_dialogue.gd` drives the constructed post-boss fixture with
normal text timing and automatic dismissal only after complete pages.
`tools/native/native_choices.gd` separately supplies YES, NO, or Cancel at Chaz's
house, starting with Chaz/Alys at 1 HP, 1 TP, and zero skill uses. Receipts
and inspected screenshots belong under `build/native-dialogue/` and
`build/native-choices/`. These are scene fixtures, not a continuous campaign.
The baseline static pagination comparison still covers all 2,736 entries;
branch tests separately cover zero/nonzero offsets and Chaz's house replies.

These native captures also exposed an unrelated save-load presentation bug:
boot always loaded Chaz's field sheet, even when the save put Alys in front.
Map/title/save initialization now selects the actual party leader's sheet.
The leader also wins equal-foot-position draw ties, and unused follower
nodes hide when a scene shrinks the party. With a 1280x800 Xvfb screen,
external X11 captures match viewport pixels exactly for the choice prompt,
NO selection, and response. Earlier 640x480 virtual screens were undersized
for the 1280x800 Godot window; use an explicit screen size in capture runs.
The dialogue/choice/recovery baseline passed 806 workspace tests.

## Connected Academy route and battle winnings

`rust/psiv-runtime/examples/academy_route.rs` starts one new game, walks
the actual map collision and doorways, resolves NPC preambles, and fights
encounters through normal command selection. It reaches Alys, the principal,
Hahn, both basement floors, Igglanova, the resumed conversation, the
principal's payment, and the Motavia exit. It never relocates or changes
stats/flags. Its dialogue acknowledgements are headless and immediate;
it does not establish presentation timing or campaign parity.

The current deterministic run wins four battles and leaves with 986 meseta:
500 initial, 100 from Hahn, 86 from battles, and 300 from the principal.
It saves and reloads with identical persistent state and map/cell. Evidence:
`build/native-route/route.log` and `build/native-route/saves/slot_1.sram`.

That connected route exposed a missing reward handoff: Godot displayed
battle meseta but `finish_battle_absorbing` never credited the purse. Victory
now pays the actual battle pool once when results finish, capped at the
retail 9,999,999. An escape after a kill pays no partial pool. Three real-pack
regressions verify normal payout/save persistence, the cap, and that escape.

`tools/native/native_route.gd` exercises the corresponding Godot route with actual
button inputs and read-only collision/menu probes. The completed run at
`build/native-route/verified-return/route.json` starts at the real title,
traverses all 22 legs, wins six battles, completes the full conversations
at retail text pace, receives the principal's payment, exits Piata and uses
camp STATE/SAVE. It ends at Motavia `(46,143)` with 990 meseta and flags
`07/08/09/0A/0B/0C/0D/0E/0F/15`. No unsupported ability was emitted. The
boss-clear screenshot was inspected. This native run differs from the
headless route's encounter count because presentation consumes RNG frames.

A separate Godot process used the actual title CONTINUE and slot 1 controls,
restored that map/cell, money, leader and flags, walked one cell south and
saved into slot 2. Both save checksums validate. Every payload byte matches
except the Y coordinate's 16-pixel step, including all roster records,
ability uses, inventory, flags and settings. Receipts and an inspected
continued field image are in `build/native-route/continued/`. These runs
use Xvfb/software graphics and Dummy audio; they establish the Academy
route, not full-campaign or real-desktop performance/audio acceptance.

## Camp restorative rules

Field healing now implements `CalcHealingValue` (`$67FC0`): sixteen accepted
`UpdateRNGSeed` values masked to 0..7, retrying a value equal to the previous
one (initially zero), then the field routine's integer formula. This differs
from battle healing. Star Dew rolls before each slot check, including dead/android slots and
the first empty slot, which ends the loop; five occupied slots use five
calculations. Full HP
does not skip the random draws.

The verified ROM's `HealingEffectData` at `$67FA4` supplies field masks
`0F/0E/0D/00/00` for healing, antidote, paralysis cure, Moon Dew, and Sol Dew.
These are deliberately distinct from the battle masks. Field Moon Dew
restores a dead human by max HP / 4 and clears status; Sol Dew also works
on living humans; Repair Kit targets androids. Ordinary restoratives cannot
heal androids or cure a dead human. Recognized no-effect uses still consume
the item. The field's `ReorderInventory` shifts the first empty suffix once,
while battle item use retains its raw slot holes.

Two core formula tests and a real-pack camp regression cover accepted/retried
draws, first-empty-slot sequencing, healing amounts, status masks, revival, android
restrictions, no-effect consumption, and inventory shifting.

## Battle return reload

The continuous native Academy run exposed a stale room after Igglanova:
flag `$0B` advanced, but the boss sprite and its two invisible blocking
objects remained. Retail battle return selects `GameMode_LoadFieldMap`
with `Map_Load_Flags` bit 0: map data and `MapDataManager` reload, while
field-object initialization is skipped.

`Runtime::return_to_field` now applies that load before the Godot shell
reveals the room; headless callers receive it on the next field tick.
Existing NPC positions, offsets, facing, active state, party trail, camera,
and movement clocks survive. New story despawns, collision/layout patches
and dialogue overrides take effect. A regression proves all three boss
objects vanish; another proves ordinary combat retains moved objects and
camera placement. The workspace passed 812 tests at this checkpoint.
The fresh native screenshot `build/native-route/verified-return/leg-10.png`
was inspected and shows the cleared boss position with the party intact.

## Field TECH and SKILL recovery

The camp menus now browse actual learned slots, select a caster and target,
and submit confirmation to Rust. Ten field techniques implement the RES/SAR
families, ANTI, RIMPA, REVER and REGEN. All five field skill records implement
both RECOVER records, MEDICE, MIRACLE and MEDIC PW. RYUKA and HINAS remain
explicitly unavailable and spend nothing.

Field casting checks the retail death and paralysis bits; technique sealing
does not block this field routine. Confirmation charges TP or one skill use
once. Browsing/cancelling and invalid commands do not charge. Full or
ineligible targets still consume a confirmed use and the appropriate RNG.
Groups calculate through their first empty party slot. RECOVER uses the
caster's strength and repairs its android caster; MEDICE/MIRACLE heal either
kind of living target. MEDIC PW uses strength to heal/revive humans and
excludes androids. The field routines inspect death bit 2 separately from
android shutdown bit 6 and clear the documented recovery masks.

The 17 field-usable ability records were checked against the local retail
ROM bytes (`build/native-camp-abilities/retail-records.json`). Three real-pack
regressions cover learned order, resources, caster/target eligibility,
revival, self targeting, group RNG sequencing and modified power stats.
An isolated native camp fixture exercises target cancel, RES, RECOVER and
saving through actual Godot input. Its first receipt proved unchanged HP/TP
on cancel, RES charging 3 TP and healing Chaz, and RECOVER charging one use
and repairing Demi. Visual inspection caught an unsupported plus-sign glyph
in the result message; the final rerun uses supported text, and both result
images were inspected. `build/native-camp-abilities/verified/receipt.json`
records the completed flow and the two retail healing SFX dispatches
`BD/CD`. Audible output and exact healing presentation timing remain open.
These fixtures do not prove campaign traversal or exact retail camp layout.

## Connected native route through Holt

The Academy save from the preceding verified native START run now continues
through the actual title menu, Mile, Zema, both Birth Valley floors, the
petrified Holt interaction, and back outside Zema. No scene injection,
relocation, edited stats, free healing, or direct combat API calls occur in
this Godot route. The driver uses read-only observations and normal input;
scene dialogue still uses the documented retail-paced automatic dismissal.

`tools/native/native_motavia.gd` shares the Academy driver's movement/menu helpers.
It healed twice outside Piata and twice after a dungeon fight, using the
camp TECH caster/target menus. Four random battles completed. Holt's actual
dialogue interaction set flag `$10`, returned the party to Zema, and paid
500 meseta. The final ordinary camp save held 1,564 meseta at Motavia cell
`(99,83)` after 9,495 native ticks. Alys/Chaz/Hahn remained alive at
40/34/30 HP and 40/1/22 TP. The in-run checkpoints and copied harnesses are
under `build/native-motavia/route/`, alongside hashes of the loaded extension
and original Academy save. The inspected Zema return shows the still
petrified townspeople and the party outside Birth Valley.

A fresh Godot process then selected title CONTINUE, loaded that save, moved
one cell down, and saved slot 2 through camp. All save payload bytes remain
identical except the Y position byte at `$309`, advancing by 16 pixels.
`build/native-motavia/continued/receipt.json` and `save-comparison.json`
record this restart and second save. These runs use software rendering and
dummy audio; desktop/controller acceptance and audible output remain open.

## Acid Breath, attack ailments, and target/death corrections

The first headless connected scout stopped when FlattrPlnt selected enemy
ability 51. Its exact ROM record is `01 01 08 18 06 01 00 00` at `$2834FC`.
The enemy's dispatcher retains one party target. Its main Acid Breath
object requests one damage reaction; the child animates without adding
another hit. The engine now uses modified battle strength, defense and
physical resistance, with 16 damage draws and no physical hit roll or
poison follow-up. Its two cartridge sound cues are attached to ability and
damage events; exact object animation and 10/36-frame cue timing remain
unimplemented. The record and ROM hash are in
`build/native-motavia/retail-record.json`.

The trace also exposed three broader errors. `loc_5ACE` reselects an enemy's
dead or freshly revived target using a new weighted draw before the ability
roll. `Character_Dead` preserves sealing, clears other ailments, and sets
android shutdown instead of human death. Finally, all 20 enemies with a
nonzero plain-attack status use poison or paralysis. These effects now roll
after damage, comparing strength against strength through poison resistance
with the retail `$70` miss threshold. Misses, dead recipients and an already
present ailment skip this extra roll; immunity still consumes it. Paralysis
clears the first sleep bit and reduces battle agility/dexterity to one.

Six new core regressions pin these boundaries, including exact draw order,
defending, one-hit behavior, status thresholds and android death. The real
pack regression `combat_enemy_attacks` verifies Acid Breath consumes its
turn, has its own sound cues, can be defeated, and survives save/reload.
The workspace passed 821 tests, then the added real-pack regression passed
separately; workspace/all-target Clippy passed with warnings denied.
The connected Godot route killed its FlattrPlnt before Acid Breath executed,
so it does not count as a native Acid Breath execution check.

The separate native fixture `tools/native/native_enemy_attacks.gd` uses retail
formation `$96` and a labelled party durability fixture. It selects DEFEND
through COMD, observes FlattrPlnt use Acid Breath in its first round, and
then wins through the attack menu. Chaz drops from 400 to 385 HP from one
hit; the other two party members remain at 400 HP. Both `$D5/$D8` sound
requests dispatch, and victory returns to the field with 30 meseta.
The ability-name and post-damage screens were inspected. Its final receipt,
copied driver and source hashes are under `build/native-motavia/acid/`.
That confirms native execution/presentation of the implemented effect;
full retail Acid Breath animation and exact cue timing remain pending.

## Map entry, live rock removal and scene-to-field positions

The connected Tonoe scout exposed three independent defects. Map changes
and loaded saves did not check their entry triggers before input. Rune's
Flaeli script omitted the four live BG chunk writes that remove the rock.
Finally, scripted party positions were discarded when a scene ended.

Map construction, doorway entry and battle return now arm the trigger check;
a map load also restores the retail ten-step encounter countdown (nine free
steps, a roll on the tenth). `RestoreMapChunks` is a distinct scene operation:
it validates the expected base chunk ids, retains unrelated patches and live
objects, and refreshes visuals/collision before the scene sets flag `$13`.
Character names and party slots resolve to the same scripted object, and the
field party takes the final scripted positions and in-progress cell steps.

The Rune regression covers the entry trigger, suspended save load, encounter
grace, rock collision before/after the flag, Rune's separate casting position
and the final leader cell. The broader scene fixtures previously invoked
Flaeli after Rune had left; their order and map placement are now corrected.
They remain direct scene tests, not campaign playthrough evidence.

Godot now has explicit temporary-object placement, character visibility and
per-object animation age for this scene. The extracted fire sprite uses the
scene's actual palette. Debris motion and oracle frame comparison remain
unfinished. Native captures of the casting pose, flame and opened rock are
in `build/native-tonoe/route`; no frame parity claim is made.

## Current implementation priorities

See [ROADMAP.md](../ROADMAP.md). The earlier Tonoe/ORDER work list has been
superseded by the verified BioPlant checkpoint and later entries below.

## Native Tonoe attempt and field-status repair (2026-09-13)

`build/native-tonoe/dorin-incomplete` preserves the connected Godot run from
Holt's native save, through a paid Mile rest, Rune's recruitment, the mountain
pass and maze, to Dorin at Tonoe. The party remained alive; the purse reached
2,125 meseta. Inputs used ordinary field, inn, camp and combat menus. Dorin's
three yes/no choices were supplied with ordinary X11 keys and recorded in
`manual-choice-inputs.jsonl` (NO, NO, YES). The driver had no choice handler.

The inspected captures show the Mile innkeeper portrait, Rune's casting pose,
flame and live opening in the rock. This is not a Tonoe completion receipt:
That binary dropped MeetingDorin's fourth resumed dialogue section and moved
NPC 0 where the original moves Alys. The repair distinguishes finished text
from an animation pause, moves Alys by identity and resumes all four sections.
Two focused runtime tests pass. The driver now answers the actual choices and
allows the pending cutscene to start. The fresh connected run completed.
The punch/startle visual staging remains outstanding.

`FieldStatusClock` now transcribes `DoCharStatsUpdate`: poison loses one HP
on every fourth eligible foot step on poison-enabled maps; paralysis has no
cure roll for five steps, a 1/8 roll on steps 6-24, then guaranteed recovery.
The byte counter wraps and is shared across the party. Demi and Wren gain one
HP per step without clearing shutdown. Map load and battle return reset both
counters. Transition cells, vehicles, idle frames, battles and open windows
must not apply a foot-step update. Poison death clears poison/paralysis,
sets the human Dead bit, and queues the original field messages. The runtime
parks until they are acknowledged and preserves deferred warp/event checks.

The unconditional one-HP revival and Igglanova retry have been removed.
Ordinary defeat preserves the defeated roster until a new runtime is created
by START or CONTINUE; it does not write any saved slot. Native isolated
fixtures live under `build/native-field-status`, separate from campaign proof.
The first render pass caught incorrect name decoding and narrow red-flash
edge gaps; both were repaired before the next verification pass.

The first full workspace check after this change passed 835 tests and failed
one: `the_retail_chain_runs_from_title_to_ending`. Its synthetic route ignored
a defeat in the guild sequence and previously depended on automatic revival.
That test relocates, directly starts scenes, and later hardens battle stats;
it is not evidence that an ordinary player can finish the campaign. Its later
first-Zio battle assumption also requires checking against the original's
scripted exit. Do not restore the blanket revival to make this test green.


The corrected native field-status run is `build/native-field-status/poison/receipt.json`.
It walks eight ordinary steps from an isolated save, shows Hahn's fall, renders
a full red flash (one uniform RGB 255,0,0 frame), then Chaz's fall and the
perished message. CONTINUE restores the saved 2/1 HP poisoned party.
`build/native-field-status/battle/receipt.json` defends with a one-HP Chaz
against formation `$96`, renders the final defeat window, returns to the title,
and restores the one-HP save through CONTINUE. Both save hashes are unchanged.
The failed initial render and early-message capture remain separate artifacts.

## First Zio encounter repair (2026-09-13)

The ROM's boss formation 4 contains enemy 152 (`Zio3`), not enemy 139 (`Zio`).
`EnemyAttack_Zio3` (`$D218` onward) overrides its random ability through the
shared `$EE98` counter: magic barrier (object `$908`, ability `$6B`), Nightmare
part 1 (`$90C`, ability zero), an empty turn, Nightmare part 2 (`$910`, `$53`),
and Black Wave (`$914`, `$70`). Black Wave selects Alys by character identity
with the first party slot as fallback. `loc_33DEC` returns directly to field
mode 8 and restores the saved camera/music; it never invokes damage or the
ability record's out-of-range effect `$2C`.

The engine now emits these five stages and a distinct `ScriptedExit`. It stops
the remaining queue and skips round-end recovery on that exit; no defeat,
revival, reward or game-over path runs. A real-pack runtime test verifies the
five-stage sequence, Alys in a nondefault slot, unchanged HP/TP/status, no
physical fallback and the scene's return edge. The Godot narration recognizes
the stages, but the barrier/Nightmare/Black Wave visual objects and exact
presentation timing are still outstanding. This is not yet native visual proof
of the first Zio encounter.


Psycho Wand's object now implements its additional Zio behavior: if the first
enemy formation entry is enemy 139, it becomes enemy 140 and `loc_7F22`
reloads the formation's enemy stats. The ordinary buff reset still applies in
other battles. The wand remains reusable, costs no random draws, and does not
turn a cleared enemy object back on or suppress Zio's queued turn. Repeat use
after conversion does not refill his HP. Tests cover the first-slot condition
and stat reload; the native effect animation remains outstanding.

The repaired `synthetic_scene_chain_reaches_ending_with_explicit_battle_fixtures`
now passes with explicit combat-stat fixtures, on-foot tower entry, the chest's
Psycho Wand supplied, an ordinary ITEM command to use it, and asserted battle
victories. Its first Zio encounter uses `ScriptedExit`. Those changes replace
previous ignored defeats and do not restore automatic revival.


## Connected native Tonoe completion (2026-09-13)

`build/native-tonoe/route/route.json` records the complete native continuation
from Holt's save: paid Mile rest, Rune, the rock and mountain routes, Dorin's
three choices and four resumed sections, Gryz replacing Rune, and ordinary
SAVE. Seven battles completed with everyone alive. The ending party is
Alys/Chaz/Hahn/Gryz at map `$43`, cell `(30,31)`, with 2,125 meseta. Slot 1
SHA-256 is `9e27b823e34058ff8cd7d8547cf914f88ea3386b9e3395661accb9f18d516aa7`.

`build/native-tonoe/continued/receipt.json` starts a fresh native process,
selects the title's CONTINUE, verifies map/party/money/flags, walks Down and
saves into slot 2. Slot 1 remains unchanged. Comparing the logical payloads
shows only the leader Y word changing from `$01F0` to `$0200`; physical header
differences encode the different slot/checksum. Captures of the completed
scene and the continued party were inspected. This proves connected native
progression and restart, not the fidelity of Dorin's temporary Rune, camera,
punch or startle animations (see `scenes/16_Dorin.md`).

The run receipt records the source and binary hashes used at launch. It
predates the new first-Zio, Psycho Wand and title-node disposal changes; those
changes require their own rebuilt native verification.


## Native field travel (2026-09-13)

HINAS/RYUKA now consume extracted JSON tables through Rust and ordinary Godot
TECH input. Town visits register on entry; previous-map words follow warps
and scene loads; the two Valley Maze entrances retain distinct HINAS exits.
Native save metadata preserves inherited exits through a process restart.
`docs/field/TRAVEL.md` documents the original sources, costs, native save extension,
seven new Rust checks, and the remaining world-travel/presentation limits.

The isolated native receipt in `build/native-travel/route` proves HINAS,
RYUKA cancellation and MILE selection, correct TP charges and map arrivals,
and SAVE. The fresh title CONTINUE receipt in `build/native-travel/continued`
restores slot 2, walks south and saves slot 3 with only leader Y changed in
the payload. Menus and arrival/movement captures were inspected. The original
Tonoe campaign save is unchanged. Full workspace: 849 tests passed; strict
Clippy and eight Python pack tests passed.

## Chest input and inventory visibility

See `../field/CHESTS.md` for the newly connected chest path and its four native
fixtures plus title CONTINUE verification. The connected Tonoe campaign
source remains untouched. `build/native-alshline` now completes ordinary-input
collection, conversation, return to Tonoe, SAVE and fresh-title Continue.

## Connected Alshline progression

The earlier native attempts resumed the Tonoe save, paid the 60-meseta inn,
and opened Gryz's storage door through ordinary dialogue/input. The first
physical-only attempt lost Chaz to the basement's BLOB formation. The next
attempt used Alys's learned FOI, won that fight, then wiped against four
TOADSTOOLs on B1. Investigation then found a runtime defect: level-ups had
discarded all learned techniques, learned skills and skill-use growth. The
saved level-four Chaz was missing TSU; Hahn was missing his level-three WAT.
`PROGRESSION.md` records the fix and explicit three-byte repair of a copy of
that campaign save. The original is preserved. Subsequent attempts
used the repaired copy and normal commands, including support techniques.
With the repaired save the party wins the first basement battle, heals
paralyzed Alys through normal RES, walks onward and wins the next B1 fight.
Alys and Hahn are dead after that second victory, so the unprepared route
stops. The receipt is retained under `build/native-alshline-under-equipped`.
That attempt was not a campaign collection/return proof. Earlier failed artifacts are retained under
`build/native-alshline-physical-attacks` and `build/native-alshline-spells`.

## Instant-death player commands

CRASH, BROSE, VOL and SAVOL now execute their original success/resistance
checks, consume the original resource, and grant normal enemy rewards on a
successful kill. See `../battle/INSTANT_DEATH.md` for source anchors, regression results
and the still-missing special animation work. This brings ordinary battle
techniques to 30 of 40 and implemented character battle skills to four of 54.

## Battle recovery techniques

ANTI, RIMPA, REVER and REGEN now work during battle as well as in the field.
Their isolated native command, victory, save and fresh-title Continue proof
is documented in `../battle/BATTLE_RECOVERY.md`. That brought the total to 34 of the 38
battle-usable techniques, plus both field-only travel techniques. Earlier
counts used all 40 techniques as their denominator.


## Party STATUS and RIMIT follow-through

`PARTY_STATUS.md` records the five-seat battle status repair, three-digit
numeric field corrections, eleven original STATUS portraits, original age
words and profession-name decoding. The native five-person battle, all five
STATUS selections and SAVE are verified; the earlier display-only fixture
also passed fresh title Continue with only the expected movement byte change.

`../battle/RIMIT.md` records effect 7, core/runtime proof, native casting/sleep/victory/
SAVE and fresh title Continue/movement/SAVE validation. Its FEEVE note corrects an apparent data interpretation: the US
routine writes a saved water-resistance byte, not live psychic resistance.


## Crawler THREAD correction

The next retreat attempt reached the final Tonoe basement floor and exposed
unsupported THREAD being replaced by physical attacks. Two party members
died; `build/native-alshline-thread-fallback` preserves that failed run.
`../battle/THREAD.md` records the original object dispatch and the correction: one
physical-resistance agility check, no HP damage or extra attack. Core and
real-formation runtime tests pass; the corrected campaign retry now completes.

The preceding `build/native-alshline-stair-avoidance` failure was confined to
the test player: it refused to cross another cell of its arrival staircase.
The game already suppresses a repeat warp between adjacent map-change tiles;
the path planner now allows leaving the trigger rectangle it starts inside.


The development launcher now checks for a running game or mapped extension
before building. A per-user launch lock prevents concurrent launch/build
attempts. `build/native-rimit/launcher-guard.json` records a real test with
two native Godot processes loading the extension: the launcher refused the
rebuild and the library hash and modification time stayed unchanged. On this
host `fuser` missed the mappings, so the guard reads `/proc/*/maps` directly.


## Connected Alshline completion

`build/native-alshline/route/route.json` completes 16 route legs and six
ordinary random encounters, all handled through RUN. The repaired Tonoe
source is copied unchanged; the 60-meseta inn restores the party, Gryz opens
the storage door, normal movement reaches the final basement chest, and
Alshline enters inventory as item 141. Its conversation completes and sets
flag 0x32. Crawler THREAD executes without the former physical fallback;
three ordinary RES casts keep the four party members alive.

At SAVE (tick 10828), the party is in Tonoe map 0x41 at (31,11), has 2065
meseta, Alshline, and Alys/Chaz/Hahn/Gryz HP 52/37/28/51 and TP 40/10/39/20.
Levels remain 7/4/4/6. The run produced no unsupported-ability diagnostic.
The opened chest, THREAD narration and saved-state captures were inspected.

The source for the next connected leg is
`build/native-alshline/saves/slot_1.sram`, SHA256
`2b0b4dd7cf2efa3b802b4dbdfd8b05d9f5aff4dcee8ac819a9624c0dc4ae154d`.
Fresh title Continue, one Down and SAVE to slot 2 pass in `continued/`: only
logical payload Y at 0x309 changes (176 to 192), with the source slot intact.
All original Tonoe and explicit progression-repair source bytes are preserved.
The library used for that Alshline proof was SHA256
`cfee377efe01420ab7fb55ffe22e3fa6cf670d719487d1cc2296f1347a900c64`;
882 workspace tests and strict Clippy pass. This proves the connected quest
through its return, not yet Zema's rescue or an end-to-end game playthrough.

## Connected Zema rescue and restart

`build/native-zema-return/route/route.json` continues the verified Alshline
save through a paid Tonoe rest, the mountain pass, all 50 rescue dialogue
pages, nine ordinary RUN escapes, and the Igglanova battle using normal
commands. Seven residents return, Alshline is consumed, flags 0x33 and 0x37
are set, and the party saves outside Zema at (99,83) with 3063 meseta.
Alys/Chaz/Hahn/Gryz have HP 53/39/33/66 and TP 31/1/31/20, levels 7/4/4/6.

That run used native library SHA256
`7c4e908f547de00e8a7612bd70a79b03d4e4679e07b4e5ff2df8e2e32d6b34ca`.
The saved slot 1 is SHA256
`75ce5fb3fb92660d9adfdb459059c516b00c0016fa078f5ed0428608d0916a40`.
Fresh title Continue, one Down, and SAVE 2 passed. The reusable validator
checks unchanged source bytes, valid slot/checksum headers and an otherwise
identical gameplay payload: logical byte 0x309 changes 48→64. Slot 2 is
`80dc8ef6f6fdc69985ee0500a3a02e19d52254d9ac5efd4f2a31a54baf594b47`.
Receipts and captures are in `continued/` beside the route.

The restored text, green Igglanova entrance, field dialogue without stale
comic panels, post-battle town and SAVE captures were inspected. The town
capture exposed a palette fault: residents were active but still rendered in
grey. The subsequent Zema equipment run below verifies consumption of their
flag-gated normal palette variants.
This is connected gameplay/restart proof, not a claim of visual or overall
original-game parity. See `../scenes/SCENE_DIALOGUE.md` and `BIOPLANT_NATIVE.md`.

## Connected Zema equipment and restart (2026-09-15)

`build/native-bioplant/outfit/route/route.json` continues the rescued-Zema
save, pays 80 meseta for a full inn recovery, buys seven items for 2470,
equips Alys with two slashers, Chaz with STEL-SWORD, Gryz with BROAD-AXE,
and Hahn with CRBN-SUIT, CRBNSHIELD and CIRCLET. The hand-selection cancel
check changes no inventory or equipment. The driver also checks all seven
residents' normal sprite sheets; their restored colours and the corrected
equipment window captures were inspected.

The saved party stands outside Zema at (99,83), with 513 meseta, full HP/TP
and the original levels. Slot 1 is SHA256
`6f257d112740505f92ad7163759da559e9d5d4f8bdb646ea1cbbb2155cf0871e`.
Fresh title Continue, one Down, and SAVE 2 pass in `outfit/continued/`:
only payload Y at 0x309 changes 48 to 64, with the source slot intact.
The original Zema rescue save is unchanged. This is the prepared source for
the connected BioPlant traversal; see `BIOPLANT_NATIVE.md`.

The prior driver's CIRCLET-on-Alys failure is retained under
`outfit/incompatible-circlet`; the game correctly enforced the cartridge's
usable-by mask. That test-player error did not justify weakening equipment
eligibility. `EQUIP_SCOUT.md` corrects the earlier mistaken claim that the
original game could not re-equip a left-hand weapon.

## Connected BioPlant elevator checkpoint (2026-09-15)

The equipped-Zema source advances through 64 ordinary training wins, a paid
inn, and two purchased CRBN-SUITs for Alys and Chaz. The earned party is
levels 7/6/6/7. The entrance, alarm and elevator scenes now execute through
normal interactions and movement, with live graphics and collision changes.
CROSSCUT, learned at level six, now executes its two independent damage hits.

`build/native-bioplant/campaign/route/saved-map-0A7.json` records the normal
SAVE at tick 3747, map `$A7`, cell `(68,66)`, 495 meseta and full HP/TP.
Its preserved SRAM copy has SHA256
`88f62eceab4d1b777e824f7dd7dc0577183a8bf8716de73b0583bfb2595d78b3`.
Fresh title CONTINUE, one Down and SAVE 2 pass in `continued-A7/`. Only
logical payload Y at `$309` changes from 32 to 48; all other gameplay bytes
and the source save remain intact, and slot/checksum headers validate.
The restored elevator and four-person party capture was inspected.

This checkpoint uses library SHA256
`651630d96cce6962da07976da67a7de7692a7688fa64c617c7eda4c17fcd2ebc`.
All 896 Rust workspace tests and strict Clippy pass. Detailed source
transcriptions, alarm pixel checks, failures and Rika fixture scope are in
`BIOPLANT_NATIVE.md`.

Connected Rika progression remains open. Both the onward run and a title
restart lose party members in Gicefalgue ambushes. The full-health `$A7` save
above is the current verified continuation source; the separate Rika fixture
does not replace that missing campaign proof.

### STATE/ORDER and the Gryz-leading checkpoint

STATE/ORDER now supports picks, undo, cancellation and automatic completion
with the last member. Runtime tests preserve character records and field
positions, verify the saved order and prove that enemy targeting uses it.
The full suite is now 898 passing Rust tests; strict Clippy passes.

`build/native-order/native-final/route/route.json` uses ordinary input to
make Gryz/Alys/Chaz/Hahn from the healthy `$A7` checkpoint and save it. The
cartridge emulator accepts that original native SRAM directly and produces
the same ordering. Four bounded ORDER-menu comparisons have RMSE 0 with no
masked pixels; broader camp/camera parity remains outside those checks.

The resulting source is `build/native-order/native-final/saves/slot_1.sram`,
SHA256 `f3e721e68d5af82fcbdc4f3b033f7d982f725b32902db119f6a682bf8a2819c5`.
Only the four party-order bytes differ from the old healthy source. Fresh
CONTINUE, one Down and SAVE 2 pass with no other gameplay changes. Source
details, receipts and limits are in `PARTY_ORDER.md` and `BIOPLANT_NATIVE.md`.

The onward trial in `build/native-order/campaign/` wins six encounters and
reaches `$A9`, but Hahn dies in the sixth fight and ordinary recovery needs
revival. That run is failed, unsaved evidence. The full-health ordered `$A7`
save remains the verified continuation source; Rika's connected join and
escape are still unfinished.

The follow-up bounded run in `build/native-order/recovery/` adds automatic
ANTI before field healing. One connected battle, two cures at exactly 2 TP
each, ordinary recovery, SAVE and fresh CONTINUE all pass. It preserves a
separate poison-free `$A7 (75,61)` save; full details and capture limits are
in `BIOPLANT_NATIVE.md`. The original full-health ordered source is untouched.
