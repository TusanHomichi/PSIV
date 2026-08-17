# `Event_NearDarkForce1`

- **Retail bytes:** `$06FAE6..$06FAF7` inclusive, 18 bytes.
- **Pointer:** `EventPtrs[$3E]`; event `$003E`.
- **Trigger:** `RunEventsJmpTbl[$2D]`, leader Y `$200`, Near Dark Force `$87`
  clear.
- **Data:** `post_zio_cutscenes.rs`, `NEAR_DARK_FORCE_1` (2 ops).

## Clone audit

This is the retail EventPtrs[$3E] body selected by the `grand_cross=0` table,
not Grand Cross story code. Actual-ROM range: `$06FAE6..$06FAF8` exclusive end.

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FAE6..$06FAEF` | dialogue entry `5` | standard dialogue |
| 1 | `$06FAF0..$06FAF7` | set `EventFlag_NearDarkForce1=$87` | flag |

