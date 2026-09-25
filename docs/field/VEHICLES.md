# Vehicles

Retail vehicle field, battle, palette and scene-entry record for the verified
US **Grand Cross** build, with `grand_cross=1` at
`reference/ps4disasm/ps4.options.asm:10`. This is the Wave 5 follow-up to the
Land Rover slice. The three selectors share the same field-object machinery,
but their collision masks and movement timing are not interchangeable.

## Selector and saved records

`Vehicle_Index` is the selector at `$FFFFF43C` (`ps4.asm:116755-116835`):

| Selector | Vehicle | Saved record | Field sheet |
|---:|---|---:|---|
| `0` | on foot | none | ordinary party |
| `1` | Land Rover | `VehicleRecord[0]` | `LandRover` |
| `2` | Ice Digger | `VehicleRecord[1]` | `IceDigger` |
| `3` | Hydrofoil | `VehicleRecord[2]` | `Hydrofoil` |

Selectors above `3` fail closed. The save layout is unchanged. The runtime
consumes the existing `Saved_Vehicle_Stats` records at `$FFFFFA80..$FFFFFB00`,
documented in [`SAVE_SCOUT.md`](../camp/SAVE_SCOUT.md): current/max HP at `$00/$02`,
the skill mask at `$04`, and eight current/max use pairs at `$06..$15`.
Mounted battle state writes current HP and current uses back to that same
record; no serialization path was added.

The retail static records at `ps4.asm:321152+` are:

| Vehicle | Static HP | STR | AGI | DEX | ATK | DEF | MDEF | Default mask | Uses |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Land Rover | `$02E4` | 64 | 60 | 70 | 200 | 80 | 50 | `$03` | `8/8` |
| Ice Digger | `$03C0` | 80 | 48 | 70 | 250 | 80 | 50 | `$50` | `8/4` |
| Hydrofoil | `$02A8` | 255 | 96 | 70 | 150 | 70 | 40 | `$0C` | `4/2` |

The battle member is built in `rust/psiv-core/src/vehicle.rs` from the saved
record's HP, mask and current/max uses. Retail's entry routine reloads static
vehicle HP in `FillBattleStats`; this project deliberately carries saved HP
through the battle because the existing save lane promises round-trippable
vehicle records. That is a project policy, not a claim about that retail read.

## Mount and dismount

The field menu fills `Vehicle_Boarding_Flags` from the low map nibble using
`VehicleBoardingFlags` (`ps4.asm:117131-117198`). The item actions then require
both `(Field_Map_Index & $FFF0) == 0` and the corresponding flag bit
(`ps4.asm:123419-123465`):

| Low map id | Allowed item mounts |
|---:|---|
| `$0` | Land Rover, Ice Digger, Hydrofoil |
| `$9` | Hydrofoil only |
| `$A`, `$B` | Land Rover, Ice Digger, Hydrofoil |
| all other low-nibble values | none |

Story events can set a selector directly. `Event_BoardingLandRover`,
`Event_BoardingIceDigger` and `Event_BoardingHydrofoil` are Event indices `9`,
`$A` and `$B` (`ps4.asm:120583-120590`, `144950-145128`). They wait for party
overlap, rebuild the ordinary party objects into the selected vehicle object,
stage the art, and write selector `1`, `2` or `3`. The Ice Digger is story-live:
`Cutscene_DarkForce1Defeated` removes the Canceller, adds Ice Digger and sets
flag `$89` (`docs/scenes/48_DarkForce1Defeated.md`). That is the Dezolis/snow
route; it is not a generic Land Rover recolour.

The runtime's `set_vehicle_index` anchors a selected machine at the leader's
current cell. Dismount is Event `$10`, `Event_GettingOffVehicle`
(`ps4.asm:145343-145440`): it samples the four-cell standing footprint at the
current anchor, accepts only an unsigned maximum of raw collision `0`, then
rebuilds the party objects and writes selector `0`. Any other raw value opens
the retail `Cannot get off!` window and leaves the selector mounted. The core
surfaces that as `VehicleDismountBlocked` and does not mutate the party.

## Field art and movement

