# `Event_MachineCenterAppearing`

- **Retail bytes:** `$06B4B2..$06B6F3` inclusive, 578 bytes.
- **Pointer:** `EventPtrs[$06]` at `$05A2B4`; scene event is `$0006`.
- **Trigger:** Motavia overworld `$00`, `RunEvent_MachineCenter` (`$06`):
  Zio `$42` set, Machine Center `$43` clear, leader Y exactly `$AD0`, X in
  `$710..$750`.
- **Data:** `post_rika_events.rs`, `MACHINE_CENTER_APPEARING` (12 ops).

## Clone audit

This is the retail `grand_cross=0` event body. The pointer-table range and the
three literal `NemDecomp_ToRAM` sources are the provenance anchors; no Grand
Cross-only scene include was used.

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0-2 | `$06B4B2..$06B4E2` | decompress `$1D3710→$FFFF0000`, `$1D2ABC→$FFFF0800`, `$1D2F4E→$FFFF1000` | three typed `LoadSceneAsset` records |
| 3 | `$06B500..$06B50E` | camera `($730,$B40)`, speed 1 | `MoveCamera` |
| 4 | `$06B510..$06B51A` | grave-opening SFX `$DD` | `PlaySound($DD)` |
| 5 | `$06B51C..$06B53A` | `moveq #$170` + `DoMainUpdatesLoop`: 369 iterations | `Wait(369)` |
| 6-8 | `$06B53C..$06B5BE` | timed grave objects `$78/$84/$7C`; periodic object updates | three `ObjectAnimation` records |
| 9 | `$06B552..$06B560` | return camera to live leader, speed 2 | camera presentation record |
| 10 | `$06B562..$06B568` | standard dialogue entry `$0B` | `RunDialogue(0x0B)` |
| 11 | `$06B56A..$06B570` | set Machine Center `$43` | `SetFlag($43)` |

The helper routines at `$06B572`, `$06B5C0` and `$06B5E0` are represented by
the object-animation records and their enclosing wait; their table data is
not promoted to persistent NPCs. This event is a visual Motavia trigger, not
the Molcum aftermath: the ROM's Molcum map has only null event index `$00`.

## Verification

The chain starts this event after the Zio rescue/wound state and asserts the
Machine Center flag without changing party, inventory or vehicle state.
