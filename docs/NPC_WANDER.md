# NPC wander: how the cartridge's townsfolk move

Scouted 2026-08-16 (core-lane) from `reference/ps4disasm`, cross-checked
against `runtime-pack/npc_commands.json` (overworld-lane's extraction of the
same tables). Written to the `BATTLE_SCOUT.md` standard: routine labels are
retail and load-bearing, **inline comments in the clone drift and are not
evidence**. Everything below is the instruction stream.

## Build provenance and the `grand_cross` trap

The checked-in disassembly is a **Grand Cross build**: `reference/ps4disasm/
ps4.options.asm:10` sets `grand_cross = 1`. A `grand_cross=0` branch in this
source is therefore not evidence that the clone assembled the retail branch;
it is conditional source text for another build. The field routines closed in
this slice are common, unguarded instruction streams: the caller/helper blocks
at `ps4.asm:95081-95168`, `95649-95677`, `95964-95992`, `96397-96423`,
`98564-98602`, `98821-98847`, `99044-99066`, `99114-99135` and
`99413-99435` contain no `grand_cross` conditional. The only conditional
near this range is the separate Big Duck/Grand Cross presentation code at
`ps4.asm:99147` onward, not a random-wander branch.

So the parity claim below is deliberately narrow: it transcribes the common
random routines present in the clone and validates them against the clone's
oracle tapes. It does not silently claim a `grand_cross=0` retail body.

Prompted by the comparator: from frame 7819 of tape 02 the cartridge's
NPCType2 townsfolk have wandered off their spawn cells while ours stand still,
and our walker then blocks on a cell the cartridge's NPC had already left.

## The headline: it is the portable RNG

**`FieldObj_GetRandomMove` calls `UpdateRNGSeed` (`$04236C`), not
`UpdateRNGSeed2`.** Wander is driven by the multiply-by-41 LCG the encounter
rolls already use — not the VDP H/V-counter generator that makes battle rolls
unreproducible.

That means **cartridge NPC positions are bit-exactly reproducible** given the
same seed and the same frame cadence. Nothing about wander needs the "which
substitute generator" decision that `BATTLE_SCOUT.md` §16.1 leaves open.

## The per-frame tick structure — and what wander is *not*

Oracle-lane measured exactly **two `UpdateRNGSeed` calls per frame in field
mode**, with the second disappearing while a menu is open, and asked whether
that second call is the NPC update. **It is not.** The two per-frame calls are:

| # | call site | retail | when |
|---|---|---|---|
| 1 | the VInt handler | `ps4.asm:617` | every frame, every game mode |
| 2 | `GameMode_Field` | `ps4.asm:107638` | every frame in field mode |

`GameMode_Field` opens with the call, before it dispatches anything:

```
GameMode_Field:
    jsr     (UpdateRNGSeed).l
    move.w  (Game_Mode_Routine).w, d0
    movea.l FieldRoutinePtrs(pc,d0.w), a0
    jmp     (a0)
```

Unconditional, once per frame, serving no particular consumer — it simply
stirs the seed before the frame's field work. That is the second tick.

**Why it vanishes with a menu open**: window-bearing routines do not return to
the mode dispatcher. `FieldRoutine_Menu` ends in `Field_MenuLoop`
(`ps4.asm:117143`) and `FieldRoutine_Shop` in `loc_65D58`, both spinning on
`Window_Update` + `VInt_Prepare`/`DMAPlane_A_VInt` until the window closes.
The VInt handler keeps ticking (call 1), but `GameMode_Field` is never
re-entered, so call 2 stops. The measurement and the instruction stream agree
exactly.

**Wander rolls are additional and conditional.** `FieldObj_GetRandomMove` draws
only when its object is idle *and* its countdown has expired — so a frame's
total is 2 plus however many NPCs happen to roll on it. The occasional 3/frame
oracle-lane saw is one NPC rolling; 4 or more is several timers expiring
together, which is uncommon but not rare. With ~8 wanderers on a map and pauses
averaging ~32 frames, the expected extra is about a quarter of a call per
frame.

`RunRandomBattles` (`ps4.asm:116868`) is a third occasional source, but it is
not per-frame either: it draws only when its step counter reaches zero, i.e.
once every ten steps the party takes.

**Consequence for the engine.** Reproducing the cartridge's stream needs *all*
of it: two unconditional ticks per field frame, plus the conditional wander and
encounter draws, on one shared seed. A runtime that only ticks the LCG when an
NPC rolls will drift immediately, and one that ticks twice while a window is
open will drift the other way.

## Which types wander

Census over the 949 objects in all packed maps, with direct random callers
checked against the disassembly:

