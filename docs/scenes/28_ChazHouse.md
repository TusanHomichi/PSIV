# `Event_ChazHouse`

- **Retail bytes:** `$06FA34..$06FACB` inclusive, 152 bytes.
- **Pointer:** `EventPtrs[$3B]` at `$05A2B4`.
- **Trigger:** ChazHouse `$5E` lists `RunEventsJmpTbl[$26]`; while temp flag
  `$18` is clear, `RunEvent_ChazHouseRest` writes event `$003B`.
- **Data:** `next_arc_followup.rs`, `CHAZ_HOUSE` (13 ops).

## Clone audit

The clone's Grand Cross branch includes `script/scenes/ChazHouse/event.asm`;
the retail `grand_cross=0` body is the 152-byte range above. The yes/no byte
test at `$06FA56` is retail's `Yes_No_Option` on this revision: zero takes the
rest/map-refresh branch, nonzero sets the temp flag and leaves the map alone.

## Retail transcription

| Op | ROM offset | Retail primitive | Scene op |
|---:|---|---|---|
| 0 | `$06FA34..$06FA3D` | test `EventFlag_Zio` (`$42`) | `BranchFlag` to portrait patch or normal dialogue |
| 1 | `$06FA3E..$06FA4D` | `GetDialogueByID($31)`; portrait byte ← 1 | `Presentation(SetDialoguePortrait { entry: $31, portrait: 1 })` |
| 2 | `$06FA4E..$06FA55` | `Event_GetAndRunDialogue($31)` | standard tree entry `$31` |
| 3 | `$06FA56..$06FAC3` | test `Yes_No_Option` | `BranchChoice` (yes → rest, no → flag only) |
| 4 | `$06FA5C..$06FA63` | stop current music | `PlaySound($FB)` |
| 5 | `$06FA64..$06FA69` | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 6 | `$06FA6A..$06FA6F` | `RecoverStats` | `RecoverStats` |
| 7 | `$06FA70..$06FA9F` | write ChazHouse/Aiedo map pair, start `($2E,$44)`, down, align 0; clear load bit 3; `RefreshMap` | `LoadMap { map: $5E, prev: $54, start: ($2E,$44) }` |
| 8 | `$06FAA0..$06FAA7` | `DoMapUpdateLoop($3B)`, 60 updates | `Wait(60)` |
| 9 | `$06FAB4..$06FABB` | `Pal_FadeIn` after palette copy | `FadeIn` |
| 10 | `$06FABC..$06FAC3` | `Sound_Index ← MusicID_MotabiaTown ($84)` | `PlaySound($84)` |
| 11 | `$06FAC4..$06FACB` | `TempEveFlags_Set($18)` on the rest branch | set temp flag `$18` |
| 12 | `$06FAC4..$06FACB` | same flag-set tail on the no branch | set temp flag `$18` |

The branch has two entries for the same retail tail because both outcomes
call `TempEveFlags_Set`; the map reload exists only on the yes/rest path. No
portrait or choice behavior is invented in the runtime: the portrait patch is
presentation data and the scene runner's choice edge defaults to the retail
yes branch for the headless harness.

## Verification

The runtime test starts event `$003B` on ChazHouse, asserts that the map
refresh lands back on `$5E`, and asserts temp flag `$18`. The adjacent exit
event is recorded separately in [29](29_LeavingChazHouse.md).
