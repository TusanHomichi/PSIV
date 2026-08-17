# `Event_LeavingChazHouse`

- **Retail bytes:** `$06FACC..$06FAD3` inclusive, 8 bytes.
- **Pointer:** `EventPtrs[$3C]` at `$05A2B4`.
- **Trigger:** Aiedo `$54` lists `RunEventsJmpTbl[$27]`; when temp flag
  `$18` is set, `RunEvent_ClrChazHouseRest` writes event `$003C`.
- **Data:** `next_arc_followup.rs`, `LEAVING_CHAZ_HOUSE` (1 op).

## Retail transcription

| Op | ROM offset | Retail bytes / primitive | Scene op |
|---:|---|---|---|
| 0 | `$06FACC..$06FAD3` | `moveq #$18`; `TempEveFlags_Clear` | clear temp flag `$18` |

This eight-byte event has no dialogue, map load or return-value write. It is
registered because the trigger is live and otherwise the Aiedo exit would
produce a missing-scene edge after Chaz's rest.

## Verification

The follow-up test runs ChazHouse, then event `$003C`, and asserts the temp
flag is cleared.
