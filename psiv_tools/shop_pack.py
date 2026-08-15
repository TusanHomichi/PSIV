"""`shops.json`: every counter in the game, what it sells and what it charges.

One file, not one section per map. Shops are unlike map effects, which have to
be applied while a map loads and so belong with the map: a shop is a global
table the interaction path consults *once*, when the player talks to somebody,
keyed by `(map, position)`. Sixty-eight rows split across 361 map files would
scatter one small table and duplicate the price and inn rules into every town,
and the runtime would still have to build the same index to answer the only
question it ever asks. So it stays one file, and a consumer that wants a
per-map view builds it in a single pass over `counters`.

What it holds
-------------

    rules         the decoded flow: how a counter is reached, what buying and
                  selling cost, whether stock runs out, what an inn does
    counters      all 68 location rows, each bound to the shopkeeper object it
                  names and carrying what that keeper is
    inventories   the 49 lists, each item priced both ways
    inns          the 18 rates, with the bill for every party size

The shopkeeper binding
----------------------

`psiv_tools.shop_flow` establishes that the location table is keyed by an
object's position rather than a collision cell. This module completes that: it
joins every row to the map's own object list, so a counter carries the
shopkeeper's record index, object id and facing. Sixty-five of the sixty-eight
rows land exactly on an object. The three that do not name position `(0, 0)` on
Tonoe and can never fire, which is what makes shop inventories 9, 10 and 11
unreachable -- see `census.unreachable_inventories`.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from .core import ITEM_TYPES, TABLES, be16, extract_items
from .shop_flow import (
    ShopFlowError,
    read_flow,
    read_greeting_selectors,
    read_inn_rates,
    sell_price,
)
from .shops import INN_GROUP, extract_shops
from .text import extract_names

#: Where the file lands in the pack.
SHOPS_NAME = "shops.json"

#: A dead row: a position no object can occupy.
DEAD_POSITION = (0, 0)


class ShopPackError(ShopFlowError):
    pass


def _display_names(rom: bytes) -> dict[int, str]:
    return {entry["id"]: entry["name"] for entry in extract_names(rom)["item_names"]}


def _prices(rom: bytes, divisor: int) -> dict[int, dict[str, Any]]:
    """Every item's two prices, straight from the record the shop code reads."""
    spec = TABLES["items"]
    out = {}
    for record in extract_items(rom):
        offset = int(record["rom_offset"], 16)
        buy = be16(rom, offset + 0x14)
        if buy != record["meseta_cost"]:
            raise ShopPackError(
                f"item {record['id']}: the price word at 0x{offset + 0x14:06X} is "
                f"{buy}, but psiv_tools.core decoded {record['meseta_cost']}"
            )
        out[record["id"]] = {
            "buy": buy,
            "sell": sell_price(buy, divisor),
            "type": record["type"]["id"],
        }
    if len(out) != spec["count"]:
        raise ShopPackError(
            f"priced {len(out)} items; the inventory table holds {spec['count']}"
        )
    return out


def _objects(maps: dict[int, dict[str, Any]]) -> dict[tuple[int, int, int], dict[str, Any]]:
    """Every packed map object, keyed by `(map id, x pixels, y pixels)`."""
    index: dict[tuple[int, int, int], dict[str, Any]] = {}
    for map_id, record in maps.items():
        for npc in record.get("npcs", []):
            index.setdefault((map_id, npc["x_pixels"], npc["y_pixels"]), npc)
    return index


