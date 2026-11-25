//! Deferred palette-copy effects in `MapDataManager` routines: decode bulk
//! CRAM-line copies and attach repainted NPC sprite sheets to the paths that
//! execute them.

use std::collections::{BTreeSet, HashMap};

use super::refuse;
use crate::PackError;
use crate::gfx;
use crate::json::Json;
use crate::maps::effects::anchors::routine_address;
use crate::maps::effects::{field_mut, int, items};
use crate::maps::records::MapRecord;
use crate::sprites::compose::SpriteCensus;
use crate::sprites::emit::{NpcMetadata, SheetRegistry};
use crate::sprites::field::{MAP_PALETTE_LINE_ORDER, StagedArt, object_sprite};
use crate::sprites::objects::FieldObjectRoutine;

const PALETTE_TABLE_BUFFER: usize = 0xFB00;
const CRAM_LINE_BYTES: usize = 32;
const PALETTE_COPY_SEARCH: usize = 0x80;

/// A decoded `lea src,a0 / lea dst,a1 / move.w #n,d7 / move.l (a0)+,(a1)+ / dbf`.
pub(super) struct PaletteCopy {
    pub(super) at: usize,
    pub(super) source: usize,
    pub(super) offset: usize,
    pub(super) words: usize,
}

/// `decode_palette_copy`: the bulk copy in the first bytes of `routine`, if any.
/// A copy that isn't one whole CRAM line on a line boundary is refused, because
/// everything claimed about what it affects rests on that.
pub(super) fn decode_palette_copy(
    rom: &[u8],
    routine: usize,
) -> Result<Option<PaletteCopy>, PackError> {
    const LENGTH: usize = 20;
    let end = (routine + PALETTE_COPY_SEARCH).min(rom.len());
    let found = rom.get(routine..end).and_then(|span| {
        span.windows(LENGTH).position(|w| {
            w[..2] == [0x41, 0xF9]
                && w[6..8] == [0x43, 0xF8]
                && w[10..12] == [0x3E, 0x3C]
                && w[14..20] == [0x22, 0xD8, 0x51, 0xCF, 0xFF, 0xFC]
        })
    });
    let Some(position) = found else {
        return Ok(None);
    };
    let at = routine + position;
    let word =
        |offset: usize| usize::from(u16::from_be_bytes([rom[at + offset], rom[at + offset + 1]]));
    let source = word(2) << 16 | word(4);
    let destination = word(8);
    let words = (word(12) + 1) * 2;
    if destination < PALETTE_TABLE_BUFFER
        || !(destination - PALETTE_TABLE_BUFFER).is_multiple_of(CRAM_LINE_BYTES)
        || words != 16
    {
        return refuse(format!(
            "the bulk copy at 0x{routine:06X} writes {words} words to 0xFFFF{destination:04X},              which is not one whole CRAM line of 0xfb00"
        ));
    }
    Ok(Some(PaletteCopy {
        at,
        source,
        offset: destination - PALETTE_TABLE_BUFFER,
        words,
    }))
}

/// `resolve_palette_effects`: attach each palette copy to the sprite sheets it
/// repaints. The effect hangs on the path, because two of the three copies sit
/// on paths whose only writes are `object_despawn`. An affected NPC is resolved
/// again under the post-copy palette and registered like any sheet. Returns the
/// counts: copies, alternate sheets, repainted NPCs.
#[allow(clippy::too_many_arguments)]
pub fn resolve_palette_effects(
    rom: &[u8],
    record: &MapRecord,
    art: &StagedArt,
    effects: &mut [Json],
    npcs: &[NpcMetadata],
    routines: &[FieldObjectRoutine],
    extents: &HashMap<u32, usize>,
    registry: &mut SheetRegistry,
    census: &mut SpriteCensus,
    map_palette: &[[u8; 3]],
    dispatch: usize,
) -> Result<[usize; 3], PackError> {
    let mut counts = [0; 3];
    for effect in effects {
        let entry = int(effect, "entry");
        let Some(Json::Array(paths)) = field_mut(effect, "paths") else {
            continue;
        };
        for path in paths {
            let copies = items(path, "deferred")
                .iter()
                .any(|note| note == &Json::from("postincrement bulk copy"));
            if !copies {
                continue;
            }
            let index = entry.unwrap_or(0) as usize;
            let routine = routine_address(rom, dispatch, index)?;
            let Some(copy) = decode_palette_copy(rom, routine)? else {
                continue;
            };
            counts[0] += 1;
            let raw = rom
                .get(copy.source..copy.source + CRAM_LINE_BYTES)
                .unwrap_or(&[]);
            let colors = gfx::decode_palette(raw)?;
            let rgb = gfx::palette_rgb(&colors);
            let line = copy.offset / CRAM_LINE_BYTES;
            let Some(position) = MAP_PALETTE_LINE_ORDER
                .iter()
                .position(|&l| usize::from(l) == line)
            else {
                return refuse(format!("{line} is not in a map palette blob"));
            };
            let mut after = map_palette.to_vec();
            after.splice(position * 16..(position + 1) * 16, rgb.iter().copied());
            let mut repainted = Vec::new();
            let mut alternates = BTreeSet::new();
            for (npc_index, (object, meta)) in record.objects.entries.iter().zip(npcs).enumerate() {
                let object = &object.record;
                let found = routines
                    .iter()
                    .find(|r| r.object_id() == usize::from(object.object_id));
                let Some(routine) = found else { continue };
                let Some(from) = &meta.sheet else { continue };
                if routine.palette_line().map(usize::from) != Some(line) {
                    continue;
                }
                let alternate = object_sprite(
                    rom,
                    routine,
                    extents,
                    art,
                    &after,
                    object.facing_dir,
                    object.art_tile,
                    Some(census),
                )?;
                let Some(sheet) = alternate.sheet else {
                    continue;
                };
                let symbol = object.symbol();
                let to = registry.register(sheet, symbol.unwrap_or("FieldObj"), false)?;
                if to == *from {
                    continue;
                }
                alternates.insert(to.clone());
                repainted.push(
                    Json::object()
                        .with("npc_index", npc_index)
                        .with("symbol", symbol)
                        .with("from", from.as_str())
                        .with("to", to),
                );
            }
            counts[2] += repainted.len();
            counts[1] += alternates.len();
            path.insert(
                "deferred_effects",
                vec![
                    Json::object()
                        .with("instruction", "postincrement bulk copy")
                        .with("resolved_as", "palette_write")
                        .with("at", format!("0x{:06X}", copy.at))
                        .with("source", format!("0x{:06X}", copy.source))
                        .with(
                            "destination",
                            format!("Palette_Table_Buffer+0x{:02X}", copy.offset),
                        )
                        .with("cram_line", line)
                        .with("words", copy.words)
                        .with("installs_chunk_definitions", false)
                        .with("colors", rgb.iter().map(|c| c.to_vec()).collect::<Vec<_>>())
                        .with(
                            "words_hex",
                            colors
                                .iter()
                                .map(|c| format!("0x{:04X}", c.raw))
                                .collect::<Vec<_>>(),
                        )
                        .with(
                            "affects",
                            Json::object()
                                .with("map_pixels", 0)
                                .with("map_render_identical", true)
                                .with(
                                    "reason",
                                    "chunk tiles reach CRAM lines 0 and 1 only; bit 14 is the \
                                     collision flag and never reaches the VDP",
                                )
                                .with("npc_sheets", repainted),
                        ),
                ],
            );
        }
    }
    Ok(counts)
}
