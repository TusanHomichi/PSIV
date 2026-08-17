# Vehicles

Scout and implementation record for the retail field vehicles. This slice was
scouted from the disassembly with `grand_cross=0` throughout. There is no
vehicle oracle tape in `oracle/`; the existing tapes stop before the Land Rover
route, and a long south-of-Zema capture was not a reasonable bounded run. Any
rule below that is not backed by an oracle is therefore marked by its retail
source range rather than presented as tape-confirmed.

## Selector and saved records

`Vehicle_Index` is the selector at `$F43C` and has the following meaning in the
field dispatcher (`ps4.asm:116755-116835`) and the three vehicle field-object
initialisers (`ps4.asm:93123-93207`):

| Selector | Vehicle | Project surface |
|---:|---|---|
| `0` | on foot | no mounted state; ordinary party movement and battles |
| `1` | Land Rover | `VehicleRecord[0]`, `LandRover` sheet |
| `2` | Ice Digger | `VehicleRecord[1]`, `IceDigger` sheet |
| `3` | Hydrofoil | `VehicleRecord[2]`, `Hydrofoil` sheet |

Selectors above `3` fail closed. Mounting anchors the vehicle at the party
leader's current field cell in the runtime. Retail scenes set the selector
explicitly; Wave 4's machine-center scene is documented in
`docs/scenes/33_GettingLandRover.md` and uses selector `1`. The field menu's
vehicle branch enters the retail getting-off event (`ps4.asm:114934-114976`,
`Event_GettingOffVehicle` at `ps4.asm:145343+`); the runtime exposes the same
state change through `Input::Action`.

The save layout is unchanged. The runtime consumes the existing
`Saved_Vehicle_Stats` records at `$FA80..$FB00` documented in
`docs/SAVE_SCOUT.md`: current/max HP at `$00/$02`, skill mask at `$04`, and
eight current/max use pairs at `$06..$15`. A mounted battle writes current HP
and current uses back to that same `VehicleRecord`; no new serialization path
was added.

## Field presentation and movement

### Sprite extraction

Retail loads the selected Nemesis art in `loc_51A7A`
(`ps4.asm:107728-107747`). The field object points `art_ptr` at `RAM_Start`,
uses art tile `$534`, sets sprite palette line `3` with `$13=$60`, and selects
the vehicle mapping table (`FieldObj_LandRover`, `FieldObj_IceDigger`, and
`FieldObj_Hydrofoil`, `ps4.asm:93123-93207`). The extractor reuses the proven
Nemesis and streamed field-sheet composers in `psiv_tools/sprites/`.

The generated pack currently contains `sprites/vehicles.json` and the three
PNG sheets:

| Selector | Sheet | Mapping table | Nemesis art | Decoded art | Sheet frames |
|---:|---|---:|---:|---:|---|
| `1` | `LandRover` | `$047EF4` | `$296320` | 3,200 bytes / 100 tiles | 8 x 40x32 |
| `2` | `IceDigger` | `$047F04` | `$296A54` | 3,200 bytes / 100 tiles | 8 x 40x32 |
| `3` | `Hydrofoil` | `$047F14` | `$2971A4` | 1,664 bytes / 52 tiles | 4 x 40x32 |

The retail mapping data is two frames per direction for Land Rover and Ice
Digger, with the left/right mirror entries at mapping indices `6/7`; Hydrofoil
has one frame per facing and a left mirror at index `3`. The mapping sequence
records use duration byte `$04`, which the pack expresses as five display
ticks. The art dimensions and the palette line are disassembly-derived. The
pack currently bakes the first selected map's 48-colour map palette for CRAM
line 3, which is a deterministic pack convention; palette variation after a
map change remains open.

Mounting swaps the ordinary party/follower field nodes for the selected vehicle
sheet. Dismounting restores them after the runtime accepts the action. Retail
does not expose a separate walking-to-vehicle animation in these field-object
initialisers; the visible mount/dismount boundary is the selector/state change
and the ordinary field object rebuild.

### Movement timing and footprint

`VehicleMove` (`ps4.asm:93478-93599`) uses a 32-pixel movement unit, unlike
the party's 16-pixel collision cell. Its step table at `loc_47C28`
(`ps4.asm:94079+`) is:

