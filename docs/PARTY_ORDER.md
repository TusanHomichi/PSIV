# STATE / ORDER

The native camp's former `ORDER NOT READY` stub now opens the cartridge's
party-order chooser. Choose the front slot first; the last remaining member
fills the final slot automatically. Cancel undoes the latest pick, restoring
its position in the remaining list. Cancel with no picks returns to STATE.
The runtime changes nothing until the full permutation is ready.

## Original behavior

References are the US-ROM disassembly in `reference/ps4disasm/ps4.asm`:

* `Win_OrderCharList` and `Win_OrderCharListMain` maintain separate old and
  new five-byte lists. `loc_5E1EE` picks and compacts; `loc_5E3C4` undoes a
  pick and resets the cursor to the first remaining member.
* `loc_5E5A0` auto-appends the last member and writes `Current_Party_Slots`
  at `$FFFFF40A`. `loc_5E6DE` changes field-object art in the existing slots;
  it does not rearrange field coordinates or character records.
* `WinGroup_Menu` `$D2..$D9` supplies the two chooser windows. For four
  members their outer cell rectangles are `(2,13,9,10)` and `(11,13,7,10)`.
  Names begin at `(5,14)` / `(13,14)` with two rows per member. Hollow
  boxes remain beside every unpicked member, and ORDER captions the lower
  border. The completed right-hand list remains briefly after the left
  chooser disappears.
* `FieldObj_RedCursor` / `RedCursor_Main` starts visible for 26 updates;
  subsequent hidden/visible periods are 26/16 updates. Moving resets its
  visible timer. Movement/selection sounds are `$F2/$F3`.
* `Win_OrderAloneMsg` says "There's only / one of you!" and returns to
  STATE after input or its 121-frame wait. Completed selection returns to
  the root after input or the 61-frame wait.

`order_camp_party` accepts only a complete permutation of the current party
and requires idle field state. Battle fighters take their slots from the
new lineup. Persistent roster records, inventory, flags and walking-slot
positions remain unchanged; SAVE already stores the party bytes.

## Why this matters in BioPlant

`Enemy_TargetCharacter` does not choose uniformly. With four living members,
the full 256-byte target-roll domain selects slots 1–4 respectively 103,
76, 51 and 26 times. The connected checkpoint originally had Alys first and
Gryz fourth. ORDER permits Gryz/Alys/Chaz/Hahn, putting the durable member
first and Hahn last through ordinary input. This changes combat targeting
without altering enemy rules, levels, equipment or resources.

## Validation

`rust/psiv-runtime/tests/camp_order.rs` checks invalid permutations, atomic
commit, unchanged roster and field positions, SAVE/load, battle slot order,
the complete target-roll distribution, and rejection during battle.
The UI unit test checks nontrivial pick/undo restoration and auto-completion.

`tools/native_party_order.gd` loads the connected map `$A7` SAVE, performs
pick/undo/cancel, commits Gryz/Alys/Chaz/Hahn and saves through the menu.
The original emulator can load the very same native SRAM without RAM
patches; `build/native-order/oracle/order.tape` performs the matching ORDER
sequence. Its RAM dump at frame 1985 contains `[4,1,0,2,255]` at `$F40A`.

`tools/verify_native_order.py` compares the ordinary-input receipt, exact
save bytes and original RAM. It also compares native captures with original
frames, allowing their independently timed cursors to reach the same phase.
No pixels inside the compared rectangles are masked. This is a bounded
ORDER-window comparison: the surrounding camp's single-character summary,
added SAVE row and field camera do not have whole-screen parity.

Run artifacts and the fresh-process CONTINUE result are recorded with the
connected campaign checkpoint in `docs/BIOPLANT_NATIVE.md`.
