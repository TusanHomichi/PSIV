"""Shop inventories, read directly from the uncompressed retail table.

`ShopInventories` in the public disassembly is a flat run of `$FF`-terminated
lists of item ids, one list per shop, with no pointer or count table in front
of it:

    dc.b    ItemID_Monomate                 ; shop 0
    dc.b    $FF
    dc.b    ItemID_Dagger, ItemID_HuntKnife, ...   ; shop 1
    dc.b    $FF

Both call sites (`Win_ShopBuySell` and the bakery path inside `Win_ShopMessage`)
index it identically::

    lea     (ShopInventories).l, a0
    moveq   #0, d0
    move.b  ($FFFFECD1).w, d0
    jsr     (GetOffsetByID_FF_Delim).l

`GetOffsetByID_FF_Delim` masks the index to 8 bits and then walks `a0` forward
past exactly that many `$FF` bytes, returning immediately when the index is 0.
So shop N is the Nth `$FF`-delimited list, counted from the table start --
the same "count the terminators" addressing the battle formations use. There
is no pointer table and no bounds check in the routine; an out-of-range index
would simply run off the end of the table.

The bound therefore lives in the data, and three independent tables agree on
49 shops (indexes `0..$30`):

* `ShopInventories` itself holds exactly 49 `$FF`-terminated lists in 255
  bytes, then an assembler `even` pad byte, then the neighbouring pointer
  block `loc_682A4`.
* `ShopPortrGroup_Shopkeepers` has 49 portrait pointers.
* The per-shop greeting-selector table `loc_68136` is 49 words (`0x62` bytes)
  and is indexed by the same `$FFFFECD1` byte.

`$FFFFECD0` is written as a word: the high byte is the shop *group* and the
low byte (`$FFFFECD1`) is the index within that group. Group 0 is an inn and
never reads `ShopInventories` at all -- its index space is a separate 0..$11
that addresses `ShopPortrGroup_Innkeepers` and `loc_68112`. Groups 1 and 2
are the two shop flavours, and both index `ShopInventories`; group 2 differs
only in its greeting text and, for shop `$1D` (the Aiedo bakery), in skipping
the buy/sell menu and opening the buy list directly.

The shop-location table at `loc_68394` is what actually assigns those values:
`$FFFF`-terminated 8-byte entries of `(map id, x, y, group<<8 | index)`,
scanned linearly by `loc_65D12`. Its group-1 and group-2 entries between them
cover indexes `0..$30` with no gaps, which is the reachability half of the
same 49-shop proof.
"""

from __future__ import annotations

from typing import Any

from .symbols import ITEM_SYMBOLS, MAP_SYMBOLS

TERMINATOR = 0xFF

# ShopInventories, end-exclusive. 255 bytes of $FF-terminated lists; 0x682A3
# is the `even` pad byte and 0x682A4 is the start of the loc_682A4 pointer
# block, so the table is flush against its neighbour on both sides
# (loc_68198's three longs end exactly at 0x681A4).
SHOP_INVENTORIES: dict[str, Any] = {
    "label": "ShopInventories",
    "start": 0x681A4,
    "end": 0x682A3,
}
SHOP_COUNT = 49

# First two shop lists: [Monomate] and [Dagger, HuntKnife, StelSword,
# Boomerang, Slasher]. Unique in this retail build, so it pins both the
# address and the item-id convention before anything is decoded.
SHOP_SIGNATURE = bytes.fromhex("7dff0102080309ff")

# Assembler `even` padding after the table, then the neighbouring block.
PAD_BYTE = 0x00
NEXT_LABEL = "loc_682A4"
NEXT_LABEL_FIRST_LONG = 0x002AC470

# loc_68394: $FFFF-terminated, 8 bytes per entry.
SHOP_LOCATIONS: dict[str, Any] = {
    "label": "loc_68394",
    "start": 0x68394,
    "entry_size": 8,
}
LOCATION_TERMINATOR = 0xFFFF

# High byte of $FFFFECD0. Only groups 1 and 2 index ShopInventories; group 0
# is an inn with its own index space.
SHOP_GROUPS = {0: "inn", 1: "shop", 2: "shop_alternate_dialogue"}
INN_GROUP = 0


class ShopError(ValueError):
    pass


