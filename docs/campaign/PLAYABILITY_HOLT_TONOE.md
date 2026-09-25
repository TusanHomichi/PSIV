_Holt and Tonoe — part of the [native playability ledger](NATIVE_PLAYABILITY.md); see the [documentation index](../README.md)._

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
