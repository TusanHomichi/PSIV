#!/usr/bin/env python3
"""Check tape-addressed Zema pages, entrance and restored resident palette.

This proves captured presentation state, not whole-scene retail pixel parity.
The palette copy is MapDataMan_ChkZemaNormal at US $051E56.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image

# Direct script execution puts tools/native, rather than the repository, on sys.path.
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_verifier import verify as rust_verify


def verify(native: dict, pack: Path) -> dict:
    result = rust_verify({"operation": "zema", "native": native,
                          "map": json.loads((pack / "maps/024_Zema.json").read_text())})
    # Pillow owns decoding and viewport dimensions, not recorded game state.
    for capture in result.pop("captures"):
        with Image.open(capture["path"]) as picture:
            if picture.size != (1280, 800):
                raise ValueError(f"{capture['label']}: expected a 1280x800 rendered capture")
            picture.load()
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="native-tape output directory")
    parser.add_argument("--pack", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = verify(json.loads((args.directory / "native.json").read_text()), args.pack)
    except (OSError, ValueError, KeyError, TypeError) as error:
        result = {"pass": False, "error": str(error)}
    output = args.directory / "zema-presentation.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Zema pages, entrance and resident palette: {'PASS' if result['pass'] else 'FAIL'} ({output})")
    return 0 if result["pass"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
