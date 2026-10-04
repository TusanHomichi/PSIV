# `Event_TylerGraveOpening`

- **Retail bytes:** `$06FC94..$06FE1B` inclusive, 392 bytes.
- **Pointer:** `EventPtrs[$44]` at `$05A2B4` (`$06FC94`; the next slot, `$45`, is `$06FE1C`); event `$0044`.
- **Trigger:** dialogue `$F6 $00 $44` in `DialogueTree14` entry 35, the
  inscription on Tyler's grave (map `$120`). Not a
  `RunEventsJmpTbl` event: the census finds it as a dialogue source
  ([EVENT_COVERAGE](EVENT_COVERAGE.md)).
- **Data:** `dezolis_route.rs`, `TYLER_GRAVE_OPENING` (19 ops), `GRAVE_DRIFT`.
- **New ops:** `BranchIfActorCoord`, `DriftNpcs` (below).

The route needs it: `RunEvent_FindingLandale` (`$2B`, Hangar) requires
`EventFlag_TylerGrave` `$84`, which only this scene sets, and the stair chunk it
writes is the way down to the Hangar.

## Clone audit

`ps4.asm:149245-149339` is the body (the clone adds no `revision`/Grand Cross
branch here). The bytes were read from the image; the clone's labels are
navigation only.

## Retail transcription

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FC94..$06FC9E` | `lea $C380.w, a4` / `cmpi.w #$120, $30(a4)` / `bne.w loc_6FE12` | `BranchIfActorCoord` object 2 X `Equal` `$120` |
| 1-2 | `$06FCA2..$06FCB0` | `EventFlags_Test $84`, `bne`; else `moveq #$1E, d0 / jsr Event_GetAndRunDialogue` | `BranchFlag`, standard `RunDialogue` `$1E` |
| 3 | `$06FCB6` | `move.b #$DD, Sound_Index` (`SFXID_GraveOpening`) | `PlaySound` |
| 4-5 | `$06FCBE..$06FCE0` | `Event_MoveCamera` (`$5AAEE`) to `Character_1` (x + 4, y) then (x, y), speed 4 | `CameraToActor` twice |
| 6 | `$06FCE4..$06FCFE` | `GetMapLayoutOffset(d1=$A, d2=$C, d3=1)` (BG), `move.b #$47, (a1)`, `RefreshPlaneBG`, `DMAPlane_B_VInt` | `WriteMapChunks` `(10,12) <- $47` |
| 7 | `$06FD04..$06FD8A` | twelve `move.l d0, $20(a4)` step constants, then `moveq #$3F, d0 / jsr DoMainUpdatesLoop` | `DriftNpcs` (64 frames) |
| 8-9 | `$06FD90..$06FDB2` | `move.l #0, $20(a4)` on the twelve; `$6(a4) = 8` on `$C380`, `$C` on `$C3C0` | `Face` objects 2 (right) and 3 (left) |
| 10-12 | `$06FDB8..$06FDE0` | sound `$DD`, camera pans again | `PlaySound`, `CameraToActor` twice |
| 13 | `$06FDE6` | `moveq #$3B, d0 / jsr DoMapUpdateLoop` | `Wait` 60 |
| 14-15 | `$06FDEE..$06FE02` | `EventFlags_Test $84`, `bne`; else `popdlg / Event_RunDialogue` | `BranchFlag`, `RunDialogueResume` |
| 16-17 | `$06FE08..$06FE0C` | `move.b #$84, d0 / jmp EventFlags_Set` | `SetFlag` `$84`, `End` |
| 18 | `$06FE12..$06FE16` | `moveq #$22, d0 / jmp Event_GetAndRunDialogue` | standard `RunDialogue` `$22` |

