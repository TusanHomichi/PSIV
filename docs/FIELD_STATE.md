# Field persistence: characters, inventory, treasure

Scouted and partly implemented 2026-08-15. Citations are to
`reference/ps4disasm/ps4.asm` and `ps4.constants.asm` unless stated otherwise.

Status at a glance:

| area | state |
| --- | --- |
| Inventory | **implemented**, `psiv-core/src/inventory.rs` |
| Treasure chests | **implemented**, `psiv-core/src/chest.rs` + `GameState::open_chest` |
| Character roster | **scouted, not implemented** — see the cut proposal at the end |

## 1. The character record

`Character_Stats` is at `$FFFFF500` with a stride of `$80`, and the RAM map
names all eleven characters individually, which pins both facts without
arithmetic: Chaz `$F500`, Alys `$F580`, Hahn `$F600`, Rune `$F680`, Gryz
`$F700`, Rika `$F780`, Demi `$F800`, Wren `$F880`, Raja `$F900`, Kyra `$F980`,
Seth `$FA00` (`ps4.constants.asm:2398-2409`). Eleven records, `$580` bytes
total, running to `Saved_Vehicle_Stats` at `$FA80`.

The layout (`ps4.constants.asm:9-61`):

| offset | field | width |
| --- | --- | --- |
| `$06` | `profession` | word |
| `$08` | `level` | word |
| `$0A` | `exp` | longword |
| `$0E` / `$10` | `curr_hp` / `max_hp` | word |
| `$12` / `$14` | `curr_tp` / `max_tp` | word |
| `$16` | `status` | byte, bitflags |
| `$18`-`$23` | strength, mental, agility, dexterity | 3 bytes each: base, `_mod`, `_battle` |
| `$24`-`$2F` | atk_pow, dfs_pow, magic_dfs | 2 words each: derived, `_battle` |
| `$30`-`$4B` | fourteen element properties | word each |
| `$4C`-`$4F` | right hand, left hand, head, body | byte each |
| `$52` | techs | |
| `$62` | skills | |
| `$6A` / `$6B` | curr / max skill uses | byte |
| `$7A` | `gain_exp_flag` | byte |
| `$7B` | `physical_prop_save` | byte |

### The three-tier stat pattern

Base, `_mod`, `_battle` is not three copies of one number. Base is the
character's own stat; `_mod` is base plus equipment; `_battle` is the working
copy a fighter carries into a round and that buffs and debuffs move.

`UpdateCharModStats` (`ps4.asm:127814`) computes the `_mod` tier: seven passes,
each summing one bonus byte across the four equipment slots via
`AddItemBonusToCharStats`, which indexes `InventoryData` records of `$16` bytes
each and adds `(a2,d1.w)` to the base.

**This is already implemented.** `psiv-core/src/battle/stats.rs` has it as
`Stats::update_mod_stats`, with the seven-row bonus table transcribed and the
byte-wrapping / word-signed distinction reproduced. Field persistence must
reuse it, not re-derive it.

### `gain_exp_flag`

`$7A`, one byte: set once a character has been in a winning battle, after which
they gain experience even while out of the party. This is what makes the
eleven-character roster necessary rather than a five-slot one — benched
characters keep levelling.

### The new-game initialiser

The pack already extracts it. `runtime-pack/game_start.json`'s `new_game_init`
carries `character_stats` (routine `0x044652`, source `0x2A8ACA`, applied to all
eleven whether or not they are in the party), `party_slots` (Chaz and Alys, four
empty), `money` (500), `settings`, and the flag banks.

## 2. Inventory — implemented

`Inventory` (`$FFFFF410`) runs to `Current_Money` (`$FFFFF438`): **forty bytes,
one item id per slot, `0` for empty**. It is **party-wide**. The only
per-character item bytes are the four equipment slots in the stat record, and
`ps4.asm:1802-1807` shows the two treated separately — `moveq #3` over
`equipment(a4)`, then `moveq #39` over `(Inventory).w`.

