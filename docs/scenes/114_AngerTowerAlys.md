# `Event_AngerTowerAlys`

- **Retail bytes:** `$07148A..$071579` inclusive, 240 bytes (the routine to the `rts` at `$071570`, then the direction table `loc_71572`).
- **Pointer:** `EventPtrs[$62]` at `$05A2B4`; event `$0062`.
- **Trigger:** Dialogue `$F6 $00 $62`, `DialogueTree41` entry 15: the illusion of Alys on Anger Tower F2 (`$FE`), where `Event_AngerTowerTop` ([85](85_AngerTowerTop.md)) leaves Chaz alone. An `Event_GetAndRunDialogue2` caller (`jsr $5ACDC` at `$071538`, row 11 of [the Dialogue2 audit](DIALOGUE2_CALLERS.md)).
- **Data:** `dezo_endgame.rs`, `ANGER_TOWER_ALYS` (11 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot and every
instruction word matches `ps4.asm:150697-150763` (body to the `rts` at
150760, table 150761-150763); the table bytes at `$071572`
are `00 01 00 FF 01 00 FF 00`, the clone's `loc_71572` rows. No `revision`
conditional, no `DialogueTreesToRAM`: both lines are entries of the map's tree.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$07148A..$071491` | `moveq #7, d0` / `jsr Event_GetAndRunDialogue` | entry 7, standard window |
| 1 | `$071492..$0714A7` | `lea Field_Obj_Secondary, a4` / `lea Character_1, a3` / `move.w $6(a3), d0` / `bchg #2, d0` / `jsr Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 2 | `$0714A8..$0714D5` | object 0: `move.w $6(a4), d2` / `lsr.w #1, d2` / the table's two bytes, `ext.w`, `swap` / `move.w #7, d2` / `jsr Event_StepObject` | `StepObjectAlongFacing` object 0, 8 frames |
| 3 | `$0714D6..$071507` | the same for `Character_1` with `neg.w d0` / `neg.w d1` and `move.w #$F, d2` | `StepObjectAlongFacing` leader, backwards, 16 frames |
| 4 | `$071508..$071535` | object 0 again, `#7` | `StepObjectAlongFacing` object 0, 8 frames |
| 5 | `$071536..$07153D` | `moveq #8, d0` / `jsr Event_GetAndRunDialogue2` | `retained(8)` |
| 6 | `$07153E..$071547` | `move.b #EventFlag_AlysFight ($E4), d0` / `jsr EventFlags_Set` | `SetFlag` `$E4` |
| 7-8 | `$071548..$07155D` | `move.w #$1F0, $30(a4)` / `move.w #$1F0, $34(a4)` / `move.w #0, $6(a4)` on `Character_1` | `PlaceActor` leader at `$1F0,$1F0`; `Face` down |
| 9 | `$07155E..$071563` | `bset #3, (Map_Load_Flags).w` | `SetMapLoadFlags` `0x08` |
| 10 | `$071564..$071570` | `move.b #$18, (Event_Battle_Index).w` / `bset #3, Routine_Exit_Flags` / `rts` | `StartBattle` `$18` |

**The steps.** `Event_StepObject` (`$5A99E`, `ps4.asm:121203`) writes `d0`/`d1`
to the object's step constants and loops `d2 + 1` times: `FieldObj_UpdatePosition`
(the step added to `curr_x_pos`/`curr_y_pos`), the sprite and animation calls,
`RunMapUpdates` (`$54938`) and `VInt_Prepare`, then clears the constants. The
table's bytes become whole pixels per frame after `ext.w` and `swap`, and the
index is the stepped object's own `facing_dir` halved, so each step runs along
that object's facing: down `(0,1)`, up `(0,-1)`, right `(1,0)`, left `(-1,0)`,
which is `Direction::delta`. Object 0 faces the leader after op 1, so it walks
eight pixels toward him; he backs sixteen pixels away (`neg.w`); it walks eight
more. It ends on his first cell and he one cell back, whichever side he talked
from.

`SceneOp::StepObjectAlongFacing` is the op: the runner reads the facing as the
op starts, moves the actor one step per frame as a sub-cell slide that rolls
into the cell (`ScriptedActor::slide_by`, the platform ride's owner too), and
reports a map object's position each frame as a drift does
(`NpcPixelPosition`). The leader is the scene's party actor, which the field
camera and the native party sprite both follow by pixel. The frames run map
updates, as the loop does.

Op 8 is a raw facing write (`move.w #0, $6(a4)`), which the port records as
`Face`; the cartridge skips `Event_UpdateObjFacing`'s sprite refresh there, and
the battle follows at once. The deviation is recorded in the
[source notes](../source-notes/README.md#other-classified-records-in-the-chronology).

## Tests

`rust/psiv-core/tests/late_dezolis_scenes.rs`: the table as cartridge literals
against `Direction::delta`; the 8/16/8 path pixel by pixel for all four facings,
ending on the cells above; the leader's sixteen frames one pixel each; and the
lines, `$E4`, the placement at cell `(31,32)`, facing down and battle `$18`, in
order. Negative controls, run by mutating `drift.rs::start_step`: with the facing
ignored (always down) or `backwards` ignored, the path test fails.
