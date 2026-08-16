# Retail SRAM save scout and implementation

Scouted from the USA retail disassembly and the existing oracle tape on
2026-08-16. The implementation lives in `rust/psiv-core/src/save.rs` and
`rust/psiv-runtime/src/save.rs`; camp presentation owns only the modern STATE
screen's SAVE row and slot chooser.

## Evidence and menu flow

The prior lane's tape was reused, not recreated:
[`oracle/tapes/24_save_slot_screen.tape`](../oracle/tapes/24_save_slot_screen.tape).
Its final settled frame is 7179. The retail capture shows the system menu's
SAVE window with the prompt `Save in number- ?` and three visible rows, `1`,
`2`, and `3`; the image is
[`oracle/frames/save_slot_idle_end/frame_7179.png`](../oracle/frames/save_slot_idle_end/frame_7179.png).

The decoded flow is:

1. Start opens the system menu, whose SAVE entry opens `Win_SaveSlots`.
2. `Win_SaveSlots` loads the three row labels and calls `loc_594F2` and
   `loc_595EA` to inspect each slot (`ps4.asm:119503-119531`). The latter
   reads each first-character level from the interleaved payload at logical
   character-record offset `$408` (`ps4.asm:119625-119745`).
   Consequently a populated row shows its slot number and the first
   character's level; an empty row has no level text. The existing tape's
   blank rows are that zero/empty path, not missing window data.
3. The cursor is a three-entry loop (`moveq #2`, `ps4.asm:119547-119550`). A
   selected empty row goes directly to `SaveGame`; a populated row opens the
   overwrite confirmation (`ps4.asm:119551-119567`).
4. `SaveGame` stores the zero-based slot, both map selectors, and the leader's
   x/y words, then calls `TransferToSRAM` (`ps4.asm:119569-119582`).
5. The success string is `-File has been saved.` (`ps4.asm:340780-340782`).

Retail's system SAVE path is separate from the camp STATE chooser. The
existing camp scout pins STATE as a two-row child window with `STATUS` and
`ORDER` only (`docs/CAMP_MENU_LAYOUT.md`, “STATE chooser”). The current Godot
slice has no system menu or title screen, so this wave adds a third, explicit
SAVE row under the camp STATE screen. The retail two-row geometry remains
oracle-pinned in `camp/layout.rs`; the added row and slot chooser are a
documented modern surface, not a claim that retail displayed SAVE there.

## SRAM device and slot count

The constants define the physical SRAM window as `$200001..$203FFF`
(`ps4.constants.asm:1976-1981`). The code addresses the even base one byte
before that window and uses 68000 `MOVEP` writes, so each logical byte is at
an odd physical address:

```text
physical_byte = block_base + 1 + 2 * logical_byte_offset
```

The common header is read/written from `SRAM_Buffer = $F000`; the signature,
selected slot, and checksum table are at `$F002`, `$F012`, and `$F014`
respectively (`ps4.constants.asm:2354-2360`). The payload begins at
`Event_Flags = $F100` and ends at `$FB00`. `TransferToSRAM` copies exactly
`#$280` longwords, or 0xA00 logical bytes, with `#$27F` as the DBF counter
(`ps4.asm:134814-134850`).

There are **three visible retail slots**, not four. The title validator loops
`d0 = 0, 1, 2` (`ps4.asm:87474-87483`), and `Win_SaveSlots` has the same
three-row cursor. The checksum table has four word positions because the
runtime masks the selected index with `#3`, but a fourth 0xA00 payload at
`$200200 + 3*$1400 = $203E00` would run past `SRAM_End`. The fourth checksum
word is therefore reserved/unreachable in the title UI and is not exposed by
the implementation.

### Physical layout

| region | logical size | physical location | notes |
|---|---:|---|---|
| common header | 0x100 | `$200001..$2001FF` | odd-byte MOVEP image of `$F000..$F100` |
| slot 1 payload | 0xA00 | `$200201..$2015FF` | block base `$200200`, stride `$1400` |
| slot 2 payload | 0xA00 | `$201601..$2029FF` | block base `$201600` |
| slot 3 payload | 0xA00 | `$202A01..$203DFF` | block base `$202A00` |
| unused tail | — | `$203E00..$203FFF` | too small for another payload |

The on-disk representation is one self-contained 0x1600-byte file per
visible slot: the 0x200-byte interleaved header followed by that slot's
0x1400-byte physical block. Retail has one shared SRAM device; three files
are the practical disk equivalent. The header in each file carries the
signature and the selected slot's checksum-table word. Even physical bytes
are left as zero holes, matching `MOVEP` rather than packing the logical
payload.

## Logical `$F100..$FB00` payload

The address constants and the save-copy comment are at
`ps4.constants.asm:2362-2417`. The logical offsets below are relative to
`$F100`.

| logical offset | retail address | size | serialized value |
|---:|---:|---:|---|
| `0x000` | `$F100` | 0x40 | base + extended event flags; the Rust `event_flags` array |
| `0x040` | `$F140` | 0x20 | temp-event flags; the Rust `temp_flags` array |
| `0x060` | `$F160` | 0x10 | town flags; the Rust `town_flags` array |
| `0x300` | `$F400` | word | world index |
| `0x302` | `$F402` | word | secondary map selector |
| `0x304` | `$F404` | word | current field map selector |
| `0x306` | `$F406` | word | leader x position, pixels |
| `0x308` | `$F408` | word | leader y position, pixels |
| `0x30A` | `$F40A` | 5 bytes | current party character ids |
| `0x310` | `$F410` | 40 bytes | inventory slots |
| `0x338` | `$F438` | longword | current money |
| `0x33C` | `$F43C` | word | vehicle selector |
| `0x33E` | `$F43E` | word | button mapping selector |
| `0x340` | `$F440` | word | message speed |
| `0x342` | `$F442` | word | battle speed |
| `0x344` | `$F444` | 160 bytes | eight 20-byte macro records |
| `0x400` | `$F500` | 11 × 0x80 | complete character records |
| `0x980` | `$FA80` | 3 × 0x20 | Land Rover, Ice Digger, Hydrofoil records |

