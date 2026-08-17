# `Cutscene_MeetingSeth`

- **Retail bytes:** `$077EAC..$077F2D` inclusive, 130 bytes.
- **Pointer:** `CutscenePtrs[$19]`; scene `$8019`.
- **Trigger:** `RunEventsJmpTbl[$3A]`, Seth Joined `$C1` clear at Motavia
  coordinates `($770,$9B0)`.
- **Data:** `dezo_campaign.rs`, `MEETING_SETH` (11 ops).

## Clone audit

Retail `grand_cross=0` selects the body; Grand Cross scene data is excluded.
ROM range: `$077EAC..$077F2E` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-5 | `$077EAC..$077F02` | panel `$118`, tree `$20DB0E`, dialogue entry `0` | panel/dialogue |
| 6-8 | `$077F03..$077F17` | Seth to party slot 5, macro `$0A`, load Motavia | party/map |
| 9-10 | `$077F18..$077F2D` | set `$C1`, return | flag |

