# `Event_BioPlantAlarm`

- **Retail bytes:** `$06C27C..$06C2BB` inclusive, 64 bytes.
- **Pointer:** `EventPtrs[$12]` at `$05A2B4`.
- **Trigger:** BioPlant Part2 `$A3`, `RunEventsJmpTbl[$0C]`; the routine
  requires `TempEveFlag_BioPlantAlarm` (`$08`) clear and leader Y exactly
  `$180`, then writes event `$0012`.
- **Data:** `next_arc_followup.rs`, `BIO_PLANT_ALARM` (6 ops).

## Clone audit

The clone's `grand_cross=1` branch includes the absent
`script/scenes/BioPlantAlarm/event.asm`; its `grand_cross=0` body is the
retail routine below. The pointer table is independently anchored in the
cartridge, so the range is not inferred from the clone label.

## Retail transcription

| Op | ROM offset | Retail bytes / primitive | Scene op |
|---:|---|---|---|
| 0 | `$06C27C..$06C283` | `move.b #$DB,Sound_Index` | `PlaySound($DB)` / alarm SFX |
| 1 | `$06C284..$06C28F` | set `$ED52` to 3; `Pal_VariableFadeToRed` | `Presentation(FadeToRed { lines: 3 })` |
| 2 | `$06C290..$06C29B` | set `$ED52` to 3; `Pal_VariableFadeFromRed` | `Presentation(FadeFromRed { lines: 3 })` |
| 3 | `$06C29C..$06C2A3` | `moveq #$A`; `Event_GetAndRunDialogue` | standard tree entry `$0A` |
| 4 | `$06C2A4..$06C2B3` | clear `Game_Mode_Routine`; `Map_LoadChunks` | `Presentation(ReloadMapChunks)` |
| 5 | `$06C2B4..$06C2BB` | `moveq #$08`; `TempEveFlags_Set` | set temp flag `$08` |

The palette-line writes are retained as presentation data. `Map_LoadChunks`
is likewise observable data: it refreshes the current BioPlant layout, but it
does not load a new map or invent a renderer-side patch.

## Verification

The runtime scene census starts event `$0012` on map `$A3`; the follow-up arc
test asserts temp flag `$08` before entering the later BioPlant B4/Rika beat.
