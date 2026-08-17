# `Cutscene_BeforeElsydeonCave`

- **Retail bytes:** `$0784B6..$078583` inclusive, 206 bytes.
- **Pointer:** `CutscenePtrs[$1D]`; scene `$801D`.
- **Trigger:** `RunEventsJmpTbl[$4F]`, Le Roof story `$D6` set and Elsydeon
  cave `$D8` clear.
- **Data:** `dezo_endgame.rs`, `BEFORE_ELSYDEON_CAVE` (16 ops).

## Clone audit

The retail `grand_cross=0` cutscene body is transcribed. The Grand Cross body
is excluded. ROM range: `$0784B6..$078584` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-8 | `$0784B6..$07853A` | Chaz movement, door SFX, panel `$18E`, dialogue `$30` | actor/presentation/dialogue |
| 9-13 | `$07853B..$07856D` | save party slots, retain Chaz only, recover stats, set `$D8` | transient party/flag |
| 14-15 | `$07856E..$078583` | load Inner Sanctuary B1 at `($3C,$32)`, return | map |

`SavePartySlots` is the transient retail `Saved_Char_ID_Mem_1/_5` bridge. It
does not enter `StateSnapshot` or save serialization.