`DialogueTree14` entry `$1E` is the grave scene's text ("So this is Tyler's
grave..."), yielding at one `$F7`; the part after it is what `popdlg /
Event_RunDialogue` plays once the plate has moved. Entry `$22` ("Wow, to think
something so big could move...") is what a second interaction reads once the
stone has moved.

### The guard

Object `$C380` is map object 2 on Tyler (`$C300 + 2 * $40`): the left
gravestone half, `FieldObj_GravestoneHalf`, at x `$120`. After the slide it is no
longer at `$120`, so the first `cmpi.w` is what makes the scene play once per
visit. A reloaded Tyler puts the stones back, so the slide replays while the
flag keeps the first dialogue and the resume out.

### The slab slide

The scene writes a longword to `x_step_constant` (`$20`) of twelve objects:

| Constant | `move.l d0, $20(a4)` | Objects (RAM, map object index) |
|---|---|---|
| `$FFFFC000` (-0.25 px/frame) | `$06FD04` | `$C380` (2), `$C400` (4), `$C440` (5), `$C500` (8), `$C540` (9), `$C580` (10) |
| `$00008000` (+0.5 px/frame) | `$06FD46` | `$C3C0` (3), `$C480` (6), `$C4C0` (7), `$C5C0` (11), `$C600` (12), `$C640` (13) |

Each frame of `DoMainUpdatesLoop` (`$5A73C`: `Field_UpdateObjects` `$447D8`,
`RunMapUpdates`, `VInt_Prepare`) runs every object's routine. Object 2 and 3 are
`FieldObj_GravestoneHalf` (`$04DDAE`: `jsr $4501C` every frame); 4..13 are
`FieldObj_InvisibleBlock` (`$0488D8`: `bsr.w $49B22`, the on-screen test, then
`bsr.w $4501C` while on screen). `$4501C` is `FieldObj_UpdatePosition`:
`move.l $20(a4), d1 / add.l d1, $30(a4) / move.l $24(a4), d1 / add.l d1,
$34(a4)`, a 16.16 add, so the integer pixel (the word at `$30`) moves
`floor((x * 65536 + n * step) / 65536)`. Over `$3F + 1 = 64` frames that is
16 px left and 32 px right. Objects 4..13 are the grave's collision: they start
on cells (18..22, 24..25) and end with cells (20..21, 24..25) empty, which is
the footprint of the stair chunk `(10,12)` (BG chunk 10 x 2 cells, row 12 x 2).

`DriftNpcs` applies exactly that per frame to the named map objects and reports
each whole-pixel position (`PresentationOp::NpcPixelPosition`); the runtime
writes it to the field map (`FieldMap::set_npc_pixel_position`), so occupancy
and collision follow the drift frame by frame and the renderer draws the
positions from the runtime.

**Not modelled (presentation only, no state):**

- `$6(a4)` = `$10` on `$C380` and `$14` on `$C3C0`, and `mappings_duration` 4,
  during the slide: the stone halves' animation words.
- The camera's 4 px lead (`x + 4` then `x`): the camera glide ends at the leader
  either way, and `CameraToActor` carries no offset.
- `Event_MoveCamera` blocks until the glide ends; the port's camera ops do not
  block (the same as every other `CameraToActor` use).
- `InvisibleBlock` skips `FieldObj_UpdatePosition` while off screen. The leader
  stands at the grave, so all twelve are on screen; the op does not model the
  off-screen skip.

## New ops

- **`BranchIfActorCoord { actor, axis, cmp, value, if_true, if_false }`**: the
  `cmpi.w #value, $30/$34(a4)` / `Bcc` pair. `CoordCmp` is the unsigned-word
  branch family (`beq`, `bne`, `bcs`, `bls`, `bhi`, `bcc`); this scene uses
  `bne.w` at `$06FC9E`. The coordinate is a pixel, read from the live map for
  objects.
- **`DriftNpcs { drifts, frames }`**: `DoMainUpdatesLoop` over objects with
  step constants, citing `FieldObj_UpdatePosition` `$04501C` and the two object
  routines above.

Tests: `rust/psiv-core/src/scene_runner/drift_tests.rs` (every compare kind, both
branch arms, per-frame positions, a wrong step constant missing the cartridge's
end, the loop's frame count) and `rust/psiv-core/tests/dezolis_scenes.rs` (each
object's end position, an intermediate frame, the stair cells blocked before and
open after, the frame a cell frees, the dialogue and flag paths, the tick
count).