| Table block | 8.8 constant | Frames / 32px | Pixels / frame |
|---:|---:|---:|---:|
| `0` | `$0200` | `16` | `2` |
| `1` | `$0400` | `8` | `4` |
| `2` | `$0800` | `4` | `8` |

The retail default `FieldObj_Step_Offset` is `1` (the default writes are at
`ps4.asm:87729` and `87741`), so a Land Rover uses 8 frames per 32-pixel
unit, or 4 pixels per frame. Hydrofoil adds `$80` to the table pointer before
selecting the step (`ps4.asm:93478-93548`), effectively using the next block:
default selector `1` is therefore 4 frames / 8 pixels per frame. Selector `2`
on Hydrofoil would read past the three-block table; the project rejects that
combination rather than inventing a speed.

The vehicle's standing and directional collision samplers are the four-cell
footprint in `UpdateVehicleStandCollision` and `UpdateVehicleCollision`
(`ps4.asm:90562-90588`, `90694-90810`). In party-cell coordinates, the
standing footprint is `(0,-1),(1,-1),(0,0),(1,0)`. Directional samples are:

```text
right: ( 2,-1), ( 3,-1), ( 2,0), ( 3,0)
left:  (-2,-1), (-1,-1), (-2,0), (-1,0)
down:  ( 0, 1), ( 1,1), ( 0,2), ( 1,2)
up:    ( 0,-3), ( 1,-3), ( 0,-2), ( 1,-2)
```

The retail routine takes the unsigned maximum of the four raw collision
values. The selector-specific pass masks in `loc_45C46`
(`ps4.asm:91065-91094`, tables at `91222-91280`) are:

| Selector | Raw collision values the vehicle can cross |
|---:|---|
| Land Rover (`1`) | `0`, `1`, sand `A` |
| Ice Digger (`2`) | `0`, `1`, sand `A`, ice `B` |
| Hydrofoil (`3`) | `0`, `1`, water `9`, sand `A` |

Raw values not listed are solid for that selector. This is the concrete
vehicle-vs-feet difference: the foot table treats raw `8/9/A/B` as blocking,
while the Land Rover admits sand (`A`) and its two-cell footprint can cross
terrain the ordinary one-cell party cannot.

Map-change raw `1` is handled separately by the vehicle transition path. The
standing raw `1` guard suppresses repeated transition firing, matching
`MapTransTile_MapChange` (`ps4.asm:91303-91319`). The runtime also carries the
retail adjacent-transition grace samples: standing raw `1/2` or a directional
raw `1` suppresses a vehicle encounter (`RunRandomBattles`,
`ps4.asm:116840-116884`). Vehicle encounters use `& $7F` (1 in 128), versus
the foot path's `& $1F` (1 in 32).

Dismounting calls the vehicle standing collision check at the current anchor
(`loc_4587A`, `ps4.asm:90641-90647`). Success requires the four-cell standing
maximum to be raw `0`; otherwise retail displays `CannotGetOffString`. The
runtime emits `VehicleDismountBlocked` for the failure and restores the party
nodes on success.

## Vehicle battle surface

### Entry and formation selection

`FieldRoutine_Battle` (`ps4.asm:117925-117945`) selects music `$96`
(`CyberneticCarnival`) when `Vehicle_Index != 0`, and `$95`
(`MeetThemHeadOn`) on foot. Mounted random encounters now dispatch `$96`
through the existing Godot audio queue. `PSIV_DEBUG_VEHICLE_BATTLE=<hex
formation>` mounts the Land Rover before boot and uses this same `$96` path;
the ordinary `PSIV_DEBUG_BATTLE=0x88` oracle selector remains the on-foot audio
probe.

For Motavia, retail reads the raw chunk at the vehicle position and indexes
`MotaBattleBGIndexes` (`GetMotaBattleBGIndex`, `ps4.asm:118165-118224`):

```text
0,3,3,3,3,1,1,1,3,3,0,1,0,1,0,1,0,3,0,3,0,3,0,0,0,0,0,0,0,0,0,0,
0,4,2,2,2,2,2,2,2,2,2
```

