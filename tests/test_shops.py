import re
import unittest
from pathlib import Path

from psiv_tools.shops import (
    SHOP_COUNT,
    SHOP_INVENTORIES,
    SHOP_SIGNATURE,
    ShopError,
    extract_shops,
    parse_shop_record,
    split_shop_records,
)
from psiv_tools.core import read_rom
from psiv_tools.symbols import ITEM_SYMBOLS

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
DISASM = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"
ASM = DISASM / "ps4.asm"
CONSTANTS = DISASM / "ps4.constants.asm"


def read_item_ids() -> dict[str, int]:
    """Parse `ItemID_* = id(Item_*)  ; <value>` out of the disassembly constants.

    `ps4.constants.asm` sets `idstart := 1` immediately above the block, so the
    ids are also verified to be sequential from 1 in file order rather than
    trusted from the trailing comments alone.
    """
    ids: dict[str, int] = {}
    pattern = re.compile(r"^ItemID_(\w+)\s*=\s*id\(Item_\w+\)\s*;\s*(\$?)([0-9A-Fa-f]+)\s*$")
    for raw in CONSTANTS.read_text(encoding="utf-8").splitlines():
        match = pattern.match(raw.strip())
        if not match:
            continue
        name, dollar, digits = match.groups()
        value = int(digits, 16) if dollar else int(digits)
        if value != len(ids) + 1:
            raise AssertionError(
                f"ItemID_{name} = {value} breaks the 1-based sequence at position {len(ids) + 1}"
            )
        ids[name] = value
    return ids


def read_shop_inventories_source() -> tuple[bytes, list[list[int]]]:
    """Rebuild the ShopInventories table from the disassembly's `dc.b` listing.

    Anything other than a comment, a blank line, the closing `even`, or a
    `dc.b` of a single `ItemID_*`/`$FF` operand means the shape of the source
    changed and the oracle can no longer be trusted, so fail loudly.
    """
    ids = read_item_ids()
    lines = ASM.read_text(encoding="utf-8").splitlines()
    try:
        start = next(i for i, line in enumerate(lines) if line.startswith("ShopInventories:"))
    except StopIteration:
        raise AssertionError("ShopInventories label not found in ps4.asm")

    data = bytearray()
    shops: list[list[int]] = []
    current: list[int] = []
    for lineno, raw in enumerate(lines[start + 1:], start=start + 2):
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        if line == "even":
            break
        directive, _, operand = line.partition("\t")
        if directive.strip() != "dc.b":
            raise AssertionError(f"ps4.asm:{lineno}: expected dc.b, got {line!r}")
        operand = operand.strip()
        if operand == "$FF":
            data.append(0xFF)
            shops.append(current)
            current = []
        elif operand.startswith("ItemID_"):
            value = ids[operand[len("ItemID_"):]]
            data.append(value)
            current.append(value)
        else:
            raise AssertionError(f"ps4.asm:{lineno}: unexpected operand {operand!r}")
    if current:
        raise AssertionError("ShopInventories source ends without a $FF terminator")
    return bytes(data), shops


