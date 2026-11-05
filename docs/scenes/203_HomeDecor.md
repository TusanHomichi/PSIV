# `Event_Pennant` and `Event_WoodCarving`

| Event | Retail bytes | Pointer | Data |
|---|---|---|---|
| `$0097` `Event_Pennant` | `$0736EA..$073753` inclusive, 106 bytes | `EventPtrs[$97]` at `$05A2B4 + $25C` | `home_decor.rs`, `PENNANT` (7 ops) |
| `$0098` `Event_WoodCarving` | `$073754..$0737BD` inclusive, 106 bytes | `EventPtrs[$98]` at `$05A2B4 + $260` | `home_decor.rs`, `WOOD_CARVING` (7 ops) |

- **Reached by:** the ITEM menu. `ItemAction_Pennant` and `ItemAction_WoodCarvin`
  (`ps4.asm:123470-123495`) accept only while `Field_Map_Index` is `MapID_ChazHouse` (`$5E`),
  write `Event_Index` and set `Routine_Exit_Flags` bit 1, so the event runs from the field with the
  menu gone (`rust/psiv-runtime/src/item_action.rs`, `ItemUseOutcome::Scene`). Anywhere else the
  item's own refusal line opens instead.
- **Flags:** `$97` sets `TempEveFlag_Pennant` (`$1C`), `$98` `TempEveFlag_WoodCarving` (`$1D`),
  both in the temporary bank (`Temp_Event_Flags`, `Flag::temp`). `MapDataMan_ChazHouse`
  (`ps4.asm:110727`) clears the Pennant and its three blocks (objects 0 to 3) while `$1C` is clear,
  and the Wood Carving and its four blocks (objects 4 to 8) while `$1D` is clear. No other lane's
  event reads them.

## Clone audit

Both bodies were read from the US image through their `EventPtrs` slots, and every word matches
`ps4.asm:153322-153369`. Neither has a `revision` or `grand_cross` conditional, and neither loads a
dialogue tree: the lines are entries of `ChazHouse`'s own tree.

## Retail transcription

The two bodies differ only in the entry, item, flag and start position.

| Op | Pennant | Wood Carving | Retail primitive | Scene record |
|---:|---|---|---|---|
| 0 | `$0736EA..$0736F1` | `$073754..$07375B` | `moveq #$64` / `#$66`, `jsr Event_GetAndRunDialogue` | standard `RunDialogue` |
| 1 | `$0736F2..$073707` | `$07375C..$073771` | `GetItem` of `$94` / `$95`; found: `clr.b (a0)`, `ReorderInventory`, again | `Rocky(RemoveAll)` |
| 2 | `$073708..$07370D` | `$073772..$073777` | `jsr PalFadeOut_ClrSpriteTbl` | `FadeOut` |
| 3 | `$07370E..$073715` | `$073778..$07377F` | `moveq #$1C` / `#$1D`, `jsr TempEveFlags_Set` | `SetFlag` temp |
| 4 | `$073716..$07371D` | `$073780..$073787` | `SFXID_Souvenir` (`$F1`) to `Sound_Index` | `PlaySound` |
| 5 | `$07371E..$07374D` | `$073788..$0737B7` | `Field_Map_Index := $5E`, `Field_Map_Index_2 := $54`, start (`$50,$46`) facing `$C` align `$10` / (`$38,$3E`) facing 4 align 4, `bclr #3, Map_Load_Flags`, `jsr RefreshMap` | `LoadMap` |
| 6 | `$07374E..$073753` | `$0737B8..$0737BD` | `jmp Pal_FadeIn` | `FadeIn` |

**The removal loop.** `GetItem` (`ps4.asm:121975`) counts the occupied inventory slots and searches
only that many from the front; `ReorderInventory` (`ps4.asm:124264`) shifts everything after the
first hole left by one. Run until `GetItem` misses, that takes every copy the counted search can see.
`RockyOp::RemoveAll` is that loop (`Inventory::find_item` asked again after each
`remove_in_field`); `Event_CatchingRocky` (#287) runs the same one, so these scenes use its op
rather than a second.

## Verification

`rust/psiv-core/tests/home_decor_scenes.rs` holds each scene's line, removal, flag and reload.
`rust/psiv-runtime/tests/session_menu_scenes.rs` uses each item from the ITEM menu with pads in
Chaz's house: the scene runs, both copies go, the house reloads with the souvenir's objects
present and the party where the body puts it; outside the house the item is refused.
