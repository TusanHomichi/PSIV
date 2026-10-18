# `Event_ReFaze`

- **Retail bytes:** `$07157A..$071759` inclusive, 506 bytes.
- **Pointer:** `EventPtrs[$63]`; event `$0063`.
- **Trigger:** `RunEventsJmpTbl[$4E]`, Alys Fight `$E4` set and ReFaze `$E1`
  clear.
- **Data:** `dezo_endgame.rs`, `REFAZE` (26 ops).

## Clone audit

Retail `grand_cross=0` selects the EventPtrs body. The clone's Grand Cross
alternative is excluded. ROM range: `$07157A..$07175A` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-14 | `$07157A..$0716B7` | dialogue `$09`, facing/waits, palette fade, object `$2C0`, fire breath | actor/presentation/SFX |
| 15-22 | `$0716B8..$07173B` | resume dialogue; the YES path brightens and restores the palette, plays and saves the Tower music, sets `$FFFFED52` (the variable fade's mode) to 2 for `Pal_VariableFadeToPal2` and sets `$E1`. It teaches nothing: no character record is written (US image `$071718..$07173B`) | choice/flag |
| 23-25 | `$071720..$071759` | no path sets `$E1`, event battle `$19`, return `1` | choice/battle |

