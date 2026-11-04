# `Event_PickingFifthCharacter`: the fifth member at the Mota Spaceport

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0068` `Event_PickingFifthCharacter` | `$071E66..$072181` code, `$072182..$0721CB` tables | `EventPtrs[$68]` at `$05A2B4 + $1A0` | `fifth_character.rs`, `PICKING_FIFTH_CHARACTER` (19 ops), `ROWS`; `scene_runner/fifth.rs` |

- **Trigger:** dialogue `$F6 $00 $68`, entry 25 of tree 39: objects 5 to 9 of `MotaSpaceport` `$0BF`
  (`GryzSpaceportWaiting`, `Hahn`, `Demi`, `Raja`, `Kyra`, dialogue 25, `$FFFFC440` on).
- **Flags:** clears all five `EventFlag_*Picked` (`$DB..$DF`) and sets the one picked. Readers:
  `MapDataMan_MotaSpaceport`, here, and `TalkOption_DialogueIDs` (`ps4.asm:125857`), the camp's Talk tree.

## Clone audit

The routine was read from the US image through `EventPtrs[$68]`, and its words match
`ps4.asm:151681-151960`; no `revision` or `grand_cross` conditional falls in the range. `loc_72182`
(`$072182`) has five eight-byte rows: X and Y, the character id, the first line, the flag, and an art offset.

## Who is at the spaceport: `MapDataMan_MotaSpaceport`

Entry `$67` (`ps4.asm:110162`) writes the object list on every load:

| Story | Objects 0-4 (`NPC*Spaceport`) | Objects 5-9 (`*SpaceportWaiting`) |
|---|---|---|
| Elsydeon `$D9` clear | cleared | cleared |
| `$D9` set, Reunion `$DA` clear | present | cleared |
| `$D9` and `$DA` set | cleared | present, less the one whose `*Picked` flag is set |

The extractor gave up on this entry until this change. Seven independent gates make 128 paths, the
reader's limit was 64 (`MAX_PATHS`, `psiv_tools/map_effects/decoder.py`), and an undecoded entry applies
nothing, so every object stood there from the first visit. The limit is now 256, and the entry decodes
(`tests/test_map_effects.py`) as every other `object_despawn` does. Correction-ladder rung: that test
pins the entry's whole table, and `census.undecoded_entries` still names each entry the reader refuses.

**The route.** An empty spaceport before Elsydeon makes the route's first visit 16 frames shorter,
and the RNG after it moves. The release route still completes all 84 chapters (6,165,067 frames, digest
`f1ac64ed4f1b3835`, was 6,260,544 and `348595da315ac589`). Its resume manifest follows the new halts:
Dark Force 2 and 3 each lose one resume, [#320](https://github.com/TusanHomichi/PSIV/issues/320).

## Retail transcription

| Ops | ROM | Retail | Scene |
|---:|---|---|---|
| 0 | `$071E66..$071EA5` | `loc_71E74`: `Field_UpdateObjects`, `RunMapUpdates` and `VInt_Prepare` until each party object's `$28`/`$2A` are zero | `FifthOp::WaitParty` |
| 1 | `$071EA6..$071EC5` | `loc_71EAE`, the five records of `loc_721B8`: `curr_hp` from `max_hp`, `curr_tp` from `max_tp`, `clr.b status` | `FifthOp::RefillRecords` |
| 2 | `$071EC6..$071F2F` | `loc_721B0` by the leader's facing gives the cell ahead; the row whose X and Y match goes in `Window_Option_Index_2`, and that object turns to the leader | `FifthOp::PickRow` |
| 3-4 | `$071F30..$071F4B` | `$5(a0)`, then `$5(a0)` plus 2 | `FifthOp::RunLine` |
| 5 | `$071F4C..$071F53` | `tst.b (Yes_No_Option).w / bne.w loc_72128` | `BranchChoice` |
| 6 | `$071F54..$071F5B` | `tst.b (Current_Party_Slot_5).w / bmi.w loc_72080` | `FifthOp::BranchFifthSlotFree` |
| 7-8 | `$071F5C..$071F8B` | `loc_72156`, the first party slot holding a table character; `$5(a0)` of that row plus 3 | `FifthOp::PickLeaver`, `RunLeaverLine` |
| 9-10 | `$071F8C..$07207F` | The exchange, below; `bra.w loc_720E8` | `FifthOp::Exchange`, `SkipOps` |
| 11 | `$072080..$0720E7` | The picked object's struct into `Character_5`, and the object cleared; `Current_Party_Slot_5`; `Event_AddMacro` | `FifthOp::TakeFifthSlot` |
| 12-17 | `$0720E8..$072127` | `EventFlags_Clear` on all five, `EventFlags_Set` on `$6(a0)` | `SetFlag` five times, `FifthOp::SetPicked` |
| 18 | `$072128..$072141` | `clr.l (Window_Option_Index).w`, `Event_MoveCamera` to the leader, speed 2 | `PanCamera` |

**The exchange** (`$071F8C..$07207C`). `Event_RemoveCharFromMacros` on the leaver. Their party object's struct
goes over their home object, which takes dialogue `$19` and `bset #3, $2(a1)`, and goes back to its stand with
`Event_MoveObjectStatic`: the leaver is back at once, facing as they did in the party. The picked object's
struct goes over the party object, whose destination is set to its own position (`$38`, `$3A`); the picked
object clears. `Current_Party_Slots` takes the new id in the leaver's slot, and `Event_AddMacro` follows.

