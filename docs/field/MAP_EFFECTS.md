# Map effects: the cartridge's flag-gated map patches

How PSIV makes a map react to story state — Igglanova gone after the fight,
Zema's doors open, Alys no longer standing in the academy. Findings are cited
to retail addresses; the disassembly clone is an orientation source only, and
where the two disagree the cartridge wins (§7).

Extractor: `psiv_tools/map_effects.py`. Emission: per-map `map_effects` and
`layout_variants` on each map record, census under `manifest.map_effects`.

---

## 1. There are three systems, not one

| System | Where it lives | When it runs | Size |
|---|---|---|---|
| **A. MapDataManager** | the map record's last section | map load only | 127 live entries, 139 maps |
| **B. Per-object live gating** | the object's own `FieldObjectsJmpTbl` routine | every frame | 4 routines |
| **C. Overworld page hooks** | `loc_53BDC`/`loc_53CD4` jump tables | per 1KB page copy | 12 patches, maps 0–1 |

Sections 2–11 describe the original **A** slice. **C** ships its raw source
records as `layout_patches` on maps 0 and 1; the native consumer added on
2026-09-23 is described in section 12. **B** is `FieldObj_EsperGuard`, `FieldObj_FellowPenguin`,
`FieldObj_InnerEsperGuards` and `FieldObj_MuskCatGuard`, which test a flag
inside their per-frame update; a despawn set computed at map build cannot
express them, and they are Slice 3.

## 2. The dispatcher

`MapDataManager` at **`0x051B38`**, its table `MapDataManagerJmpTbl` at
**`0x051B64`** (160 `bra.w` entries):

```
move.w  (a0), d0
cmpi.w  #$FFFF, d0        ; the record's list is $FFFF-terminated
beq.s   done
lea     (MapDataManagerJmpTbl).l, a1
add.w   d0, d0
add.w   d0, d0
move.l  a0, -(sp)
jsr     (a1,d0.w)
movea.l (sp)+, a0
bne.s   abort             ; a non-zero return skips the rest of the list
lea     $2(a0), a0
bra.s   MapDataManager
```

