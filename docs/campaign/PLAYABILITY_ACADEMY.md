_Academy — part of the [native playability ledger](NATIVE_PLAYABILITY.md); see the [documentation index](../README.md)._

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