| packed symbol | count | routine entry | random helper | init leash |
|---|---:|---|---|---|
| `NPCType2` | 275 | `ps4.asm:95081` | `FieldObj_GetRandomMove` (`96693`) | 4/4, 2/2 |
| `NPCType3` | 22 | `ps4.asm:95113` | `FieldObj_GetRandomMove2` (`96711`) | 4/4, 2/2 |
| `NPCType4` | 12 | `ps4.asm:95142` | `FieldObj_GetRandomMove3` (`96730`) | 4/4, 2/2 |
| `NPCType28` | 2 | `ps4.asm:95649` | `FieldObj_GetRandomMove3` (`96730`) | 8/8, 4/4 |
| `loc_490B8` | 1 | `ps4.asm:95964` | `FieldObj_GetRandomMove` (`96693`) | 8/8, 4/4 |
| `loc_49746` | 3 | `ps4.asm:96397` | `FieldObj_GetRandomMove` (`96693`) | 4/4, 2/2 |
| `Xanafalgue` | 1 | `ps4.asm:98564` | `FieldObj_GetRandomMove` (`96693`) before `$100` escape | 2/2, 1/1 |
| `Penguin` | 4 | `ps4.asm:98821` | `FieldObj_GetRandomMove` (`96693`) | 8/8, 4/4 |
| `loc_4B4B4` | 0 | `ps4.asm:99044` | `FieldObj_GetRandomMove` (`96693`), `d7=1` | 16/16, 8/8 |
| `Butterfly` | 2 | `ps4.asm:99114` | `FieldObj_GetRandomMove3` (`96730`) | 16/16, 8/8 |
| `MuskCat` | 10 | `ps4.asm:99413` | `FieldObj_GetRandomMove` (`96693`) | 16/16, 8/8 |

The ten packed random families above are **332 placements**. The remaining
617 placements are not generic random walkers. They are intentionally not
registered in `WanderSet`; registering every unknown symbol as a Type2 would
corrupt the shared RNG stream.

### Bespoke-family transcription, wave 4

The first pass over the non-random debt covers the seven families with the
largest placement counts. These are static or animation-only bodies, so their
correct parity consequence is **no wander state and no RNG draw**, not a new
random-walk kind:

| routine family | placements | proven body | retail source |
|---|---:|---|---|
| `Elevator` | 92 | updates its map position, selects tile props from `BG_Alternate_Color_Flag`, then runs `FieldObj_Animate2`; interaction bit 3 is cleared | `FieldObj_Elevator`, `ps4.asm:101622-101650` |
| `InvisibleBlock` | 90 | interaction/event trigger with no sprite; sets no-sprite bit 1, updates position and recalculates coordinates | `FieldObj_InvisibleBlock`, `ps4.asm:95407-95424` |
| `NPCType32` | 51 | clears its facing animation byte, updates position, recalculates coordinates, and animates in place; fixed down-facing shopkeeper | `FieldObj_NPCType32`, `ps4.asm:96225-96255` |
| `FireplaceFire` | 48 | recalculates coordinates, applies the on-screen gate, and runs `FieldObj_Animate2`; no movement or interaction | `FieldObj_FireplaceFire`, `ps4.asm:98712-98730` |
| `NPCType1` | 37 | calls `FieldObj_NPCMoveDown`; its zero-height leash rejects the down step, leaving a fixed down-facing NPC, then updates/animates | `FieldObj_NPCType1`, `ps4.asm:95047-95079`; helper at `96686` |
| `Fire` | 32 | recalculates coordinates, applies the on-screen gate, and animates in place; no movement or interaction | `FieldObj_Fire`, `ps4.asm:98675-98692` |
| `Statue` | 24 | static non-interactable art: on-screen test, position update, coordinate calculation, idle animation | `FieldObj_Statue`, `ps4.asm:98619-98642` |

That is **374 of the 617** placements. The remaining census is below. It is
grouped by count so the full symbol set remains reviewable without pretending
that one of these routines shares another routine's movement contract.

