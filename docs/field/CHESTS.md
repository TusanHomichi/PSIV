# Native chest interaction

The field now loads all 155 extracted chest placements into the same object
pool as NPCs. They block movement and answer normal talk input. Previously,
the pack and core rules existed, but the runtime never attached the chests
and Godot neither drew them nor handled their contents.

## Original records and art

`LoadTreasureChests` loads NPCs first, then chests. Normal chests use object
`$A0`; the plant/white boxes use `$1D4`. Their opened bits live in `$F120`,
shared with extended event flags. Facing 0 is closed; facing 4 is open.

`psiv_tools/chest_sprites.py` decodes the original Nemesis assets and both
lid mappings into the existing field-object sheet index. The 155 placements
deduplicate to three sheets: normal, map-family `$32`, and white. The legacy
`npc_placements` manifest count includes the chest users of this shared index.

| Variant | Nemesis source | VRAM tile | Mapping pointer table | Palette |
| --- | --- | --- | --- | --- |
| Normal | `$2975AE` | `$4DC` | `$5143A` | fixed CRAM line 2 |
| Map general variable `$32` | `$2976FA` | `$4DC` | `$5143A` | map CRAM line 1 |
| White | `$297674` | `$4E6` | `$51442` | fixed CRAM line 2 |

Normal mappings retain their `(0,8)..(16,32)` union bounds; white mappings
retain `(-4,6)..(20,30)`. Closed/open sequences each use the original 16-tick
single frame. `Treasure.sprite` references these sheets, and pack loading
validates that both lid sequences exist. Old packs may omit this reference.

Chest item IDs are one-based. The map exporter previously indexed its
symbol list directly with the ID, naming every item one entry too late.
For example, item 133 is Escapipe and item 141 is Alshline. The numeric
contents were already correct. Gameplay labels use the extracted cartridge
display names.

## Grant and full inventory

`FieldRoutine_ItemFound` (`ps4.asm:137246`) opens the lid and plays `$E1`
before attempting the grant. Items take the first empty slot; meseta adds
the record's amount in hundreds. A successful grant sets the chest flag
once. Reopening a claimed chest reports `It is already open.`

The runtime owns a blocking loot transaction. Movement and story triggers
stay parked until the message is acknowledged. In particular, collecting
Alshline sets chest flag `$08`, then acknowledgement allows its conversation
to start; that scene sets event flag `$32`.

When all 40 slots are occupied, the lid opens but the chest flag stays clear.
The player can choose an inventory item to discard or return the found item
to the chest. Confirmation defaults to NO. Returning closes the lid, leaves
inventory and flags unchanged, and lets the chest be opened again.

The discard guard follows `loc_67ABA`: type 9 or a zero price cannot be
discarded. The equipment JSON now includes the original `$14` price word;
older packs without that word protect the item. The discard transaction
follows `loc_67CC6`: find the first matching item ID even if a later duplicate
was selected, remove it, compact once, and put the found item in slot 39.
It does not replace the selected slot in place.

Godot uses the original window/font assets. Its normal ITEM list now pages
all 40 items in eight-row pages, with matching text/cursor spacing. Child
ITEM, target, STATE and SAVE windows draw only the moving hollow selection
marker. The root keeps its separate red selection marker.

## Verification

- `rust/psiv-runtime/src/loot_tests.rs`: normal field input, solid objects,
  first-free grant, repeated opening, parked control, full-pack return,
  necessary-item protection, first-duplicate compaction, Alshline trigger,
  battle refresh and save reload. A core regression verifies that invalid
  chest or slot data cannot remove an inventory item before failure.
- The extraction tests cover chest records and sheets.
- `tools/native/native_chests.gd` opens a normal, a meseta, a white and a
  full-pack chest with ordinary input from isolated fixtures and saves; a fresh
  process loads the save through the title's CONTINUE
  (`tools/native/verify_native_continue.py`). The fixtures are transaction
  proof, not a campaign playthrough; the campaign route opens the Tonoe
  basement chest with ordinary input.

## Remaining fidelity work

The original full-pack window also offers USE for a carried or found item;
that branch is still missing. Its exact window geometry, two initial full
pack acknowledgements and original input timing also need comparison.
Chest-specific event staging (including the Psycho Wand and fake chest),
late map-object rewrites and a connected campaign route through these
treasures remain separate work. The campaign route opens the Tonoe basement
chest (item 141) with ordinary input; other chest-specific story paths still
need connected proof.

