# Field persistence: characters, inventory, treasure

Scouted and partly implemented 2026-08-15. Citations are to
`reference/ps4disasm/ps4.asm` and `ps4.constants.asm` unless stated otherwise.

Status at a glance:

| area | state |
| --- | --- |
| Inventory | **implemented**, `psiv-core/src/inventory.rs` |
| Treasure chests | **implemented**, `psiv-core/src/chest.rs` + `GameState::open_chest` |
| Map-load flag clears | **implemented**, `psiv-core/src/map_load.rs` |
| Character roster | **implemented**, `psiv-core/src/roster.rs` |

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

The award routine (`ps4.asm:4735-4782`, reached from `Battle_VictoryMessage`)
does it in two passes, and the details matter for the round trip:

**In the party.** For each occupied slot, `st gain_exp_flag(a0)` sets the flag
to `$FF` **unconditionally** — before any other check. Then `status & $44` is
tested, masking bits 2 and 6 (`StatusDead` and the android dead bit); if either
is set the character gains **no experience**. Otherwise `exp += award`, clamped
to **9,999,999**. So a character who died during the battle still earns the
flag, just not the experience.

**Out of the party.** Gated on `EventFlag_Reunion` being **clear**. While it is,
a second pass walks all eleven records (`moveq #$A, d7`, stride `$80`), skips
anyone already awarded as a party member, and gives the same award to any
character whose `gain_exp_flag` is set — same 9,999,999 clamp. Once Reunion is
set, benched characters stop gaining.

Slot to record is `lsl.w #7, d0`, i.e. id x `$80`, which is a third independent
confirmation of the stride.

## The shared invariant: `Stats` is the persistent record

**Adjudicated 2026-08-15 (team lead): option 1.** `battle::Stats` *is* the
persistent per-character record. `GameState` owns eleven of them and there is no
projection or conversion layer.

The cartridge settles this rather than taste. There is one copy of each
character at `$F500 + n * $80`, and battles read and write it **in place** —
`FillBattleStats` (`ps4.asm:11272`) walks `Character_Stats` itself with
`moveq #$A, d7` and stride `$80`, and the experience award above writes `exp`
straight into the same records. Nothing anywhere copies a character into a
battle-local structure and back. A projection layer would be inventing a seam
the hardware does not have, and inventing a seam is inventing a place to drift.

### Governance

`psiv-core/src/battle/stats.rs` remains **battle-scout-lane's file**. `Stats` is
a **declared shared interface type**: neither lane changes its shape without the
lead's sign-off. Both lanes cite this section.

### The round-trip contract

State survives a battle unless it is in the `_battle` tier.

| tier | fields | survives a battle? |
| --- | --- | --- |
| persistent | `level`, `exp`, `curr_hp`, `curr_tp`, `max_hp`, `max_tp`, `status`, `gain_exp_flag`, `profession`, equipment, element props | **yes** |
| base | `strength`, `mental`, `agility`, `dexterity` | yes — only levelling changes them |
| `_mod` | the same four, plus `atk_pow` / `dfs_pow` / `magic_dfs` | yes — derived from base + equipment by `UpdateCharModStats`, so it changes only when equipment or base does |
| `_battle` | the same seven | **no** — `FillBattleStats` overwrites every one from its `_mod` at the start of every battle |

Stating it as a tier rule rather than a field list is deliberate: a list drifts
as fields are added, whereas "everything except `_battle`" stays true and tells
a reader what to check when they add one.

Two consequences worth naming, because they are the ones a conversion layer
would have got wrong:

- **HP and TP are not restored after a battle.** They are the same words the
  battle spent, so the field carries the damage out. Healing is an explicit act.
- **`_battle` values are persisted, and that is correct.** An earlier revision
  of this section said they must never be written to a save. That was wrong.
  The save routine (`ps4.asm:134843`) copies `#$27F` longwords from
  `Event_Flags` — 2560 bytes, `$F100`..`$FB00`, annotated in the source as
  "save data for the Event Flags through the Vehicle Stats". That span contains
  all eleven `Character_Stats` records whole, `_battle` bytes included, so
  retail saves them.

  The real invariant is narrower and is about reads, not writes: **a `_battle`
  value must never be read before `FillBattleStats` has recomputed it.** It is
  stale between battles; it is simply never consulted while stale.