def build_shops(rom: bytes, maps: dict[int, dict[str, Any]],
                complete: bool = True) -> dict[str, Any]:
    """The whole shop section, ready to write.

    `complete` says whether `maps` is the whole game. When it is, every live
    counter must land on an object, and one that does not stops the build --
    that join is the evidence for the whole shopkeeper binding. A filtered
    build cannot make the claim, so it only records the misses.
    """
    flow = read_flow(rom)
    shops = extract_shops(rom)
    locations = shops["locations"]
    divisor = flow["prices"]["sell"]["divisor"]
    prices = _prices(rom, divisor)
    names = _display_names(rom)
    objects = _objects(maps)

    inn_indexes = sorted({
        entry["index"] for entry in locations["entries"]
        if entry["group_id"] == INN_GROUP
    })
    rates = read_inn_rates(rom, int(flow["inn"]["table"], 16), len(inn_indexes))
    if inn_indexes != [entry["index"] for entry in rates]:
        raise ShopPackError(
            f"the location table binds inn indexes {inn_indexes}, which is not the "
            f"{len(rates)}-entry rate table's index space"
        )
    greetings = read_greeting_selectors(
        rom, int(flow["presentation"]["greetings"]["table"], 16), shops["shop_count"]
    )
    portraits = {
        group["group"]: group["art"]
        for group in flow["presentation"]["portraits"]["groups"]
    }

    counters = [
        _counter(entry, row, objects, portraits, greetings, maps)
        for row, entry in enumerate(locations["entries"])
    ]
    bound: dict[int, list[int]] = {}
    for counter in counters:
        if counter["shop_inventory_index"] is not None and counter["live"]:
            bound.setdefault(counter["shop_inventory_index"], []).append(counter["id"])

    inventories = [
        _inventory(record, prices, names, bound.get(record["index"], []))
        for record in shops["shops"]
    ]
    inns = [
        _inn(rate, [c for c in counters if c["inn_index"] == rate["index"]],
             portraits, flow["inn"]["cost"]["max_party"])
        for rate in rates
    ]

    census = _census(counters, inventories, inns, prices, flow)
    if complete and census["live_counters_without_an_object"]:
        raise ShopPackError(
            "these live counters name a position no map object occupies, so the "
            "shopkeeper binding does not hold: "
            f"{census['live_counters_without_an_object']}"
        )
    return {
        "kind": "shops",
        "counter_count": len(counters),
        "inventory_count": len(inventories),
        "inn_count": len(inns),
        "source": {
            "locations": {
                "label": locations["label"],
                "rom_offset": locations["rom_offset"],
                "entry_bytes": locations["entry_size"],
                "terminator": locations["terminator"],
            },
            "inventories": {
                "label": shops["table"]["label"],
                "rom_offset": shops["table"]["rom_offset"],
                "rom_end_exclusive": shops["table"]["rom_end_exclusive"],
                "terminator": shops["table"]["terminator"],
                "indexing": shops["table"]["indexing"],
            },
            "inn_rates": {
                "label": "loc_68112",
                "rom_offset": flow["inn"]["table"],
                "entry_bytes": flow["inn"]["entry_bytes"],
            },
            "greetings": {
                "label": "loc_68136",
                "rom_offset": flow["presentation"]["greetings"]["table"],
                "entry_bytes": flow["presentation"]["greetings"]["entry_bytes"],
            },
            "portraits": {
                "label": "ShopPortraitGroupsPtrs",
                "rom_offset": flow["presentation"]["portraits"]["pointers"],
            },
        },
        "rules": flow,
        "counters": counters,
        "inventories": inventories,
        "inns": inns,
        "census": census,
    }


def _counter(entry: dict[str, Any], row: int,
             objects: dict[tuple[int, int, int], dict[str, Any]],
             portraits: dict[int, list[str]],
             greetings: list[dict[str, Any]],
             maps: dict[int, dict[str, Any]]) -> dict[str, Any]:
    is_inn = entry["group_id"] == INN_GROUP
    live = (entry["x"], entry["y"]) != DEAD_POSITION
    keeper = objects.get((entry["map_id"], entry["x"], entry["y"]))
    group_for_art = 0 if is_inn else 1
    art = portraits.get(group_for_art, [])
    index = entry["index"]
    out: dict[str, Any] = {
        "id": row,
        "rom_offset": entry["rom_offset"],
        "raw_hex": entry["raw_hex"],
        "map_id": entry["map_id"],
        "map_symbol": entry["map_symbol"],
        "x_pixels": entry["x"],
        "y_pixels": entry["y"],
        # The same standing-cell convention every other packed position uses:
        # `GetChunkAndCollision` adds $10 to Y before shifting down to a cell.
        "x_cell": entry["x"] >> 4,
        "y_cell": (entry["y"] + 0x10) >> 4,
        "selector": entry["selector"],
        "group": {"id": entry["group_id"], "name": entry["group"]},
        "index": index,
        "kind": "inn" if is_inn else "shop",
        "shop_inventory_index": entry["shop_inventory_index"],
        "inn_index": index if is_inn else None,
        "live": live,
        "portrait": art[index] if index < len(art) else None,
        # What the shopkeeper is standing on. Recorded because it is *not*
        # what selects the shop -- see the census.
        "collision": _collision_at(maps.get(entry["map_id"]),
                                  entry["x"] >> 4, (entry["y"] + 0x10) >> 4),
        "shopkeeper": None if keeper is None else {
            "object_index": keeper["index"],
            "object_id": keeper["object_id"],
            "symbol": keeper["symbol"],
            "facing": keeper["facing"],
            "dialogue_id": keeper["dialogue_id"],
            "x_cell": keeper["x_cell"],
            "y_cell": keeper["y_cell"],
        },
    }
    if not is_inn:
        out["greeting"] = greetings[index] if index < len(greetings) else None
    return out


def _collision_at(record: dict[str, Any] | None, x: int, y: int) -> int | None:
    """The collision code under a cell, or `None` when the map is not packed."""
    if record is None:
        return None
    grid = record["collision"]
    if not (0 <= y < grid["height_cells"] and 0 <= x < grid["width_cells"]):
        return None
    return grid["rows"][y][x]


