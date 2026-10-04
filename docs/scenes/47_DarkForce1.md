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
| 0 | `$06FAF8..$06FB03` | `Event_GetAndRunDialogue2`, entry `6` | retained-window dialogue |
| 1-2 | `$06FB04..$06FB13` | set Dark Force 1 `$83`; set map-load bit 7 | flag/map flags |
| 3-4 | `$06FB14..$06FB1D` | event battle index `9`; routine-exit handoff | battle/return |

The scene writes `$83` before the battle request, matching the retail
observable order. The following trigger `$32` dispatches `$8011` once the
battle is resolved.


## In the route (C6)

`RunEvent_FindDarkForce` tests `curr_y_pos == $0D0` (`ps4.asm:115903-115913`):
**row 14** of Kuran F3, any x, `$83` clear. The `kuran-dark-force-1` chapter
walks to (30,14); the dialogue (entry 6) runs, `$83` is set and event battle 9
starts (enemy 130, 4,540 HP; FLARE SHOT, PHONONMASR and BURSTROC strike the
whole party). The engine runs no player skill beyond Crosscut and Vortex, so
Chaz's Rayblade (anti-evil, which Dark Force 1 is weak to) and Rune's Efess are
never cast: the party wins on techniques and plain attacks, and it took levels
38 to 46 to do it ([RUNNER_LOG_KURAN.md](../campaign/RUNNER_LOG_KURAN.md#h34-dark-force-1-is-a-balance-loss-until-the-party-is-trained)).