The oracle's level-up tape is the ground truth for what actually moves, and
pinning the round trip against it belongs to the roster slice.

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

## 2a. Camp menu Tier 1

`psiv-runtime/src/camp.rs` exposes a renderer-safe snapshot of the live party
roster, forty-slot inventory, read-only equipment names, next-level experience,
and meseta. `psiv-godot/src/camp/` owns only the decoded windows and input.
Consumable item commands return to the runtime and mutate `GameState` through
`inventory_mut().remove()` and `roster_mut().get_mut()`. Healing and status
cures use the `item_effects` records from `battle/abilities.json`; equipment
slots are display-only in this slice.

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

## 4. Map load clears flags — implemented

Until this, the engine never cleared a flag by itself. Oracle tape 18 measured
that it must: a clear lands 39 frames after arriving on the destination map,
which is what respawns the Xanafalgue and turns the Piata Academy basement into
a repeatable un-looter of the Garuberk Tower Moon Slasher chest.

`MapDataManager` (`0x051B38`, `ps4.asm:107815`) is called from
`GameMode_LoadFieldMap` and walks the map record's `$FFFF`-terminated list of
entry ids, dispatching each through `MapDataManagerJmpTbl`. Some of those
routines clear specific `$F140` flags.

- **Which routine**: whichever `MapDataMan_*` entries the *destination* map's
  list names. There is no single clearing routine.
- **Which ids**: an explicit literal list per routine — not a range, not a bulk
  pass, not derived. Seven routines name twenty ids between them, every one
  written out as an immediate.
- **Destination-load-tied?** Yes, and the Xanafalgue case shows it plainly:
  `MapDataMan_NearPiataBasement` is entry `$18` and it appears on map `$012`,
  `PiataAcademyNearBasement` — the map you *arrive* on, not the basement you
  left. That is exactly the oracle's load+39.

| entry | routine | clears | on maps |
| --- | --- | --- | --- |
| `$14` | `MapDataMan_MovingPlatforms` | `$09 $0A $0D $0E $0F $10` | Motavia, Dezolis |
| `$17` | `MapDataMan_Terminals` | `$0B $0C $11 $12` | Vahal Fort, Weapon Plant |
| `$18` | `MapDataMan_NearPiataBasement` | `$13` | `$012` |
| `$3D` | `MapDataMan_GaruberkTowerPart4` | `$15` | `$19C` |
| `$3E` | `MapDataMan_GaruberkTowerPart5` | `$17` | `$19D` |
| `$47` | `MapDataMan_LeRoofRoom` | `$19` | `$0F0` |
| `$84` | `MapDataMan_ClrBioPlantAlarm` | `$08` | Bio Plant exit |

Two clear sites are deliberately excluded because they are not map load.
`FieldObj_EsperGuard` clears `$1A` while the guards animate — that belongs to
the object's routine. And `MapUpdate_ClrChestFlag` (`MapUpdateJmpTbl` entry
`$38`) clears `$A9` every frame, but the disassembly annotates it **"Not
referenced"**: dead code, recorded here so nobody else chases it.

Every id above is a `$F140` id, and `$F140` carries **only** temp flags. An
earlier revision concluded that these clears un-loot chests — that clearing
`$13` re-armed `ChestFlag_GrbrkTwMoonSlashr`. Withdrawn: chest flags are
`$F120` bits and nothing clears them.

Tape 18's *measurements* stand — the clear happens on destination load and the
Xanafalgue respawns. Only the chest consequence, which the ledger itself
flagged as an inference rather than a measurement, is gone.

### Withdrawn: the temp/chest alias

Two earlier revisions of this document claimed temp flags and chest flags share
bits — first six colliding ids, then "essentially the whole `$08`..`$1D`
range". **Both are withdrawn.** They rested on `$F140` being the chest bank,
which it is not. `TempEveFlag_*` ids and `ChestFlag_*` ids of the same number
are different bits in different arrays and never interact.

What survives is the *other* sharing, which is real and deliberate: chest flags
and extended event flags are one bank, `$F120`.

### Extractor gap: the pack carries no flag-clear data

The map-effects extractor already emits each map's `MapDataManager` entry list,
which is what `map_load::apply_map_load` consumes — so the binding is available.
But the decoder does not recognise the clears themselves:

- The three entries that call the `$F140` clear door at `0x0576BC` (`$17`,
  `$3D`, `$3E`) **fail to decode**, two on `opcode 0x0C6C` and one on `opcode
  0x103C` — `move.b #id, d0`, the byte form of the immediate load. The decoder
  knows `move.w` (`303C`) but not `move.b`.
- The entries that *do* decode (`$14`, `$18`, `$47`) come back with
  `kinds: []` — walked successfully, but with no vocabulary for a flag-clear
  write, so nothing is emitted.

Either way the pack contains no record that any flag is ever cleared. The table
in `map_load.rs` is transcribed from the disassembly to fill that gap; adding a
`flag_clear` kind to the decoder (and the `move.b` immediate form) would let it
come from data instead, and is a `psiv_tools` change rather than a core one.

## Save-format deltas

`StateSnapshot` gained `inventory: [u8; 40]`. A snapshot written before this
existed has no item list; loading one should treat the party as carrying
nothing, which is what a new game holds anyway. This is the second delta of the
day — the flag-bank merge collapsed `chest_flags`/`temp_flags` into a single
32-byte array.

## Open

- **Character roster.** The live Tier-1 camp display is implemented in the
  runtime camp snapshot and Godot camp menu; save/load presentation remains
  outside this slice.
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

## The roster, as built

`psiv-core/src/roster.rs`. `GameState` owns a `CharacterRoster` of eleven
`Option<Stats>` seats, seated through battle-scout-lane's
`PartyMember::seat` / `Stats::from_character` path — no constructor of its own,
because a second way to build a character record is a second way for one to be
wrong.

**The round trip is `absorb`, and it replaces records whole.** `into_party`
hands back the same `Stats` that went in, so nothing is picked out and nothing
is merged. That is what makes "everything survives except `_battle`" true by
construction rather than by maintenance: a field added to `Stats` later
round-trips without anyone remembering to update this.

**`gain_exp_flag`, both passes.** `award_party` sets the flag on every occupied
slot *before* the status check and pays only the living; `award_absent` pays
every other seated character carrying the flag, and returns nothing at all when
`EventFlag_Reunion` (`$DA`) is set. `GameState::award_experience` runs both and
reads Reunion from its own flags. Both clamp at 9,999,999.

Note the asymmetry, which is the cartridge's: the party pass tests status, the
absent pass does not. A dead benched character is paid.

`battle::split_rewards` documents the absent pass as out of its scope —
"that needs the roster of everyone recruited so far, which battle does not
own" — so the two halves meet here.

### Pinned against the level-up tape

`the_level_up_tape_round_trips_through_the_roster` uses tape 10's measured
numbers (`oracle/logs/verify_levelup.csv`), Chaz's second level-up:

| frame | change |
| --- | --- |
| f51600 | level 1, exp 17, hp 20/25, tp 10/10, str 8 men 6 agi 7 dex 5 |
| f51785 | exp 17 -> 26 (pool 27, three recipients, 9 each) |
| f51786 | level 1 -> 2 |
| f51817 | str 8 -> 9, men 6 -> 7 |
| f51833 | agi 7 -> 8, dex 5 -> 6 |
| f51849 | maxhp 25 -> 31, maxtp 10 -> 13 |
| f51899 | level 2, exp 26, hp **20**/31, tp **10**/13 |

The stat rises are staged over ~64 frames for presentation; only the end state
persists. The load-bearing line is the last one: **a level-up raises the maxima
and does not refill.** Chaz walks out on 20 of 31.

Computing the rise is `battle::apply_level_ups` and is tested there. What this
pins is the half the roster owns — the award arithmetic and what persists.

## Superseded: the cut proposal

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

## Interaction areas precede the object probe (2026-08-15)

`FieldRoutine_Interaction` checks the map's fixed interaction-area records on
confirm before it probes field objects. The map-`$17` boss approach is the
first record: raw coordinates `$1E,$12` (8-pixel units), range `$09`
(`XPlus20_YPlus10`), routine type 2, parameter `$0B`. Its pack rectangle is
collision cells `(15,10)` through `(16,10)`, so a party standing at `(15,11)`
and facing up resolves Event `$6B` before the invisible blocker can answer
"Nothing here". The runtime carries the explicit area record and event-index
lookup so this ordering is preserved in the headed game as well as in tests.

Only when no enabled interaction area matches does the field path continue to
the object probe. The separate object rule still matters for ordinary NPCs
and objects, but it is not the route that starts the Academy Basement boss.
