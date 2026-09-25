# Shops, inns and what they charge

Scout + extraction, 2026-08-15. Every address below was verified against the
retail image (`Phantasy Star IV (USA).md`, sha256 `511f35cc…`) before it was
used; `reference/ps4disasm` supplies the labels only. Citation standard is
`docs/battle/BATTLE_SCOUT.md`'s: a claim is either a byte offset you can check or it
is marked as an inference.

Extractors: `psiv_tools/shops.py` (the two tables, pre-existing),
`psiv_tools/shop_flow.py` (the six routines), `psiv_tools/shop_pack.py`
(`runtime-pack/shops.json`). Tests: `tests/test_shop_flow.py`.

---

## 1. The three systems behind one counter

| system | table | retail | count |
|---|---|---|---|
| what a shop sells | `ShopInventories` | `$0681A4..$0682A3` | 49 lists |
| where a counter is | `loc_68394` | `$068394` | 68 rows |
| what an inn charges | `loc_68112` | `$068112` | 18 words |
| which greeting a shop uses | `loc_68136` | `$068136` | 49 words |
| which portrait a keeper has | `ShopPortraitGroupsPtrs` | `$067FFE` | 2 groups, 18 + 49 |

The four data tables are flush against each other: the shopkeeper portrait
group ends exactly where `loc_68112` begins, which is what bounds it, and
`ShopInventories` ends at its `even` pad byte before `loc_682A4`. Neither
length is carried as a constant anywhere in the extractor.

## 2. How a shop opens

`Interaction_ChkObjects` → `loc_59148` → `loc_65D12` (`$065D12`) →
`Interaction_ChkShop` → `Game_Mode_Routine = $18` (`FieldRoutine_Shop`).

`loc_65D12` walks `loc_68394` comparing three words against the *object* the
interaction matched:

```
41F9 00068394   lea     (loc_68394).l, a0
3438 EC28       move.w  (Field_Map_Index).w, d2
0C50 FFFF       cmpi.w  #$FFFF, (a0)        ; terminator
B450 / B068 0002 / B268 0004                ; map, x, y
31E8 0006 ECD0  move.w  $6(a0), ($FFFFECD0).w
5048            addq.w  #8, a0
```

Entry: `(map id, x, y, group<<8 | index)`, eight bytes, `$FFFF`-terminated.
`$FFFFECD0` is the selector the whole rest of the system reads: high byte the
group, low byte (`$FFFFECD1`) the index within it.

### Finding 1 — the counter tile is not the trigger

`docs/source-notes/formats.md` currently says a shop-location entry "lands on a type-`$C`
cell". **That is a correlation, not the rule.** The scan never touches the
collision grid; its caller (`loc_59148`) has already matched an object and
passes that object's `$30`/`$34` position. The table is keyed by **the
shopkeeper's position**.

The emitted census measures the correlation exactly — `counter_collision`:

| collision under the named cell | counters |
|---|---|
| `$C` shop | 52 |
| `$0` normal floor | 13 |
| `$8` solid | 3 (the dead rows, §5) |

Thirteen shopkeepers stand on ordinary walkable floor. A runtime that gated
the shop on collision type would break all thirteen. The pack therefore binds
each counter to the object standing on it: 65 of 68 rows land exactly on a
map-object placement, and `psiv_tools.shop_pack` **refuses to build** if a
live row does not.

## 3. Buying and selling

Both prices come from the same field: the word at `$14` of the 22-byte
`InventoryData` record, binary (not BCD — it is halved with a shift and
subtracted from a long).

| | retail | instruction |
|---|---|---|
| buy | `$0651D8` | `31F0 0014 E3F8` — `move.w $14(a0,d0.w), ($FFFFE3F8).w` |
| sell | `$065758` | `3030 0014` + `E248` — `move.w $14(a0,d0.w), d0` / `lsr.w #1, d0` |

- **Buy price** is the record word, spent unchanged. Affordability is
  `sub.l` against `Current_Money` with a `bmi` refusal, so the check is on the
  full long.
- **Sell price is exactly half, rounding down** — `lsr.w #1`, so an 11-meseta
  item sells for 5. Read, not assumed; the divisor is derived from the shift
  count rather than hard-coded.

### Finding 2 — stock is unlimited, structurally

There is no stock counter anywhere. `Win_ShopBuyList` re-reads
`ShopInventories` (ROM) on every visit through `GetOffsetByID_FF_Delim`, and
the completed purchase at `loc_659D4` writes exactly two things: the first free
`Inventory` slot and `Current_Money`. Nothing decrements a shop, and nothing
could — the list lives in ROM.

The only limit is the buyer's: `Win_ShopBuyConfirm` counts occupied inventory
slots and refuses at `$28` (40) items.

## 4. Inns

Group 0 is the inn system, with its own `0..$11` index space that never reads
`ShopInventories`. `Win_ShopMessage` (`$065E90`):

```
43F9 00068112   lea     (loc_68112).l, a1
1038 ECD1       move.b  ($FFFFECD1).w, d0     ; inn index
D040            add.w   d0, d0                ; word entries
43F1 0000       lea     (a1,d0.w), a1
1629 0001       move.b  $1(a1), d3            ; low byte = rate
4EB9 0005803A   jsr     (CalcWinIDAndCharNum).l
C6F9 FFFFE3F0   mulu.w  (Win_Char_Num).l, d3  ; x party size
31C3 E3EC       move.w  d3, ($FFFFE3EC).w     ; the bill
11D1 E3EE       move.b  (a1), ($FFFFE3EE).w   ; high byte = text variant
```

