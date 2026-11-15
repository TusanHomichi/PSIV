# `Event_FractOozeFound`

- **Retail bytes:** `$072B2C..$072B51` inclusive, 38 bytes.
- **Pointer:** `EventPtrs[$7D]` at `$05A2B4`; event `$007D`.
- **Trigger:** Dialogue `$F6 $00 $7D`, `DialogueTree12` entry 38 (Monsen); an `Event_GetAndRunDialogue2` caller (`jsr $5ACDC` at `$072B2E`, row 12 of [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit), issue #71).
- **Data:** `census_events.rs`, `FRACT_OOZE_FOUND` (5 ops).

## Clone audit

The body was read from the US image through its `EventPtrs` slot; the clone's label
names it and nothing else. Transcribed by the event census lane (S7,
[EVENT_COVERAGE](EVENT_COVERAGE.md)).

## Retail transcription

| Op | ROM offsets | Retail primitive | Scene record |
|---:|---|---|---|
| 0 | `$072B2C..$072B33` | `moveq #$2F, d0` / `jsr Event_GetAndRunDialogue2` (`$5ACDC`) | `retained(0x2F)` |
| 1 | `$072B34..$072B3B` | `moveq #$2F, d0` / `jsr EventFlags_Set` | `SetFlag` `$2F` |
| 2 | `$072B3C..$072B41` | `bset #3, Map_Load_Flags` | `SetMapLoadFlags` `0x08` |
| 3 | `$072B42..$072B4D` | `move.b #7, Event_Battle_Index` / `bset #3, Routine_Exit_Flags` | `StartBattle` `7` |
| 4 | `$072B4E..$072B50` | `moveq #1, d0` / `rts` | `Return` `1` |

The retained window stays up through the battle request, as for every `Dialogue2` caller.

## Where the window goes (#644)

Tape 45 (`tools/certify.py` `TAPE_45`) runs the event on Monsen from a fixture.
The C on frame 7799 ends the last page ("It's coming for us!"), and the routine
returns with the window and Rika's portrait up: frame 7800 shows both
untouched. `RunEventBattle` (`ps4.asm:120803`) then runs `FieldRoutine_Battle2`'s
event arm, `loc_58498`: fifteen passes, each writing a white plane-A band two
cells taller (rows 104-119 on 7801, one cell more above and below on each
frame after) and calling `Pal_IncreaseTone`. The band covers the portrait by
7805 and the window by 7812, and the whole screen by 7814. `Game_Mode` turns to
the battle on 7815, which holds white until 7856 and fades the battle in from
7857.

The port closes the window with the text and draws the battle stage on the
next tick (`PSIV_DEBUG_EVENT=0x7D`: the last page through tick 513, the stage
from 514), with no band, tone or white hold
([source notes](../source-notes/README.md#other-recorded-deviations)). The
`retained-window-page` pair certifies the last page over the window's box,
`(24,160)` 272x48, at tick 511 against frame 7797.