## Retail chest rules

### A chest is a field object

`LoadTreasureChests` (`ps4.asm:110966`) loads each chest through
`Field_LoadObject` into `Field_Obj_Secondary` — the same pool the map's NPCs
occupy — as type `$A0` (`FieldObj_TreasureChest`) or `$1D4`
(`FieldObj_WhiteTreasureChest`, the ones in plants). Both `bset #3, $2(a4)`, so
they are solid and talkable by the same bit-3 rule as any NPC, and both call
`FieldObj_OnScreenTest`, so they freeze off screen. Both ids are in
`psiv-core`'s `type_tests_visibility` table, which is a pleasing independent
confirmation of that table.

`GameMode_LoadFieldMap` calls `LoadMapObjects` before `LoadTreasureChests`, and
`Field_LoadObject` takes the first free slot, so **a map's chests occupy the
object slots immediately after its NPCs**. `Chest::object_slot` encodes that.

### The record

Four bytes (`ps4.asm:111000-111004`): white flag, `item_type` (`$39`),
`chest_flag` (`$3A`), `item_id` (`$3B`). `item_type` zero means `item_id` is an
item; non-zero means it is meseta in hundreds. The pack already emits all of
this per map as `treasure_chests`, and `psiv-data` validates the type/contents
pairing.

### Opening

`Interaction_ChkIfTreasureChest` (`ps4.asm:118579`) runs on whatever object the
ordinary talk probe found, matches `$A0`/`$1D4`, copies the record into the
interaction globals and switches to `FieldRoutine_ItemFound`. That routine, in
order:

1. tests the chest flag, and leaves without granting if it is set;
2. plays `SFXID_ChestOpened` and sets animation frame 4, the open lid;
3. grants — `money += item_id * 100` for meseta, first-free insert for an item,
   diverting to the swap path when the count reaches forty;
4. sets the chest flag, via `ChestFlags_Set` because `Interaction_Event_Type`
   is 1. (Type 0 would set an ordinary event flag; the same routine serves both,
   which is why chests and scripted item grants share it.)

**The grant precedes the flag, and that ordering is load-bearing.** A full
inventory diverts with the flag still clear, so the chest stays shut and can be
opened again once the player makes room. `GameState::open_chest` reproduces
this, returning `ChestOutcome::Full` without granting or flagging.

Open-state needs no storage: it is derived from the flag, exactly as
`LoadTreasureChests` derives the sprite frame by calling `ChestFlags_Test` as it
loads each chest.

### Which bank, and why it is shared

**Chest flags are `$F120` bits**, the same array as the extended event flags —
not `$F140`, whatever the disassembly's `Chest_Flags` label says. `state.rs`
carries the byte-level proof and the evidence chain. `Flag::chest(n)` is
exactly `Flag::event(0x100 + n)`.

The sharing is deliberate. Every literal-id `$F120` test site in the ROM reads
a real chest's flag — the five tower rings at `$A1`-`$A5`, plus `EclpsTorch`,
`FradeMantl`, `Canceller`, `PalmaRing`, `AeroPrism` and `RepairKit` at
`$08`-`$0D`. Story logic gates content on "has the player opened this chest".

And **nothing ever clears one**. The `$F120` clear door at `0x0576B2` has a
single caller in the whole ROM, `MapUpdate_ClrChestFlag`, which the
disassembly annotates "Not referenced" and which clears the unused id `$A9`.
Dead code. A chest, once opened, stays open for the life of the save — and the
eleven the initialiser pre-sets (`$27 $28 $2A $34 $38 $50 $5B $5D $6B $78
$A7`) can never be looted at all. Seven of them hold a HuntKnife across
unrelated maps, which reads as placeholder records deliberately switched off.

Temp flags are independent: `a_temp_flag_is_independent_of_the_chest_flag_of_that_id`
pins that, and `the_eleven_preloaded_chests_read_as_open_at_a_new_game` pins the
census.

A new game preloads eleven ids into the bank: `Event_GameStart` writes
`$F120-$F13F` at f902, and the runtime pack's `game_start` snapshot of
`extended_event_flags` carries the same thirty-two bytes:

```
0000000001a00880000080140010008000000000010000000000000000000000
```

