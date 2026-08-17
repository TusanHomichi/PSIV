# The field camera

Transcribed 2026-08-16 from `reference/ps4disasm/ps4.asm`. Every claim here
carries the routine and line it came from; where the cartridge states a number
twice in independent places, both are cited, because that is the cheapest
available proof that a transcription is right.

The source build is pinned: `reference/ps4disasm/ps4.options.asm:10` sets
`grand_cross = 1`. The camera routines at `ps4.asm:89548-90375`, the sprite
routine at `89801`, and the map-load/placement paths at `107749` and `111050`
contain no `grand_cross` conditional. The `grand_cross=0` branches elsewhere
in the clone are therefore not substituted into this field-camera proof; the
closed path is the common instruction stream of the checked-in Grand Cross
build.

## Why this is not a rendering concern

The camera decides which field objects are on screen. `FieldObj_OnScreenTest`
freezes every off-screen object *completely* — no movement, no step-duration
countdown, no timer decrement, no animation — and a frozen wanderer never
reaches `FieldObj_GetRandomMove`, so it never calls `UpdateRNGSeed`.

The seed is shared with encounter rolls and everything else that draws from it.
So the camera position feeds back into the random stream through the rolls that
off-screen objects do not make. A camera that is wrong by sixteen pixels wakes
objects at the wrong frame, consumes the wrong number of rolls, and desynchronises
the whole simulation. This is why the camera lives in `psiv-core` and not in the
renderer.

Measured consequence on tape 02: a leader-centred placeholder camera woke slot 1
nine frames early, and slot 0 — visible the whole time and correct until then —
diverged at its next roll, 64 frames later. With the camera below, the same tape
runs 937 frames of field play with zero divergences across all 303 compared
columns, objects included.

## Fixed point

Positions are 16.16 longwords, not integers. `curr_x_pos` (`$30`) and
`curr_y_pos` (`$34`) on a field object, `Camera_X_Pos_FG` (`$FFFFEF94`) and
`Camera_Y_Pos_FG` (`$FFFFEF90`) for the camera. A `move.w` at those offsets
reads the integer pixel, because the high word comes first on a big-endian
machine — that idiom appears throughout the camera code and is easy to misread
as a 16-bit position.

Velocities are the same format: `x_step_constant` (`$20`) and `y_step_constant`
(`$24`). `FieldObj_UpdatePosition` (`ps4.asm:89791`) is exactly
`curr_pos += step_constant`. The party walks at `$00020000` (2 px/frame);
wanderers move at `$00008000` (half a pixel), which is the reason the low word
cannot be dropped.

## Sprite space

`FieldObj_CalcSpritePos` (`ps4.asm:89801`):

```
sprite = curr_pos + $800000 - Camera_Pos - Camera_Step_Counter   (high word)
```

So `$80` is screen pixel 0. The routine subtracts both the position and the
step counter, but it runs *before* the commit that folds the step into the
position — so `Pos_before + StepCounter` is the same number as the committed
position, and an engine that keeps the committed position must not subtract the
step a second time.

The object bit is a separate branch: `btst #0,$2(a4)` followed by `bne` skips
both camera subtractions for that object. It does **not** select BG. The global
`$FFFFEC24` byte selects FG (`0`) or BG (`nonzero`) for ordinary objects. The
additive pack schema now carries `camera_bypass` on each NPC placement and the
core visibility gate honors it. The Grand Cross scan found no ordinary field
init that sets bit 0, so every current placement is explicitly false with the
loader-zero provenance retained in `sprites/npcs.json`; a later dynamic writer
will remain visible as a routine-level change rather than being flattened.

## The BG path and driver gates

The BG routines are parallel transcriptions, not a presentation approximation:

| operation | FG routine | BG routine | driver gate |
|---|---|---|---|
| X latch | `FieldObj_CameraXPos_FG`, `ps4.asm:89548` | `FieldObj_CameraXPos_BG`, `89628` | `$FFFFEC25` / `$FFFFEC26` |
| Y latch | `FieldObj_CameraYPos_FG`, `ps4.asm:89590` | `FieldObj_CameraYPos_BG`, `89670` | `$FFFFEC25` / `$FFFFEC26` |
| X commit | `UpdateCameraXPosFG`, `ps4.asm:90288` | `UpdateCameraXPosBG`, `90346` | — |
| Y commit | `UpdateCameraYPosFG`, `ps4.asm:90317` | `UpdateCameraYPosBG`, `90375` | — |
| sprite subtraction | `Camera_*_Pos_FG` | `Camera_*_Pos_BG` | global `$FFFFEC24` |

Both latches read the same driver's last-frame sprite coordinates (`$2C/$2E`)
and the same velocity source, but they write independent positions and step
counters. `psiv-core::Camera` carries both planes, `CameraGates` carries
`EC24/EC25/EC26`, and `Camera::tick` clears a disabled plane's step before the
commit. The bridge now consumes the map record's `scroll` section, preserving
the record-specific values rather than forcing `1/1/1`. The census is 278 maps
at `1/1/1`, 65 at `0/1/1`, and 18 at `0/1/0`; all currently decoded initial
step counters are zero. Those values come directly from `loc_51AB2`, not from
a scene or object side channel.

The gate bytes are read during map setup at `loc_51AB2` (`ps4.asm:107749`):
`EC24` selects the sprite plane; each driver gate selects whether its plane
latches and its step counters are refreshed. Placement at `loc_53854`
(`ps4.asm:111050`) initializes both plane positions when their gates are set.
The runtime's replay rows compare FG and BG pixel positions, both step
counters, the raw 16.16 positions, and all three logged gate columns.

## The follow rule: a one-sided latch

`FieldObj_CameraYPos_FG` (`ps4.asm:89590`) and `FieldObj_CameraXPos_FG`
(`ps4.asm:89548`):

```
if (Field_Map_Index & $FFFE) == 0 -> the wrapping variant (loc_44F80 / loc_44F4C)
if ($FFFFEC25).b == 0 -> rts                  ; this object does not drive the camera
clr.l   Camera_Y_Step_Counter_FG              ; default: do not scroll at all
if y_step_constant <  0:                      ; moving up
    (map top-edge check, which stops the *object*)
    if sprite_y <= $D8: Camera_Y_Step_Counter_FG = y_step_constant
else:                                         ; moving down, or standing still
    (map bottom-edge check)
    if sprite_y >= $D8: Camera_Y_Step_Counter_FG = y_step_constant
```

**The camera does not re-centre on the party.** It moves at exactly the driver's
own velocity, and only while the driver is past the threshold in the direction
it is travelling. There is no restoring force and no easing. Three consequences
follow, and all three are observable:

1. While the camera is following, the driver's *screen* position is frozen
   wherever it happens to be. It is not pinned to the centre of the view.
2. When the driver is on the near side of the threshold, the camera does not
   move at all and the driver walks across a still screen.
3. When the driver stops, the camera stops **dead** on the same frame, because
   the scroll is the driver's velocity and that velocity is now zero. There is
   no glide-out.

No constant offset from the leader reproduces this, which is what makes a
leader-centred model wrong rather than merely imprecise.

### The two thresholds, stated twice by the ROM

| axis | threshold | screen pixel | `Event_MoveCamera` |
| --- | --- | --- | --- |
| x | `$118` | 152 | subtracts `$98` = 152 |
| y | `$D8` | 216 - 128 = 88 | subtracts `$58` = 88 |

`Event_MoveCamera` (`ps4.asm:121468`) converts a subject position into a camera
position by subtracting `$98` and `$58`. Those are the same two numbers the
latch thresholds imply once the `$80` sprite origin is removed, derived from
completely different code. The driver's home offset is (152, 88) from the
top-left of the view — note it is *not* the centre of a 320x224 screen, which
would be (160, 112).

## Commit, clamp, and wrap

`UpdateCameraYPosFG` (`ps4.asm:90317`), and it runs **after** the latch within a
frame:

```
step = latch(this frame's velocity, last frame's sprite position)
Camera_Pos += step
```

