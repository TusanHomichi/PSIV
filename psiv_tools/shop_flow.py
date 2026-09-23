"""How a shop is reached, what it charges, and what an inn does for it.

`psiv_tools.shops` decodes the two shop tables -- 49 `$FF`-terminated
inventories and the 68-entry location table. Neither says how a shop opens,
what an item costs, what a sale pays, whether stock runs out, or what a night
at an inn restores. Six routines do, and this module reads them out of retail
rather than transcribing the disassembly.

============================  =========  ==================================
routine                       retail     what is taken from it
============================  =========  ==================================
`loc_65D12`                   `$065D12`  the counter scan: which table it
                                         walks, its stride and terminator,
                                         and that it matches on a *position*
                                         rather than a collision type
`Win_ShopBuyList`             `$0651D8`  the buy price: the word at item
                                         record `$14`, spent as-is
`Win_ShopSellConfirm`         `$065758`  the sell price: the same word,
                                         `lsr.w #1` -- half, rounding down
`Win_ShopMessage`             `$065E90`  the inn rate table, and that the
                                         bill is rate x party size
`RecoverStats`                `$0662DA`  what a night restores
`Win_ShopMeseta`              `$065E04`  which portrait group a shop draws
                                         its keeper from
============================  =========  ==================================

What the flow is not
--------------------

Two things this deliberately stops short of, because they are window plumbing
rather than data:

* the menu state machine (`Win_ShopBuySell` and the eight windows around it)
  is presentation -- which window is created next, where the cursor starts,
  how the text is composed tile by tile;
* the greeting *text* itself. What is extracted here is the binding -- which
  selector picks which fragment and which portrait -- not the fragments, which
  are ordinary window text and belong with the dialogue half of the pack.

The counter is not a collision type
-----------------------------------

`SOURCE_NOTES` records that a shop-location entry "lands on a type-$C cell".
Reading `loc_65D12` and its one caller, that is a correlation, not the rule:
the caller reaches the scan only after `Interaction_ChkObjects` has already
matched an *object*, and passes that object's `$30`/`$34` position. The scan
compares map index, x and y and never reads the collision grid at all. So the
table is keyed by the shopkeeper's position; the `$C` counter tile in front of
them is scenery that happens to be under 52 of the 68 entries. Thirteen
shopkeepers stand on ordinary floor, and the three remaining entries name a
position no object occupies -- see `dead_counters`.
"""

from __future__ import annotations

from typing import Any

from . import m68k
from .shops import SHOP_GROUPS, extract_shops

#: Signatures, each with the number of times retail contains it.
SIGNATURES: dict[str, tuple[str, int]] = {
    # lea (loc_68394).l, a0 -- the counter scan naming the location table.
    "CounterScan": ("41f900068394", 1),
    # jsr (loc_65D12).l -- its one caller, inside the interaction check.
    "CounterScanCall": ("4eb900065d12", 1),
    # move.w $14(a0,d0.w), ($FFFFE3F8).w -- the buy price.
    "BuyPrice": ("31f00014e3f8", 1),
    # move.w $14(a0,d0.w), d0 / lsr.w #1, d0 -- the sell price.
    "SellPrice": ("30300014e248", 1),
    # lea (loc_68112).l, a1 -- the inn rate table.
    "InnRates": ("43f900068112", 1),
    # mulu.w (Win_Char_Num).l, d3 -- rate times party size.
    "InnPartyMultiply": ("c6f9ffffe3f0", 1),
    # lea (loc_68136).l, a0 -- the per-shop greeting selector.
    "GreetingSelector": ("41f900068136", 1),
    # lea (ShopPortraitGroupsPtrs).l, a0 -- the two portrait groups.
    "PortraitGroups": ("41f900067ffe", 1),
    # move.w max_hp(a0), curr_hp(a0) -- RecoverStats' first act.
    "RecoverHp": ("31680010000e", 5),
    # cmpi.w #6, ($FFFFECD0).w -- the one inn with a scripted rest.
    "RestEvent": ("0c780006ecd0", 1),
}

