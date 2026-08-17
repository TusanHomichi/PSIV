# `Event_DarkForce1`

- **Retail bytes:** `$06FAF8..$06FB1D` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$3F]`; event `$003F`.
- **Trigger:** `RunEventsJmpTbl[$2E]`, leader Y `$0D0`, Dark Force 1 `$83`
  clear.
- **Data:** `post_zio_cutscenes.rs`, `DARK_FORCE_1` (5 ops).

## Clone audit

The body at `ps4.asm:149134` is the retail EventPtrs[$3F] body selected by the
`grand_cross=0` pointer table. Actual-ROM range: `$06FAF8..$06FB1E` exclusive
end. No hack-only scene is involved.

## Retail transcription

| Ops | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$06FAF8..$06FB03` | `Event_GetAndRunDialogue2`, entry `6` | standard dialogue |
| 1-2 | `$06FB04..$06FB13` | set Dark Force 1 `$83`; set map-load bit 7 | flag/map flags |
| 3-4 | `$06FB14..$06FB1D` | event battle index `9`; routine-exit handoff | battle/return |

The scene writes `$83` before the battle request, matching the retail
observable order. The following trigger `$32` dispatches `$8011` once the
battle is resolved.

