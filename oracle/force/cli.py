"""The command line: the arguments, and the process exit status.

`--formation` and `--out` are required; everything else has a default that the
ledger's captures were taken with. The exit status is the tool's own verdict:
`0` when every check passed, `1` when a required ability never fired or
`oracle/rng_trace.py check` failed on the capture, `2` when the request itself
was refused (`ForceError`) or the parser rejected it.
"""
from __future__ import annotations

import argparse
import sys

from .errors import ForceError
from .phases import run
from .runs import ORACLE, ROOT

DEFAULT_TAPE = ORACLE / "tapes" / "07_first_battle.tape"
DEFAULT_DATA_DIR = ROOT / "generated"
DEFAULT_RAM_MAP = ORACLE / "ram_map.json"


def parser() -> argparse.ArgumentParser:
    parsed = argparse.ArgumentParser(
        description="Force a formation into a battle and capture it.")
    parsed.add_argument("--formation", required=True,
                        help="formation id, decimal or 0x-prefixed")
    parsed.add_argument("--out", required=True, help="output directory")
    parsed.add_argument("--policy", default="attack",
                        choices=("attack", "defend"))
    parsed.add_argument("--delay", type=int, default=0,
                        help="idle frames between the field prefix and the "
                             "policy; shifts every roll in the fight")
    parsed.add_argument("--repeats", type=int, default=900,
                        help="policy blocks (attack: one 16-frame press)")
    parsed.add_argument("--vehicle", type=int, default=None,
                        help="which VehicleData record fights the forced "
                             "battle: 1 Land Rover, 2 Ice Digger, "
                             "3 Hydrofoil. Only a formation in one of the "
                             "four vehicle tables (8, 9, 10, 13) has one, and "
                             "the table's own region decides which values it "
                             "can seat")
    parsed.add_argument("--base-tape", default=str(DEFAULT_TAPE))
    parsed.add_argument("--data-dir", default=str(DEFAULT_DATA_DIR),
                        help="the generated pack directory")
    parsed.add_argument("--ram-map", default=str(DEFAULT_RAM_MAP))
    parsed.add_argument("--require-ability", action="append", default=[],
                        type=lambda v: int(v, 0),
                        help="fail unless this enemy ability id was used")
    parsed.add_argument("--scout", default="", help="scout cache path")
    parsed.add_argument("--refresh-scout", action="store_true")
    parsed.add_argument("--dry-run", action="store_true",
                        help="write the tape and the selector patches, then "
                             "stop (needs scout data, starts no oracle run)")
    return parsed


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        return run(args)
    except ForceError as error:
        print(f"force_battle: {error}", file=sys.stderr)
        return 2