Decoded with the chest rule (`base $F120`, byte `id >> 3`, mask
`1 << (7 - (id & 7))`), those eleven bits are ids `$27 $28 $2A $34 $38 $50 $5B
$5D $6B $78 $A7`, which the pack lists with a +256 bank offset (295 for `$27`).
The chest call sites (`LoadTreasureChests` at `$537E0`, the `ItemFound` entry
test at `$66B2A` and set at `$66DE0`) all address the `$F120` door, and
`$FFFFF143`, where these ids would land if `$F140` were the chest bank, stays
`00` on every chest tape. `ps4.constants.asm` mislabels the two doors: the one
named `Chest_Flags` (`$F140`) holds the temp event flags.

## Measured on the cartridge

Measured with the [oracle](../../oracle/README.md); frame numbers cite marks in
the named tape's log, which `oracle/verify.sh` regenerates.

### Object slots on a chest-bearing map

Tape 02's map has no chests, so the comparator had never exercised the slot
offset chest maps introduce. `AcademyBasement` ($15) carries 1 NPC and 2
chests, which is enough to settle the layout.

Measured at the spawn, before anything moves:

| slot | raw id | masked | cell | offscreen | rflags | timer |
|---|---|---|---|---|---|---|
| 0 | `$8184` | 388 | (44,18) | 1 | `$44` | 2 |
| 1 | `$80A0` | **`$A0`** | (40,8) | 0 | `$4C` | 0 |
| 2 | `$80A0` | **`$A0`** | (14,19) | 1 | `$4C` | 0 |

**NPCs fill the pool first, then chests, each in pack record order.** Slot 0 is
the Xanafalgue (`object_id` 388, `npcs[0]`); slots 1 and 2 are
`treasure_chests[0]` (Dimate, cell (40,8)) and `treasure_chests[1]` (100
meseta, cell (14,19)). So a chest's slot index is `npc_count + chest_index`.

The chest object ids are confirmed from `Interaction_ChkIfTreasureChest`:
**`$A0` = `FieldObj_TreasureChest`, `$1D4` = `FieldObj_WhiteTreasureChest`**.
Both basement chests are the plain kind; no white chest appears on this map, so
`$1D4` is unobserved here.

Two incidental checks fall out. Chests carry **`timer` = 0 permanently**, which
is consistent with the wander rule above — they are exactly the "never wanders"
case, and a core that ticks the RNG for every zero-timer object would
over-consume on every chest map. And `offscreen_flag` tracks visibility
sensibly: the near chest reads 0 and the far one 1 from the spawn.

### Opening a chest

The tape walks to (41,8), turns to face the chest at (40,8), and presses Speak.
(A straight walk west from the spawn does **not** reach it — a wall at column 46
stops the party at (47,8). The route is planned around the chest's own cell,
since objects block.)

The interaction itself is clean and gives `GameState::open_chest` its
timing:

| frame | event |
|---|---|
| f24665 | `Game_Mode_Routine` = `$08` (`FieldRoutine_Interaction`) |
| f24666 | `$24` (`FieldRoutine_ItemFound`); `Interaction_Event_Flag` = `$18` (24), `Interaction_Event_Type` = 1 (chest), `Found_Item` = `$7D` (125, Dimate), `Found_Item_Location` = 0 (from a chest) |
| f24675 | window opens and **`Inventory[0]` becomes `$7D`** — the item is granted 9 frames after the routine starts |
| f25069 | window closes on the dismissal press |
| f25080 | back to `FieldRoutine_Controls` |

Every value matches the pack record: `chest_flag` 24 = `ChestFlag_PiataMonomate`
($18), item 125 = Dimate.

**The chest flag is written inline, to `$FFFFF123`**, in the bank the clone
labels `Extended_Event_Flags`, not in `$FFFFF140-$FFFFF17F`, the bank the
constants file calls `Chest_Flags`. A whole-RAM diff (64KB of work RAM before
the open against after a round trip) finds it.

With that address logged, the write is exactly where the code says:
**`$FFFFF123` goes `00` -> `$80` at f24675 — the same frame the Dimate lands in
`Inventory[0]`.** Grant and flag are simultaneous, not deferred to a transition.
Byte 3, bit 7 is flag id 24 under the same `bit = 7 - (id & 7)` rule.

`$FFFFF143` — where id 24 would live if `$F140` were the chest bank — stays
`00` through the whole tape. Both facts are pinned in `verify.sh`.

**The lesson, worth generalising:** a negative result about RAM is only as good
as the address range it watched. "No write observed" needed a whole-RAM diff to
be trustworthy, and `--dump-ram <frame>:<path>` now exists on the host for
exactly that — dump two frames, diff, find what survives.