#: `Character_Stats` offsets `RecoverStats` writes, and what they mean.
RECOVERY_FIELDS = {
    0x0E: "curr_hp", 0x10: "max_hp", 0x12: "curr_tp", 0x14: "max_tp",
    0x16: "status", 0x6A: "curr_skill_uses", 0x6B: "max_skill_uses",
}

#: Item record offset holding the shop price, and its width.
PRICE_OFFSET = 0x14
PRICE_BYTES = 2

#: The most party slots `CalcWinIDAndCharNum` will count.
MAX_PARTY = 5

#: Skill slots per character, the same eight `InitialCharStats` fills.
SKILL_SLOTS = 8


class ShopFlowError(m68k.DecodeError):
    pass


def _sites(rom: bytes, label: str) -> list[int]:
    signature, expected = SIGNATURES[label]
    try:
        return m68k.find_exactly(rom, signature, expected, label)
    except m68k.DecodeError as exc:
        raise ShopFlowError(str(exc)) from None


def _site(rom: bytes, label: str) -> int:
    return _sites(rom, label)[0]


def _expect(rom: bytes, at: int, opcode: int, what: str) -> None:
    """`m68k.expect`, reported as this module's error."""
    try:
        m68k.expect(rom, at, opcode, what)
    except m68k.DecodeError as exc:
        raise ShopFlowError(str(exc)) from None


# ---------------------------------------------------------------------------
# Reaching a shop
# ---------------------------------------------------------------------------
def read_counter_scan(rom: bytes) -> dict[str, Any]:
    """`loc_65D12`: the linear scan that turns a position into a shop selector.

    The routine's own constants give the table it walks, the stride it steps
    by, the terminator it stops on and the offset of the selector it stores --
    so the location table's shape is proven by its only reader.
    """
    site = _site(rom, "CounterScan")
    table = m68k.l(rom, site + 2)
    _expect(rom, site + 6, 0x3438, "move.w (Field_Map_Index).w, d2")
    field_map_index = 0xFFFF0000 | m68k.w(rom, site + 8)

    _expect(rom, site + 10, 0x0C50, "cmpi.w #terminator, (a0)")
    terminator = m68k.w(rom, site + 12)

    # Three compares, each followed by the `bne.s` that rejects the entry: map
    # index against d2, then x against d0 and y against d1.
    compares = []
    at = site + 16
    for register in ("d2", "d0", "d1"):
        opcode = m68k.w(rom, at)
        if opcode == 0xB450:  # cmp.w (a0), d2
            compares.append((0, register))
            at += 2
        elif opcode in (0xB068, 0xB268):  # cmp.w d16(a0), d0 / d1
            compares.append((m68k.w(rom, at + 2), register))
            at += 4
        else:
            raise ShopFlowError(
                f"0x{at:06X}: unexpected opcode 0x{opcode:04X} in the counter scan"
            )
        if rom[at] != 0x66:  # bne.s: this entry is not the one
            raise ShopFlowError(
                f"0x{at:06X}: the counter scan's {register} compare is not "
                "followed by a `bne.s`"
            )
        at += 2
    _expect(rom, at, 0x31E8, "move.w d16(a0), ($FFFFECD0).w")
    selector_offset = m68k.w(rom, at + 2)
    selector_ram = 0xFFFF0000 | m68k.w(rom, at + 4)

    stride = _scan_stride(rom, at)
    if selector_offset + 2 != stride:
        raise ShopFlowError(
            f"the counter scan reads its selector at +0x{selector_offset:X} of a "
            f"{stride}-byte entry; the selector is not the entry's last word"
        )
    return {
        "routine": f"0x{site:06X}",
        "called_from": f"0x{_site(rom, 'CounterScanCall'):06X}",
        "table": f"0x{table:06X}",
        "entry_bytes": stride,
        "terminator": f"0x{terminator:04X}",
        "field_map_index": f"0x{field_map_index:08X}",
        "selector_ram": f"0x{selector_ram:08X}",
        "fields": [
            {"offset": f"0x{offset:02X}", "compared_against": register}
            for offset, register in compares
        ] + [{"offset": f"0x{selector_offset:02X}", "compared_against": None}],
        "matches_on": "position",
        "note": (
            "the caller reaches this only after Interaction_ChkObjects has "
            "matched an object, and passes that object's $30/$34 position. The "
            "scan never reads the collision grid: a shop is bound to the "
            "shopkeeper's position, not to the counter tile in front of them."
        ),
    }