def _item_symbol(value: int) -> dict[str, Any]:
    """Item ids are 1-based: `ItemID_Dagger = 1`, `ItemID_Monomate = $7D`."""
    idx = value - 1
    return {
        "id": value,
        "symbol": ITEM_SYMBOLS[idx] if 0 <= idx < len(ITEM_SYMBOLS) else None,
    }


def _check_signature(data: bytes) -> None:
    start = SHOP_INVENTORIES["start"]
    actual = data[start:start + len(SHOP_SIGNATURE)]
    if actual != SHOP_SIGNATURE:
        raise ShopError(
            f"ShopInventories signature mismatch at 0x{start:06X}: expected "
            f"{SHOP_SIGNATURE.hex()}, got {actual.hex()}"
        )
    if data.count(SHOP_SIGNATURE) != 1:
        raise ShopError(
            f"ShopInventories signature {SHOP_SIGNATURE.hex()} occurs "
            f"{data.count(SHOP_SIGNATURE)} times in this ROM; it must be unique "
            "for the offset to be self-verifying"
        )


def _check_boundary(data: bytes) -> dict[str, Any]:
    """Prove the end of the table against the block that follows it."""
    end = SHOP_INVENTORIES["end"]
    pad = data[end]
    if pad != PAD_BYTE:
        raise ShopError(
            f"Expected the `even` pad byte 0x{PAD_BYTE:02X} at 0x{end:06X}, "
            f"got 0x{pad:02X}"
        )
    following = int.from_bytes(data[end + 1:end + 5], "big")
    if following != NEXT_LABEL_FIRST_LONG:
        raise ShopError(
            f"Expected {NEXT_LABEL} to begin with 0x{NEXT_LABEL_FIRST_LONG:08X} "
            f"at 0x{end + 1:06X}, got 0x{following:08X}"
        )
    return {
        "pad_byte": f"0x{pad:02X}",
        "pad_offset": f"0x{end:06X}",
        "next_label": NEXT_LABEL,
        "next_label_offset": f"0x{end + 1:06X}",
        "next_label_first_long": f"0x{following:08X}",
    }


def split_shop_records(block: bytes) -> list[tuple[int, bytes]]:
    """Split the table into `(block offset, record)` pairs on `$FF`.

    This is a byte-by-byte terminator scan because that is literally what
    `GetOffsetByID_FF_Delim` does. Item ids run 1..160, so `$FF` can never be
    an item byte and the scan is unambiguous.
    """
    records: list[tuple[int, bytes]] = []
    start = 0
    for cursor, value in enumerate(block):
        if value == TERMINATOR:
            records.append((start, block[start:cursor + 1]))
            start = cursor + 1
    if start != len(block):
        raise ShopError(
            f"ShopInventories ends with {len(block) - start} unterminated bytes: "
            f"{block[start:].hex()}"
        )
    return records


def parse_shop_record(record: bytes) -> dict[str, Any]:
    if not record or record[-1] != TERMINATOR:
        raise ShopError(f"Unterminated shop record: {record.hex()}")
    body = record[:-1]
    if not body:
        raise ShopError("Empty shop record: a shop with nothing to sell")
    for value in body:
        if not 1 <= value <= len(ITEM_SYMBOLS):
            raise ShopError(
                f"Shop record {record.hex()} references item id {value}, which is "
                f"outside the 1..{len(ITEM_SYMBOLS)} inventory table"
            )
    return {
        "item_count": len(body),
        "items": [_item_symbol(value) for value in body],
        "raw_hex": record.hex(),
    }


def extract_shop_inventories(data: bytes) -> dict[str, Any]:
    _check_signature(data)
    start, end = SHOP_INVENTORIES["start"], SHOP_INVENTORIES["end"]
    block = data[start:end]
    if len(block) != end - start:
        raise ShopError("ShopInventories runs past the end of the ROM")

    records = split_shop_records(block)
    if len(records) != SHOP_COUNT:
        raise ShopError(
            f"ShopInventories holds {len(records)} $FF-terminated lists between "
            f"0x{start:06X} and 0x{end:06X}; the shop index space proven from "
            f"ShopPortrGroup_Shopkeepers, loc_68136 and loc_68394 is {SHOP_COUNT}"
        )

    boundary = _check_boundary(data)
    shops = []
    for index, (block_offset, record) in enumerate(records):
        offset = start + block_offset
        shops.append({
            "index": index,
            "rom_offset": f"0x{offset:06X}",
            "rom_end_exclusive": f"0x{offset + len(record):06X}",
            "block_offset": f"0x{block_offset:04X}",
            **parse_shop_record(record),
        })

    return {
        "table": {
            "label": SHOP_INVENTORIES["label"],
            "rom_offset": f"0x{start:06X}",
            "rom_end_exclusive": f"0x{end:06X}",
            "size_bytes": len(block),
            "terminator": f"0x{TERMINATOR:02X}",
            "indexing": (
                "GetOffsetByID_FF_Delim: shop N is the Nth $FF-terminated list, "
                "counted from the table start. No pointer table, no bounds check."
            ),
            "signature_hex": SHOP_SIGNATURE.hex(),
            **boundary,
        },
        "shop_count": len(shops),
        "shops": shops,
    }