def _inventory(record: dict[str, Any], prices: dict[int, dict[str, Any]],
               names: dict[int, str], bound: list[int]) -> dict[str, Any]:
    items = []
    for item in record["items"]:
        price = prices[item["id"]]
        items.append({
            "item_id": item["id"],
            "symbol": item["symbol"],
            "display_name": names.get(item["id"]),
            "type": {"id": price["type"], "name": ITEM_TYPES.get(price["type"])},
            "buy_price": price["buy"],
            "sell_price": price["sell"],
        })
    return {
        "index": record["index"],
        "rom_offset": record["rom_offset"],
        "raw_hex": record["raw_hex"],
        "item_count": record["item_count"],
        "items": items,
        "bound_by": bound,
        "reachable": bool(bound),
    }


def _inn(rate: dict[str, Any], counters: list[dict[str, Any]],
         portraits: dict[int, list[str]], max_party: int) -> dict[str, Any]:
    return {
        "index": rate["index"],
        "rom_offset": rate["rom_offset"],
        "raw_hex": rate["raw_hex"],
        "rate_per_character": rate["rate"],
        "text_variant": rate["text_variant"],
        "portrait": portraits[0][rate["index"]],
        "cost_by_party_size": {
            str(size): rate["rate"] * size for size in range(1, max_party + 1)
        },
        "bound_by": [counter["id"] for counter in counters],
        "maps": [counter["map_symbol"] for counter in counters],
    }


def _census(counters: list[dict[str, Any]], inventories: list[dict[str, Any]],
            inns: list[dict[str, Any]], prices: dict[int, dict[str, Any]],
            flow: dict[str, Any]) -> dict[str, Any]:
    groups: dict[str, int] = {}
    for counter in counters:
        key = str(counter["group"]["id"])
        groups[key] = groups.get(key, 0) + 1
    sold = sorted({item["item_id"] for inv in inventories for item in inv["items"]})
    live = [counter for counter in counters if counter["live"]]
    return {
        "counters_by_group": groups,
        "live_counters": len(live),
        # Three rows name (0,0), a position no object occupies, so they can
        # never match. They are what makes three inventories unreachable.
        "dead_counters": [
            {"id": counter["id"], "map_symbol": counter["map_symbol"],
             "selector": counter["selector"], "rom_offset": counter["rom_offset"]}
            for counter in counters if not counter["live"]
        ],
        "counters_bound_to_an_object": sum(
            1 for counter in counters if counter["shopkeeper"] is not None
        ),
        "live_counters_without_an_object": [
            counter["id"] for counter in live if counter["shopkeeper"] is None
        ],
        "unreachable_inventories": [
            inv["index"] for inv in inventories if not inv["reachable"]
        ],
        "inventory_sizes": {
            "min": min(inv["item_count"] for inv in inventories),
            "max": max(inv["item_count"] for inv in inventories),
        },
        "distinct_items_sold": len(sold),
        "items_never_sold": [
            item_id for item_id in prices if item_id not in sold
        ],
        "items_sold_with_no_price": sorted({
            item["item_id"] for inv in inventories for item in inv["items"]
            if item["buy_price"] == 0
        }),
        "price_range": {
            "min": min(prices[i]["buy"] for i in sold),
            "max": max(prices[i]["buy"] for i in sold),
        },
        "inn_rates": {
            "min": min(inn["rate_per_character"] for inn in inns),
            "max": max(inn["rate_per_character"] for inn in inns),
        },
        "inns_with_alternate_text": [
            inn["index"] for inn in inns if inn["text_variant"]
        ],
        "distinct_portraits": len({
            art for group in flow["presentation"]["portraits"]["groups"]
            for art in group["art"]
        }),
        "maps_with_a_counter": len({counter["map_id"] for counter in counters}),
        # The counter tile is scenery. `loc_65D12` matches on the shopkeeper's
        # position and never reads the collision grid, and the spread here is
        # the evidence: most keepers stand behind a shop-collision tile, but a
        # sizeable minority stand on ordinary floor.
        "counter_collision": {
            str(code): sum(1 for c in counters if c["collision"] == code)
            for code in sorted({
                c["collision"] for c in counters if c["collision"] is not None
            })
        },
    }


def emit_shops(rom: bytes, maps: dict[int, dict[str, Any]], out_dir: str | Path,
               version: int, complete: bool = True) -> dict[str, Any]:
    """Write `shops.json` and return the manifest fragment describing it."""
    payload = build_shops(rom, maps, complete)
    directory = Path(out_dir)
    directory.mkdir(parents=True, exist_ok=True)
    text = json.dumps({"format_version": version, **payload}, indent=2,
                      sort_keys=True) + "\n"
    data = text.encode("utf-8")
    (directory / SHOPS_NAME).write_bytes(data)
    return {
        "file": SHOPS_NAME,
        "sha256": hashlib.sha256(data).hexdigest(),
        "counters": payload["counter_count"],
        "inventories": payload["inventory_count"],
        "inns": payload["inn_count"],
        "prices": {
            "offset": payload["rules"]["prices"]["price_offset"],
            "sell_divisor": payload["rules"]["prices"]["sell"]["divisor"],
            "unlimited_stock": payload["rules"]["prices"]["stock"]["unlimited"],
        },
        "census": payload["census"],
    }