def _scan_stride(rom: bytes, at: int) -> int:
    """The `addq.w #n, a0` that advances the scan by one entry."""
    for probe in range(at, at + 0x20, 2):
        opcode = m68k.w(rom, probe)
        if opcode & 0xF1FF == 0x5048:  # addq.w #n, a0
            count = (opcode >> 9) & 7
            return 8 if count == 0 else count
    raise ShopFlowError(f"0x{at:06X}: the counter scan never advances a0")


# ---------------------------------------------------------------------------
# Prices
# ---------------------------------------------------------------------------
def read_prices(rom: bytes) -> dict[str, Any]:
    """The buy and sell rules, from the two instructions that apply them."""
    buy = _site(rom, "BuyPrice")
    if m68k.w(rom, buy + 2) != PRICE_OFFSET:
        raise ShopFlowError(
            f"0x{buy:06X}: the buy price is read from item offset "
            f"0x{m68k.w(rom, buy + 2):02X}, not 0x{PRICE_OFFSET:02X}"
        )
    price_ram = 0xFFFF0000 | m68k.w(rom, buy + 4)

    sell = _site(rom, "SellPrice")
    if m68k.w(rom, sell + 2) != PRICE_OFFSET:
        raise ShopFlowError(
            f"0x{sell:06X}: the sell price is read from item offset "
            f"0x{m68k.w(rom, sell + 2):02X}, not 0x{PRICE_OFFSET:02X}"
        )
    shift = m68k.w(rom, sell + 4)
    if shift & 0xFFC0 != 0xE240 or shift & 0x0007:  # lsr.w #n, dN
        raise ShopFlowError(
            f"0x{sell + 4:06X}: the sell price is not adjusted by an `lsr.w` "
            f"(found 0x{shift:04X})"
        )
    count = (shift >> 9) & 7
    divisor = 1 << (8 if count == 0 else count)
    return {
        "price_offset": f"0x{PRICE_OFFSET:02X}",
        "price_bytes": PRICE_BYTES,
        "encoding": "binary",
        "price_ram": f"0x{price_ram:08X}",
        "buy": {
            "rom_offset": f"0x{buy:06X}",
            "rule": "the item record's price word, spent unchanged",
            "affordability": (
                "sub.l against Current_Money; a negative result refuses the sale"
            ),
        },
        "sell": {
            "rom_offset": f"0x{sell:06X}",
            "rule": f"the same word divided by {divisor}, rounding down",
            "divisor": divisor,
            "arithmetic": f"lsr.w #{count if count else 8}",
        },
        "stock": {
            "unlimited": True,
            "note": (
                "the buy list is re-read from ShopInventories, which is ROM, on "
                "every visit, and the purchase path writes only the inventory "
                "slot and Current_Money. Nothing anywhere decrements a shop."
            ),
        },
    }


def sell_price(buy: int, divisor: int) -> int:
    return buy // divisor