**The bill is `rate × party slots`**, where the multiplier is the number of
occupied entries in `Current_Party_Slots` (max 5). A dead party member still
counts — `CalcWinIDAndCharNum` counts slots, not survivors.

The eighteen rates, in index order: 5, 10, 20, 15, 15, 25, 50, 40, 45, 50, 80,
90, 100, 110, 120, 130, 140, 150. Four inns (2, 4, 12, 16) set the high byte
and use the alternate greeting text.

### What a night restores

`RecoverStats` (`$0662DA`) → `DoCharRecovery` (`$066306`), per occupied party
slot:

- `curr_hp` ← `max_hp`
- `curr_tp` ← `max_tp`
- `status` ← `0` (every ailment, including death)
- all eight `curr_skill_uses[n]` ← `max_skill_uses[n]`

It then calls `DoVehicleRecovery`, which restores the eight skill-use counts
for each of the three saved vehicles. That routine does **not** write saved
vehicle HP. The native inn and story-rest paths share this recovery operation.

Note that this is the *only* full-party revive in the game's economy: there is
no church or clinic service (§6).

### Finding 3 — one inn runs a scene instead of a night

`$066148`: `0C78 0006 ECD0` — when the selector is exactly `6` (group 0 index
6, the Aiedo counter), and **both** `EventFlag_Zio` and `EventFlag_GirlsCaught`
are clear, resting calls `Event_GirlsSneakingOut` instead of an ordinary
night. The routine saves and restores the bill, selector and text variant
around the call, so the transaction completes normally afterwards.

## 5. Finding 4 — three shops were cut and their table rows left behind

Three `loc_68394` rows name map `$41` (Tonoe) at position `(0, 0)`:

| row | rom | selector | inventory |
|---|---|---|---|
| 21 | `$06843C` | `$0109` | 9 |
| 22 | `$068444` | `$010A` | 10 |
| 23 | `$06844C` | `$010B` | 11 |

Position `(0, 0)` is the top-left corner and no object occupies it, so these
rows can never match. **Shop inventories 9, 10 and 11 are unreachable in
play**; Tonoe's only live counter is its inn.

This qualifies the existing 49-shop proof rather than breaking it. The claim in
`docs/source-notes/disassembly-discrepancies.md` that the location table's entries "cover indexes `0..$30` with
no gaps" is still true of the *bytes* — but three of those references are dead,
so the reachable set is 46. `census.unreachable_inventories` records it.

## 6. Finding 5 — there is no church or clinic mechanism

Checked, because the brief asked. The shop system has exactly three groups
(`0` inn, `1` shop, `2` shop with alternate greeting) and the location table
contains only those three values. Nothing else in the game consults
`$FFFFECD0`, and no second counter table exists.

The church and clinic maps (`MapDataMan_JutChurch`, `MapDataMan_KadaryChurchPeople`,
`MapDataMan_MeeseClinic*`) are ordinary `MapDataManager` entries — flag-gated
object despawns, already extracted in `docs/field/MAP_EFFECTS.md` — with ordinary
dialogue. Revival and cure are items and techniques; the inn is the only paid
service in the game.

## 7. Who is behind the counter

`Win_ShopMeseta` (`$065E04`) selects the portrait:

```
41F9 00067FFE   lea     (ShopPortraitGroupsPtrs).l, a0
1038 ECD0       move.b  ($FFFFECD0).w, d0     ; the group
0C00 0002       cmpi.b  #2, d0
6604 / 103C 0001                              ; group 2 reads group 1's
```

Two groups: 18 innkeeper pointers at `$068006`, 49 shopkeeper pointers at
`$06804E`. Between them they name **eight** distinct portraits — the seven
`ArtNem_ShopkeeperDialPortrait1..7` and `ArtNem_BakerDialPortrait`, which
only shop `$1D` (the Aiedo bakery) uses.

The greeting itself is assembled, not stored: `loc_68136`'s word per shop
gives a text variant (high byte, one of two) and a trade-noun fragment (low
byte, one of the three pointers at `loc_68198`), which are concatenated around
the shop's name fragment.

**Shopkeeper lines are not dialogue-tree entries.** Talking to a shopkeeper
routes to `FieldRoutine_Shop` before the dialogue tree is ever consulted,
which is why most shop objects carry `dialogue_id` 0. The pack binds the
portrait and the greeting selector per counter; the fragments themselves are
window text and stay with the dialogue half.

## 8. What is emitted

`runtime-pack/shops.json`, one file rather than a per-map section: a counter is
looked up by `(map, position)` once, when the player talks to somebody, and the
price and inn rules are global. A consumer wanting a per-map view builds it in
one pass over `counters`.

```
rules         the six decoded routines, with their retail offsets
counters      68 rows: map, position (pixels and cells), selector, group,
              the bound shopkeeper object, portrait, greeting, collision
inventories   49 lists, every item with buy_price and sell_price
inns          18 rates, each with cost_by_party_size for 1..5
census        the counts and every anomaly above
```

`manifest.shops` carries the file hash, the three counts, the price rules and
the census.

## 9. The boundary

Not extracted, deliberately, per the brief's "data and rules, not window
plumbing":

- the shop menu state machine (`Win_ShopBuySell` and the eight windows around
  it) — which window opens next, cursor placement, tile composition;
- the greeting text fragments themselves;
- the bakery's `$1D` special-case flow (it skips the buy/sell menu and opens
  the buy list directly) beyond recording that group 2 exists and which shop
  index takes it.

None of these change what a shop sells or charges.