#### The chest is open on return, and there is no duplication

Tape 20 opens the chest, presses it again where it stands, then walks out to
`PiataAcademyNearBasement` and back:

| frame | event |
|---|---|
| f24667 | chest object's `facing_dir` 0 -> **4** (opened), one frame into ItemFound |
| f24675 | Dimate to `Inventory[0]` **and** `$FFFFF123` bit 7 set |
| f25573 | **re-press on the same visit**: routine `$08` -> `$24`, ItemFound entered again |
| f25582 | its window opens 9 frames later, as on the first press — but `Inventory[1]` stays `00` and `$FFFFF123` stays `$80` |
| f26381 | warp to map $12 |
| f27157 | back on map $15 |
| f27192 | **slot 1 (the opened chest) spawns with `facing` = 4; slot 2 (the untouched one) spawns `facing` = 0** |

So the chest **stays open** across the round trip, the item is kept, and it
cannot be taken again on either visit. No duplication.

(One authoring trap worth recording, because it silently voided the first cut
of this tape: the re-press opens a window, and an undismissed window pins
`Game_Mode_Routine` at `$24` indefinitely. The party took no further step and
the whole round trip below it never happened, while every position assertion
still passed because the party was standing where it was supposed to end up.
The tape now dismisses that window before walking.)

#### The no-duplicate guard

Two mechanisms, and they are separate:

- **The visible state is the chest object's own `facing_dir` (offset `$6`)**,
  which `ItemFound` overwrites with 4 (`ps4.asm`, `move.w #4, $6(a4)`). It is
  the only chest-slot byte that changes on opening. On reload it comes back as
  4 for the opened chest, so the load path initialises it from the flag.
- **The grant guard is the flag test at `ItemFound`'s entry** (`$66B2A`), not a
  refusal to spawn the chest as interactable. This is measured, and it is the
  one place the two candidate guards give different observable answers:
  re-pressing still reaches routine `$24` and still opens a window, so the
  object is fully interactable — it is the grant inside that is skipped. So
  "entered ItemFound" is never evidence of a grant; the inventory write is.

  The code agrees exactly. `FieldRoutine_ItemFound` (`ps4.asm:137246`) opens
  with `jsr (ChestFlags_Test)` and, on a set bit, branches to `loc_66E0C`,
  which creates a window, renders the string at `loc_2AA70A` and waits for a
  press — a full interaction with no grant in it. That is the second window the
  tape sees.

**And the same routine explains the whole bank question.** Its tail dispatches
on `Interaction_Event_Type` (`ps4.asm:137463-137475`):

| `Interaction_Event_Type` | call | door |
|---|---|---|
| 0 | `EventFlags_Set` | `$F100` |
| 1 | `ChestFlags_Set` | `$F120` |
| else | `TempEveFlags_Set` | `$F140` |

`Interaction_ChkIfTreasureChest` (`ps4.asm:118579`) hardcodes
`move.b #1, (Interaction_Event_Type)` for both chest object ids (`$A0` and
`$1D4`), so a chest always takes the middle row. Three distinct setters, three
distinct banks, chosen by a field the chest path pins to 1 — which is why a
chest flag and a "temp" flag of the same id can never be the same bit.

**For `psiv-core`:** grant and flag write are simultaneous, at routine-start
+ 9 frames. A chest's rendered open/closed state is `facing_dir`, not a
separate field, and it is derived from the flag at map load.

#### The second chest, and what it pays

Tape 21 opens `AcademyBasement`'s other chest, at (14,19), to test the bit rule
on a second id (one chest cannot separate the bit rule from a coincidence, because
byte 3 bit 7 is what a dozen arithmetics give for a single id):

| frame | event |
|---|---|
| f25129 | `Game_Mode_Routine` = `$08` |
| f25130 | `$24`; `Found_Item` = 1, **`Found_Item_Type` = 1 (meseta, not an item)**, `Treasure_Addr` = `$FFFFC380` (slot 2) |
| f25131 | chest `facing_dir` 0 -> 4 |
| f25139 | **`$FFFFF123` bit 6 set** and `Current_Money` 600 -> 700 |

Identical shape to the item chest — routine-start +1 to open, +9 to pay — with
`Found_Item_Type` discriminating meseta from inventory. The `Found_Item` value
is a **multiplier of 100**, not an amount and not an item id:
`mulu.w #100, d0 / add.l d0, (Current_Money)` (`ps4.asm:137348-137351`), so this
chest's `1` pays 100.

