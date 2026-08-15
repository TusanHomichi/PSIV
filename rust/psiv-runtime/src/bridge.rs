//! Pack-to-engine conversion: bridge errors, map construction, and record helpers.

use std::fmt;

use crate::effects::EffectOutcome;
use psiv_core::{
    Cell, CollisionGrid, Direction, FieldMap, MapId, Npc, NpcId, WanderKind, WanderSet, Warp,
    WarpTrigger,
};
use psiv_data::{GameData, MapRecord, TransitionTable};

/// A defect found while converting a pack record into an engine map.
///
/// Bridge errors mean the pack and the engine disagree about what a map is —
/// a data bug or a schema drift, never a gameplay condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// A dimension or coordinate did not fit the engine's cell space.
    OutOfRange(String),
    /// The engine rejected the converted map.
    Rejected(String),
    /// A warp's facing byte is not one of the four the ROM defines.
    BadWarpFacing {
        /// The map being converted.
        map: u16,
        /// The warp's index in the record.
        warp: u32,
        /// The raw facing byte.
        byte: u8,
    },
    /// The requested map is not in the pack.
    NotPacked(u16),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::OutOfRange(what) => write!(f, "value out of engine range: {what}"),
            BridgeError::Rejected(why) => write!(f, "engine rejected converted map: {why}"),
            BridgeError::BadWarpFacing { map, warp, byte } => write!(
                f,
                "map {map:#05x} warp {warp}: facing byte {byte:#04x} is not one of 0/4/8/$C"
            ),
            BridgeError::NotPacked(id) => write!(f, "map {id:#05x} is not in the pack"),
        }
    }
}

impl std::error::Error for BridgeError {}

fn cell_u16(x: u32, y: u32, what: &str) -> Result<Cell, BridgeError> {
    let x = u16::try_from(x).map_err(|_| BridgeError::OutOfRange(format!("{what} x={x}")))?;
    let y = u16::try_from(y).map_err(|_| BridgeError::OutOfRange(format!("{what} y={y}")))?;
    Ok(Cell::new(x, y))
}

fn direction(d: psiv_data::Direction) -> Direction {
    match d {
        psiv_data::Direction::Up => Direction::Up,
        psiv_data::Direction::Down => Direction::Down,
        psiv_data::Direction::Left => Direction::Left,
        psiv_data::Direction::Right => Direction::Right,
    }
}

/// Converts one pack record into an engine [`FieldMap`].
///
/// Warps with no trigger area (`rect == None` — dead data the manifest
/// counts) are dropped. A warp facing byte outside the ROM's four values is a
/// hard error; an NPC facing byte outside them falls back to down, because
/// scenery objects reuse the byte for non-directional state and their facing
/// never affects collision.
pub fn field_map(record: &MapRecord) -> Result<FieldMap, BridgeError> {
    field_map_patched(record, None)
}