Retail's field vehicle object uses the Nemesis art at `loc_51A7A`
(`ps4.asm:107728-107747`), the vehicle mapping tables at
`ps4.asm:047EF4/047F04/047F14`, and palette selector `$60` (`CRAM` line 3).
The pack emits the three decoded sheets and their mapping-defined sequences:

| Selector | Nemesis art | Decoded bytes | Frames | Mapping table |
|---:|---:|---:|---:|---:|
| `1` | `$296320` | 3,200 / 100 tiles | 8 × 40×32 | `$047EF4` |
| `2` | `$296A54` | 3,200 / 100 tiles | 8 × 40×32 | `$047F04` |
| `3` | `$2971A4` | 1,664 / 52 tiles | 4 × 40×32 | `$047F14` |

Land Rover and Ice Digger have two frames per direction; their left/right
entries mirror mapping indices `6/7`. Hydrofoil has one frame per facing and a
left mirror at index `3`. The mapping duration byte is `$04`, emitted as five
display ticks. The generated files are `sprites/vehicles.json` and
`sprites/vehicles/*.png` in the ignored runtime pack.

`VehicleMove` (`ps4.asm:93478-93599`) moves on a 32-pixel unit. Its table at
`loc_47C28` (`ps4.asm:94079+`) is:

| Table block | 8.8 constant | Frames / 32 px | Pixels / frame |
|---:|---:|---:|---:|
| `0` | `$0200` | 16 | 2 |
| `1` | `$0400` | 8 | 4 |
| `2` | `$0800` | 4 | 8 |

The retail default `FieldObj_Step_Offset` is `1` (`ps4.asm:87729, 87741`).
Land Rover and Ice Digger therefore move at 4 px/frame. Hydrofoil adds `$80`
to the table pointer before choosing the block (`ps4.asm:93478-93548`), so
the same selector moves at 8 px/frame. Hydrofoil selector `2` would read past
the retail table and is rejected by the clone rather than assigned an invented
speed.

The standing footprint is `(0,-1),(1,-1),(0,0),(1,0)` in 16-pixel party-cell
coordinates. Directional samples from `UpdateVehicleCollision`
(`ps4.asm:90562-90588, 90694-90810`) are:

```text
right: ( 2,-1), ( 3,-1), ( 2,0), ( 3,0)
left:  (-2,-1), (-1,-1), (-2,0), (-1,0)
down:  ( 0, 1), ( 1,1), ( 0,2), ( 1,2)
up:    ( 0,-3), ( 1,-3), ( 0,-2), ( 1,-2)
```

The selector-specific collision pointers at `loc_45C46`
(`ps4.asm:91065-91094`, tables `91222-91280`) admit these raw terrain values:

| Selector | Crossable raw collision values | Domain |
|---:|---|---|
| Land Rover (`1`) | `0`, `1`, `$A` | normal, map-change, sand |
| Ice Digger (`2`) | `0`, `1`, `$A`, `$B` | normal, map-change, sand, Dezolis snow/ice |
| Hydrofoil (`3`) | `0`, `1`, `9`, `$A` | normal, map-change, Motavia water, sand |

Everything else blocks that selector. The foot table treats raw `8/9/$A/$B`
as blocking, so the vehicle footprint and masks are real behavior differences,
not presentation metadata.

Map-change raw `1` is handled by the vehicle transition path and standing raw
`1/2` plus directional raw `1` provide the retail transition/encounter grace.
Vehicle random encounters use `UpdateRNGSeed & $7F` (1 in 128), while the foot
path uses `& $1F` (1 in 32). `FieldRoutine_VehicleControls`
(`ps4.asm:116805-116884`) runs collision, map transitions, scene events,
random battles, input and object updates in that order.

## Vehicle battle surface

`FieldRoutine_Battle` (`ps4.asm:117926-117949`) selects music `$96`
(`CyberneticCarnival`) while mounted and `$95` (`MeetThemHeadOn`) on foot.
The Godot path dispatches `$96` for mounted random/debug battles. Retail's
`loc_78EE` (`ps4.asm:11408-11414`) replaces the ordinary party with one
vehicle fighter whose id is `Vehicle_Index + $B` and sets `Obj_Fighters = 1`.
The runtime uses `Battle::start_vehicle`, keeps the saved member's current
uses in the battle copy, halves the vehicle reward surface, and copies current
HP/current uses back to the selected record on exit.

