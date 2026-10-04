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


## In the route (C6)

`RunEvent_NearDarkForce` tests `curr_y_pos == $200`
(`ps4.asm:115892-115902`); a standing cell's y byte is its row less one, so the
trigger is **row 33** of Kuran F3 `$198`, any x. The `kuran-near-dark-force`
chapter walks the corridor (x 29..32) to (30,33) and asserts `$87` set and `$83`
clear. F3 rolls no random battles. `kuran_arc.rs`
(`the_two_f3_rows_fire_their_events_and_the_second_starts_battle_9`) drives the
same two rows from a hand-built save.