| placements per symbol | symbols |
|---:|---|
| 13 | `NPCType30` |
| 12 | `NPCType9`, `NPCType10` |
| 10 | `Mouse` |
| 9 | `XeAThoulAirCastle` |
| 7 | `NPCType8`, `NPCType20` |
| 6 | `NPCType31`, `loc_49502` |
| 4 | `NPCType16`, `NPCType17`, `loc_48F96` |
| 3 | `BigFire`, `NPCRune`, `NPCType19`, `NPCType21`, `NPCType22`, `NPCType27`, `PrisonDoor`, `StrayRocky`, `loc_49542`, `loc_4980A` |
| 2 | `Barrier`, `Blindheads`, `EsperGuard`, `GiLeFarg`, `GravestoneHalf`, `InnerEsperGuards`, `MuskCatGuard`, `NPCHahn`, `NPCKyraSpaceport`, `NPCRajaSpaceport`, `NPCScriptMove`, `NPCType6`, `NPCType24`, `NPCWren`, `Pana`, `Rika`, `StudentInBed`, `loc_49406`, `loc_49442`, `loc_4BDF0` |
| 1 | `AlysAngerTower`, `BarrierBeam1`, `BarrierBeam2`, `BarrierBeam3`, `BarrierBeam4`, `BigDuck`, `CaveWallPiece`, `ChestBarrier`, `DElmLars`, `DarkForce1`, `DarkForce2`, `DeVars`, `DemiSpaceportWaiting`, `DemiTrapped`, `DorinChair`, `EclipseTorch`, `FellowPenguin`, `FractOoze`, `GryzSpaceportWaiting`, `HahnSpaceportWaiting`, `Igglanova`, `Juza`, `KingRappy`, `KingRappyFlyingAway`, `KyraSpaceportWaiting`, `LutzMirror`, `LyingDownMuskCat`, `MileSandWorm`, `MuskCatChiefBottomHalf`, `MuskCatChiefTopHalf`, `NPCAlysInBed`, `NPCAlysPiata`, `NPCDemiSpaceport`, `NPCGryz`, `NPCGryzSpaceport`, `NPCHahnNearBasement`, `NPCHahnSpaceport`, `NPCKyra`, `NPCRika`, `NPCType5`, `NPCType7`, `NPCType11`, `NPCType13`, `NPCType14`, `NPCType18`, `NPCType23`, `NPCType25`, `NPCType26`, `NPCType29`, `NPCType33`, `NPCType34`, `NPCType35`, `NPCType36`, `Pennant`, `Prisoner`, `ProfHoltPetrified`, `Raja`, `RajaInBed`, `RajaSpaceportWaiting`, `Rocky`, `SaLews`, `SandWormCarving`, `SmallBrownDuck`, `SmallWhiteDuck`, `TallasShoes`, `TonoeBasementDoor`, `TrappingRopes`, `ZemaRocks`, `loc_48F36`, `loc_48FF4`, `loc_49128`, `loc_49192`, `loc_49212`, `loc_496C6`, `loc_497A8`, `loc_4986C`, `loc_498CA`, `loc_4BE38`, `loc_4BE80` |

`Pana` is included in the two-placement census because its body is fixed and
not a random family. `MileSandWorm` is the one-placement special animation;
neither is safe to fold into the generic walker.

## Bespoke completion: wave 8

The post-wave-7 census is now closed against the live pack: **243 placements
across 121 symbols before this wave, 0 placements across 0 symbols after it**.
The 949-placement total is unchanged: the ten random families remain in
`WanderSet`, the seven wave-4 static families remain their existing no-RNG
classification, and all 121 symbols covering those 243 placements now have an
explicit `BespokeKind` entry in `rust/psiv-runtime/src/bridge.rs`.

The implementation is deliberately split by what the routine actually does:

| engine classification | families | shared-stream consequence |
|---|---|---|
| `Pattern` | `NPCType5`, `NPCType17`, `NPCType35`, `NPCType36`, `Rocky`, `loc_48F36`, `loc_49128`, `NPCHahnNearBasement` | literal command tables, selector 0/1 as shown by each caller, no RNG |
| `Fixed` | `NPCType9`, `NPCType10`, `NPCType16`, `NPCType18`, `NPCType19`, `NPCType27`, `NPCType30`, `NPCType31`, `NPCType33`, `NPCType34`, `Pana`, `Juza`, `StrayRocky`, `loc_48F96`, `loc_48FF4`, `loc_497A8`, `loc_4980A`, `loc_4986C`, `loc_498CA` | raw facing is written; leash/terrain/object gates decide whether a step starts; no RNG |
| `Random` | `NPCType11`, `NPCType12`, `NPCType13`, `Mouse`, `Prisoner`, `SmallWhiteDuck`, `SmallBrownDuck` | one `Rolls` stream in map-object order; helper-specific masks and retry rules are transcribed |
| `FlaggedRandom` | `NPCType8` | no draw while `EventFlag_IgglanovaZema` is clear; `loc_49CEA` draws only after it is set |
| `Follow` / `FlaggedFollow` | `NPCType6`, `NPCType7`, `FellowPenguin`, `loc_49192`, `loc_496C6` | relationship/object-target command, no RNG; FellowPenguin clears interaction bit 3 when joined |
| `FlaggedPattern` | `EsperGuard`, `InnerEsperGuards`, `MuskCatGuard` | fixed pre-flag command, then the extracted literal route; no RNG |

