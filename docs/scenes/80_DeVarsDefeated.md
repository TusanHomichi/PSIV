# `Event_DeVarsDefeated`

- **Retail bytes:** `$07175A..$071821` inclusive, 200 bytes.
- **Pointer:** `EventPtrs[$64]`; event `$0064`.
- **Trigger:** `RunEventsJmpTbl[$4B]`, DeVars `$D4` set and defeat `$E5`
  clear.
- **Data:** `dezo_endgame.rs`, `DE_VARS_DEFEATED` (6 ops).

## Clone audit

The retail `grand_cross=0` EventPtrs body is selected; Grand Cross-only code is
excluded. ROM range: `$07175A..$071822` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$07175A..$07179A` | dialogue `$04`, fade and palette ramp | dialogue/presentation |
| 2-4 | `$07179B..$0717F5` | five object clears/updates, barrier SFX | object/SFX |
| 5 | `$0717F6..$071821` | set DeVars defeated `$E5` | flag |

