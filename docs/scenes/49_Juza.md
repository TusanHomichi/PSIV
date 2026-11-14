# `Event_Juza`

- **Retail bytes:** `$06FB1E..$06FB5D` inclusive, 64 bytes.
- **Pointer:** `EventPtrs[$40]`; event `$0040`.
- **Reachability:** Kuran-side interaction path; the map trigger immediately
  after it is `RunEventsJmpTbl[$2F]` / `Event_JuzaDefeated`.
- **Data:** `post_zio_cutscenes.rs`, `JUZA` (6 ops).

## Clone audit

The source body is explicitly wrapped in `if grand_cross=0` at the retail
clone's `Event_Juza` label (`ps4.asm:149144`). The Grand Cross branch is not
transcribed. Actual-ROM pointer range: `$06FB1E..$06FB5E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FB1E..$06FB33` | `Field_Obj_Secondary` (Juza) turns to the leader's facing with bit 2 flipped (`bchg #2`), `Event_UpdateObjFacing` | `FaceOppositeOf` object 0, of the leader |
| 1 | `$06FB34..$06FB3F` | `$FFFFED52 = 2`; `Pal_VariableFadeOut_Lines_1_2` | typed red fade |
| 2-3 | `$06FB40..$06FB4F` | dialogue entry `$48`; set Juza `$41` | dialogue/flag |
| 4-5 | `$06FB50..$06FB5D` | event battle index `3`; routine-exit handoff | battle/return |

The one frame `Event_UpdateObjFacing` spends in its closing `VInt_Prepare`
is not spent: the port's facing ops take no frame (#164), and this file's
facing calls are not yet in the wait census's helper rows (#572).

The dialogue call is `Event_GetAndRunDialogue2` (`$5ACDC`), the retained window
(`retained(0x48)`): see [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit).