The motion classes all use the same cartridge gates as the ordinary walkers:
facing is written before a leash refusal, accepted movement commits the map
cell immediately, and a step uses the caller's extracted speed selector. The
runtime ticks wanderers and bespoke actors by **one ascending NPC index** so a
new random helper cannot silently get a private RNG stream. The routine and
table provenance is `FieldObj_NPCType5..36` (`ps4.asm:95171-95834`),
`loc_48F36..loc_49128` (`95858-95993`), the object-follow bodies
`loc_49192`/`loc_496C6` (`96019`/`96366`) and `loc_4A05E`/`loc_4B366`
(`97139`/`98916`), and helpers `loc_49BDA..loc_4A0F4` (`96744-97195`); the
executable tables are constants in `rust/psiv-core/src/bespoke.rs`.

### Why the largest families do not become fake walkers

The 13 `NPCType30`, 12 `NPCType9/10`, and 6 `NPCType31` placements are
deterministic **attempts**, not free-standing movers. For example, Type9's
up command starts at `y=8` with a zero-height y leash; Type10's left command
starts at `x=0` with a zero-width x leash; Type30's right command starts at
the x maximum. They still write their facing and run the animation/position
helpers, so dropping them entirely would be wrong; giving them random movement
would be worse. `Fixed` preserves exactly the useful state and spends zero
RNG.

The small random helpers are likewise not generalized:

- Type11 (`loc_49D34`) draws once to choose one of eight literal sequences;
  the sequence itself draws nothing.
- Type12/Mouse (`loc_49DCA`) decrements its routine timer and draws once on
  the expiry branch, with `Main_Frame_Count & 3` selecting the two masks.
- Type13 and both ducks (`loc_49E20`) draw until the low three bits are 3 or 4.
- Prisoner (`loc_49EA4`) draws one low-six-bit value; zero alternates left and
  right, nonzero chooses down.
- Type8 (`loc_49CEA`) stays quiet until the Zema event gate is set, then uses
  the same timer-plus-one-draw contract.

### Explicit stays: scene and presentation ownership

The remaining routines are registered, but their why-not is structural rather
than a missing census entry:

| classification | symbols | why the field tick does not synthesize movement |
|---|---|---|
| `SceneDriven` | `AlysAngerTower`, `DemiSpaceportWaiting`, `GryzSpaceportWaiting`, `HahnSpaceportWaiting`, `KingRappyFlyingAway`, `KyraSpaceportWaiting`, `NPCDemiSpaceport`, `NPCGryz`, `NPCGryzSpaceport`, `NPCHahnSpaceport`, `NPCKyra`, `NPCKyraSpaceport`, `NPCRajaSpaceport`, `NPCRika`, `NPCScriptMove`, `NPCWren`, `Raja`, `RajaSpaceportWaiting`, `Rika`, `NPCAlysPiata` | the body reads `FieldObj_GetInput` under event/player authority or consumes a scripted destination; scene/event authority owns the input |
| `StaticAnimation` | `BigFire`, `CaveWallPiece`, `DElmLars`, `DarkForce1`, `DarkForce2`, `DeVars`, `DemiTrapped`, `DorinChair`, `EclipseTorch`, `FractOoze`, `GravestoneHalf`, `Igglanova`, `KingRappy`, `loc_49212`, `loc_49406`, `loc_49442`, `loc_49502`, `loc_49542`, `loc_4BDF0`, `loc_4BE38`, `loc_4BE80`, `LutzMirror`, `LyingDownMuskCat`, `MuskCatChiefBottomHalf`, `MuskCatChiefTopHalf`, `NPCAlysInBed`, `NPCHahn`, `NPCRune`, `Pennant`, `PrisonDoor`, `ProfHoltPetrified`, `RajaInBed`, `SaLews`, `SandWormCarving`, `StudentInBed`, `TallasShoes`, `TonoeBasementDoor`, `TrappingRopes`, `ZemaRocks` | deterministic position/animation only; no cell command and no shared RNG. The engine records the routine as a no-RNG actor rather than pretending it is absent |
| `PresentationOnly` | `Barrier`, `BarrierBeam1..4`, `BigDuck`, `Blindheads`, `ChestBarrier`, `GiLeFarg`, `MileSandWorm`, `XeAThoulAirCastle` | special art, hide/show, or absolute-pixel state. `MileSandWorm` also draws only from its visual animation state (`$10/$11`); that state is outside the field tick, so the engine records the structural boundary instead of inventing a draw or cell occupancy |

