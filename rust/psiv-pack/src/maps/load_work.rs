//! Decompressor calls selected by a parsed US field-map record, in loader
//! order. This is only intrinsic decompressor CPU work, not map-load timing.
//!
//! Retail `GameMode_LoadFieldMap` calls `loc_519D2`, `loc_51A1A`,
//! `loc_51A5C`, `loc_51A7A`, `Map_LoadChunks`, `LoadTreasureChests`,
//! `loc_539E2` / `loc_53A04`, and `Map_DialogueTreesToRAM` in that order
//! (`ps4.asm:107505-107635`, `:107660-107815`, `:110966-110983`,
//! `:111200-111225`, `:111677-111694`). Their US call targets and the
//! compressed dialogue branch were checked against the retail ROM.
//!
//! Map redirects must already have selected the record supplied here. Extra
//! decompression selected by MapDataManager, objects or animations is outside
//! this component, as are caller instructions, scans, transfers, interrupts,
//! refresh and VDP/bus stalls. World-map plane loading copies raw pages and
//! is explicitly excluded; it is not a pair of Kosinski streams.

use super::records::MapRecord;
use crate::PackError;
use crate::decode::{kosinski, nemesis};
use crate::sprites::field::chest_art_source;

const CHUNK_TABLE: u32 = 0xFFFF6000;
const VEHICLE_POINTERS: usize = 0x051AA6;

/// A dispatch site, not a whole-load phase or an elapsed-frame budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Tilesets,
    Sprites,
    SpriteData,
    Vehicle,
    Chunks,
    ChestArt,
    ForegroundLayout,
    BackgroundLayout,
    Dialogue,
}

/// The decompressor's destination: a logical 32-bit 68000 RAM address or
/// the 16-bit VRAM byte offset selected before the VDP entry is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Ram(u32),
    Vram(u16),
}

/// Statistics from the existing decoders' single parsing paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlgorithmWork {
    Kosinski(kosinski::Work),
    Nemesis(nemesis::Work),
}

/// One actual decompressor call. Repeated sources remain repeated calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamWork {
    pub stage: Stage,
    pub source: usize,
    pub destination: Destination,
    /// `loc_519D2` decompresses into RAM first, then transfers to this VRAM
    /// offset. The transfer itself is not in the intrinsic CPU count.
    pub transfer_to_vram: Option<u16>,
    pub decoded_bytes: usize,
    pub consumed_bytes: usize,
    pub work: AlgorithmWork,
}

