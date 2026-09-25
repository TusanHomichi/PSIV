# EQUIP / UNEQUIP scout

Status: **scouted and implemented**. This is the field-side companion to the
shared battle `Stats` record. The persistent equipment bytes remain the exact
four-byte interface at `$4C..$4F`; no field was added or repurposed in
`psiv-core/src/battle/stats.rs`.

## Retail flow

The camp path has two related filters:

1. The selected-item LOOK prompt only offers an equip prompt when the
   `InventoryData` type byte is `<= 7` (`LookOpt_ChkCreateEquipWindow`,
   `reference/ps4disasm/ps4.asm:123689-123700`).
2. The actual equipment list scans all forty inventory bytes, skips empty
   entries, rejects type `> 7`, then tests the selected character's bit in
   record offset `$08` (`Win_EquippedItemsMain`,
   `reference/ps4disasm/ps4.asm:127032-127056`). The list is therefore
   inventory-slot based, not an item-id set: duplicate items are distinct
   choices.

The decoded pack carries the same rule in
`runtime-pack/battle/equipment.json:42-80`: 132 equippable records, maximum
equippable type 7, and item 118 (`NOTHING`, type 9) has an all-party mask but
is deliberately not equippable. The runtime treats an absent or malformed
mask as no permission, never as all-party permission.

The usable-by bit mapping is character id order: bit 0 Chaz, bit 1 Alys, then
Hahn, Rune, Gryz, Rika, Demi, Wren, Raja, Kyra, Seth. The pack strings are
parsed at the `psiv-data` boundary and passed separately to the core seam so
the shared `Stats` shape stays unchanged.

## Slot dispatch and exchange

The persistent order and default dispatch are:

| byte | core slot | retail type(s) |
|---|---|---|
| `$4C` | right hand | 1, 2, 3, 4 |
| `$4D` | left hand | 5 (shield) |
| `$4E` | head | 6 |
| `$4F` | body | 7 |

The jump table is explicit in `EquipItemTypeJmpTbl`
(`reference/ps4disasm/ps4.asm:127743-127750`). Types 1 and 2 replace only
the right hand (`127755-127758`); types 3 and 4 replace the right hand and
clear the left, returning both displaced bytes (`127763-127768`); types 6
and 7 replace head/body (`127797-127808`).

The default shield handler has the cartridge's asymmetric branch
(`reference/ps4disasm/ps4.asm:127773-127793`): if the current right-hand item
is type 3 or 4, the right hand is cleared and the shield goes left; otherwise
the old left hand is returned and the shield occupies left.

**Correction, 2026-09-15:** `loc_5F93A..loc_5FAEE` subsequently offers a hand
selector for types 1, 2 and 5. It rebuilds the tentative equipment from the
saved record on each cursor update, writes the selected hand, and commits
only on confirmation. Left-hand selection displaces a two-handed right
weapon; right-hand selection preserves the left. Cancel changes neither
equipment nor inventory. The earlier initial-data-only dual-wielding claim
missed this later routine and was wrong.

The native EQUIP page now exposes that hand choice, including two slashers
for Alys and a shield in the right hand. Core regressions cover re-equipping
the left weapon, shield placement, two-handed displacement and rejection of
invalid hand selectors. A real-pack runtime test verifies two slashers and
their derived attack value through reload. The native text windows are wider
to contain the shell's labels; their layout is not retail pixel parity.

The retail equip commit counts the inventory, replaces the selected filtered
entry with the first displaced item, compacts when that replacement is empty,
then writes the second displaced item into the first empty byte
(`reference/ps4disasm/ps4.asm:128148-128240`). It rejects a two-handed exchange
that would need more than the forty-byte inventory can hold. Unequip similarly
finds a free inventory byte, writes the removed item there, clears the selected
raw slot, and returns to the equipment stats window
(`reference/ps4disasm/ps4.asm:128259-128298`).

The core implementation is `battle::equipment::{equipment_candidates,
equip_item,unequip_item}`. It preflights all lookups, masks, displaced records,
and capacity before changing either `Stats` or `Inventory`, so a rejected
command is atomic and fail-closed.

## Derived-stat timing

`UpdateEquipment` dispatches the type handler and immediately calls
`UpdateCharModStats` before drawing the changed numbers
(`reference/ps4disasm/ps4.asm:127702-127738`). The equipment stats window also
refreshes modified stats on entry (`126699-126727`). Thus modified values and
the battle copies are observable immediately after the command.

Retail refreshes element properties through `Win_UpdateStatsElems` when the
equipment flow commits/exits (`reference/ps4disasm/ps4.asm:128426-128433`),
while the source's `bugfixes=1` branch also does it on the message-window exit
(`127566-127606`). The runtime seam refreshes both modified stats and element
caches at the successful command boundary. That is the useful persistent
invariant: the next battle sees exactly what `PartyMember::seat` would derive
from the resulting four item bytes.

## Unequip and curses

There is no equipment curse flag, cursed-item guard, or unremovable-equipment
branch in the relevant retail path. Unequip is driven only by the selected
slot and available inventory space (`Win_EquippedItemsMain` and
`loc_5FC36`, `reference/ps4disasm/ps4.asm:127310-127345` and
`128259-128298`). The pack's negative equipment bonuses are signed stat data,
not a curse mechanism. No cursed/unremovable behavior was invented in the
runtime seam.

## Oracle boundary

The reachable Piata tape does not buy gear. The only item-shop inventory bound
to `PiataItemShop` is one type-8 MONOMATE at price 20
(`runtime-pack/shops.json:3228-3251`), and
`oracle/tapes/23_piata_shop_reach.tape:71-91` buys and sells that consumable.
The weapon inventory is bound to `MileWeaponShop`, not the taped Piata item
shop (`runtime-pack/shops.json:3253-3265`). There is consequently no tape
RAM effect for buy-plus-equip to report. The equip behavior above is grounded
in the disassembly citations and pack rules; no oracle write was performed.

## Implementation and tests

- `psiv-core/src/battle/equipment.rs` owns the command seam; `Stats`'s shape is
  unchanged.
- `psiv-runtime/src/camp.rs` supplies masks from the pack and commits the
  cloned transaction into the persistent roster/inventory only on success.
- `psiv-godot/src/camp/` now makes only EQUIP interactive: character chooser,
  stats/equipment slots, filtered item list, unequip/equip result. STATE wiring
  is unchanged. The screen geometry comes from `WinGroup_Menu` `$93..$C1`
  (`reference/ps4disasm/ps4.asm:140623-140762`); there is still no oracle
  `camp_equip` capture.
- Tests pin legal and illegal filters, type-before-mask rejection, immediate
  modified/battle refresh, two-handed displacement, permanent left-hand loss,
  and an equip-to-`PartyMember::seat` derivation round trip.
