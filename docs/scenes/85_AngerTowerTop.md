# `Event_AngerTowerTop`

- **Retail bytes:** `$0721CC..$072261` inclusive, 150 bytes.
- **Pointer:** `EventPtrs[$69]`; event `$0069`.
- **Trigger:** `RunEventsJmpTbl[$51]`, Alys Fight `$E4` clear at
  `x=$1C0..$1D0`, `y=$250..$260`.
- **Data:** `dezo_endgame.rs`, `ANGER_TOWER_TOP` (12 ops).

## Clone audit

The `grand_cross=0` EventPtrs body is selected; the clone's Grand Cross body is
excluded. ROM range: `$0721CC..$072262` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-5 | `$0721CC..$07222C` | save party slots, retain Chaz, recover, stairs/fade, load Anger Tower F2 | transient party/map |
| 6-11 | `$07222D..$072261` | camera movement, fade/music, waits | camera/music |