`$F100..$F13F` is the combined 512-bit event space: `$F100..$F11F` is
ordinary event flags and `$F120..$F13F` is the extended/chest half. That is
why `StateSnapshot` has one 64-byte `event_flags` bank even though the source
names two 32-byte regions. Together with the 32-byte temp bank and 16-byte
town bank, this covers the four cartridge-named flag regions without reviving
the old false `$F156` temp-bank split.

The 11 character records are Chaz through Seth at `$F500 + n*$80`
(`ps4.constants.asm:2398-2409`). The serializer writes every field represented
by `Stats` at its retail offset, including the stale `_battle` tier, current
HP/TP, status, equipment, element pairs, `gain_exp_flag`, and the physical
property save byte. It does not rederive battle values; the retail save copy
is whole-record state.

## Signature and checksum

`CheckPS4String` compares the physical odd bytes corresponding to logical
`$F002..$F010` against `PHANTASY STAR 4` (`ps4.asm:134718-134732` and
`134768-134771`). `InitSRAM` clears the interleaved device and writes that
signature (`ps4.asm:134737-134763`).

`SumSRAMBytes` adds all 0xA00 payload bytes as a wrapping 16-bit word
(`ps4.asm:134885-134894`). `TransferToSRAM` writes the result to logical
`$F014 + slot*2`; validation reads the corresponding physical words at
`$200029 + slot*4` and clears a bad payload (`ps4.asm:134777-134812`). The
Rust serializer reproduces that exact byte sum and rejects a bad signature or
checksum before construction.

The Rust header also uses logical `$F020` as a two-byte occupancy mask for
`Option<Stats>` seats. No retail routine in the scouted header range addresses
that word. This is the deliberate compatibility extension that preserves an
intentionally all-zero `Stats` record; ordinary retail payloads without the
extension use the all-zero-record heuristic.

## Retail title-continue semantics

At title boot, `TitleRoutine_PickOption` enables SRAM, checks the signature,
and initializes SRAM if it is absent (`ps4.asm:87455-87471`). If the signature
exists, it validates slots 0, 1, and 2 with `loc_64D8A`; a bad checksum marks
the slot bad and clears that physical payload (`ps4.asm:87474-87489` and
`134777-134812`). Only then does the title expose Continue alongside Start
and Erase.

The Continue path displays the three slots, reads the selected zero-based
slot, and calls `loc_64E62` to copy the common header and selected payload back
to RAM (`ps4.asm:87587-87620`, `87711-87724`). It then restores
`Saved_Field_Map_Index` and `Saved_Field_Map_Index_2`, sets bit 1 of
`Map_Load_Flags`, and enters the field. `GameMode_LoadFieldMap` consumes that
bit by copying `Saved_Char_X_Pos` and `Saved_Char_Y_Pos` into the map start
position (`ps4.asm:87719-87724`, `110806-110812`). Facing is not in the saved
range.

The current runtime has no title screen. `Runtime::from_save` is the intended
load seam: decode the slot, rebuild `GameState`, evaluate the selected map's
load effects against it, construct the party at the saved cell, and default
facing to Down. Godot selects it with `PSIV_LOAD_SLOT=1..3` or
`--psiv-load-slot=N` until the title flow exists.

## Divergences and load semantics

The implementation is byte-compatible for the retail header, interleaving,
payload offsets, character stride, signature and checksum. These are the
known, bounded divergences:

* The disk uses three per-slot files instead of one shared SRAM device. Files
  are `saves/slot_1.sram` through `saves/slot_3.sram`; `saves/` is ignored by
  the repository. `PSIV_SAVE_DIR` overrides that directory for a run or test.
* `StateSnapshot` does not model vehicles, button mappings, message/battle
  speed, macros, vehicle records, or the unidentified tech/skill/use bytes in
  each 0x80-byte character record. Those bytes are written as zero. The
  modeled roster, inventory, flags, money, equipment and live battle-carried
  fields are retained whole.
* `RetailLocation` carries world, both map selectors, and pixel coordinates.
  The current `Runtime` has one map namespace, so saves write world and the
  secondary selector as zero and load the primary map selector. A title seam
  that models planet/secondary-map selection can consume the preserved fields
  without changing the payload shape.
* Retail does not save facing in this range. `Runtime::from_save` explicitly
  starts a loaded party facing Down. The title screen is not present yet, so
  `PSIV_LOAD_SLOT=1`, `2`, or `3` (or `--psiv-load-slot=N`) selects the boot
  path; a missing selector starts the normal new-game path. A failed load is
  logged and falls back to new game rather than constructing a half-loaded
  runtime.
* The camp STATE screen's SAVE row and its wider three-slot chooser are modern
  wiring for this pre-title slice. Retail's actual SAVE entry remains the
  system menu route described above.

The round-trip tests in `psiv-core/src/save.rs` cover an all-bank,
all-roster, inventory, money, location save, checksum corruption, and a
mid-progress state with flags set, items held, damaged/statused HP, and changed
equipment. `GameState::from_snapshot` equality is asserted after decode.