**Walking in.** The new member's party object stands where the friend stood, far from the trail. It stays
until the leader moves; then, like any party object with a distant destination, it walks to the cell the member
ahead vacated (`FieldObj_GetAutoInput`, `ps4.asm:93232`, X first by default) and is on the trail when it gets
there. The port's followers otherwise hold an adjacent trail, and snap when one is not, so `Party::walk_in` keeps
this walk to the slot the scene seated and those behind it, ending at their destination or a map load.

**Macros.** `GameState::remove_from_macros` and `add_to_macros` are `Event_RemoveCharFromMacros` and
`Event_AddMacro` on the saved macros: each macro in use (first command set) loses the leaver with the rest moving
up, and takes the new member's plain attack in its first free slot.

## Deviations

- The eighth byte of each row and `loc_721AA` build the object routine words (`$82BC` plus the byte, and `$8000`
  plus the byte). The port draws a restored object from its map record and a party member from its roster id, so
  neither is carried. The restored object's facing is the party object's, as the struct copy leaves it.
- Where the cartridge's two table walks find nothing they run on past the table. The first would need a friend who
  is not on a stand; the second a full party with no table character, which no story state after `Cutscene_Reunion`
  allows. The port faults (`SceneFault::NoFifthRow`) instead of inventing a row.
- The cartridge traps (`ErrorTrap`) on a macro in use that lacks the leaver or has no free slot. The port leaves that
  macro alone: a port save whose scenes never wrote the macros (`PresentationOp::AddMacro` prints and writes nothing)
  can hold such a macro.

## Tests

- `rust/psiv-core/tests/fifth_character_scenes.rs`: each friend into the free slot, the exchange in slot 4 and slot 1,
  NO, the macros, and the fault. `rust/psiv-core/src/party.rs`: the walk-in, X first, ending on the trail and at a
  map load. `tests/test_map_effects.py`: the spaceport's table from the ROM.
- `rust/psiv-campaign/tests/fifth_character.rs`: pads on the real map from hand-built saves, since the route reaches
  this map after the reunion but takes no fifth and a chapter would run most of the game for a side choice. Who stands
  there in each story state, Hahn joining and walking in, Kyra replacing him and back, NO. State and input-path
  evidence in the headless runtime.