The battle formation setup then chooses the vehicle group in
`Battle_SetupEnemyData` (`ps4.asm:11813+`): Motavia defaults to group `8`,
uses group `9` for background `1`, group `A` for background `3`, and uses group
`D` on non-Motavia vehicle maps. The selected group entry is rolled with
`UpdateRNGSeed2 & $1F`. These groups are present in the generated battle pack:
Motavia `8/9/A` and Dezolis `D`.

### Party swap and saved vehicle member

`loc_78EE` (`ps4.asm:11408-11414`) is the entry difference that matters:
vehicle entry writes fighter id `Vehicle_Index + $B` and sets
`Obj_Fighters = 1`, replacing the ordinary party with one vehicle fighter.
The static `VehicleData` block (`ps4.asm:321152+`) is represented in
`rust/psiv-core/src/vehicle.rs`:

| Vehicle | Static HP | STR | AGI | DEX | ATK | DEF | MDEF | Default skill mask |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Land Rover | `$2E4` | 64 | 60 | 70 | 200 | 80 | 50 | `$03` |
| Ice Digger | `$3C0` | 80 | 48 | 70 | 250 | 80 | 50 | `$50` |
| Hydrofoil | `$2A8` | 255 | 96 | 70 | 150 | 70 | 40 | `$0C` |

The battle member takes current/max HP, the saved skill mask, and current/max
skill uses from the selected `VehicleRecord`. The skill IDs are the eight
retail vehicle slots from `ps4.constants.asm:614-621` (Cluster, Graviton,
Thunder Grid, X-Burst, Napalm, Nothing, N-Sphere, Nothing 2); unavailable
slots remain zero in the battle stats. On battle exit the runtime copies
current HP and current skill uses back to the selected record and does not
award ordinary party XP or absorb the member into the character roster.

There is one deliberate parity note: retail's `FillBattleStats`
(`ps4.asm:11272-11403`) reloads the static vehicle HP on entry and its battle
exit path only copies current skill uses (`ps4.asm:6375-6387`). This project
uses the saved current/max HP as the save lane promises round-trippable vehicle
records and the acceptance requires the battle to consume that record. That is
an explicit project policy, not a claim that the retail entry routine read
`$FA80` HP.

The runtime now starts `Battle::start_vehicle`, sets the one-fighter vehicle
flag, applies the retail half-XP reward seam, and exposes the selected saved
member to the Godot battle renderer. The renderer uses the first frame of the
selected vehicle sheet, hides the field party while mounted, and restores the
mounted field node after battle.

## Verification and open edges

Implemented and covered by source/unit tests:

- selector range and selector-specific terrain masks;
- four-cell standing/directional collision and 32-pixel movement timing;
- action-edge dismount success/failure;
- saved vehicle HP, skill mask, and skill uses becoming the one battle member;
- Motavia raw-chunk background/group selection and vehicle encounter mask;
- generated Nemesis vehicle sheets and manifest/index records;
- `$96` vehicle battle music dispatch.

Still open, and intentionally not guessed:

1. No vehicle oracle tape exists yet, so live timing/terrain/formation captures
   on the south-of-Zema route remain unverified against the emulator.
2. The current battle command engine exposes Attack/Defend/Run only. Vehicle
   skill IDs and uses are carried into and out of the battle member, but the
   vehicle skill-selection/use animation surface is not implemented yet.
3. The baked vehicle palette is sourced from the first selected map. A
   per-map CRAM-line-3 palette switch needs a verified retail capture before
   it should replace that deterministic pack rule.
4. Retail's separate vehicle battle setup is implemented for random/debug
   entry. Whether every scene-owned/event battle may be entered while mounted
   needs a separate disassembly census; scene battle entry remains on its
   existing boss path.

The ownership split is deliberate: core rules and battle-member construction
are in `rust/psiv-core/src/vehicle.rs`, mutable field/battle bridges are in
`rust/psiv-runtime/src/vehicle.rs`, Godot presentation/entry is in the field
and battle modules, and extraction is in
`psiv_tools/sprites/vehicles.py` plus the existing pack emitters. No scene
runner, title implementation, save layout, or camera/wander internals were
changed for this slice.