impl StreamWork {
    /// Intrinsic MC68000 clock cycles, not master cycles or elapsed frames.
    /// None preserves the Nemesis decoder's unsupported zero-row timing.
    #[must_use]
    pub fn cpu_cycles(&self) -> Option<u64> {
        match self.work {
            AlgorithmWork::Kosinski(work) => Some(work.cpu_cycles()),
            AlgorithmWork::Nemesis(work) => work.cpu_cycles(match self.destination {
                Destination::Ram(_) => nemesis::Destination::Ram,
                Destination::Vram(_) => nemesis::Destination::Vdp,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordDecompressionWork {
    pub streams: Vec<StreamWork>,
    /// Maps 0 and 1 use camera-selected raw pages instead of record planes.
    pub world_layout_pages_excluded: bool,
}

impl RecordDecompressionWork {
    /// Sum only the enumerated decompressor calls. One unsupported stream
    /// makes the whole sum unsupported rather than disappearing from it.
    #[must_use]
    pub fn cpu_cycles(&self) -> Option<u64> {
        self.streams.iter().try_fold(0_u64, |total, stream| {
            total.checked_add(stream.cpu_cycles()?)
        })
    }
}

fn context(
    record: &MapRecord,
    stage: Stage,
    source: usize,
    error: impl std::fmt::Display,
) -> PackError {
    PackError(format!(
        "map 0x{:03X} {stage:?} stream at 0x{source:06X}: {error}",
        record.id
    ))
}

impl RecordDecompressionWork {
    fn kosinski(
        &mut self,
        rom: &[u8],
        record: &MapRecord,
        stage: Stage,
        source: usize,
        destination: u32,
        transfer_to_vram: Option<u16>,
    ) -> Result<usize, PackError> {
        let (bytes, consumed_bytes, work) = kosinski::decompress_with_work(rom, source)
            .map_err(|error| context(record, stage, source, error))?;
        let decoded_bytes = bytes.len();
        self.streams.push(StreamWork {
            stage,
            source,
            destination: Destination::Ram(destination),
            transfer_to_vram,
            decoded_bytes,
            consumed_bytes,
            work: AlgorithmWork::Kosinski(work),
        });
        Ok(decoded_bytes)
    }

    fn nemesis(
        &mut self,
        rom: &[u8],
        record: &MapRecord,
        stage: Stage,
        source: usize,
        destination: Destination,
    ) -> Result<(), PackError> {
        let (bytes, consumed_bytes, work) = nemesis::decompress_with_work(rom, source)
            .map_err(|error| context(record, stage, source, error))?;
        self.streams.push(StreamWork {
            stage,
            source,
            destination,
            transfer_to_vram: None,
            decoded_bytes: bytes.len(),
            consumed_bytes,
            work: AlgorithmWork::Nemesis(work),
        });
        Ok(())
    }
}

/// Compose record-selected decompression work for the retail loader. Vehicle
/// selector zero skips art, 1..=3 indexes `loc_51AA6`; other selectors are
/// refused. Tile-to-byte shifts preserve the retail word-width wrap.
pub fn record_decompression_work(
    rom: &[u8],
    record: &MapRecord,
    retained_vehicle: u16,
) -> Result<RecordDecompressionWork, PackError> {
    if retained_vehicle > 3 {
        return Err(PackError(format!(
            "invalid retained vehicle selector {retained_vehicle}"
        )));
    }
    let world = record.id & 0xFFFE == 0;
    if world != record.layout.planes.is_none() {
        return Err(PackError(format!(
            "map 0x{:03X} has inconsistent record layout pointers",
            record.id
        )));
    }
    let mut result = RecordDecompressionWork {
        streams: Vec::new(),
        world_layout_pages_excluded: world,
    };
    for entry in &record.tilesets.entries {
        result.kosinski(
            rom,
            record,
            Stage::Tilesets,
            entry.source,
            CHUNK_TABLE,
            Some(entry.word.wrapping_shl(5)),
        )?;
    }
    for entry in &record.sprites.entries {
        let offset = entry.tile_number.wrapping_shl(5);
        let destination = if entry.to_ram {
            Destination::Ram(0xFFFF0000 | u32::from(offset))
        } else {
            Destination::Vram(offset)
        };
        result.nemesis(rom, record, Stage::Sprites, entry.source, destination)?;
    }
    for entry in &record.sprite_data.entries {
        result.kosinski(
            rom,
            record,
            Stage::SpriteData,
            entry.source,
            0xFFFF0000 | u32::from(entry.word),
            None,
        )?;
    }
    if retained_vehicle != 0 {
        let at = VEHICLE_POINTERS + usize::from(retained_vehicle - 1) * 4;
        let pointer = rom.get(at..at + 4).ok_or_else(|| {
            context(
                record,
                Stage::Vehicle,
                at,
                "vehicle pointer table is truncated",
            )
        })?;
        let source = u32::from_be_bytes(pointer.try_into().expect("four-byte slice")) as usize;
        result.nemesis(
            rom,
            record,
            Stage::Vehicle,
            source,
            Destination::Ram(0xFFFF0000),
        )?;
    }
    let mut chunk_destination = CHUNK_TABLE;
    for &source in &record.chunks.pointers {
        let bytes = result.kosinski(rom, record, Stage::Chunks, source, chunk_destination, None)?;
        // a1 advances across calls, unlike the tileset staging buffer.
        chunk_destination = chunk_destination.wrapping_add(bytes as u32);
    }
    for (white, tile) in [(false, 0x4DC_u16), (true, 0x4E6_u16)] {
        result.nemesis(
            rom,
            record,
            Stage::ChestArt,
            chest_art_source(white, record.general_var),
            Destination::Vram(tile.wrapping_shl(5)),
        )?;
    }
    if let Some((foreground, background)) = record.layout.planes {
        result.kosinski(
            rom,
            record,
            Stage::ForegroundLayout,
            foreground,
            0xFFFFA000,
            None,
        )?;
        result.kosinski(
            rom,
            record,
            Stage::BackgroundLayout,
            background,
            0xFFFFB000,
            None,
        )?;
    }
    result.kosinski(
        rom,
        record,
        Stage::Dialogue,
        record.dialogue.pointer,
        0xFFFF3000,
        None,
    )?;
    Ok(result)
}
