# `Event_ReFaze`

- **Retail bytes:** `$07157A..$071759` inclusive, 480 bytes.
- **Pointer:** `EventPtrs[$63]`; event `$0063`.
- **Trigger:** `RunEventsJmpTbl[$4E]`, Alys Fight `$E4` set and ReFaze `$E1`
  clear.
- **Data:** `dezo_endgame.rs`, `REFAZE` (27 ops).

## Clone audit

Retail `grand_cross=0` selects the EventPtrs body. The clone's Grand Cross
alternative is excluded. ROM range: `$07157A..$07175A` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-17 | `$07157A..$07167B` | dialogue `$09`, facing/waits, palette fade, object `$2C0`, fire breath, the resume dialogue and the choice (`tst.b (Yes_No_Option).w; beq.w loc_7173C`) | actor/presentation/SFX, choice |
| 18-23 | `$07167C..$07173B` | the YES path writes `TechID_Megid` into the last technique slot (`lea (Character_Stats).w,a0; move.b #TechID_Megid, techs+15(a0)`, US image `$07167C..$071685`), brightens and restores the palette, plays and saves the Tower music, sets `$FFFFED52` to 2 for `Pal_VariableFadeToPal2` and sets `$E1`, then returns `0` | `LearnTechnique { character: 0, technique: 0x10 }`, choice/flag |
| 24-26 | `$07173C..$071759` | the NO path sets `$E1`, event battle `$19`, return `1` | choice/battle |

