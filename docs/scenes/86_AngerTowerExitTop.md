# `Event_AngerTowerExitTop`

- **Retail bytes:** `$072262..$0722D1` inclusive, 112 bytes.
- **Pointer:** `EventPtrs[$6A]`; event `$006A`.
- **Trigger:** `RunEventsJmpTbl[$52]`, Anger Tower End `$E7` clear at
  `x=$1A0..$1B0`, `y=$260`.
- **Data:** `dezo_endgame.rs`, `ANGER_TOWER_EXIT_TOP` (10 ops).

## Clone audit

Retail `grand_cross=0` selects the body; Grand Cross-only code is excluded.
ROM range: `$072262..$0722D2` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-5 | `$072262..$0722B3` | restore saved party, stairs/fade, load Anger Tower F1 | transient party/map |
| 6-9 | `$0722B4..$0722D1` | ReFaze branch: Tower music when clear, set `$E7` when set | branch/music/flag |