# ---------------------------------------------------------------------------
# Inns
# ---------------------------------------------------------------------------
def read_inn_rules(rom: bytes) -> dict[str, Any]:
    """The inn rate table, the bill, and what a night restores."""
    site = _site(rom, "InnRates")
    table = m68k.l(rom, site + 2)
    _expect(rom, site + 6, 0x1038, "move.b ($FFFFECD1).w, d0")
    index_ram = 0xFFFF0000 | m68k.w(rom, site + 8)
    _expect(rom, site + 10, 0xD040, "add.w d0, d0")
    _expect(rom, site + 12, 0x43F1, "lea (a1,d0.w), a1")
    _expect(rom, site + 18, 0x1629, "move.b d16(a1), d3")
    rate_offset = m68k.w(rom, site + 20)

    multiply = _site(rom, "InnPartyMultiply")
    party_ram = m68k.l(rom, multiply + 2)
    _expect(rom, multiply + 6, 0x31C3, "move.w d3, (total).w")
    total_ram = 0xFFFF0000 | m68k.w(rom, multiply + 8)
    _expect(rom, multiply + 10, 0x11D1, "move.b (a1), (variant).w")

    event = _site(rom, "RestEvent")
    return {
        "table": f"0x{table:06X}",
        "entry_bytes": 2,
        "rate_offset": f"0x{rate_offset:02X}",
        "variant_offset": "0x00",
        "index_ram": f"0x{index_ram:08X}",
        "party_count_ram": f"0x{party_ram:08X}",
        "total_ram": f"0x{total_ram:08X}",
        "rom_offset": f"0x{site:06X}",
        "cost": {
            "formula": "rate * party_slots",
            "party_slots": (
                "CalcWinIDAndCharNum counts entries in Current_Party_Slots up to "
                f"the {MAX_PARTY}-slot terminator; a dead member still counts"
            ),
            "max_party": MAX_PARTY,
        },
        "restores": read_recovery(rom),
        "scripted_rest": {
            "rom_offset": f"0x{event:06X}",
            "selector": f"0x{m68k.w(rom, event + 2):04X}",
            "note": (
                "one selector runs an event instead of an ordinary night, and "
                "only while both of its gate flags are clear"
            ),
        },
    }


def read_recovery(rom: bytes) -> dict[str, Any]:
    """`DoCharRecovery`: the writes a night makes to one character.

    Walked rather than transcribed -- the routine is a straight run of
    `move.w`/`move.b` from a max field to its current one, plus the status
    clear, and it ends at the `rts`.
    """
    site = _sites(rom, "RecoverHp")[3]
    writes: list[dict[str, Any]] = []
    at = site
    while True:
        opcode = m68k.w(rom, at)
        if opcode == 0x4E75:
            break
        if opcode == 0x3168:  # move.w d16(a0), d16(a0)
            writes.append(_recovery_write(m68k.w(rom, at + 2), m68k.w(rom, at + 4), 2))
            at += 6
        elif opcode == 0x1168:  # move.b d16(a0), d16(a0)
            writes.append(_recovery_write(m68k.w(rom, at + 2), m68k.w(rom, at + 4), 1))
            at += 6
        elif opcode == 0x117C:  # move.b #imm, d16(a0)
            value = m68k.w(rom, at + 2)
            offset = m68k.w(rom, at + 4)
            writes.append({
                "field": RECOVERY_FIELDS.get(offset, f"0x{offset:02X}"),
                "offset": f"0x{offset:02X}", "bytes": 1,
                "from": None, "value": value,
            })
            at += 6
        else:
            raise ShopFlowError(
                f"0x{at:06X}: unexpected opcode 0x{opcode:04X} in DoCharRecovery"
            )
        if at - site > 0x80:
            raise ShopFlowError(f"0x{site:06X}: DoCharRecovery does not end")
    return {
        "routine": f"0x{site:06X}",
        "per_character": writes,
        "applies_to": "every occupied party slot, then the three vehicles",
    }


def _recovery_write(source: int, destination: int, width: int) -> dict[str, Any]:
    def name(offset: int) -> str:
        # The eight skill-use slots are curr/max byte pairs from $6A, so the
        # range has to be checked before the flat table.
        if 0x6A <= offset < 0x6A + SKILL_SLOTS * 2:
            slot, half = divmod(offset - 0x6A, 2)
            field = "curr_skill_uses" if half == 0 else "max_skill_uses"
            return f"{field}[{slot}]"
        return RECOVERY_FIELDS.get(offset, f"0x{offset:02X}")

    return {
        "field": name(destination),
        "offset": f"0x{destination:02X}",
        "bytes": width,
        "from": name(source),
        "value": None,
    }


def read_inn_rates(rom: bytes, table: int, count: int) -> list[dict[str, Any]]:
    """The rate table itself: one word per inn, `variant << 8 | rate`."""
    rates = []
    for index in range(count):
        offset = table + index * 2
        word = m68k.w(rom, offset)
        rates.append({
            "index": index,
            "rom_offset": f"0x{offset:06X}",
            "raw_hex": f"{word:04x}",
            "rate": word & 0xFF,
            "text_variant": word >> 8,
        })
    return rates