The plain Attack is the vehicle's **own** swing, not `Character_Attack`. The
vehicle menu's first option writes command 6 (`loc_684A`, `ps4.asm:9852-9878`),
and `loc_5B8E`'s seven-entry table (`ps4.asm:8410-8416`) sends command 6 to
`loc_AF9C` (`ps4.asm:16810`), so `Character_Attack`'s weapon check and its
`Character_AttackActionOffs` dispatch are never reached. `psiv-core`'s
`resolve_attack` routes a vehicle fighter to `battle/vehicle_attack.rs`'s
`resolve_vehicle_attack` before that check: one target (`loc_1152`), the hit
passes its attack object's wind-up decides (below), and the damage from
`loc_280A` — the **target's** energy property (`element_factor(2)`), with the
actor's hands never read.

How many `loc_B6A2` passes a swing draws is the vehicle's own, and the attack
object state 4 creates is what says so (`loc_B15C`, `ps4.asm:16964-16968`; the
object ids name routines in the table at `ps4.asm:74660-74662`):

| `Vehicle_Index` | record | attack object | `$1C` wind-up | hit passes |
|---:|---|---|---:|---:|
| 1 | Land Rover | `$644` `BattleObj_LandRoverAtk` (`ps4.asm:82940`) | `#$C` (`82945`) | 3 |
| 2 | Ice Digger | `$648` `BattleObj_IceDiggerAtk` (`ps4.asm:82991`) | `0` (`82996`) | 2 |
| 3 | Hydrofoil | `$64C` `BattleObj_HydrofoilAtk` (`ps4.asm:83066`) | `#$C` (`83071`) | 3 |

The object's first state hands the vehicle's `action_routine` back to 5
(`move.w #5, $32(a0)`) once its wind-up expires, and only a **nonzero** wind-up
adds a pass: a `#$C` wind-up counts down for thirteen frames and lands the write
after state 5 has already moved the vehicle on to 6, so state 5 runs again
(three passes), while a zero wind-up expires in the creation frame, before state
4's own `addq.w #1, $32(a4)` — the write changes nothing and state 5 runs once
(two passes). `hit_passes` is that rule, one entry per `VehicleData` record; the
last pass decides either way, because `loc_B6A2` blanks all nine hit flags
before each one. Both captures measure it: the Land Rover's swing has passes at
f25059, f25060 and f25072 landing 165, the Ice Digger's at f25058 and f25059
landing 212 from its own 250 attack byte
([`BATTLE_ORACLE_FORCED.md`](../oracle/BATTLE_ORACLE_FORCED.md) §5.1).

### Vehicle skill menu

Retail's main battle options are `ATTAC`, `OPTIN`, `RUN`
(`Battle_VehOpenMainOptions`, disassembly around `$0007B4`). Selecting `OPTIN`
opens `Battle_VehOpenSkills` (around `$001CEA`): a 14×6 window, two columns,
with the eight vehicle names from `VehicleAttackNames` and the current/max
use pair from the saved battle record. A zero-current entry remains visible but
is drawn with the disabled cursor word (`$C000`) and cannot be accepted; an
available entry uses the normal cursor word (`$E000`).

`rust/psiv-godot/src/battle/vehicle_ui.rs` and the battle screen now reproduce
that surface. The menu is mask-driven, keeps the current/max pair visible,
supports the two-column cursor, disables zero-use entries, and decrements the
screen copy on acceptance. `Command::VehicleSkill` validates the same mask and
use count in `psiv-core`; a valid command decrements the battle copy and emits
`BattleEvent::VehicleSkillUsed`, so the runtime save bridge persists the
decrement. Invalid commands emit `VehicleSkillRejected` and do not attack.

The actual 44-entry skill-effect dispatcher and vehicle effect animation are
now transcribed for every record present in the retail vehicle masks. The
dispatcher is `rust/psiv-core/src/battle/vehicle_skill.rs`; it keeps the
cartridge's ordinary hit-flag pass, then reads bytes 1–6 of
`VehicleSkillData` (`ps4.asm:321275-321299`) for the damage/effect path. It
never turns an unavailable record into a physical attack.