class TestShopRecordParsing(unittest.TestCase):
    """Structural unit tests. These need no ROM and no disassembly."""

    def test_split_on_terminators(self):
        block = bytes.fromhex("7dff0102ff")
        self.assertEqual(
            split_shop_records(block),
            [(0, bytes.fromhex("7dff")), (2, bytes.fromhex("0102ff"))],
        )

    def test_unterminated_tail_raises(self):
        with self.assertRaises(ShopError):
            split_shop_records(bytes.fromhex("7dff0102"))

    def test_record_without_terminator_raises(self):
        with self.assertRaises(ShopError):
            parse_shop_record(bytes.fromhex("7d"))

    def test_empty_record_raises(self):
        with self.assertRaises(ShopError):
            parse_shop_record(bytes.fromhex("ff"))

    def test_out_of_range_item_raises(self):
        with self.assertRaises(ShopError):
            parse_shop_record(bytes([len(ITEM_SYMBOLS) + 1, 0xFF]))

    def test_item_ids_are_one_based(self):
        parsed = parse_shop_record(bytes.fromhex("01ff"))
        self.assertEqual(parsed["items"], [{"id": 1, "symbol": "Dagger"}])


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestShopsFromRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.result = extract_shops(cls.data)
        cls.shops = cls.result["shops"]
        cls.locations = cls.result["locations"]

    def test_signature_is_unique(self):
        self.assertEqual(self.data.count(SHOP_SIGNATURE), 1)
        start = SHOP_INVENTORIES["start"]
        self.assertEqual(self.data[start:start + len(SHOP_SIGNATURE)], SHOP_SIGNATURE)

    def test_altered_table_fails_closed(self):
        for offset in (SHOP_INVENTORIES["start"], SHOP_INVENTORIES["end"]):
            with self.subTest(hex(offset)):
                corrupt = bytearray(self.data)
                corrupt[offset] ^= 0xFF
                with self.assertRaises(ShopError):
                    extract_shops(bytes(corrupt))

    def test_table_geometry(self):
        table = self.result["table"]
        self.assertEqual(table["rom_offset"], "0x0681A4")
        self.assertEqual(table["rom_end_exclusive"], "0x0682A3")
        self.assertEqual(table["size_bytes"], 255)
        self.assertEqual(table["terminator"], "0xFF")

    def test_table_is_flush_against_its_neighbours(self):
        """255 data bytes, one `even` pad, then the loc_682A4 pointer block.

        The preceding block loc_68198 is three longs ending exactly at the
        table start, so ShopInventories is flush on both sides.
        """
        table = self.result["table"]
        self.assertEqual(table["pad_byte"], "0x00")
        self.assertEqual(table["pad_offset"], "0x0682A3")
        self.assertEqual(table["next_label"], "loc_682A4")
        self.assertEqual(table["next_label_first_long"], "0x002AC470")
        preceding = self.data[0x68198:0x681A4]
        self.assertEqual(preceding.hex(), "002ac6a0002ac6a8002ac6ae")

    def test_shop_count(self):
        self.assertEqual(self.result["shop_count"], 49)
        self.assertEqual(SHOP_COUNT, 49)
        self.assertEqual(len(self.shops), 49)
        self.assertEqual([s["index"] for s in self.shops], list(range(49)))

    def test_pinned_shop_zero(self):
        shop = self.shops[0]
        self.assertEqual(shop["rom_offset"], "0x0681A4")
        self.assertEqual(shop["block_offset"], "0x0000")
        self.assertEqual(shop["raw_hex"], "7dff")
        self.assertEqual(shop["item_count"], 1)
        self.assertEqual(shop["items"], [{"id": 0x7D, "symbol": "Monomate"}])

    def test_pinned_shop_one(self):
        shop = self.shops[1]
        self.assertEqual(shop["rom_offset"], "0x0681A6")
        self.assertEqual(shop["raw_hex"], "0102080309ff")
        self.assertEqual(shop["item_count"], 5)
        self.assertEqual(
            shop["items"],
            [
                {"id": 1, "symbol": "Dagger"},
                {"id": 2, "symbol": "HuntKnife"},
                {"id": 8, "symbol": "StelSword"},
                {"id": 3, "symbol": "Boomerang"},
                {"id": 9, "symbol": "Slasher"},
            ],
        )

    def test_pinned_last_shop(self):
        shop = self.shops[-1]
        self.assertEqual(shop["index"], 0x30)
        self.assertEqual(shop["rom_end_exclusive"], "0x0682A3")
        self.assertEqual(
            [i["symbol"] for i in shop["items"]],
            ["Trimate", "Antidote", "CureParal", "Telepipe", "Escapipe"],
        )

    def test_every_item_resolves(self):
        for shop in self.shops:
            with self.subTest(shop["index"]):
                self.assertGreaterEqual(shop["item_count"], 1)
                for item in shop["items"]:
                    self.assertTrue(1 <= item["id"] <= len(ITEM_SYMBOLS))
                    self.assertIsNotNone(item["symbol"])
                    self.assertEqual(item["symbol"], ITEM_SYMBOLS[item["id"] - 1])

    def test_provenance_covers_the_table_exactly(self):
        start = int(self.result["table"]["rom_offset"], 16)
        end = int(self.result["table"]["rom_end_exclusive"], 16)
        cursor = start
        for shop in self.shops:
            self.assertEqual(shop["rom_offset"], f"0x{cursor:06X}")
            raw = bytes.fromhex(shop["raw_hex"])
            self.assertEqual(self.data[cursor:cursor + len(raw)], raw)
            self.assertEqual(len(raw), shop["item_count"] + 1)
            cursor += len(raw)
            self.assertEqual(shop["rom_end_exclusive"], f"0x{cursor:06X}")
        self.assertEqual(cursor, end)

    def test_terminator_scan_matches_the_games_own(self):
        """`GetOffsetByID_FF_Delim` counts $FF bytes one at a time."""
        block = self.data[SHOP_INVENTORIES["start"]:SHOP_INVENTORIES["end"]]
        self.assertEqual(block.count(0xFF), len(self.shops))
        for index, shop in enumerate(self.shops):
            skipped = 0
            cursor = 0
            while skipped < index:
                if block[cursor] == 0xFF:
                    skipped += 1
                cursor += 1
            self.assertEqual(shop["rom_offset"], f"0x{SHOP_INVENTORIES['start'] + cursor:06X}")

    def test_shop_locations(self):
        locations = self.locations
        self.assertEqual(locations["rom_offset"], "0x068394")
        self.assertEqual(locations["rom_end_exclusive"], "0x0685B6")
        self.assertEqual(locations["entry_count"], 68)
        self.assertEqual(locations["entry_size"], 8)
        self.assertEqual(locations["inn_count"], 18)
        # Groups 1 and 2 between them cover every shop index with no gaps.
        self.assertEqual(locations["referenced_shop_indexes"], list(range(49)))
        by_group: dict[int, set[int]] = {}
        for entry in locations["entries"]:
            by_group.setdefault(entry["group_id"], set()).add(entry["index"])
        self.assertEqual(sorted(by_group), [0, 1, 2])
        self.assertEqual(by_group[0], set(range(0x12)))
        self.assertEqual(by_group[2], {0x1D, 0x1E})
        self.assertEqual(by_group[1] | by_group[2], set(range(49)))
        self.assertEqual(by_group[1] & by_group[2], set())

    def test_inn_entries_do_not_index_shop_inventories(self):
        for entry in self.locations["entries"]:
            with self.subTest(entry["rom_offset"]):
                if entry["group_id"] == 0:
                    self.assertEqual(entry["group"], "inn")
                    self.assertIsNone(entry["shop_inventory_index"])
                else:
                    self.assertEqual(entry["shop_inventory_index"], entry["index"])

    def test_first_location_entry(self):
        first = self.locations["entries"][0]
        self.assertEqual(first["rom_offset"], "0x068394")
        self.assertEqual(first["raw_hex"], "0019029001d00000")
        self.assertEqual(first["map_id"], 0x0019)
        self.assertEqual((first["x"], first["y"]), (0x0290, 0x01D0))
        self.assertEqual(first["selector"], "0x0000")
        self.assertEqual(first["group"], "inn")

    @unittest.skipUnless(ASM.is_file(), f"disassembly oracle not present at {DISASM}")
    def test_oracle_round_trip(self):
        """The predicted table must be byte-identical and unique in the ROM."""
        expected, shops = read_shop_inventories_source()
        self.assertEqual(len(expected), 255)
        self.assertEqual(len(shops), 49)
        start = SHOP_INVENTORIES["start"]
        self.assertEqual(self.data[start:start + len(expected)], expected)
        self.assertEqual(self.data.count(expected), 1)
        self.assertEqual(
            [[i["id"] for i in shop["items"]] for shop in self.shops],
            shops,
        )

    @unittest.skipUnless(ASM.is_file(), f"disassembly oracle not present at {DISASM}")
    def test_item_symbols_match_the_disassembly_constants(self):
        ids = read_item_ids()
        self.assertEqual(len(ids), len(ITEM_SYMBOLS))
        for name, value in ids.items():
            self.assertEqual(ITEM_SYMBOLS[value - 1], name)

    def test_location_map_symbols_resolve(self):
        from psiv_tools.symbols import MAP_SYMBOLS
        self.assertEqual(len(MAP_SYMBOLS), 417)
        self.assertEqual(MAP_SYMBOLS[0x10], "Piata")
        for entry in self.result["locations"]["entries"]:
            self.assertIsNotNone(entry["map_symbol"], entry)

    @unittest.skipUnless(ASM.is_file(), f"disassembly oracle not present at {DISASM}")
    def test_map_symbols_match_the_disassembly_constants(self):
        """MAP_SYMBOLS[i] must equal the MapID_* constant with value i."""
        import re
        from psiv_tools.symbols import MAP_SYMBOLS
        pattern = re.compile(r"MapID_(\w+)\s*=\s*id\(PtrMap_\w+\)\s*;\s*(\S+)")
        constants = {}
        for line in (DISASM / "ps4.constants.asm").read_text(errors="replace").splitlines():
            m = pattern.match(line)
            if m:
                raw = m.group(2)
                value = int(raw.lstrip("$"), 16) if raw.startswith("$") else int(raw)
                constants[value] = m.group(1)
        self.assertEqual(len(constants), len(MAP_SYMBOLS))
        for value, name in constants.items():
            self.assertEqual(MAP_SYMBOLS[value], name)


if __name__ == "__main__":
    unittest.main()
