//! A walked record as the document `walk_map` returns in Python.

use super::*;

fn extend(mut base: Json, more: Json) -> Json {
    let Json::Object(pairs) = more else {
        panic!("extend with a non-object");
    };
    for (key, value) in pairs {
        base.insert(key, value);
    }
    base
}

fn art_list_json(list: &ArtList, data: &[u8], key: &str, sign_extend: bool) -> Json {
    let entries: Vec<Json> = list
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let value: Json = if sign_extend {
                format!("0x{:08X}", 0xFFFF0000u32 | u32::from(entry.word)).into()
            } else {
                entry.word.into()
            };
            Json::object()
                .with("index", index)
                .with("rom_offset", hex6(entry.span.start))
                .with(key, value)
                .with("source_offset", hex6(entry.source))
                .with("compression", "kosinski")
                .with("raw_hex", hex(&data[entry.span.start..entry.span.end]))
        })
        .collect();
    list.span
        .json(data)
        .with("terminator", format!("0x{:04X}", list.terminator))
        .with("entry_count", entries.len())
        .with("entries", entries)
}

fn scroll_plane_json(base: Json, plane: &str, scroll: &ScrollPlane) -> Json {
    let counters = scroll.step_counters.map(|(x, y)| {
        Json::object()
            .with("x", format!("0x{x:08X}"))
            .with("y", format!("0x{y:08X}"))
    });
    base.with(format!("{plane}_scroll_mode"), scroll.word & 0xFF)
        .with(
            format!("{plane}_scroll_word"),
            format!("0x{:04X}", scroll.word),
        )
        .with(format!("{plane}_step_counters"), counters)
}

fn index_list_json(list: &IndexList, data: &[u8]) -> Json {
    let terminator = if list.width == 1 {
        format!("0x{BYTE_TERMINATOR:02X}")
    } else {
        format!("0x{WORD_TERMINATOR:04X}")
    };
    list.span
        .json(data)
        .with("terminator", terminator)
        .with("count", list.ids.len())
        .with("ids", list.ids.clone())
}

fn fixed_json<T: Record>(list: &Fixed<T>, data: &[u8]) -> Json {
    let entries: Vec<Json> = list
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            extend(
                Json::object()
                    .with("index", index)
                    .with("rom_offset", hex6(entry.rom_offset)),
                entry.record.json(),
            )
            .with(
                "raw_hex",
                hex(&data[entry.rom_offset..entry.rom_offset + list.record_size]),
            )
        })
        .collect();
    list.span
        .json(data)
        .with("terminator", format!("0x{:04X}", list.terminator))
        .with("record_size", list.record_size)
        .with("count", entries.len())
        .with("entries", entries)
}

