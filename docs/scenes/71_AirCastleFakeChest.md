# `Event_AirCastleFakeChest`

- **Retail bytes:** `$070B56..$070C2F` inclusive, 218 bytes.
- **Pointer:** `EventPtrs[$5A]`; event `$005A`.
- **Trigger:** `RunEventsJmpTbl[$45]`, Eclipse Torch chest `$10C` set and
  Spector `$A6` clear.
- **Data:** `dezo_campaign_late.rs`, `AIR_CASTLE_FAKE_CHEST` (11 ops).

## Clone audit

Retail `grand_cross=0` selects the EventPtrs body. The clone's Grand Cross
body is excluded. ROM range: `$070B56..$070C30` exclusive end.

## Retail transcription

| Ops | ROM offsets | Retail primitive / literal | Scene record |
|---:|---|---|---|
| 0-2 | `$070B56..$070B79` | dialogue `$3C`; set Spector `$A6`; remove Eclipse Torch `$8E` | dialogue/inventory/flag |
| 3-8 | `$070B7A..$070C21` | five character-position/facing write groups and map-load bit `$08` | object/map presentation |
| 9-10 | `$070C22..$070C2F` | event battle `$0F`, return `1` | battle |

The dialogue call is `Event_GetAndRunDialogue2` (`$5ACDC`), the retained window
(`retained(0x3C)`): see [the Dialogue2 audit](DIALOGUE_CALLERS.md#the-dialogue2-audit).

The last page and its portrait remain drawn after the final Speak press while
this scene runs, as for every `Retained` entry and resume
([window lifetime](DIALOGUE_CALLERS.md#the-routines-and-their-windows)).
`RemoveItem` and all five `ObjectAnimation` records run on that same close
frame, followed immediately by `StartBattle`. They spend **zero waited
frames**: `ps4.asm:150184-150228` sets the flag, calls `GetItem` and
`ReorderInventory`, then writes each character's `$30/$34/$38/$3A` positions
and `$6` facing directly. There is no animation loop, `VInt_Prepare`, or
`DoMapUpdateLoop` between the dialogue return and the battle request.
`ObjectAnimation.frames = 1` here is a presentation record, not a scene wait.
Consequently there is no distinct frame inside these writes for an oracle
capture; the battle's field-entry cover is the next visible phase
([#665](https://github.com/TusanHomichi/PSIV/issues/665)).

The runtime suite `suites::retained_window` drives the actual `$5A` dialogue
with Speak presses and checks that removal, all five records and the battle
request share its close frame, with the page and portrait still available.
Its isolated caller fixtures check every intervening frame of an eight-frame
scene wait and disappearance at scene end, including retained resumes and
`Standard` as a negative control. These are runtime input/state checks,
not a native visual comparison or a connected campaign route.
