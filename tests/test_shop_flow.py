import json
import tempfile
import unittest
from pathlib import Path

from psiv_tools.core import read_rom
from psiv_tools.pack import PACK_FORMAT_VERSION, build_pack
from psiv_tools.shop_flow import (
    MAX_PARTY,
    PRICE_OFFSET,
    SIGNATURES,
    ShopFlowError,
    read_counter_scan,
    read_flow,
    read_inn_rates,
    read_inn_rules,
    read_presentation,
    read_prices,
    sell_price,
)
from psiv_tools.shop_pack import SHOPS_NAME, ShopPackError, build_shops
from psiv_tools.shops import extract_shops

ROM = Path(__file__).resolve().parents[1] / "Phantasy Star IV (USA).md"
REFERENCE = Path(__file__).resolve().parents[1] / "reference" / "ps4disasm"

#: Zema's two shops and its inn, Piata's item shop and inn, and Tonoe, whose
#: three rows are the dead ones. Enough to exercise every binding.
FIXTURE_MAPS = [0x19, 0x1B, 0x26, 0x27, 0x2A, 0x41]

#: The three location rows naming a position no object occupies.
DEAD_ROWS = [21, 22, 23]
#: The inventories only those rows reference.
UNREACHABLE = [9, 10, 11]
#: Retail counts: 68 rows, 49 inventories, 18 inns.
COUNTERS, INVENTORIES, INNS = 68, 49, 18


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheFlow(unittest.TestCase):
    """The six routines, decoded out of retail rather than transcribed."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls.flow = read_flow(cls.data)

    def test_every_signature_occurs_exactly_as_often_as_declared(self):
        for label, (signature, expected) in SIGNATURES.items():
            with self.subTest(routine=label):
                pattern = bytes.fromhex(signature)
                count = 0
                at = self.data.find(pattern)
                while at != -1:
                    count += 1
                    at = self.data.find(pattern, at + 1)
                self.assertEqual(count, expected)

    def test_a_rom_that_lost_a_signature_is_refused(self):
        broken = bytearray(self.data)
        site = self.data.find(bytes.fromhex(SIGNATURES["CounterScan"][0]))
        broken[site:site + 2] = b"\x4e\x71"
        with self.assertRaises(ShopFlowError):
            read_counter_scan(bytes(broken))

    # ------------------------------------------------------------ reaching one
    def test_the_counter_scan_describes_the_location_table(self):
        scan = self.flow["interaction"]
        locations = extract_shops(self.data)["locations"]
        self.assertEqual(scan["table"].lower(), locations["rom_offset"].lower())
        self.assertEqual(scan["entry_bytes"], locations["entry_size"])
        self.assertEqual(scan["terminator"], locations["terminator"])
        self.assertEqual(scan["selector_ram"], "0xFFFFECD0")

    def test_the_scan_matches_on_position_and_never_reads_collision(self):
        scan = self.flow["interaction"]
        self.assertEqual(scan["matches_on"], "position")
        self.assertEqual(
            [(field["offset"], field["compared_against"]) for field in scan["fields"]],
            [("0x00", "d2"), ("0x02", "d0"), ("0x04", "d1"), ("0x06", None)],
        )

    # ------------------------------------------------------------------ prices
    def test_the_buy_price_is_the_record_word_spent_unchanged(self):
        prices = self.flow["prices"]
        self.assertEqual(prices["price_offset"], f"0x{PRICE_OFFSET:02X}")
        self.assertEqual(prices["price_bytes"], 2)
        self.assertEqual(prices["encoding"], "binary")

    def test_the_sell_price_is_half_rounding_down(self):
        sell = self.flow["prices"]["sell"]
        self.assertEqual(sell["divisor"], 2)
        self.assertEqual(sell["arithmetic"], "lsr.w #1")
        # `lsr` truncates, so an odd price loses the remainder.
        self.assertEqual(sell_price(11, 2), 5)
        self.assertEqual(sell_price(10, 2), 5)
        self.assertEqual(sell_price(0, 2), 0)

    def test_stock_is_unlimited(self):
        self.assertTrue(self.flow["prices"]["stock"]["unlimited"])

    def test_a_rom_whose_sell_path_does_not_shift_is_refused(self):
        broken = bytearray(self.data)
        site = self.data.find(bytes.fromhex(SIGNATURES["SellPrice"][0]))
        broken[site + 4:site + 6] = b"\x4e\x71"  # nop instead of lsr.w #1
        with self.assertRaises(ShopFlowError):
            read_prices(bytes(broken))

    # -------------------------------------------------------------------- inns
    def test_the_bill_is_the_rate_times_the_party(self):
        inn = self.flow["inn"]
        self.assertEqual(inn["cost"]["formula"], "rate * party_slots")
        self.assertEqual(inn["cost"]["max_party"], MAX_PARTY)
        self.assertEqual(inn["entry_bytes"], 2)
        self.assertEqual(inn["rate_offset"], "0x01")

    def test_a_night_restores_hp_tp_status_and_every_skill_use(self):
        restores = self.flow["inn"]["restores"]["per_character"]
        fields = [write["field"] for write in restores]
        self.assertEqual(fields[:3], ["curr_hp", "curr_tp", "status"])
        self.assertEqual(
            fields[3:], [f"curr_skill_uses[{slot}]" for slot in range(8)]
        )
        self.assertEqual(restores[0]["from"], "max_hp")
        self.assertEqual(restores[1]["from"], "max_tp")
        # The status clear is an immediate, not a copy.
        self.assertEqual((restores[2]["from"], restores[2]["value"]), (None, 0))

    def test_the_rate_table_is_the_inn_index_space(self):
        rates = read_inn_rates(self.data, int(self.flow["inn"]["table"], 16), INNS)
        self.assertEqual([rate["index"] for rate in rates], list(range(INNS)))
        self.assertEqual([rate["rate"] for rate in rates],
                         [5, 10, 20, 15, 15, 25, 50, 40, 45, 50,
                          80, 90, 100, 110, 120, 130, 140, 150])

    def test_one_inn_runs_an_event_instead_of_a_night(self):
        self.assertEqual(self.flow["inn"]["scripted_rest"]["selector"], "0x0006")

    def test_a_rom_whose_inn_table_moved_is_refused(self):
        broken = bytearray(self.data)
        site = self.data.find(bytes.fromhex(SIGNATURES["InnRates"][0]))
        broken[site + 10:site + 12] = b"\x4e\x71"  # drop the `add.w d0, d0`
        with self.assertRaises(ShopFlowError):
            read_inn_rules(bytes(broken))

    # ------------------------------------------------------------ who is there
    def test_the_portrait_groups_are_bounded_by_the_rate_table(self):
        groups = read_presentation(self.data)["portraits"]["groups"]
        self.assertEqual([group["count"] for group in groups], [INNS, INVENTORIES])
        art = {entry for group in groups for entry in group["art"]}
        # Seven shopkeepers and the Aiedo baker.
        self.assertEqual(len(art), 8)

    def test_the_greeting_selector_is_one_word_per_shop(self):
        greetings = self.flow["presentation"]["greetings"]
        self.assertEqual(greetings["entry_bytes"], 2)
        self.assertEqual(greetings["table"], "0x068136")


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
class TestTheSection(unittest.TestCase):
    """`shops.json`, built over a filtered pack."""

    @classmethod
    def setUpClass(cls):
        cls.data = read_rom(ROM)
        cls._temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls._temp.name) / "pack"
        cls.manifest = build_pack(cls.data, cls.root, map_ids=FIXTURE_MAPS)
        cls.payload = json.loads((cls.root / SHOPS_NAME).read_text())
        cls.counters = {counter["id"]: counter for counter in cls.payload["counters"]}

    @classmethod
    def tearDownClass(cls):
        cls._temp.cleanup()

    def test_the_counts_are_what_the_cartridge_holds(self):
        self.assertEqual(self.payload["counter_count"], COUNTERS)
        self.assertEqual(self.payload["inventory_count"], INVENTORIES)
        self.assertEqual(self.payload["inn_count"], INNS)
        self.assertEqual(len(self.payload["counters"]), COUNTERS)
        self.assertEqual(len(self.payload["inventories"]), INVENTORIES)
        self.assertEqual(len(self.payload["inns"]), INNS)

    def test_the_file_is_versioned_and_named(self):
        self.assertEqual(self.payload["kind"], "shops")
        self.assertEqual(self.payload["format_version"], PACK_FORMAT_VERSION)

    def test_the_manifest_fragment(self):
        shops = self.manifest["shops"]
        self.assertEqual(shops["file"], SHOPS_NAME)
        self.assertEqual(shops["counters"], COUNTERS)
        self.assertEqual(shops["inventories"], INVENTORIES)
        self.assertEqual(shops["inns"], INNS)
        self.assertEqual(shops["prices"]["sell_divisor"], 2)
        self.assertTrue(shops["prices"]["unlimited_stock"])

    def test_the_three_dead_rows_and_what_they_strand(self):
        census = self.payload["census"]
        self.assertEqual([entry["id"] for entry in census["dead_counters"]], DEAD_ROWS)
        for row in DEAD_ROWS:
            counter = self.counters[row]
            with self.subTest(counter=row):
                self.assertFalse(counter["live"])
                self.assertEqual((counter["x_pixels"], counter["y_pixels"]), (0, 0))
                self.assertEqual(counter["map_symbol"], "Tonoe")
        self.assertEqual(census["unreachable_inventories"], UNREACHABLE)
        for index in UNREACHABLE:
            with self.subTest(inventory=index):
                inventory = self.payload["inventories"][index]
                self.assertFalse(inventory["reachable"])
                self.assertEqual(inventory["bound_by"], [])

    def test_a_packed_counter_names_the_object_standing_there(self):
        for counter in self.payload["counters"]:
            if counter["map_id"] not in FIXTURE_MAPS or not counter["live"]:
                continue
            with self.subTest(counter=counter["id"]):
                keeper = counter["shopkeeper"]
                self.assertIsNotNone(keeper)
                # Same cell, which is the join being the same convention.
                self.assertEqual(keeper["x_cell"], counter["x_cell"])
                self.assertEqual(keeper["y_cell"], counter["y_cell"])

    def test_the_counter_tile_is_not_what_selects_the_shop(self):
        # If the shop were keyed on a collision type, every counter would sit
        # on one. Thirteen sit on ordinary floor.
        spread = self.payload["census"]["counter_collision"]
        self.assertGreater(spread.get("0", 0), 0)
        self.assertGreater(spread.get("12", 0), 0)
        self.assertEqual(
            sum(spread.values()),
            sum(1 for c in self.payload["counters"] if c["collision"] is not None),
        )

    def test_every_sold_item_carries_both_prices(self):
        for inventory in self.payload["inventories"]:
            for item in inventory["items"]:
                with self.subTest(inventory=inventory["index"], item=item["item_id"]):
                    self.assertGreater(item["buy_price"], 0)
                    self.assertEqual(item["sell_price"], item["buy_price"] // 2)
                    self.assertIsNotNone(item["display_name"])

    def test_every_inn_prices_a_full_party(self):
        for inn in self.payload["inns"]:
            with self.subTest(inn=inn["index"]):
                costs = inn["cost_by_party_size"]
                self.assertEqual(sorted(costs), [str(n) for n in range(1, MAX_PARTY + 1)])
                for size in range(1, MAX_PARTY + 1):
                    self.assertEqual(costs[str(size)],
                                     inn["rate_per_character"] * size)
                self.assertTrue(inn["bound_by"])

    def test_zemas_shops_are_the_ones_behind_the_doors(self):
        zema = [c for c in self.payload["counters"] if c["map_symbol"].startswith("Zema")]
        self.assertEqual(
            sorted((c["map_symbol"], c["kind"]) for c in zema),
            [("ZemaInn", "inn"), ("ZemaItemShop", "shop"),
             ("ZemaWeaponShop", "shop"), ("ZemaWeaponShop", "shop")],
        )
        inn = next(c for c in zema if c["kind"] == "inn")
        self.assertEqual(
            self.payload["inns"][inn["inn_index"]]["rate_per_character"], 20
        )

    def test_a_filtered_build_does_not_claim_the_binding(self):
        # Most counters are on maps this build skipped, so `complete` has to be
        # false or the join check would fire on maps that were never emitted.
        census = self.payload["census"]
        self.assertLess(census["counters_bound_to_an_object"], census["live_counters"])
        with self.assertRaises(ShopPackError):
            build_shops(self.data, {}, complete=True)

    def test_emitting_twice_produces_identical_bytes(self):
        with tempfile.TemporaryDirectory() as other:
            second = Path(other) / "pack"
            build_pack(self.data, second, map_ids=FIXTURE_MAPS)
            self.assertEqual((self.root / SHOPS_NAME).read_bytes(),
                             (second / SHOPS_NAME).read_bytes())

    def test_the_file_is_sorted_and_newline_terminated(self):
        text = (self.root / SHOPS_NAME).read_text()
        self.assertTrue(text.endswith("\n"))
        self.assertEqual(
            text, json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n"
        )


@unittest.skipUnless(ROM.exists(), f"ROM fixture not present at {ROM}")
@unittest.skipUnless(
    REFERENCE.exists(),
    f"Disassembly clone not present at {REFERENCE}; "
    "git clone --depth 1 https://github.com/alechenninger/ps4disasm reference/ps4disasm",
)
class DisassemblyOracleTest(unittest.TestCase):
    """The labels behind the addresses retail already proved."""

    @classmethod
    def setUpClass(cls):
        cls.source = (REFERENCE / "ps4.asm").read_text(errors="replace")

    def test_the_routines_exist_under_the_names_used_here(self):
        for label in ("Win_ShopBuySell:", "Win_ShopBuyList:", "Win_ShopBuyConfirm:",
                      "Win_ShopSellConfirm:", "Win_ShopMessage:", "Win_RestConfirm:",
                      "RecoverStats:", "DoCharRecovery:", "ShopInventories:",
                      "ShopPortraitGroupsPtrs:", "FieldRoutine_Shop:"):
            with self.subTest(label=label):
                self.assertIn(label, self.source)

    def test_the_sell_price_halving_is_in_the_source(self):
        self.assertIn("move.w\t$14(a0,d0.w), d0\n\tlsr.w\t#1, d0", self.source)

    def test_the_shop_is_reached_through_the_object_check(self):
        # The caller passes an object's position, which is the whole basis for
        # the shopkeeper binding.
        self.assertIn("jsr\t(loc_65D12).l", self.source)
        self.assertIn("Interaction_ChkShop:", self.source)


if __name__ == "__main__":
    unittest.main()