| id | name | effect | power byte 4 | resistance byte 5 | element byte 6 | proven animation path |
|---:|---|---|---:|---|---:|---|
| 1 | CLUSTER | damage | `$80` | defence | physical | standard damage result |
| 2 | GRAVITN | damage | `$F0` | magic defence | gravity | standard damage result |
| 3 | TH.GRID | damage | `$00` | magic defence | lightning | standard damage result |
| 4 | X-BURST | damage | `$F0` | magic defence | energy | standard damage result |
| 5 | NAPALM | damage | `$80` | magic defence | fire | standard damage result |
| 6 | NOTHING | damage | `$80` | defence | physical | standard damage result |
| 7 | N-SPHER | death | `$20` | mental | destroy | `BattleObj_DeathEffect` (`ps4.asm:8658`, `74778`) |
| 8 | NOTHING | damage | `$80` | defence | physical | standard damage result |

All eight records use strength (byte 2) and all-enemy range (byte 3). Damage
uses `Battle_CalculateDamage` with strength as attack, the selected resistance,
the selected element factor, and byte 4 as the power bonus. N-Spher first takes
the same proven damage path, then runs `AbilityEffect_Death`'s separate chance
roll (`ps4.asm:9098`, `9513-9608`) against mental resistance and the Destroy
factor; a landed effect marks the target dead and emits the vehicle effect
timeline event. The Godot timeline consumes that event without adding a fake
vehicle-specific damage number; the existing resolved-damage and death beats
carry the visible result.

### Mounted scene battles

The scene census gives the retail answer. The three Dezo campaign routines that
can run while the party is mounted are explicit:

| Scene | Retail behavior |
|---|---|
| Carnivorous Trees (`docs/scenes/53_CarnivorousTrees.md`) | dismount, then event battle `$0A` |
| Saving Kyra (`docs/scenes/54_SavingKyra.md`) | vehicle branch rebuilds party, then event battle `$0A` |
| Eclipse Torch (`docs/scenes/56_EclipseTorchUsed.md`) | dismount before its presentation; no battle |

The generic scene battle setup does not independently clear `Vehicle_Index`.
If an event caller leaves the selector mounted, retail's common battle setup
therefore takes the one-fighter vehicle branch; event battle background
selection still has priority. The ordinary retail scene records above choose
to dismount where their authored behavior requires it. The runtime boss/event
bridge follows the same rule: `start_vehicle` is selected when the scene enters
battle with a mounted vehicle, and the vehicle remains the sole party fighter.

The oracle fixture below enters that common battle path while mounted. It is
not evidence that a natural story scene skips its explicit dismount; it proves
the mounted entry surface itself.

## Per-map vehicle palettes

The vehicle field object writes sprite palette selector `$60`, which is CRAM
line 3. The map loader's `Pal_Init`/`Palette_Line_3` data supplies that line;
the vehicle art is not using the map's line 1 or line 2 palette. The additive
pack extraction keys the 16 vehicle colors from each map palette's line 3 and
emits `VehiclePaletteVariant` records. Across the 361 real emitted maps there
are 14 distinct line-3 palettes:

```text
variant: map count
0: 234, 1: 16, 2: 3, 3: 1, 4: 20, 5: 6, 6: 46,
7: 4, 8: 1, 9: 5, 10: 5, 11: 3, 12: 10, 13: 7
```

`GameData::vehicle_sheet_for_map` resolves the map-specific three-sheet set,
falling back to the base selector sheet only for filtered/older packs. Both
the field renderer and mounted battle setup use that lookup. The palette
counts are disassembly/data-backed extraction results; tape 29 is a movement
and entry oracle, not a CRAM-capture tape.

## Mounted battle backgrounds

`Battle_SetupBackground` chooses in this order:

1. `EventBattleBGIndexes`, 27 entries, when `Event_Battle_Index` is active;
2. `Battle_BackgroundIndexes`, 416 entries indexed by `Field_Map_Index`;
3. on Motavia map `0`, the raw terrain chunk through `MotaBattleBGIndexes`.

The Mota table is exactly 42 bytes at `ps4.asm:0586EC..058716`, not 43:

```text
0,3,3,3,3,1,1,1,3,3,0,1,0,1,0,1,0,3,0,3,0,3,0,
0,0,0,0,0,0,0,0,0,0,4,2,2,2,2,2,2,2,2,2
```

`GetMotaBattleBGIndex` stores index+1 and reserves zero for background zero;
the runtime subtracts that bias. `psiv_tools/battle_art_pack.py` extracts all
32 composite battle backgrounds and emits the three selection tables into
`runtime-pack/battle/art/backgrounds.json`. Mounted maps use the ordinary field
table when it has an entry. A mounted debug/map path whose field entry is
`$FF` falls back to the raw Mota terrain table; if its raw chunk byte is outside
the table's 42 named classes, it uses the table's reserved background-0 class.
That is what closes the former `no asset for map 0x013` failure for the
Piata Academy F1 fixture without claiming that an indoor chunk is Motavia
terrain. Event battles still win over both terrain paths.

The raw chunk grid is emitted per map as `vehicle_battle` in each map JSON.
The runtime keeps a variant's grid when an active layout replacement supplies
one and otherwise uses the base grid. This matters for mounted entry after a
map effect: a render-only variant must not erase the battle-background lookup.

## Oracle tape and verification

`oracle/tapes/29_vehicle_land_rover_probe.tape` is the vehicle tape. It boots
retail through the normal title and field path, then uses the documented
`--ram-patch` fixture boundary to load Motavia, set the principal-confession
flag that prevents the retail Piata re-entry branch, and place a Land Rover in
clear field state. The tape itself then holds movement input, probes dismount,
acknowledges the refusal window and leaves a frame for the mounted battle
fixture. The full replay command and patch provenance are in
[`oracle/README.md`](../../oracle/README.md#vehicle-tape-29).

The pinned replay produced these checks:

| Tape/frame | Observation |
|---|---|
| `29 / 7200` | mounted Land Rover, selector `1`, position `(2336,512)`, standing collision `0` |
| `29 / 7201` | right input begins the 32-pixel step at `+4 px`, confirming 4 px/frame |
| `29 / 7300` | vehicle stops at the solid boundary instead of crossing it |
| `29 / 7605` | fixture moves the anchor over raw standing `9`; selector remains mounted |
| `29 / 7726` | dumped retail frame visibly says `Cannot get off!` |
| `29 / 7830` | explicit fixture enters `FieldRoutine_Battle` while selector remains `1` |
| `29 / 7850` | retail RAM is in battle game mode with the mounted vehicle fighter |
| `29 / 7900` | battle setup has three enemies and vehicle selector `1` |

A second replay of the same tape without the bad-terrain patch dismounts
successfully by frame `8125`, proving the zero-standing-cell success side as
well as the refusal side. The mounted battle entry is intentionally a
documented fixture after the ordinary mounted field path; it does not claim a
natural random encounter happened at a chosen frame.

## Natural route receipt: tape 31

`oracle/tapes/31_natural_land_rover.tape` is the input-only naturalization
attempt. It starts from power-on, reaches Alys and the principal confession,
survives the Academy basement and Igglanova beats with ordinary field inputs,
recovers the party at Piata's live inn, crosses the Motavia/Edge route into
Zema, enters Birth Valley B1, completes the real Prof. Holt scene, and returns
to the Motavia overworld. The replay log is
`oracle/logs/31_natural_land_rover.csv`; it was generated with no `--ram-patch`
and contains 131,130 rows.

The stable frame marks are:

| Frame | Mark | Retail state |
|---:|---|---|
| `81,409` | `piata_inn_recovered` | Piata inn map `0019`, live recovery complete; selector `0` |
| `102,951` | `zema_town_warp` | Zema map `0024`, leader `(1568,1312)`; selector `0` |
| `103,071` | `zema_town_cross` | Zema town cross `(496,768)`; event flags `01FF0C00` |
| `103,675` | `zema_to_birth_valley` | Birth Valley load map `002B`; selector `0` |
| `113,419` | `birth_valley_b1_enter` | Birth Valley B1 map `002C`, leader `(528,224)` |
| `129,139` | `holt_scene_done` | return to Zema map `0024`, leader `(480,160)`, event flags `01FF8C00` |
| `131,011` | `natural_prefix_holt_world` | Motavia map `0000`, leader `(1584,1312)`; selector `0` |

`vehicle_index` is `0000` on every row. That is the important negative
receipt: this natural prefix never mounts, so it has no honest natural marks
for vehicle-terrain movement, dismount refusal, successful dismount, or a
mounted encounter. The tape stops here because `GettingLandRover` is later:
after the Zema/Krup/Tonoe/Rune/Alshline/Zio/BioPlant/Rika branch, the Control
Key prerequisite, and Machine Center B1 Part2. Tape 29 therefore remains the
fixture-only receipt for those vehicle behaviors; tape 31 does not overwrite it
with invented natural evidence.

The practical stop is also measured, not cosmetic: after Holt the party is
Chaz `27`, Alys `44`, Hahn `13` HP. The shortest attempted Piata healing
detour hit three natural formations whose retail rolls refused RUN; ordinary
combat inputs left only Alys alive before the inn inputs could be consumed.
The pre-`IgglanovaZemaDefeated` Zema inn is locked as well. Continuing from
this state needs a new long, battle-safe route plan, so it is documented as
remainder rather than smuggled into the tape as a fixture.

The clone replay comparator walks the same input schedule from its first
engine-aligned mark, `await_control` at frame `6,539`. It is clean for the
first 560 frames through frame `7,098` (33 modeled columns, zero
divergences). At frame `7,099` the retail party continues through a cell where
the clone's static NPC position blocks; that is the known NPC/wander frontier,
outside this vehicle slice and explicitly excluded from ownership. The clone
does not claim a vehicle or encounter receipt for the unmounted prefix.

## Verification status

Headless debug boots (the available Godot binary exits successfully and
reaches the field/battle surface):

```sh
env PSIV_DEBUG_VEHICLE=2 PSIV_DEBUG_VEHICLE_BATTLE=0x88 \
  /home/peter/.local/bin/cairn-godot-4.7.1 --headless --audio-driver Dummy \
  --path godot --quit-after 120
env PSIV_DEBUG_VEHICLE=3 PSIV_DEBUG_VEHICLE_BATTLE=0x88 \
  /home/peter/.local/bin/cairn-godot-4.7.1 --headless --audio-driver Dummy \
  --path godot --quit-after 120
```

For visual integration, run the same command under
`xvfb-run -a ... --display-driver x11` when the host permits X11 ownership.
This sandbox does not permit the Xvfb server to own `/tmp/.X11-unix`, so the
headless receipts are the integration proof available here.

Implemented and tested:

- all three selector profiles, masks, 32-pixel movement timing and footprint;
- Ice Digger snow/ice and Hydrofoil water masks, including Hydrofoil's 8 px/frame speed;
- menu/story mount and raw-zero dismount refusal/success;
- saved vehicle battle member, skill mask/use overlay and use decrement;
- mounted random/event battle party swap and `$96` music;
- 14 map-specific CRAM-line-3 vehicle palette variants;
- 27 event, 416 field-map and 42 Mota background indexes plus all 32 assets;
- oracle tape 29's mounted movement, dismount refusal, successful dismount and mounted entry fixture.

Still open:

1. The full natural Land Rover route is still owed. Tape 31 closes the
   power-on-to-Holt prefix, but it does not yet reach `GettingLandRover`; tape
   29 remains the explicit fixture for mounted movement, dismount and battle
   entry. The Ice Digger and Hydrofoil story routes are also still separate
   natural runs.
2. The Godot shell can report one non-fatal `AudioStreamGeneratorPlayback`
   ObjectDB leak at shutdown on some headless runs. The field/battle boot exits
   successfully; this is the pre-existing audio lifecycle, outside the vehicle
   slice.

Ownership remains split cleanly: core movement and battle-member rules are in
`rust/psiv-core/src/vehicle.rs`, mutable field/battle bridges in
`rust/psiv-runtime/src/vehicle.rs` and `boss_battles.rs`, Godot vehicle/battle
presentation in the runtime vehicle and battle modules, and additive extraction
in `psiv_tools/vehicle_pack.py` and `psiv_tools/battle_art_pack.py`. No scene
runner, dialogue system, enemy animation code or save layout was changed for
this slice. `title.rs` is touched only by the separate ERASE DATA closure.