This ordering was measured, not read. `Event_MoveCamera`'s inner sequence puts
`UpdateCameraXPosFG` before `Field_UpdateObjects`, and transcribing that order
gave a camera one frame behind the cartridge's — visible as a two-pixel lag in
`cam_y_fg_px` the moment the party started walking. But `Event_MoveCamera` is a
*scene* helper driving its own loop, not the field one. The decisive evidence is
oracle-lane's camera columns: `leader - camera_FG` is exactly (152, 88) on
**every one** of tape 02's 1563 field-control frames — one distinct offset, no
lag anywhere — which only latch-then-commit produces.

The lesson generalises: an order read off one call site is a hypothesis, and a
logged RAM column settles it in one run.

```
Camera_Pos += Camera_Step_Counter
if Camera_Pos < 0:    Camera_Pos = 0;   Camera_Step_Counter = 0
if Camera_Pos > max:  Camera_Pos = max; Camera_Step_Counter = 0
```

The maximum is the map's pixel extent less the screen — `loc_45780` for y as
`(Map_Column_Size_FG + 1) * 32 - $E0`, `loc_45758` for x as
`(Map_Row_Size_FG + 1) * 32 - $140`, both shifted into 16.16. PiataAcademy_F1
(1024x512 px) therefore clamps to x in [0, 704] and y in [0, 288].

The clamp is re-applied every frame rather than latched once, so a driver that
keeps walking into a map edge never drags the view off the map.

**Torus maps** (`Field_Map_Index & $FFFE == 0`, the two overworlds) take
`loc_45640` / `loc_45686` instead, which wrap and never clamp: the routine adds
the screen size back onto the clamp maximum to recover the map's full extent,
then folds the position into `[0, extent)`. The map-edge checks that stop the
*object* are also absent from the wrapping latch variants.

Sprite positions on a torus need the seam too (`loc_4509C`, `ps4.asm:89860`).
The cartridge decides with one comparison — `lsr.w #1` on the camera, then
`cmp`/`bgt`:

```
obj = wrap(curr_pos)
if obj < camera / 2:  camera -= map_extent
sprite = obj + $80 - camera
```

Reading: if the object sits in the near half of the space behind the camera, it
is really *ahead* of the camera across the seam, so shift the camera back by a
full map before subtracting.

## The visibility test

**Visibility is not universal.** Whether an object freezes off screen is a
property of its object type, because `FieldObj_OnScreenTest` is a call each
routine makes or does not make. `FieldObj_NPCAlysPiata` (`$68`, `ps4.asm:92120`)
runs straight into `FieldObj_NPCMove`, `UpdateStepDuration`, `UpdatePosition`
and `CalcSpritePos` with no test at all — so its `offscreen_flag` keeps the
value its slot was initialised with, forever, however far off screen it drifts.
Tape 02 shows Alys's sprite x crossing below `$60` at frame 7303 with her flag
still reading 0.

87 of the 222 `FieldObjectsJmpTbl` entries call it. The generic townsfolk do —
`NPCType1` (`$38`) through `NPCType12` (`$64`), which covers both wanderers,
`NPCType2` (`$3C`) and `NPCType3` (`$40`). The party members and the named story
NPCs do not. `psiv-core`'s `type_tests_visibility` holds the table, derived
mechanically from each entry's routine body rather than by hand.

The test itself, `FieldObj_OnScreenTest` (`ps4.asm:96661`), in sprite
coordinates:

| axis | on-screen range | in screen pixels |
| --- | --- | --- |
| x | `$60 ..= $1E0` | -32 ..= 352 |
| y | `$60 ..= $180` | -32 ..= 256 |

The 320x224 view plus a 32-pixel margin on all four sides.

**A sprite coordinate of exactly 0 short-circuits to on-screen**, on either axis
independently: the routine tests `beq` before it tests the range at all. An
object whose sprite x lands on 0 is on screen no matter how far off its y is.
This is pinned by `a_sprite_coordinate_of_exactly_zero_short_circuits_to_on_screen`
in `psiv-core/src/camera.rs`.