For `grand_cross=0`, the barrier beams retain the source's hide/show bounds and
parity animation but omit the Grand Cross-only sound/flag side effects. Big
Duck uses its direct pixel table; Mile Sand Worm hides for its opening 0x78
frames and later has a four-way absolute-position RNG write; neither is a
cell-walk contract. `XeAThoulAirCastle` is a deterministic map/frame flicker.
Those routines stay `PresentationOnly` with the reason visible in the census.
The field tick does not consume Mile Sand Worm's later four-way draw because it
does not own the `$10/$11` animation-state transition that gates it; a visual
lane that models that transition can take ownership of the draw without
changing field movement order. This is the honest boundary: no fake movement,
no silent RNG draw, no palette regeneration.

Types 2 and 3 are still 297 objects and differ in exactly one pause mask. The
newly transcribed families share the same remap and three collision gates, but
their helper choice and initialization leash are now explicit in the core and
bridge. Xanafalgue's post-threshold escape is a separate branch, described
below; it does not draw RNG.

## The decision, per frame

`FieldObj_NPCType2` (`ps4.asm:95081`) runs, per frame:

```
    bsr.w   FieldObj_OnScreenTest
    bne.s   loc_484E4               ; off-screen -> skip ALL movement
    bsr.w   FieldObj_GetRandomMove
    moveq   #0, d7                  ; speed selector 0
    bsr.w   FieldObj_NPCMove
    bsr.w   FieldObj_UpdateStepDuration
    bsr.w   FieldObj_UpdatePosition
```

**Off-screen NPCs are frozen.** `FieldObj_OnScreenTest` (`$049B34`,
`ps4.asm:96661`) rejects sprite positions outside `$60..$1E0` horizontally and
`$60..$180` vertically, and a rejected object skips the move, the duration
update *and* the position update. A wanderer only wanders while it is on
camera, so its position is a function of where the player has been looking.

`FieldObj_GetRandomMove` (`$049B62`, `ps4.asm:96693`):

```
    moveq   #0, d0
    move.w  x_step_duration(a4), d1
    or.w    y_step_duration(a4), d1
    beq.s   loc_49B6C               ; idle -> consider a new move
    bpl.s   loc_49B88               ; still stepping -> command 0
loc_49B6C:
    subq.w  #1, timer(a4)
    bpl.s   loc_49B88               ; still counting down -> command 0
    jsr     (UpdateRNGSeed).l
    move.w  (RNG_Seed).w, d0
    andi.w  #$3F, d0                ; NPCType3 uses $7F here
    move.w  d0, timer(a4)           ; the pause, 0..63
    andi.w  #7, d0                  ; the direction index, 0..7
loc_49B88:
    bra.w   loc_4A144
```

Three things worth stating plainly:

1. **One roll decides both the pause and the direction.** `timer = r & $3F` and
   `index = r & 7`, from the *same* word — so `index == timer & 7`. They are
   not independent draws, and a model that rolls twice is wrong.
2. **The only difference between Type2 and Type3 is the timer mask**: `$3F`
   (pause 0..63) versus `$7F` (pause 0..127). Type3 is the same walker, idling
   about twice as long.
3. The countdown is `subq.w #1` then `bpl`, so a timer of `n` waits `n + 1`
   frames before the next roll — and the roll's own value becomes the next
   pause, so an NPC's rhythm is self-similar.

`FieldObj_GetRandomMove3` (`ps4.asm:96730`) is the other closed helper. It
enters the same `UpdateRNGSeed` path only when both step durations are zero,
then masks the seed with `& 7` and remaps it. It does **not** decrement or
reload `$1C`; Type4, Type28 and Butterfly therefore draw on every visible idle
frame. A mid-step object still consumes no roll, and every caller still runs
`FieldObj_OnScreenTest` first.

The named callers are not guesses based on art. Their instruction streams each
show the same sequence — on-screen test, the helper named in the table above,
`moveq #0,d7`, `FieldObj_NPCMove`, `FieldObj_UpdateStepDuration`, and
`FieldObj_UpdatePosition` — at the routine entries recorded above. The one
packed exception is Xanafalgue (`ps4.asm:98564`): while the leader's Y is at or
below `$100` it follows that sequence; after `Character_1.$34 > $100`, it
sets `$20 = $FFFC` and updates position without the random helper until its X
falls below `$1A0`, then sets `TempEveFlag_Xanafalgue` and clears its object
slot (`ps4.asm:98564-98602`). The runtime models the movement and slot clear;
the temporary flag remains in the event-state lane.

### The direction remap

