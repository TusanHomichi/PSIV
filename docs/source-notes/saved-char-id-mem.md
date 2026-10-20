# `Saved_Char_ID_Mem`: the retail bridge and the native save field

Scope: `$FFFFED54..58` (`Saved_Char_ID_Mem_1..5`), the transient party bridge
between `Cutscene_BeforeElsydeonCave` and `Cutscene_Elsydeon`
(`ps4.asm:157427..157760`), and between `Event_AngerTowerTop` and
`Event_AngerTowerExitTop` (`:151963..151997`). Index: [source notes](README.md).

## What retail does

- `Cutscene_BeforeElsydeonCave` (`ps4.asm:157427`) copies the live party slots
  into `Saved_Char_ID_Mem_1..5` (`:157451-157455`: one longword plus slot 5),
  then writes `#(CharID_Chaz<<24)|$FFFFFF` and `#$FF` into the live slots, so
  the scene continues with Chaz alone.
- `Cutscene_Elsydeon` (`:157468`) copies the five bytes back into the party
  slots (`:157747-157749`). `Event_AngerTowerTop` and
  `Event_AngerTowerExitTop` do the same, which is why #148 covers both scenes.
- The bridge lives in work RAM, outside the SRAM payload: `TransferToSRAM`
  copies `$F100..$FB00` only ([save format](../camp/SAVE_FORMAT.md);
  `ps4.asm:134814-134850`).
- Boot clears the **entire** work RAM: the reset code's `ClrRAMLoop`
  (`ps4.asm:240-242`) writes zero longwords over it before the game starts. A
  power-on RAM read of `$FFFFED54..58` is therefore five zero bytes.
- The party slot encoding is the same one the scene writes with
  `CharID_Chaz<<24`: `0` is Chaz and `$FF` is empty (`CalcPartyNumber`,
  `ps4.asm:120885`, counts to `$FF`). The field's zeroed bytes restore as
  **five Chaz slots** — a corrupted party, not a sensible one.
- The cartridge oracle run (save between the scenes, power-cycle, CONTINUE,
  capture the five slots before and after) was not performed in this checkout:
  the repository holds the disassembly and the built packs, but no emulator or
  ROM harness. The static reading fixes the outcome above; a future oracle run
  should be recorded here.

## The port's deviation

The bridge is scene/runtime state, not `StateSnapshot` (it is not persistent
game state), and the port used to keep it runtime-only. A fresh CONTINUE had no
bridge, and `RestorePartySlots` faulted `BadWrite`; the route's resume check met
that at the `elsydeon` and `alys-illusion` resumes (the `before-elsydeon-cave`
and `anger-tower-top` chapter saves).

A SAVE is reachable between the scenes: both leave the player on the field
with the camp menu, so a player can save there and power off. Retail then
corrupts the party. The port now **deliberately fixes** that window: every SAVE
taken while the bridge is pending carries the five slots in the common header's
unused native space (`$F024..$F02B`: `P`, `S`, the five slot bytes, the
complement of their sum — [save format](../camp/SAVE_FORMAT.md)). A CONTINUE
between the scenes restores the parked party. Saves outside the bridge write
nothing (the header keeps retail's zeroes); a retail save has no marker, reads
no bridge, and the restore faults rather than inventing a party, so a file the
port cannot repair is reported instead of silently corrupted.

Precedent: the deliberate retail battle-bug fixes recorded in
[disassembly-discrepancies.md](disassembly-discrepancies.md). This is a fix, not
a model of retail, and it is stated as such here.