**Finding 1 — they run at map load and only at map load.** `MapDataManager` is
reached from exactly two `jsr` sites in the image, both inside a map loader
(`0x051B38`'s caller and the second loader near `0x05AF9C`). `Map_Data_Manager_Addr`
is written at both and **read nowhere**. A flag that changes while the player
stands on a map changes nothing until the map is loaded again. A runtime that
re-applies these on a flag change is not reproducing the cartridge.

**Finding 2 — a routine can abort the rest of the list.** `movea.l` does not
touch the condition codes, so the `bne.s` tests what the routine left. Every
retail routine ends `moveq #0,d7 / rts`, so it never fires — but the emitted
data carries `aborts_remaining_entries` per path, because a consumer that
ignored it would apply patches the cartridge would have skipped.

## 3. Object patches address record objects, not RAM

`LoadMapObjects` fills slots through `Field_LoadObject`, a linear first-free
scan from `Field_Obj_Secondary` (`$FFFFC300`) to `$FFFFCFC0` in `$40` steps.
On a fresh load the record's object *N* is therefore at `$C300 + N * $40`, and
a routine's `clr.w $XX(a0)` despawns record object `(addr - $C300) / $40`.

Verified across the whole game: every object write lands on an aligned slot,
and `Alys_Piata` (`$FFFFC4C0`) resolves to index 7, which is
`PiataAcademy_F1`'s object 7, `NPCAlysPiata`.

**Finding 3 — two routines clear more slots than their map has objects.**
Entries `$64` and `$65` (maps `$0F6`, `$0FB`) clear indices past the record's
object count. On a fresh load those slots are empty, so the write is a no-op;
the routine is written for the largest map that uses it. Carried in
`census.object_writes_past_map_object_count` so a consumer ignores such an
index instead of panicking on it.

## 4. The kinds this slice decodes

| kind | what it does | instances | maps |
|---|---|---|---|
| `object_despawn` | clears a slot: the object never spawns | 339 | 72 |
| `layout_write` | writes chunk ids into `Map_Layout` | 194 | 13 |
| `object_rewrite` | writes a different object id into a slot | 8 | 6 |
| `object_dialogue` | changes an object's dialogue id (`$14(aN)`) | 4 | 3 |
| `layout_replace` | decompresses a whole different layout over a plane | 4 | 2 |

Layout writes go through **`GetMapLayoutOffset`** at **`0x053514`**:

```
tst.w   d3                     ; 0 selects FG, non-zero BG
bne.s   +
move.b  (Map_Row_Size_FG).w, d3
lea     (Map_Layout_FG).w, a1
...
ext.w   d3 / addq.w #1, d3
mulu.w  d3, d2                 ; d2 is a chunk row
add.w   d2, d1                 ; d1 is a chunk column
adda.w  d1, a1
```

The row size is the **map's**, so a displacement off the returned pointer is
only a coordinate once the map is known — which is why the pack resolves
displacements at emission and one routine serving two maps of different widths
resolves differently on each.

## 5. Zema's doors, and the four other stuck doorways

`MapDataMan_ChkZemaNormal` (entry `$05`, `0x051E4C`) is the shape that needed a
loop-executing decoder. It walks `Zema_LockedDoorsOffs`, a four-entry table of
`(column, row)` bytes, and writes chunks `$59`/`$5A` at each once
`EventFlag_IgglanovaZema` (`$33`) is set:

| door | chunk coords | cells |
|---|---|---|
| ZemaHouse1 | (21, 11) + (22, 11) | (42, 22) |
| ZemaWeaponShop | (20, 21) + (21, 21) | (40, 42) |
| ZemaInn | (12, 15) + (13, 15) | (24, 30) |
| ZemaItemShop | (17, 15) + (18, 15) | (34, 30) |

Since the pack first reported it, `warps.doors_without_map_change_cell` has
listed exactly five doorways whose trigger covers no map-change cell in the
stored layout: Zema's four and `BirthValley_B1` → `BioPlant`. **All five are
covered by a `layout_write` from this slice.** That is the acceptance proof
that Slice 1 unblocks the story.

The disassembly labels the table `Zema_LockedDoorsOffs`, which is backwards:
the offsets are where the doors *open*.

## 6. Layout replacement ships decoded

Two maps swap an entire layout rather than patching cells:

| map | entry | gate | planes written |
|---|---|---|---|
| `$19A` GaruberkTower_Part2 | `$3B` | `$F140` bank flag `$14`, set | FG `0x1C8F7A`, BG `0x1C92BA` |
| `$19E` GaruberkTower_Part6 | `$3C` | `$F140` bank flag `$16`, set | BG `0x1CA2AE`, FG `0x1CA54E` |

All four blobs decompress to exactly 2304 bytes = 48×48, matching the maps'
declared dimensions.

**Finding 4 — in both maps one of the two planes is the map's own base blob.**
Part2 rewrites FG with `0x1C8F7A`, which *is* its record's FG layout pointer;
Part6 rewrites BG with `0x1CA2AE`, which is its record's BG pointer. So only
one plane per map actually changes. Recorded as `identical_to_base` per plane
rather than hidden.

The pack emits each replacement as a first-class variant — decoded grid,
collision rows, composed PNG and priority overlay — so no consumer needs a
decompressor. Part2's variant changes 96 collision cells against its base.

## 7. Discrepancies against the disassembly clone

**The clone names a flag bank the cartridge does not have.** It defines
`Temp_Event_Flags = ramaddr($FFFFF156)` and a `TempEveFlags_Test` routine that
loads it. **No routine anywhere in the retail image loads `$FFFFF156`.** The
routine the clone labels `TempEveFlags_Test` is the `$FFFFF140` door — the same
one it elsewhere calls `ChestFlags_Test`. `$F156` is 22 bytes into that bank, so
what the disassembly's prose calls a "temporary event flag" is a bit of the
`$F140` bank at a high id, not a bank of its own.

Retail has exactly **four** flag doors, in three blocks:

| block | base | doors |
|---|---|---|
| test | `0x057624` | `$F100`, `$F120`, `$F140`, `$F160` |
| set | `0x057666` | `$F100`, `$F120`, `$F140`, `$F160` |
| clear | `0x0576A8` | `$F100`, `$F120`, `$F140` |

**Finding 5 — the clear block is one door short: nothing in the cartridge
clears a town flag.**

A consumer modelling five banks will not line up with the cartridge, and temp
flag *N* and chest flag *N* are the same bit.

This is the second confusion found in this region; `psiv_tools/newgame.py`
records the first (the clone's `loc_44414` copies its 32-byte table to
`Chest_Flags` where retail writes `Extended_Event_Flags`).

## 8. What gates what

Only **two** of the four banks gate any map effect: `event_flags` (234 gates,
65 distinct flags) and the `$F140` bank (82 gates, 12 distinct). Extended and
town flags gate nothing here.

166 paths are gated; **3 are unconditional** — patches that run on every load
of their map regardless of story state.

## 9. Coverage, and what is deferred

127 of the 160 table entries are referenced by a map record (33 are dead), over
188 (map, entry) pairs on 139 maps. **113 entries decode; 14 do not** and are
listed in `census.undecoded_entries` with the offset, the instruction that
stopped the read, and the maps affected — never half-decoded.

A routine that mixes Slice 1 with later work is decoded for its Slice-1 writes
and carries the rest in `deferred` on the path: `postincrement bulk copy`
(palette and art loops), `write to 0xFFFF….` (palette, scroll, party slots),
`chest_flags write`, `loop`, `call to 0x…`. Palette, scroll, chunk-table
rewrites and alternate colours are Slices 2 and 3.

## 10. Emission shape

Per-map, not a central file: the data arrives with the map the runtime is
building, so a consumer cannot forget to consult a second file, and a kind it
does not know fails that map's build rather than the whole pack. It matches the
`layout_patches` idiom the overworlds already use.

```
maps/<id>_<symbol>.json
  map_effects: [ { entry, entry_hex, routine, raw_hex, decoded,
                   kinds, paths: [ { gates: [{bank, flag, flag_hex, symbol,
                                              required: set|clear}],
                                     unconditional, aborts_remaining_entries,
                                     deferred: [...], writes: [...] } ] } ]
  layout_variants: [ { id, planes: [{plane, source, identical_to_base}],
                       png, png_over, png_sha256, png_over_sha256,
                       priority_tiles, unloaded_patterns,
                       collision: {plane, width_cells, height_cells, rows, sha256},
                       differs_from_base_cells } ]
maps/<id>_<symbol>_variant.png
maps/<id>_<symbol>_variant_over.png
manifest.map_effects — the census
```

Write shapes: `object_despawn`/`object_rewrite`/`object_dialogue` carry
`object_index` (plus `object_id` / `dialogue_id`); `layout_write` carries
`plane`, `chunk_x`, `chunk_y`, `cell_x`, `cell_y`, `chunk_id` already resolved
for that map; `layout_replace` carries `plane` and `source` and is realised by
the matching entry in `layout_variants`.

## 11. Retail addresses used

| what | address |
|---|---|
| `MapDataManager` dispatcher | `0x051B38` |
| `MapDataManagerJmpTbl` | `0x051B64` (160 `bra.w`) |
| flag test / set / clear blocks | `0x057624` / `0x057666` / `0x0576A8` |
| `GetMapLayoutOffset` | `0x053514` |
| `KosDecomp` | `0x041BEA` |
| `Field_Obj_Secondary` | `$FFFFC300`, stride `$40`, last `$FFFFCFC0` |
| `Map_Layout_FG` / `_BG` | `$FFFFA000` / `$FFFFB000` |
| `MapDataMan_ChkZemaNormal` | `0x051E4C` |
| `Zema_LockedDoorsOffs` | four `(column, row)` byte pairs |

Every one is derived from the image at extraction time — the dispatcher names
its own table, the flag blocks are read door by door, and `KosDecomp` is the
routine the layout-replace sites agree on — so a ROM that disagrees fails
rather than decoding something else.

## 12. Native overworld page-hook consumption (2026-09-23)

The connected post-Rika route exposed an omitted consumer: the pack already
decoded the page hooks, but `MapRecord` discarded them and the runtime built
the unpatched world. With `$35` set, the party still stopped at Motavia
`(84,68)` below a broken bridge. The retained failure is
`build/native-post-rika-20260923/attempt-02/route/failure.json`.

US retail `loc_53D16` at `$053D16..$053D39` tests event `$35`, clears FG chunk
`(42,33)` to `$00`, and writes BG chunk `$48`. The four collision cells
`(84..85,66..67)` change from water `$9` to normal `$0`. The inn and route
conditions, connected input result and SAVE/CONTINUE receipts belong in the
[travel ledger](TRAVEL.md#post-rika-northern-crossing-2026-09-23).

`resolve_overworld_patches` retains the raw hook list and emits a separate
`overworld_patches` list. Each flag group folds its ordered plane writes over
the base FG/BG pair at every touched coordinate. The resolved tile carries
both final chunk IDs, four collision nibbles, the collision-authoritative raw
chunk ID and a composed atlas index. Atlas tiles for these pairs have
`chunk_id: null`; they cannot masquerade as a raw scene/MapDataManager chunk.
The base and priority PNGs both replace their full 32×32 area, including
transparent priority pixels that erase the old overlay.

Rust validates the raw/resolved correspondence, flag and coordinate bounds,
collision authority, atlas references and coverage. Unsupported cross-flag
overlaps fail explicitly; the current 9 Motavia and 3 Dezolis source records
contain none. Old overworld packs missing the resolved data fail with rebuild
guidance. Regenerate the **full** pack using [DEVELOPMENT.md](../DEVELOPMENT.md#prepare-the-local-pack).

`effects::evaluate` applies these results before MapDataManager effects on map
construction, normal entry and battle refresh. Godot uses its existing atlas
blitter; it owns no story flag or collision rule. This full-map representation
does **not** reproduce retail page streaming after a flag changes while the
same map remains loaded. Later dynamic chunk-definition writes, live object
gating and whole-overworld visual parity remain separate work.
