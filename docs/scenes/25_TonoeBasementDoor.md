# `Event_TonoeBasementDoor`

- **Retail bytes:** `$06F1A4..$06F2E9` inclusive, 326 bytes.
- **Pointer:** `EventPtrs[$33]` from `$05A2B4`.
- **Entry:** Tonoe tree 7 entry `$29` (41); the door itself is NPC 0 in
  TonoeStorageRoom `$42`.
- **Data:** `next_arc.rs`, `TONOE_BASEMENT_DOOR_OPS`.

## Clone audit

The retail body is the `grand_cross=0` side of the conditional source. The
Grand Cross side does not contain the retail body. The complete 326-byte
retail range was compared against the surviving branch, including the
conditional Gryz flag exit and the three-object door clear.

## Retail transcription

1. Run standard dialogue entry `$24`; if `EventFlag_GryzJoined` (`$30`) is
   clear, return without opening the door.
2. Resume dialogue, wait corrected `$27 + 1 = 40` map-update ticks, copy
   Gryz's current position into `Character_1`'s destination and move without
   following or camera motion. Face Gryz up, wait 40, and resume dialogue.
3. Set the door object facing up, mapping duration 7 and mapping index 0;
   play `SFX_DoorOpened` (`$E2`) and run the 47-tick main-update animation.
4. Clear the door plus two invisible blocks (`NPC 0..2`), wait one tick,
   restore step offset 1, face Gryz up, resume dialogue, set
   `EventFlag_TonoeDoorOpen` (`$31`) and return zero.

`MoveActorToActor` is the small interpreter extension required by step 2: a
literal target would lose the retail copy-from-Gryz behavior. The runtime
movement edge still lands on the live field map for ordinary NPC actors.

## Verification

The scene suite covers the clear-flag early return. The arc path reaches the
set-flag branch after Dorin and exercises the dialogue/movement/door sequence
before continuing into the basement route.