`loc_4A144` (`ps4.asm:97233`) puts the 0..7 index through an 8-byte table at
`loc_4A150`:

```
    dc.b $00, $01, $02, $04, $08, $00, $00, $00
```

So the index maps to a command byte: `0→stand, 1→up, 2→down, 3→left,
4→right, 5,6,7→stand`. The distribution over eight equally likely indices is
therefore **50% stand still, 12.5% each of the four directions** — symmetric.

This remap is easy to miss, and missing it is a trap: the raw command table has
*three* ids for left (4, 5, 6) and three for right (8, 9, 10), so masking the
roll with `& 7` and using it as a command byte directly makes right unreachable
and left three times as likely. The remap exists precisely to prevent that.

## The move, and the three gates

`FieldObj_NPCMove` (`$04A1D3`, `ps4.asm:97257`) returns immediately if either
step duration is nonzero, then selects the speed table by `d7 * 4` and runs
three checks in order. **Any refusal jumps to `loc_4A188`**, which zeroes the
step constants, resets the animation, and still writes the facing byte — so a
blocked NPC turns on the spot without moving.

| order | routine | retail | what it rejects |
|---|---|---|---|
| 1 | `loc_4A3B6` | `ps4.asm:97532` | the leash |
| 2 | `loc_45CA4` | `ps4.asm:91098` | terrain collision |
| 3 | `loc_4A316` | `ps4.asm:97461` | other objects |

### 1. The leash

Each object carries four bytes: `x_max_move_boundary` (`$3C`),
`y_max_move_boundary` (`$3D`), `x_move_boundary` (`$3E`), `y_move_boundary`
(`$3F`). `FieldObj_NPCType2`'s init writes `move.w #$404, $3C(a4)` and
`move.w #$202, $3E(a4)` — maxima of 4 on both axes, current offsets starting at
2. So a wanderer roams a **5×5 cell box with its spawn cell at the centre**,
two cells in each direction.

`loc_4A3B6` looks the move's cell delta up in `loc_4A3FE` (indexed by command
byte doubled: `up (0,-1)`, `down (0,+1)`, `left (-1,0)`, `right (+1,0)`), adds
it to the current offsets, and refuses if either goes negative or above its
maximum.

**The offsets are committed before the other two checks run.** A move that
passes the leash but is then refused by terrain or by an object has *already*
consumed its leash budget — so the box drifts relative to the spawn cell over
time. That is a genuine cartridge quirk, not a tidy invariant, and reproducing
it matters for long-running position comparisons.

### 2. Terrain

`loc_45CA4` offsets the position by ±16 px from `loc_45CDC`, calls
`GetChunkAndCollision`, and dispatches through `TileCollNormalPtrs` — the same
blocking set the party walker uses. Nothing NPC-specific.

### 3. Objects

`loc_4A316` scans `Character_1` and the four slots after it, comparing
extrapolated positions, so **a wanderer will not walk into the party**. It is
gated on `btst #3, render_flags(a4)`: an object with bit 3 clear does no object
collision at all, which is the same bit that gates the talk probe and
`FieldObj_DoObjCollision`. One flag, three readers.

## Corrections, 2026-08-15

Two conflicts with oracle-lane's report, both settled against the re-logged
`oracle/logs/02_walk_timing.csv` object columns. **Both resolve in favour of
this document**; the numbers below are read straight off slot `o00`.

**Cadence — 32 frames per cell stands.** The oracle's report gave 8
frames/cell, reading the duration as counting down in `$0200` steps. That is
the *party's* rate: the party runs at `FieldObj_Step_Offset = 1`, velocity
`$0200`. An object's duration decrements by `$80` a frame — frames 6901-6915
show `ydur` going 3712, 3584, 3456, 3328 … exactly 128 apart — and the step
from frame 6963 (`ydur $0F80`) to frame 6994 (`ydur 0`) spans **32 frames**,
over which `y` advances 240 → 256, one cell. Position corroborates the
duration independently: `y` gains a pixel every *other* frame, 0.5 px/frame.

**Decision schedule — a roll fires on the frame the timer reads zero.** The
oracle is right that this is per-zero-frame rather than an expiry edge, and the
implementation already matched: `subq.w #1` then `bpl` means the roll happens
when the *pre-decrement* timer is 0, so a reload of 0 rolls again the next
frame. Frames 6940-6963 show the countdown 22, 21, … 1, 0 and then the reload
to 10 on the very next frame. Two details the log confirms for free:

- The reload was **10**, and `10 & 7 = 2` → the remap table's index 2 → *down*
  — and `y` duly increases. The one-roll-two-uses finding and the remap table
  are both confirmed from hardware, not just from the instruction stream.