impl MapRecord {
    /// The record as `_walk_map` returns it, plus the encounter binding.
    pub fn to_json(&self, data: &[u8]) -> Json {
        let index = i32::from(self.music_id) - MUSIC_ID_BASE;
        let music_symbol = usize::try_from(index)
            .ok()
            .and_then(|i| MUSIC_NAMES.get(i).copied());
        let music = Json::object()
            .with("id", self.music_id)
            .with("id_hex", format!("0x{:02X}", self.music_id))
            .with("symbol", music_symbol)
            .with("changes_music", self.music_id != 0);

        let sprites: Vec<Json> = self
            .sprites
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Json::object()
                    .with("index", index)
                    .with("rom_offset", hex6(entry.span.start))
                    .with("destination", if entry.to_ram { "ram" } else { "vram" })
                    .with("tile_number", entry.tile_number)
                    .with("source_offset", hex6(entry.source))
                    .with("compression", "nemesis")
                    .with("raw_hex", hex(&data[entry.span.start..entry.span.end]))
            })
            .collect();
        let sprites = self
            .sprites
            .span
            .json(data)
            .with("terminator", format!("0x{:04X}", self.sprites.terminator))
            .with("entry_count", sprites.len())
            .with("entries", sprites);

        let dimensions = self
            .dimensions
            .span
            .json(data)
            .with("fg_row_size", self.dimensions.fg_row_size)
            .with("fg_column_size", self.dimensions.fg_column_size)
            .with("bg_row_size", self.dimensions.bg_row_size)
            .with("bg_column_size", self.dimensions.bg_column_size);

        let scroll = self
            .scroll
            .span
            .json(data)
            .with("mode", self.scroll.mode)
            .with("padding_byte", format!("0x{:02X}", self.scroll.padding));
        let scroll = scroll_plane_json(scroll, "fg", &self.scroll.fg);
        let scroll = scroll_plane_json(scroll, "bg", &self.scroll.bg);

        let chunks = self
            .chunks
            .span
            .json(data)
            .with("terminator", format!("0x{WORD_TERMINATOR:04X}"))
            .with("pointer_count", self.chunks.pointers.len())
            .with("compression", "kosinski")
            .with(
                "pointers",
                self.chunks
                    .pointers
                    .iter()
                    .map(|p| hex6(*p))
                    .collect::<Vec<_>>(),
            );

        let layout = self.layout.span.json(data);
        let layout = match self.layout.planes {
            Some((fg, bg)) => layout
                .with("present", true)
                .with("fg_offset", hex6(fg))
                .with("bg_offset", hex6(bg))
                .with("compression", "kosinski"),
            None => layout.with("present", false).with(
                "reason",
                "loc_539E2/loc_53A04 test Field_Map_Index & $FFFE and take the world-map \
                 path, which reads no bytes from the record",
            ),
        };

        let dialogue = self
            .dialogue
            .span
            .json(data)
            .with("pointer", hex6(self.dialogue.pointer))
            .with("compression", "kosinski")
            .with("tree", self.dialogue.tree)
            .with("label", format!("DialogueTree{}", self.dialogue.tree));

        let pointer = self.palette.pointer;
        let mut lines = Json::object();
        for (slot, line) in PALETTE_LINES.iter().enumerate() {
            let chunk = pointer + slot * 32;
            lines.insert(
                line.to_string(),
                Json::object()
                    .with("rom_offset", hex6(chunk))
                    .with("raw_hex", hex(&data[chunk..chunk + 32])),
            );
        }
        let palette = self
            .palette
            .span
            .json(data)
            .with("pointer", hex6(pointer))
            .with("palette_size_bytes", PALETTE_SIZE)
            .with("lines_from_map", PALETTE_LINES.to_vec())
            .with("line_from_pal_init_3", PALETTE_LINE_FROM_PAL_INIT)
            .with("lines", lines);

        let teleport = self.flags.dungeon_teleport;
        let flags = self
            .flags
            .span
            .json(data)
            .with("poison", self.flags.poison)
            .with("random_battles", self.flags.random_battles)
            .with("town_teleport", self.flags.town_teleport)
            .with(
                "dungeon_teleport_index",
                (teleport & 0x80 == 0).then_some(teleport),
            )
            .with("dungeon_teleport_raw", format!("0x{teleport:02X}"));

        map_reference(self.id)
            .with("is_null", false)
            .with(
                "pointer_offset",
                hex6(FIELD_MAP_PTRS + self.id * POINTER_SIZE),
            )
            .with("pointer", hex6(self.pointer))
            .with("rom_offset", hex6(self.pointer))
            .with("rom_end_exclusive", hex6(self.end))
            .with("size_bytes", self.end - self.pointer)
            .with("general_var", self.general_var)
            .with("music", music)
            .with(
                "tilesets",
                art_list_json(&self.tilesets, data, "vram_tile", false),
            )
            .with("sprites", sprites)
            .with(
                "sprite_data",
                art_list_json(&self.sprite_data, data, "ram_address", true),
            )
            .with("dimensions", dimensions)
            .with("scroll", scroll)
            .with("chunks", chunks)
            .with("map_updates", index_list_json(&self.map_updates, data))
            .with("transitions", fixed_json(&self.transitions, data))
            .with("transitions_2", fixed_json(&self.transitions_2, data))
            .with("objects", fixed_json(&self.objects, data))
            .with("treasure_chests", fixed_json(&self.treasure_chests, data))
            .with("tile_animations", fixed_json(&self.tile_animations, data))
            .with("layout", layout)
            .with("dialogue", dialogue)
            .with(
                "interaction_areas",
                fixed_json(&self.interaction_areas, data),
            )
            .with("events", index_list_json(&self.events, data))
            .with("palette", palette)
            .with("flags", flags)
            .with(
                "map_data_manager",
                index_list_json(&self.map_data_manager, data),
            )
            .with("raw_hex", hex(&data[self.pointer..self.end]))
            .with("encounters", self.encounters.clone())
    }
}
