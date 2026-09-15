"""LoadTreasureChests art and the two lid mappings, from the US cartridge."""
from .nemesis import decompress
from .sprites.compose import TileSource
from .sprites.field import _sheet_for_sequences, _table_sequences, sprite_palette


def chest_sprite(rom, map_palette, *, white=False, general_var=0):
    # LoadTreasureChests stages normal art at $4DC and plant/white art at
    # $4E6. The $32 map family selects a second normal chest and CRAM line 1.
    special = general_var == 0x32 and not white
    offset = 0x297674 if white else (0x2976FA if special else 0x2975AE)
    tile = 0x4E6 if white else 0x4DC
    table = 0x51442 if white else 0x5143A
    line = 1 if special else 2
    source = TileSource()
    data, _ = decompress(rom, offset)
    source.add(tile, data)
    return _sheet_for_sequences(
        rom, _table_sequences(rom, table, 2), source,
        art_tile=tile, tile_props=line << 5, streamed=False,
        palette=sprite_palette(rom, map_palette, line), palette_line=line,
        required=("down", "up"),
    )


def bind_chest_sprites(rom, chests, map_palette, general_var, registry):
    """Attach original-art references to the already converted chest records."""
    for chest in chests:
        sheet = chest_sprite(rom, map_palette, white=chest["white_chest"],
                             general_var=general_var)
        name = registry.register(sheet, chest["object_symbol"] or "TreasureChest")
        chest["sprite"] = {
            "sheets": "sprites/npcs.json", "sheet": name, "facing": "down",
            "idle_sequence": "idle_down", "walk_sequence": "walk_down",
        }
