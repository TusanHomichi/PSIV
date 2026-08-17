# `Event_SaLewsDefeated`

- **Retail bytes:** `$071822..$0718E5` inclusive, 196 bytes.
- **Pointer:** `EventPtrs[$65]`; event `$0065`.
- **Trigger:** `RunEventsJmpTbl[$4C]`, Sa Lews `$D2` set and defeat `$E6`
  clear.
- **Data:** `dezo_endgame.rs`, `SA_LEWS_DEFEATED` (6 ops).

## Clone audit

Retail `grand_cross=0` selects the body; the Grand Cross alternative is
excluded. ROM range: `$071822..$0718E6` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-1 | `$071822..$071862` | dialogue `$02`, fade and palette ramp | dialogue/presentation |
| 2-4 | `$071863..$0718B9` | five object clears/updates, barrier SFX | object/SFX |
| 5 | `$0718BA..$0718E5` | set Sa Lews defeated `$E6` | flag |