def extract_shop_locations(data: bytes, shop_count: int | None = None) -> dict[str, Any]:
    """Decode `loc_68394`: which map position opens which shop.

    Entries are `(map id, x, y, group<<8 | index)`, terminated by `$FFFF` in
    the map-id field. `loc_65D12` walks this table comparing the current map
    index and the interaction's x/y, then stores the fourth word into
    `$FFFFECD0`.
    """
    start = SHOP_LOCATIONS["start"]
    entry_size = SHOP_LOCATIONS["entry_size"]
    entries = []
    offset = start
    while True:
        if offset + entry_size > len(data):
            raise ShopError(
                f"{SHOP_LOCATIONS['label']} at 0x{start:06X} is not terminated "
                f"before the end of the ROM"
            )
        map_id = int.from_bytes(data[offset:offset + 2], "big")
        if map_id == LOCATION_TERMINATOR:
            break
        record = data[offset:offset + entry_size]
        value = int.from_bytes(record[6:8], "big")
        group_id = value >> 8
        shop_index = value & 0xFF
        uses_inventory = group_id != INN_GROUP
        if uses_inventory and shop_count is not None and shop_index >= shop_count:
            raise ShopError(
                f"{SHOP_LOCATIONS['label']} entry at 0x{offset:06X} selects shop "
                f"index 0x{shop_index:02X}, but ShopInventories only holds "
                f"{shop_count} lists"
            )
        entries.append({
            "rom_offset": f"0x{offset:06X}",
            "map_id": map_id,
            "map_id_hex": f"0x{map_id:04X}",
            "map_symbol": MAP_SYMBOLS[map_id] if map_id < len(MAP_SYMBOLS) else None,
            "x": int.from_bytes(record[2:4], "big"),
            "y": int.from_bytes(record[4:6], "big"),
            "selector": f"0x{value:04X}",
            "group_id": group_id,
            "group": SHOP_GROUPS.get(group_id),
            "index": shop_index,
            # Inns have their own 0..$11 index space into
            # ShopPortrGroup_Innkeepers and loc_68112; they never read
            # ShopInventories.
            "shop_inventory_index": shop_index if uses_inventory else None,
            "raw_hex": record.hex(),
        })
        offset += entry_size

    if not entries:
        raise ShopError(f"{SHOP_LOCATIONS['label']} at 0x{start:06X} is empty")

    referenced = sorted({e["shop_inventory_index"] for e in entries} - {None})
    return {
        "label": SHOP_LOCATIONS["label"],
        "rom_offset": f"0x{start:06X}",
        "rom_end_exclusive": f"0x{offset + 2:06X}",
        "entry_size": entry_size,
        "terminator": f"0x{LOCATION_TERMINATOR:04X}",
        "entry_count": len(entries),
        "groups": {str(k): v for k, v in SHOP_GROUPS.items()},
        "referenced_shop_indexes": referenced,
        "inn_count": sum(1 for e in entries if e["group_id"] == INN_GROUP),
        "entries": entries,
    }


def extract_shops(data: bytes) -> dict[str, Any]:
    result = extract_shop_inventories(data)
    locations = extract_shop_locations(data, result["shop_count"])
    referenced = set(locations["referenced_shop_indexes"])
    expected = set(range(result["shop_count"]))
    if referenced != expected:
        raise ShopError(
            "loc_68394 and ShopInventories disagree about the shop index space: "
            f"unreachable {sorted(expected - referenced)}, "
            f"dangling {sorted(referenced - expected)}"
        )
    result["locations"] = locations
    return result
