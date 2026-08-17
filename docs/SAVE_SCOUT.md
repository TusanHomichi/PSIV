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
(`ps4.constants.asm:2398-2409`). The serializer writes every modeled byte at
its retail offset, including the stale `_battle` tier, current HP/TP, status,
equipment, element pairs, name buffer, techniques, skills, skill-use pairs,
`gain_exp_flag`, and the physical-property save byte. It does not rederive
battle values; the retail save copy is whole-record state.

### Character-record byte census

There is no unexplained character-record byte left in the serializer. The
following is the complete `$00..$7F` census for one record:

| record bytes | meaning | status |
|---:|---|---|
| `$00..$05` | six-byte WinCharset name buffer; `InitializeCharStats` writes `$FE` after the source name and the remaining bytes stay zero | named |
| `$06..$07` | profession word | named |
| `$08..$09` | level word | named |
| `$0A..$0D` | experience longword | named |
| `$0E..$0F` | current HP word | named |
| `$10..$11` | maximum HP word | named |
| `$12..$13` | current TP word | named |
| `$14..$15` | maximum TP word | named |
| `$16` | status bits | named |
| `$17` | no stored field; the physical-attack routine uses `$17 + 3*n` as an arithmetic base to reach the battle-stat bytes, but no character-record routine reads or writes this byte by itself | proven padding/unused |
| `$18..$20` | strength, mental and agility triples: base, modified, battle | named |
| `$21..$23` | dexterity triple: base, modified, battle | named |
| `$24..$27` | attack power and battle attack power | named |
| `$28..$2B` | defence power and battle defence power | named |
| `$2C..$2F` | magic-defence power and battle magic-defence power | named |
| `$30..$4B` | fourteen two-byte element properties; high bytes are live/derived and low bytes are the record shadow | named |
| `$4C..$4F` | right hand, left hand, head and body equipment ids | named |
| `$50..$51` | cached right/left weapon elements | named |
| `$52..$61` | sixteen technique ids | named |
| `$62..$69` | eight character skill ids; enemy stats instead use `$68..$69` as the `enemy_id` union | named |
| `$6A..$79` | eight current/max skill-use pairs, current at even offsets and maximum at odd offsets | named |
| `$7A` | gain-experience flag | named |
| `$7B` | physical-property save byte in the project’s ratified bugfix branch | named/project extension |
| `$7C..$7F` | no character-stat consumer addresses this tail; the stride is `$80` and the retail initializer leaves it zero | proven padding |

The name copy is `ps4.asm:88679-88697`. The technique, skill and current/max
use copies are `ps4.asm:88793-88810`; level-up writes the same arrays at
`ps4.asm:5951-5972`, and macro skill selection reads the skill list and use
bytes at `ps4.asm:7077-7091`. The field names and offsets are also explicit in
`ps4.constants.asm:6-61`. The `$17` references at
`ps4.asm:9642-9647` are indexed arithmetic, while the standalone `$17` uses
at `ps4.asm:89311-90212` belong to field-sprite records, not
`Character_Stats`; no character-stat path addresses `$7C..$7F`.

### Vehicle records

`Saved_Vehicle_Stats` starts at `$FA80`, with Land Rover, Ice Digger and
Hydrofoil at `$FA80`, `$FAA0` and `$FAC0` respectively
(`ps4.constants.asm:2412-2415`). Each record is 0x20 bytes:

| record offset | meaning | status |
|---:|---|---|
| `$00..$01` | current HP | named |
| `$02..$03` | maximum HP | named |
| `$04` | skill availability mask | named |
| `$05` | no saved-vehicle consumer | preserved reserved byte |
| `$06/$07`, `$08/$09`, ... `$14/$15` | eight current/max skill-use pairs | named |
| `$16..$1F` | no saved-vehicle consumer | preserved reserved tail |

The new-game values are written at `ps4.asm:88707-88730`. Battle setup loads
the static vehicle profile from `VehicleData` and then overlays the saved mask
and sixteen use bytes at `ps4.asm:11295-11403`; the static profiles and their
vehicle ids are at `ps4.asm:321107-321173`. Battle exit copies live vehicle
current/max uses back to the selected saved record at `ps4.asm:6375-6387`,
and `DoVehicleRecovery` refreshes each current byte from its paired maximum at
`ps4.asm:136536-136550`. That is the save mapping: HP and mask at the front,
then eight interleaved current/max use pairs, with a 0x20-byte stride.

`StateSnapshot` now carries all three records, including the reserved bytes,
so a fabricated mid-game vehicle state survives core serialization and the
runtime file load/save seam. Settings at `$F43C..$F442` and the eight 20-byte
macro records at `$F444` are modeled alongside them. The new-game defaults are
the values written at `ps4.asm:88667-88749`; macro command byte meanings are
documented in `ps4.constants.asm:2391-2396`.

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

`Title_EraseOption` uses a separate two-choice confirmation window. YES passes
the selected zero-based slot to `loc_64DC0` while SRAM write mode is enabled
(`ps4.asm:87746-87910`). The routine masks the selector with `#3`, multiplies
it by the physical slot stride `$1400`, and executes `#$280` `MOVEP.L` writes
of zero from `$200200 + slot*$1400` (`ps4.asm:87931-87953`). Therefore the
retail erase clears exactly the selected 0x1400-byte physical payload block;
the common 0x200-byte header, including the signature and checksum-table
words, survives. The resulting selected slot is checksum-invalid until a new
save overwrites it. The three-file disk model applies the same operation to
`saves/slot_N.sram`; `PSIV_SAVE_DIR` is the test/runtime boundary.

`Runtime::from_save` remains the load seam: decode the slot, rebuild
`GameState`, evaluate the selected map's load effects against it, construct the
party at the saved cell, and default facing to Down. Godot's title Continue
path and the `PSIV_LOAD_SLOT=1..3` / `--psiv-load-slot=N` debug selectors use
that seam.

## Divergences and load semantics

The implementation is byte-compatible for the retail header, interleaving,
payload offsets, character stride, signature, checksum, settings, macros,
character records and vehicle records. The only remaining save-format
divergence is the disk representation:

* The disk uses three per-slot files instead of one shared SRAM device. Files
  are `saves/slot_1.sram` through `saves/slot_3.sram`; `saves/` is ignored by
  the repository. `PSIV_SAVE_DIR` overrides that directory for a run or test.

The following are runtime-surface notes, not save-payload divergences: the
current runtime has one active map namespace and therefore consumes the
primary map selector; it retains the world and secondary selectors through a
load/save cycle. Retail does not save facing in this range, so
`Runtime::from_save` starts the party facing Down. The title screen and the
retail system-menu SAVE route are not present in this slice; the modern STATE
SAVE row and chooser are only the current shell’s access path.

The round-trip tests in `psiv-core/src/save.rs` cover an all-bank,
all-roster, inventory, money, settings, macros, location save, checksum
corruption, and a mid-progress state with flags set, items held,
damaged/statused HP, changed equipment, skill bytes, and vehicle state.
`GameState::from_snapshot` equality and logical payload byte identity are
asserted after decode.