# ---------------------------------------------------------------------------
# Who is behind the counter
# ---------------------------------------------------------------------------
def read_presentation(rom: bytes) -> dict[str, Any]:
    """The portrait groups and the greeting selector, with their sizes proven.

    `ShopPortraitGroupsPtrs` holds two pointers; each group is a run of art
    pointers, and the second group ends exactly where the inn rate table
    begins, which is what bounds it.
    """
    site = _site(rom, "PortraitGroups")
    pointers = m68k.l(rom, site + 2)
    _expect(rom, site + 8, 0x1038, "move.b ($FFFFECD0).w, d0")
    _expect(rom, site + 12, 0x0C00, "cmpi.b #group, d0")
    alternate_group = m68k.w(rom, site + 14)

    groups = [m68k.l(rom, pointers + i * 4) for i in range(2)]
    if groups[0] != pointers + 8:
        raise ShopFlowError(
            f"the first portrait group is at 0x{groups[0]:06X}, not immediately "
            f"after the two pointers at 0x{pointers + 8:06X}"
        )
    rates = m68k.l(rom, _site(rom, "InnRates") + 2)
    bounds = [groups[1], rates]
    sizes = [(bounds[i] - groups[i]) // 4 for i in range(2)]
    if any(size <= 0 for size in sizes):
        raise ShopFlowError("the portrait groups do not run in table order")

    selector = _site(rom, "GreetingSelector")
    return {
        "portraits": {
            "pointers": f"0x{pointers:06X}",
            "rom_offset": f"0x{site:06X}",
            "groups": [
                {"group": index, "rom_offset": f"0x{groups[index]:06X}",
                 "count": sizes[index],
                 "art": [f"0x{m68k.l(rom, groups[index] + i * 4):06X}"
                         for i in range(sizes[index])]}
                for index in range(2)
            ],
            "alternate_group_uses": alternate_group - 1,
            "note": (
                "the selector's high byte picks the group, except that group "
                f"{alternate_group} reads group {alternate_group - 1}'s "
                "portraits; the low byte indexes within it"
            ),
        },
        "greetings": {
            "table": f"0x{m68k.l(rom, selector + 2):06X}",
            "rom_offset": f"0x{selector:06X}",
            "entry_bytes": 2,
            "note": (
                "one word per shop: the high byte picks which of two greeting "
                "texts is used and the low byte picks the noun fragment naming "
                "the shop's trade. Inns ignore it and use their rate entry's "
                "high byte instead. The fragments are window text, not dialogue "
                "tree entries -- talking to a shopkeeper never runs the tree."
            ),
        },
    }


def read_greeting_selectors(rom: bytes, table: int, count: int) -> list[dict[str, Any]]:
    return [
        {
            "index": index,
            "rom_offset": f"0x{table + index * 2:06X}",
            "raw_hex": f"{m68k.w(rom, table + index * 2):04x}",
            "text_variant": m68k.w(rom, table + index * 2) >> 8,
            "trade_fragment": m68k.w(rom, table + index * 2) & 0xFF,
        }
        for index in range(count)
    ]


# ---------------------------------------------------------------------------
# Everything at once
# ---------------------------------------------------------------------------
def read_flow(rom: bytes) -> dict[str, Any]:
    """Every rule `shops.json` depends on, decoded from retail in one pass."""
    counter = read_counter_scan(rom)
    shops = extract_shops(rom)
    locations = shops["locations"]
    if counter["table"].lower() != locations["rom_offset"].lower():
        raise ShopFlowError(
            f"the counter scan walks {counter['table']}, but psiv_tools.shops "
            f"decodes the location table at {locations['rom_offset']}"
        )
    if counter["entry_bytes"] != locations["entry_size"]:
        raise ShopFlowError(
            f"the counter scan steps {counter['entry_bytes']} bytes; the location "
            f"table is decoded with {locations['entry_size']}-byte entries"
        )
    return {
        "interaction": counter,
        "prices": read_prices(rom),
        "inn": read_inn_rules(rom),
        "presentation": read_presentation(rom),
        "groups": {str(key): value for key, value in SHOP_GROUPS.items()},
    }
