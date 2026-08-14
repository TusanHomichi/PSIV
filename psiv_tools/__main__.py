from __future__ import annotations

import argparse
import json
from pathlib import Path

from .core import RomError, extract_all, inspect_rom, read_rom, write_extract


def main() -> int:
    parser = argparse.ArgumentParser(prog="psiv-tools", description="Inspect and extract known structures from Phantasy Star IV (USA).")
    sub = parser.add_subparsers(dest="command", required=True)

    p_inspect = sub.add_parser("inspect", help="Verify and print ROM metadata")
    p_inspect.add_argument("rom", type=Path)

    p_extract = sub.add_parser("extract", help="Extract normalized JSON datasets")
    p_extract.add_argument("rom", type=Path)
    p_extract.add_argument("output", type=Path)

    p_dump = sub.add_parser("dump", help="Print the complete normalized extract to stdout")
    p_dump.add_argument("rom", type=Path)

    args = parser.parse_args()
    try:
        data = read_rom(args.rom)
        if args.command == "inspect":
            info = inspect_rom(data)
            print(json.dumps(info, indent=2))
        elif args.command == "extract":
            result = write_extract(data, args.output)
            print(f"Verified PSIV US retail ROM: {result['metadata']['hashes']['sha256']}")
            print(f"Extracted {len(result['characters'])} characters")
            print(f"Extracted {len(result['techniques'])} techniques")
            print(f"Extracted {len(result['skills'])} skills")
            print(f"Extracted {len(result['combos'])} combos")
            print(f"Extracted {len(result['vehicles'])} vehicles")
            print(f"Extracted {len(result['items'])} items/equipment records")
            print(f"Extracted {len(result['enemies'])} enemy records")
            print(f"Extracted {len(result['enemy_skills'])} enemy skill records")
            print(f"Extracted {result['progression']['total_level_records']} level-progression records")
            print(f"Wrote JSON to {args.output}")
        elif args.command == "dump":
            print(json.dumps(extract_all(data), indent=2))
    except (OSError, RomError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