/// [`field_map`], with a map-effects outcome applied during construction —
/// the cartridge's own order: `MapDataManager` runs inside map load, so a
/// patched map never exists unpatched.
pub fn field_map_patched(
    record: &MapRecord,
    outcome: Option<&EffectOutcome>,
) -> Result<FieldMap, BridgeError> {
    if let Some(out) = outcome
        && !out.unknown_banks.is_empty()
    {
        return Err(BridgeError::Rejected(format!(
            "map {}: effect schema drift: {:?}",
            record.label(),
            out.unknown_banks
        )));
    }
    // The grid: the variant's collision when a layout_replace is active,
    // the record's otherwise; then any resolved layout_write cells.
    let (width_u32, height_u32, mut cells): (u32, u32, Vec<u8>) = match outcome
        .and_then(|o| o.variant)
    {
        Some(index) => {
            let variant = record.layout_variants.get(index).ok_or_else(|| {
                BridgeError::Rejected(format!("map {}: variant {index} missing", record.label()))
            })?;
            let flat: Vec<u8> = variant.collision.rows.iter().flatten().copied().collect();
            (
                variant.collision.width_cells,
                variant.collision.height_cells,
                flat,
            )
        }
        None => {
            let grid = &record.collision.grid;
            (
                grid.width(),
                grid.height(),
                grid.cells().iter().map(|c| c.code()).collect(),
            )
        }
    };
    if let Some(out) = outcome {
        for &(x, y, collision) in &out.cell_patches {
            if x < width_u32 && y < height_u32 {
                cells[(y * width_u32 + x) as usize] = collision;
            } else {
                return Err(BridgeError::OutOfRange(format!(
                    "map {}: layout_write cell ({x},{y})",
                    record.label()
                )));
            }
        }
    }
    let width = u16::try_from(width_u32)
        .map_err(|_| BridgeError::OutOfRange(format!("width {width_u32}")))?;
    let height = u16::try_from(height_u32)
        .map_err(|_| BridgeError::OutOfRange(format!("height {height_u32}")))?;
    let grid = CollisionGrid::new(width, height, cells)
        .map_err(|e| BridgeError::Rejected(e.to_string()))?;

    let mut warps = Vec::new();
    for warp in &record.warps {
        let Some(rect) = &warp.rect else { continue };
        let origin = cell_u16(rect.x, rect.y, "warp rect")?;
        let far = cell_u16(
            rect.x + rect.width.saturating_sub(1),
            rect.y + rect.height.saturating_sub(1),
            "warp rect end",
        )?;
        let source = psiv_core::CellRect::new(
            origin.x,
            origin.y,
            far.x - origin.x + 1,
            far.y - origin.y + 1,
        );
        let trigger = match warp.table {
            TransitionTable::Normal => WarpTrigger::NormalGround,
            TransitionTable::MapChange => WarpTrigger::MapChange,
        };
        let facing = warp
            .facing
            .direction()
            .map(direction)
            .ok_or(BridgeError::BadWarpFacing {
                map: record.id.0,
                warp: warp.index,
                byte: warp.facing.id,
            })?;
        warps.push(Warp {
            source,
            trigger,
            target_map: MapId(warp.target.id.0),
            target_cell: cell_u16(
                warp.destination.x_cell,
                warp.destination.y_cell,
                "warp destination",
            )?,
            facing,
        });
    }

    let mut npcs = Vec::new();
    for (index, npc) in record.npcs.iter().enumerate() {
        let cell = cell_u16(npc.x_cell, npc.y_cell, "npc")?;
        let facing = npc
            .facing
            .direction()
            .map(direction)
            .unwrap_or(Direction::Down);
        // 85 retail objects sit on half-cells; the sub-cell offset is what
        // lets the ±8px talk range reach them from both straddled cells.
        let offset =
            psiv_core::SubCellOffset::new((npc.x_pixels % 16) as u8, (npc.y_pixels % 16) as u8);
        // Effects apply at construction: a rewritten object carries its new
        // id from the first tick, a despawned one is born inactive.
        let object_id = outcome
            .and_then(|o| {
                o.rewrites
                    .iter()
                    .find(|(i, _)| *i == index)
                    .map(|(_, id)| *id)
            })
            .unwrap_or(npc.object_id);
        let active = outcome.is_none_or(|o| !o.despawns.contains(&index));
        npcs.push(
            Npc::with_offset(NpcId(object_id), cell, offset, facing)
                .with_interactable(npc.interactable)
                .with_active(active),
        );
    }

    // The overworlds are tori: the cartridge itself selects the paged/wrapping
    // path by `Field_Map_Index & $FFFE == 0`, so hard-coding ids 0 and 1 here
    // is fidelity, not shortcut. Layout patches are event-flag-gated and no
    // flags exist at a fresh spawn, so none apply yet; when flag state lands,
    // the bridge applies active patches to the grid and rebuilds the map (see
    // FieldMap's contract docs).
    let topology = match record.id.0 {
        0 | 1 => psiv_core::Topology::Torus,
        _ => psiv_core::Topology::Bounded,
    };
    FieldMap::with_topology(MapId(record.id.0), grid, warps, npcs, topology)
        .map_err(|e| BridgeError::Rejected(e.to_string()))
}

/// The wandering objects on a map: exactly the pack NPCs whose behaviour
/// routine is NPCType2 or NPCType3 — the two types that share the cartridge's
/// random walker (`docs/NPC_WANDER.md`). Everything else stands still until
/// its own routine is transcribed.
pub(super) fn build_wander(map: &FieldMap, record: &MapRecord) -> Result<WanderSet, BridgeError> {
    let objects: Vec<(usize, WanderKind)> = record
        .npcs
        .iter()
        .enumerate()
        .filter_map(|(i, npc)| match npc.symbol.as_deref() {
            Some("NPCType2") => Some((i, WanderKind::Type2)),
            Some("NPCType3") => Some((i, WanderKind::Type3)),
            _ => None,
        })
        .collect();
    WanderSet::build(map, &objects).map_err(|e| BridgeError::Rejected(e.to_string()))
}

/// Character id by party-sheet symbol (`CharFieldArtPtrs` order is the id).
pub(super) fn char_id_by_symbol(data: &GameData, symbol: &str) -> Option<u8> {
    (0..11)
        .find(|&slot| {
            data.party_sheet(slot)
                .is_some_and(|sheet| sheet.id == symbol)
        })
        .map(|slot| slot as u8)
}
