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

    p_art = sub.add_parser("art", help="Decode ROM artwork to PNG sheets (writes Sega pixels; keep the output gitignored)")
    p_art.add_argument("rom", type=Path)
    p_art.add_argument("output", type=Path)

    p_planes = sub.add_parser("planes", help="Compose Enigma plane mappings into finished PNGs (writes Sega pixels; keep the output gitignored)")
    p_planes.add_argument("rom", type=Path)
    p_planes.add_argument("output", type=Path)

    p_pack = sub.add_parser("pack", help="Emit the runtime pack for psiv-data (writes Sega pixels; keep the output gitignored)")
    p_pack.add_argument("rom", type=Path)
    p_pack.add_argument("output", type=Path)
    p_pack.add_argument("--map", dest="map_ids", type=lambda v: int(v, 0), action="append",
                        help="restrict the pack to these map ids (repeatable)")

    p_sound = sub.add_parser(
        "sound",
        help="Extract raw music, SFX, voices, envelopes and DAC data (writes the gitignored sound pack)",
    )
    p_sound.add_argument("rom", type=Path)
    p_sound.add_argument("output", type=Path)

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
            print(f"Extracted {result['formations']['total_formations']} battle formations")
            print(f"Extracted {result['formations']['total_boss_formations']} boss formations")
            print(f"Extracted {result['formation_indexes']['group_count']} encounter formation-index groups")
            print(f"Extracted {result['shops']['shop_count']} shop inventories, {result['shops']['locations']['entry_count']} shop locations")
            print(f"Extracted graphics metadata: {result['graphics']['total_tiles_decoded']} tiles across named art, portraits, and battle backgrounds")
            print(f"Extracted {len(result['names']['tables'])} name tables and {result['dialogue']['total_entries']} dialogue entries in {len(result['dialogue']['trees'])} trees")
            print(f"Extracted {result['maps']['real_map_count']} map records ({result['maps']['null_map_count']} null) with encounter binding")
            print(f"Extracted {result['planes']['distinct_mappings']} Enigma plane mappings ({result['planes']['total_cells_decoded']} cells)")
            print(f"Wrote JSON to {args.output}")
        elif args.command == "dump":
            print(json.dumps(extract_all(data), indent=2))
        elif args.command == "art":
            from .gfx import export_art_pngs
            written = export_art_pngs(data, args.output)
            print(f"Wrote {len(written)} PNG sheets to {args.output}")
        elif args.command == "planes":
            from .planes import export_plane_pngs
            written = export_plane_pngs(data, args.output)
            print(f"Wrote {len(written)} composed PNGs to {args.output}")
        elif args.command == "pack":
            from .pack import PackError, build_pack
            try:
                manifest = build_pack(data, args.output, map_ids=args.map_ids)
            except PackError as exc:
                parser.error(str(exc))
            print(f"Packed {manifest['map_count']} maps to {args.output}")
            print(
                f"Dialogue: {manifest['dialogue']['trees']['tree_count']} trees emitted"
            )
            battle = manifest["battle"]["files"]
            print(
                f"Battle: {battle['enemies']['count']} enemies, "
                f"{battle['formations']['count']}+{battle['formations']['boss_count']} "
                f"formations, {battle['levels']['records']} level records, "
                f"{sum(battle['abilities'][k] for k in ('techniques','skills','enemy_skills','item_effects'))} "
                "ability records"
            )
            print(
                f"Party: {battle['characters']['count']} character records, "
                f"{battle['equipment']['count']} inventory records "
                f"({battle['equipment']['equippable']} equippable)"
            )
            shops = manifest["shops"]
            print(
                f"Shops: {shops['counters']} counters, "
                f"{shops['inventories']} inventories, {shops['inns']} inns"
            )
            print(f"Skipped {len(manifest['skipped'])} entries; {manifest['warps']['count']} warps")
            doors = manifest["warps"]["doors_without_map_change_cell"]
            if doors:
                print(f"{len(doors)} doorways have no map-change cell in the stored layout")
        elif args.command == "sound":
            from .sound import SoundError, emit_sound
            try:
                result = emit_sound(data, args.output)
            except SoundError as exc:
                parser.error(str(exc))
            census = result
            print(
                f"Sound: {census['music']['records']} music records, "
                f"{census['sfx']['regular_records']} regular SFX records, "
                f"{census['sfx']['special_records']} special SFX records, "
                f"{census['track_count']} tracks"
            )
            print(
                f"Command vocabulary: {census['command_vocabulary_size']} observed "
                f"primary opcodes ({', '.join(census['command_opcodes_seen'])})"
            )
            print(f"Wrote raw sound pack to {args.output / 'sound'}")
    except (OSError, RomError) as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
