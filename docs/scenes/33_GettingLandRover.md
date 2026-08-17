# `Event_GettingLandRover`

- **Retail bytes:** `$06DEBE..$06E0E9` inclusive, 556 bytes.
- **Pointer:** `EventPtrs[$2B]` at `$05A2B4`; scene event is `$002B`.
- **Trigger:** Machine Center B1 Part2 `$B9`, `RunEvent_GettingLandRover`
  (`$1B`): Control Key chest `$10A` set, Land Rover `$44` clear, leader
  Y `>=$1C0`.
- **Data:** `post_rika_events.rs`, `GETTING_LAND_ROVER` (30 ops).

## Clone audit

The pointer range is retail cartridge data. The scene body is transcribed from
the retail (`grand_cross=0`) branch; the Grand Cross include is not an
authority for this record.

## Retail transcription

| Op(s) | ROM offset | Retail primitive / literal | Scene op |
|---:|---|---|---|
| 0 | `$06DEBE` | standard dialogue entry 9 | `RunDialogue(9)` |
| 1-4 | `$06DEFC..$06DF1E` | step offset 0, follow bit 0, `DoMainUpdatesLoop($63+1=100)`, `popdlg` | state/wait/resume |
| 5-7 | `$06DF24..$06DF40` | radar `$F8`, map update `$27+1=40`, camera `($1E0,$200)`, speed 1 | sound/wait/camera |
| 8-9 | `$06DF46..$06DF6E` | conveyor `$E7`, conveyor loop `$167+1=360` | sound + `Wait(360)` |
| 10-13 | `$06DF7A..$06DFA6` | stop special SFX `$FD`; face Demi up; wait `$1D+1=30`; resume dialogue | sound/presentation/wait/resume |
| 14-16 | `$06DFAC..$06DFC4` | clear follow; move leader to `($1E0,$190)`; restore step offset 1 | follow/move/step |
| 17-21 | `$06DFCA..$06DFF8` | fade out, stop music, wait `$E+1=15`, save Land Master `$8D`, VInt | fade/sound/waits |
| 22-23 | `$06DFFE..$06E02E` | vehicle index 1; load Motavia `$00`, previous Machine Center `$B7`, start `($E4,$160)`, up, align 4 | `SetVehicleIndex`, `LoadMap` |
| 24-26 | `$06E09A..$06E0AA` | Land Master `$8D`, fade in, dialogue entry `$0C` | sound/fade/dialogue |
| 27-29 | `$06E0AE..$06E0CC` | remove Control Key `$99`, reorder, add Land Rover `$96`, set flag `$44` | inventory + `SetFlag($44)` |

The inventory edge is deliberately two typed state operations. Retail clears
the Control Key slot, compacts the list, and writes Land Rover into the first
free slot; the runner's `RemoveItem` and `AddItem` preserve that observable
result. `Vehicle_Index` is the saved selector, not a sound-only presentation
hint.

## Verification

The arc test seeds Control Key and asserts after this event that vehicle 1 is
selected, Control Key is absent, Land Rover is present, and flag `$44` is set.
