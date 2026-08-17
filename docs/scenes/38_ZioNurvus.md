# `Event_ZioNurvus`

- **Retail bytes:** `$06F2EA..$06F439` inclusive, 336 bytes.
- **Pointer:** `EventPtrs[$34]` at `$05A2B4`; scene event is `$0034`.
- **Trigger:** Nurvus B4 Part2 `$D3`, `RunEvent_ZioNurvus` (`$1F`):
  Zio Nurvus `$65` clear and leader Y exactly `$1E0`.
- **Data:** `post_rika_events.rs`, `ZIO_NURVUS` (17 ops).

## Clone audit

The retail `grand_cross=0` body occupies the pointer range above. The Grand
Cross source was not used for the two art objects, palette writes, or battle
handoff.

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0-2 | `$06F2EA..$06F302` | stop music; map update `$27+1=40`; VDP/palette write `$8C89` | sound/wait/presentation |
| 3-5 | `$06F306..$06F37C` | load art `$347/$580`; stage objects `$1E8/$1EC`; enemy appearance `$A3` | art/object/sound |
| 6-7 | `$06F384..$06F3AA` | 126 update iterations; wait `$13+1=20`; palette write `$8C81` | waits/presentation |
| 8-11 | `$06F3B2..$06F3CC` | step object; Black Blood `$A8`; wait one; resume dialogue `$0B` | object/sound/wait/dialogue |
| 12-14 | `$06F3DA..$06F3F0` | set Nurvus `$65`; save Stop All `$FE`; set map-load bits `$88` | flag/music/map flags |
| 15-16 | `$06F3F4..$06F400` | battle index 6, routine-exit bit, return | `StartBattle(6)`, `Return(1)` |

The source returns through the routine-exit battle path rather than loading a
new map itself. The runtime therefore sets `$65` before blocking at battle 6,
matching the cartridge's observable order.

## Verification

The headless arc resolves event battle 6 and asserts Nurvus `$65` before
starting `Cutscene_ZioDefeated`.