## Frame order, and the latency it creates

Every field object's routine has the same shape — `FieldObj_NPCType1Main`
(`ps4.asm:95060`) is representative:

```
bsr FieldObj_OnScreenTest      ; tests the sprite position written LAST frame
bne +                          ; off-screen: skip all movement
  ...move, step duration, position...
+
bsr FieldObj_CalcSpritePos     ; ALWAYS runs, on-screen or not
tst.b offscreen_flag(a4)
bne +
  bsr FieldObj_Animate2        ; animation is gated too
+
```

`FieldObj_CalcSpritePos` runs unconditionally, so sprite positions are never
stale — but the gate decision is a frame behind by construction, and so is the
latch, which reads the driver's `sprite_y_pos` from the same place.

So a frame is:

1. objects: test visibility against last frame's sprite positions, and freeze
   the ones that fail — but only for the types that run the test at all
2. the driver latches this frame's scroll, from its velocity and its (one frame
   old) sprite position
3. the scroll is folded into the camera position, then clamped or wrapped
4. every object recomputes its sprite position against the new position

The engine reproduces this order in `Camera::tick` and in the field-object phase
of `Runtime::tick`, which deliberately runs before the camera's own tick.

## Map entry

Entering a map **places** the view; it does not scroll in. `Runtime` therefore
re-places the camera on every map load, framing the party wherever the warp
dropped them, with the map's edge rule applied.

For replays this makes the camera *inherited state*, exactly like the RNG seed:
at an alignment frame the retail camera holds whatever the opening scene left,
and the engine cannot execute that scene. `psiv-replay --camera <x,y>` exists to
supply it.

## Remaining camera debt

The three accumulated camera debts are now represented end to end:

- the additive pack schema carries per-object render-flags bit 0, and runtime
  visibility/sprite placement consumes it independently of interaction bit 3;
- map records carry the ordinary gate bytes and the initial fixed-point step
  counters, while replay exposes all three gate bytes; and
- replay rows expose the raw 16.16 FG/BG camera words, including their low
  words, instead of comparing only integer pixels.

### Post-entry gate writes

The disassembly has one gate-loader routine, `loc_51AB2`
(`ps4.asm:107749-107763`), and two callers: ordinary
`GameMode_LoadFieldMap` (`ps4.asm:107556`) and `RefreshMap`
(`ps4.asm:121797`). `RefreshMap` is the post-entry path reached by scene and
warp-time refreshes. It rereads the current map record; the scout found no
separate literal `EC25`/`EC26` writer outside `loc_51AB2`.

`Camera::apply_gate_write` implements the loader's exact rule: write all three
gate bytes, load the FG counters only when `EC25 == 0`, load the BG counters
only when `EC26 == 0`, and preserve an existing pair for a nonzero gate.
`Runtime::refresh_map_camera_gates` is the runtime seam for those post-entry
`RefreshMap` calls. The focused receipt is
`oracle/states/refresh_map_camera_gate_receipt.json`, and the core test is
`camera::tests::refresh_map_rewrites_gate_bytes_and_only_zero_gates_consume_counters`.

## Status

Tape 02 is the field receipt for the closed path: its 1080 frames cover the
FG and BG pixel columns, both raw 16.16 position pairs, both step-counter
pairs, all three gate bytes, and all 32 object slots. The focused core receipt
is `camera::tests::the_existing_field_tape_replays_both_camera_planes`. The
2026-08-17 rebuilt `psiv-replay` receipt is **CLEAN** over 350 compared
columns, with zero divergences, using the inherited map/seed/camera/object
state at `settle` (`--start-from-log --camera 616,200 --seed 0xCB5A53D3
--restore-objects`) against the regenerated `camera` oracle group in
`oracle/logs/field_parity_02.csv`. The current structure-only pack check is
4,518/4,518 files identical with zero differing files. That comparison is a
same-tree integrity check; the Python pack tests also pass their independent
temporary-build byte comparison, and no palette regeneration was performed
for this wave.
