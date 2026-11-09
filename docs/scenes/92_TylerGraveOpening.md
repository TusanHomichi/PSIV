# `Event_TylerGraveOpening`

- **Retail bytes:** `$06FC94..$06FE1B` inclusive, 392 bytes.
- **Pointer:** `EventPtrs[$44]` at `$05A2B4` (`$06FC94`; the next slot, `$45`, is `$06FE1C`); event `$0044`.
- **Trigger:** dialogue `$F6 $00 $44` in `DialogueTree14` entry 35, the
  inscription on Tyler's grave (map `$120`). Not a
  `RunEventsJmpTbl` event: the census finds it as a dialogue source
  ([EVENT_COVERAGE](EVENT_COVERAGE.md)).
- **Data:** `dezolis_route.rs`, `TYLER_GRAVE_OPENING` (21 ops), `GRAVE_DRIFT`.
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
| 4-5 | `$06FCBE..$06FCE0` | `Event_MoveCamera` (`$5AAEE`) to `Character_1` (x + 4, y) then (x, y), speed 4 (`addq.w #4, d0` / call / `subq.w #4, d0` / call) | `PanCamera` `ActorShifted` +4, then `PanCamera` `Actor`; each blocks until the camera arrives |
| 6 | `$06FCE4..$06FCFE` | `GetMapLayoutOffset(d1=$A, d2=$C, d3=1)` (BG), `move.b #$47, (a1)`, `RefreshPlaneBG`, `DMAPlane_B_VInt` | `WriteMapChunks` `(10,12) <- $47` |
| 7 | `$06FD04..$06FD8A` | twelve `move.l d0, $20(a4)` step constants, then `moveq #$3F, d0 / jsr DoMainUpdatesLoop` | `DriftNpcs` (64 frames) |
| 8-9 | `$06FD90..$06FDB2` | `move.l #0, $20(a4)` on the twelve; `$6(a4) = 8` on `$C380`, `$C` on `$C3C0` | `Face` objects 2 (right) and 3 (left) |
| 10-12 | `$06FDB8..$06FDE0` | sound `$DD`, the same camera pair again | `PlaySound`, the two `PanCamera` |
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
and collision follow the drift frame by frame. The scene's own actor for each
object takes the same position as its cell and its offset into it
(`ScriptedActor::place_at_pixel`, the `slide` `Event_StepObject` leaves too),
and the renderer draws a scene actor at that cell plus offset, so the stone
halves glide a pixel at a time. Before #163 the actor took the cell alone and
jumped a cell at a time
(`a_drifting_actor_is_drawn_at_its_pixel_every_frame`,
`rust/psiv-core/src/scene_runner/drift_tests.rs`).

### Slab animation and camera timing

`$06FD0E..$06FD18` writes selector word `$10` and duration byte 4 on
`$C380`; `$06FD50..$06FD5A` writes `$14` and 4 on `$C3C0`. The US bytes
at `$04DF1C` form the six-entry sequence pointer table used by
`FieldObj_GravestoneHalf` (`$04DDAE`). Entries `$10/$14` point to
`$04E5D0/$04E5E6`: both begin `84 04 04 04 34`, then padding and four
mapping pointers. Left: `$04F844,$04F884,$04F8DC,$04F928`; right:
`$04F864,$04F8B0,$04F902,$04F954`.

`FieldObj_Animate2` (`$04472E`, `loc_4477E` for per-frame durations)
subtracts one from the countdown byte, advances `mappings_idx` only when
that byte is signed-negative, and reloads the new index's duration. From
initial index 0/countdown 4, updates 1..4 draw mapping 0, 5..9 mapping 1,
10..14 mapping 2 and 15..64 mapping 3. The selector then becomes 8/$C;
these point at the final single-frame mappings. `SetActorAnimation` carries
the literal selector/countdown, `DriftNpcs` advances it on object updates,
and `Face` replaces it. Both pack builders include the two scripted selectors
as `walk_facing_0x10/14`; Godot selects the runtime's mapping index.

Both camera pairs use `PanCamera`: `Event_MoveCamera` (`$05AAEE`) subtracts
`$98/$58`, pans X before Y, and blocks at speed 4 until each target is reached.
`CameraSubject::ActorShifted` retains the leader's 4 px lead. From a camera
already framing the leader, each outward/return pan consumes one frame.

The reproducible [oracle recipe](../../oracle/CAPTURE.md#tylers-grave)
uses a constructed Tyler field fixture with flag `$84` already set, so the
inscription dialogue is skipped; it is not a campaign-route proof. The
frame-boundary trace records lead/back at 7200/7201, first/last slab updates
at 7204/7267, selectors 8/$C and second lead at 7268, back at 7269 and field
routine return at 7330. `oracle/tyler_grave.py` verifies that 131-frame retail
envelope and the mapping-index chain. The runtime regression checks the
four one-frame pans, 64 consecutive object updates and 60-frame map-update
hold. Map-refresh CPU/DMA overhead between 7201 and 7204 and the event-dispatch
return boundary are outside that runtime timing claim; fixed artificial waits
are not substituted for those unmodelled boundaries.

`InvisibleBlock` skips `FieldObj_UpdatePosition` while off screen. The leader
stands at the grave, so all twelve are on screen; the op does not model the
off-screen skip.

## New ops

- **`SetActorAnimation { actor, animation }`**: object sequence
  selector and countdown writes before a `DriftNpcs` loop. The runner owns
  advancement; Godot consumes its current mapping index.

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

## Pack and route

- **The stair chunk needs an atlas.** `WriteMapChunks` resolves against the current map's
  patch atlas, and Tyler's carried no `$47`: the first run halted with
  `MapRefreshFailed("scene chunk atlas is absent")` (the #67 class).
  `psiv_tools.map_patches.SCENE_CHUNK_WRITES` is now the one cited table of scene chunk writes
  (this scene: `move.b #$47, (a1)` at `$06FCF4`, pinned by its bytes), and
  `tests/test_scene_chunk_atlas.py` fails any registry write the pack's atlas cannot resolve. A pack
  built before the row existed must be rebuilt.
- **Reach.** `DialogueTree14` entry 29 (the grave's middle blocks, objects 5 and 6) is
  `FA 81,6`: with `$81` set it skips to entry 35, whose `$F6 $44` starts this scene, and `$81` is
  only set by [`Event_Gyuna`](98_Gyuna.md). The route chapters `dezolis-gyuna` and
  `dezolis-tyler-grave` play both with pads, then walk warp 7 (cells (20..21, 24..25)) to the
  Hangar (`$15F`).
- **Deviation from the brief's wording (recorded):** the stop byte `Event_Gyuna` needs reaches the
  runner through `SceneRunner::set_dialogue_stop_byte`, set by the runtime when the dialogue closes,
  not on the `SceneInput` itself, so no existing `SceneInput` match changed.
