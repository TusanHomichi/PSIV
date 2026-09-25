# `Event_GirlsSneakingOut`

- **Retail bytes:** `$06D4A6..$06D615` inclusive, 368 bytes.
- **Pointer:** `EventPtrs[$23]` at `$05A2B4`.
- **Trigger surface:** not a map `RunEventsJmpTbl` entry. At the Aiedo
  supermarket inn selector `$06`, the shop routine calls this event directly
  when `EventFlag_Zio` (`$42`) and `EventFlag_GirlsCaught` (`$46`) are both
  clear. See [SHOPS.md](../camp/SHOPS.md), §4.
- **Data:** `next_arc_followup.rs`, `GIRLS_SNEAKING_OUT` (27 ops).

## Clone audit

The clone includes the Grand Cross `script/scenes/GirlsSneakingOut/event.asm`
only behind `grand_cross=1`; the `grand_cross=0` body below is the retail
routine. The first pointer is the authoritative start, and the next retail
event pointer is `$06D616`.

## Retail transcription

| Op | ROM offset | Retail primitive | Scene op |
|---:|---|---|---|
| 0 | `$06D4A6..$06D4B1` | `VInt_Prepare` loop, corrected `$1E+1=31` | `WaitFrames(31)` |
| 1 | `$06D4B2..$06D4BD` | set render mode 1; `Window_Destroy` | `Presentation(WindowDestroy)` |
| 2 | `$06D4BE..$06D4C9` | repeat window destroy | `Presentation(WindowDestroy)` |
| 3 | `$06D4CA..$06D4D5` | repeat window destroy | `Presentation(WindowDestroy)` |
| 4 | `$06D4D6..$06D4DB` | `DMAPlane_A_VInt` | `DmaPlanes` |
| 5 | `$06D4DC..$06D4E7` | `Field_BuildSprites`; one VInt prep | `Presentation(RebuildSprites)` |
| 6 | `$06D4E8..$06D502` | save five `$2C` X words; park each at `$65` | `Presentation(SavePartySpriteX { parked_x: $65 })` |
| 7 | `$06D506..$06D510` | palette `$FB44/$FB46` ← `$AAA/$666` | `Presentation(SetPaletteWords { offset: $44 })` |
| 8 | `$06D512..$06D51C` | palette `$FB5C/$FB5E` ← `$620/$EEE` | `Presentation(SetPaletteWords { offset: $5C })` |
| 9 | `$06D51E..$06D523` | `Pal_FadeIn` | `FadeIn` |
| 10 | `$06D524..$06D52B` | `Sound_Index ← SFXID_DoorOpened ($E2)` | `PlaySound($E2)` |
| 11 | `$06D52C..$06D533` | `DoMapUpdateLoop($3B)`, 60 updates | `Wait(60)` |
| 12 | `$06D534..$06D539` | `Game_Mode_Routine ← $0C` | `Presentation(SetGameMode { mode: $0C })` |
| 13 | `$06D53A..$06D541` | `Event_GetAndRunDialogue($3E)` | standard tree entry `$3E` |
| 14 | `$06D542..$06D547` | `PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 15 | `$06D548..$06D54D` | `Game_Mode_Routine ← $18` | `Presentation(SetGameMode { mode: $18 })` |
| 16 | `$06D54E..$06D565` | restore the five saved X words | `Presentation(RestorePartySpriteX)` |
| 17 | `$06D566..$06D569` | `Joypad_Held ← 0` | `Presentation(ClearHeldInput)` |
| 18 | `$06D56A..$06D573` | clear `$C3C0` and `$C400` | `DespawnNpc { npc_index: 3, count: 2 }` |
| 19 | `$06D574..$06D585` | `Field_LoadSprites`; `Field_BuildSprites`; VInt prep | `Presentation(RebuildSprites)` |
| 20 | `$06D586..$06D593` | create window group 0 | `Presentation(WindowCreate)` |
| 21 | `$06D594..$06D5C0` | read group 0 geometry; load `WinTiles_Meseta` at `$C000` | `Presentation(LoadWindowTiles)` |
| 22 | `$06D5C2..$06D5CF` | create window group 1 | `Presentation(WindowCreate)` |
| 23 | `$06D5D0..$06D5E5` | load `ArtNem_ShopkeeperDialPortrait2` at tile `$55C` | `Presentation(LoadPortrait)` |
| 24 | `$06D5E6..$06D5FF` | map `loc_2A2B36` to Plane A at `(6,7)`, 6×6 | `Presentation(DrawPortrait)` |
| 25 | `$06D600..$06D60D` | create window group 2 | `Presentation(WindowCreate)` |
| 26 | `$06D60E..$06D615` | `EventFlags_Set(EventFlag_GirlsCaught)` | set event flag `$46` |

The window allocation, portrait decompression and palette writes remain typed
presentation data. The two cleared objects are real consecutive field-object
slots, so the runtime still applies that despawn to the live map when a scene
is run in the shop context.

## Verification

The headless scene census starts event `$0023` on Aiedo Supermarket `$63` and
asserts event flag `$46`. The trigger census records the direct shop call
separately from map event lists; treating it as a normal map trigger would
miss the selector and both flag gates.