Item ids start at 1 (`ItemID_Dagger = 1`), so `0` is unambiguously empty and can
never collide with a real item.

Two loops in `FieldRoutine_ItemFound` (`ps4.asm:137246`) define the semantics:

- **Counting** walks all forty slots and counts the non-empty ones, then
  `cmpi.w #40, d0 / bcs` takes the normal path only when there is room. It
  counts rather than finding the first gap, so a list with holes counts right.
- **Inserting** scans for the first zero and writes there. **First-free, not
  appended and not sorted** — a hole left by a removal is refilled before the
  tail is used.

One retail detail worth recording: the insert loop has no guard of its own. If
it ever ran on a full list it would fall out of the `dbf` with the pointer one
past the end and write to `-(a0)`, clobbering slot 39. Retail never reaches it
because the count above diverts a full inventory first. `Inventory::add`
returns `MapError::InventoryFull` instead of reproducing an overwrite the
cartridge cannot actually perform.

## 3. Treasure chests — implemented

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

### The alias is live here

Chest flags are `$F140` flags, and retail uses that bank for temp event flags
too — six ids under both names (see `psiv-core/src/state.rs`). So opening
certain chests writes state a trigger reads as a temp flag, and tripping certain
temp flags marks a chest as already looted. `a_chest_flag_and_its_alias_temp_flag_are_one_bit`
pins the Alshline/BioPlantAlarm case in both directions. Nothing here tries to
prevent it.

## Save-format deltas

`StateSnapshot` gained `inventory: [u8; 40]`. A snapshot written before this
existed has no item list; loading one should treat the party as carrying
nothing, which is what a new game holds anyway. This is the second delta of the
day — the flag-bank merge collapsed `chest_flags`/`temp_flags` into a single
32-byte array.

## Open

- **Character roster.** Scouted above, not implemented — see below.
- **Object slots on maps with chests.** The comparator's object columns index
  the shared pool, and `psiv-core`'s object list is currently the pack's NPCs
  only. On a map with chests the engine's slot indices would run short of the
  oracle's by the chest count. Tape 02's map (PiataAcademy_F1) has no chests, so
  the clean 339-column result does not exercise this. A tape on a chest-bearing
  map would, and `Chest::object_slot` is the piece that makes it correct once
  `FieldMap` carries chests.
- **Equip and unequip.** The `$4C..$4F` slots are read by `update_mod_stats`,
  but nothing yet writes them outside of the initialiser. The menu flow that
  does — and whether equipping re-derives `_mod` immediately — is unscouted.
- **`InventoryData` bonus records** are `$16` bytes each and already decoded on
  the Python side; the `_mod` derivation consumes them through `psiv-data`'s
  `ItemRecord`.

## Cut proposal: the character roster is its own slice

The roster is not deep in RE terms — the struct is transcribed above and the
hard part, `update_mod_stats`, already exists. It is deep in *ownership* terms,
and that is the reason to split it.

`battle::Stats` is already, field for field, the cartridge's `$80` character
record: profession, level, experience, hp, tp, status, the three-tier stat
triples, the derived pairs, element properties, equipment, `gain_exp_flag`. The
persistent field record and the battle record are not two structures that need
converting between — they are the same structure, which is exactly why the
cartridge keeps one copy at `$F500` and battles read and write it in place.

So the design question is one decision: **does `battle::Stats` become the
persistent per-character record that `GameState` owns eleven of, or does
`GameState` own a separate record that projects into `Stats`?**

The first is faithful and avoids a conversion layer that can drift. It also
means the type lands in battle-scout-lane's file, and the round-trip contract
(what persists out of a battle: HP, TP, exp, level, status, `gain_exp_flag`)
becomes a shared invariant rather than one lane's. Doing that unilaterally is
how the runtime `lib.rs` interleaving happened, so I have not.

Recommendation: settle the ownership question first, then implement the roster
against the oracle's level-up tape in one focused slice. The scout above is
complete enough that it should be a short one.