- The timer is **untouched for the whole step** (holds 10 across frames
  6963-6994) and the arriving frame does no timer work either; 6995 is the
  first frame to decrement it again. That is the `beq`/`bpl` guard on the two
  durations doing its job.

**A third thing the object columns settled**: `NPCAlysPiata` (slot `o07`,
id `$8068`) holds `timer 0` for all 1,563 field frames and consumes nothing.
A timer of zero is not sufficient to roll — the object's *routine* has to be
`GetRandomMove`. Registering the wrong types would consume rolls the cartridge
does not and desynchronise the shared seed for everything else.

## Cadence

`d7 = 0` selects the first speed table (`$04A20E`). Its entries give a step
duration of `$1000` (16.0 px in 8.8 fixed point) and a velocity of `$80`
(0.5 px/frame), so **a wandering NPC takes 32 frames per cell** — four times
slower than the party's 8. Confirmed on hardware; see the corrections above.
The extraction and core now carry all three records:

| selector | ROM range | velocity | frames/cell | packed random callers |
|---:|---|---:|---:|---|
| 0 | `$04A20E..$04A266` | `$80` | 32 | all packed random families |
| 1 | `$04A266..$04A2BE` | `$100` | 16 | no packed random placement; `loc_4B4B4` uses it |
| 2 | `$04A2BE..$04A316` | `$200` | 8 | extracted and executable, no random caller in the pack |

The third record is no longer an open extraction gap. `psiv_tools` already
emitted it in `runtime-pack/npc_commands.json`; `WanderSpeed::Selector2` and
the speed-table test pin its `$200`/8-frame contents. The random callers in
this slice all pass `d7=0`, except the uninstantiated `loc_4B4B4` probe at
`ps4.asm:99059`, which passes `d7=1`.

## Interaction while moving

- **A mid-step NPC can still be talked to.** `Interaction_ChkObjects` reads
  `curr_x_pos`/`curr_y_pos` in pixels and extrapolates by the object's
  remaining step duration, so it finds the object at its *destination*, not its
  spawn. Our engine's cell-based probe therefore needs a mid-step NPC to occupy
  its destination cell for talk purposes.
- **A wanderer does not path around the party** — it simply refuses the move
  and turns, per gate 3. There is no re-routing.
- **Nothing re-faces an NPC after a conversation** in the wander path itself;
  turn-to-face is the interaction code's doing, and the next accepted wander
  command overwrites the facing.

## The visibility gate, measured (2026-08-15)

An off-screen object is not updated **at all** — no timer decrement, no roll, no
movement. This is not a rendering optimisation we may skip: because the roll
comes from the shared `UpdateRNGSeed` stream, whether an object is on screen
decides how many times that stream advances, so the gate is load-bearing for
every other consumer of the seed.

`FieldObj_OnScreenTest` (`ps4.asm:96661`) tests the sprite-space position, where
`$80` is screen pixel 0 (`FieldObj_CalcSpritePos`, `ps4.asm:89801`, computes
`sprite = obj + $80 - camera - camera_step_counter`):

| axis | on-screen range | in screen pixels |
| --- | --- | --- |
| x | `$60 ..= $1E0` | `-32 ..= 352` |
| y | `$60 ..= $180` | `-32 ..= 256` |

That is the 320x224 view plus a 32-pixel margin on all four sides. A sprite
coordinate of exactly 0 short-circuits to on-screen on either axis.

Tape 02 confirms the gate is real and that the box is right, and it isolates
the camera as the one remaining unknown. Of the eight objects on the academy
map, slots 3-6 (x 256-352, far to the left) never update in 1080 frames, slot 0
updates from the first frame, and slots 1 and 2 wake mid-tape as the leader
walks north:

| slot | object at | wakes (cartridge) | wakes (engine) |
| --- | --- | --- | --- |
| 0 | (736, 240) | 6940 | 6940 |
| 1 | (752, 144) | 7567 | 7558 |
| 2 | (639, 128) | 7606 | 7597 |

The wake-ups were **nine frames early** in the engine, which modelled the camera
as instantaneously centred on the leader. The real camera is a threshold latch
that holds the driver at (152, 88) from the top-left of the view rather than at
its centre — sixteen pixels of difference, which is eight frames at the walking
rate of 2 px/frame. `docs/CAMERA.md` has the full transcription. With that
camera implemented, all three wake frames reproduce exactly.

### A correction, and the trap it came from (2026-08-15)

An earlier revision of this section claimed slot 2's wake at 7606 could not be a
camera event, on the grounds that the leader had been parked since 7578 and a
stationary driver cannot scroll a threshold-latched camera. The reasoning was
sound; the premise was not. Slot 2's wake is not a visibility event at all — the
object was **mid-step**, and `FieldObj_GetRandomMove` skips the timer decrement
entirely while a step duration is running:

```
move.w  x_step_duration(a4), d1
or.w    y_step_duration(a4), d1
beq.s   loc_49B6C          ; both zero -> decrement the timer
bpl.s   loc_49B88          ; still stepping -> skip it
```

Its `xdur` counts `256, 128, 0` across frames 7603-7605 and the timer starts
moving at 7606, the first frame after the step lands. The engine reproduces all
of it.

The trap worth remembering: the "nothing else in the log changed at 7606"
observation that started the whole investigation came from diffing against a
*stale engine CSV* rather than the oracle's own columns, and `o02_xdur` had in
fact been changing the whole time. A frozen-looking column is only evidence of a
freeze if every column that would move is checked in the same source.

## What this leaves open
- The initial `timer` value at spawn. `timer` is the word at `$1C`
  (`ps4.constants.asm:96`); the init block's `move.l d0, $28(a4)` clears the
  two duration words and does not touch it, so the timer's value at spawn is
  whatever the object slot's block-clear left — zero on a fresh map load, which
  makes an NPC roll on its first on-screen frame.
- Runtime event-state side effects after Xanafalgue crosses `$100`: the
  movement and object-slot clear are modelled, but `TempEveFlag_Xanafalgue`
  remains in the event-state lane and is not folded into save serialization.
- Visual animation state for `StaticAnimation` and `PresentationOnly` actors.
  Their movement/RNG ownership is closed here; sprite art, frame tables, and
  absolute-pixel presentation remain in the visual lane by scope.

## Closed parity receipts

- **Random-family transcription:** `FieldObj_GetRandomMove` (`ps4.asm:96693`),
  `GetRandomMove2` (`96711`), `GetRandomMove3` (`96730`), the remap at
  `loc_4A150` (`97241`), and the shared move gates at `FieldObj_NPCMove`
  (`97257`), `loc_4A3B6` (`97532`), `loc_45CA4` (`91098`) and `loc_4A316`
  (`97461`). Tape-backed test: `oracle/tapes/02_walk_timing.tape`; the
  deterministic profile test also exercises all ten packed families in source
  order on that tape's parsed frame stream. The Xanafalgue escape receipt is
  `oracle/tapes/18_flag_round_trip.tape`, whose existing oracle path already
  covers the temporary-flag/object-slot transition.
- **Wave-4 static-family receipt:** the same tape-02 replay compares all 32
  object slots, including the eight live academy objects, after inheriting the
  retail object state at `settle`; the regenerated camera-group oracle and
  replay are clean across the 1080-frame field segment.
- **Bespoke completion receipt:** `oracle/tapes/30_bespoke_piata.tape` reaches
  map `$0010` (Piata), walks to the two `NPCType6` guards, and holds them on
  screen. Its oracle log is `oracle/logs/30_bespoke_piata.csv`; replay from
  mark `bespoke_guards` with the inherited map/seed/camera/object state reports
  **CLEAN: 120 frames, 350 columns compared, zero divergences**. The guards'
  facing writes and zero-RNG fixed/follow behavior are therefore covered in
  cartridge order, not just by a unit test.
- **Census closure:** the live `runtime-pack` count is 949 placements across
  138 symbols: 332 random, 374 wave-4 static, and 243 wave-8 bespoke. The
  post-wave-7 remainder is zero; `BespokeKind` is explicit for all 121
  symbols, including the scene/presentation stays and their structural
  reasons above.
- **Post-entry gate receipt:** `oracle/states/refresh_map_camera_gate_receipt.json`
  records the `loc_51AB2` contract. There are two call sites for the loader:
  ordinary `GameMode_LoadFieldMap` (`ps4.asm:107556`) and `RefreshMap`
  (`121797`); the latter is reached by scene/warp-time refreshes. No separate
  literal EC25/EC26 writer was found. `Camera::apply_gate_write` and
  `Runtime::refresh_map_camera_gates` preserve the zero-gate counter loads and
  nonzero-gate counter preservation.
- **Speed table:** `loc_4A202` pointer table and records at `$04A20E`,
  `$04A266`, `$04A2BE`, ending `$04A316`; tape-independent extraction receipt
  is `runtime-pack/npc_commands.json`, whose selector-2 SHA is pinned by the
  existing `tests/test_npc_commands.py` suite.
- **Grand Cross provenance:** `reference/ps4disasm/ps4.options.asm:10`,
  `grand_cross = 1`; no `grand_cross=0` branch is used to justify the common
  field routines above.
