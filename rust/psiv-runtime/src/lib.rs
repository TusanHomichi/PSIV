//! The bridge between the pack schema (`psiv-data`) and the field engine
//! (`psiv-core`), plus the game shell that owns map changes.
//!
//! `psiv-core` knows no schema and `psiv-data` knows no rules; this crate is
//! the only place the two meet. `psiv-godot` drives a [`Runtime`] and never
//! touches either lower layer directly.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::float_arithmetic)]

use std::fmt;

use psiv_core::{
    Cell, CollisionGrid, Direction, Effect, FieldMap, FieldState, Input, MapId, Npc, NpcId,
    StepFrames, Warp, WarpTrigger,
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
    let grid = &record.collision.grid;
    let width = u16::try_from(grid.width())
        .map_err(|_| BridgeError::OutOfRange(format!("width {}", grid.width())))?;
    let height = u16::try_from(grid.height())
        .map_err(|_| BridgeError::OutOfRange(format!("height {}", grid.height())))?;
    let cells: Vec<u8> = grid.cells().iter().map(|c| c.code()).collect();
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
    for npc in &record.npcs {
        let cell = cell_u16(npc.x_cell, npc.y_cell, "npc")?;
        let facing = npc
            .facing
            .direction()
            .map(direction)
            .unwrap_or(Direction::Down);
        npcs.push(Npc::new(NpcId(npc.object_id), cell, facing));
    }

    FieldMap::new(MapId(record.id.0), grid, warps, npcs)
        .map_err(|e| BridgeError::Rejected(e.to_string()))
}

/// What a [`Runtime`] tick produced, for the presentation layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// The party finished a step.
    StepCompleted {
        /// The cell landed on.
        cell: Cell,
    },
    /// A transition fired and the runtime switched maps. The renderer should
    /// rebuild the scene for [`Runtime::map_id`]; `trigger` says whether this
    /// was a doorway (fade) or ground transition (scroll), per the design doc.
    MapChanged {
        /// The map now loaded.
        map: MapId,
        /// Which transition class fired.
        trigger: WarpTrigger,
    },
    /// A transition targeted a map the pack does not contain (skipped or
    /// unpacked). The party stays put; the presentation layer should surface
    /// this as a data warning.
    UnpackedTarget {
        /// The map the transition wanted.
        map: MapId,
    },
    /// The engine reported a type-1 cell with no matching doorway record — a
    /// pack defect. Surface it, never swallow it.
    WarpUnmapped {
        /// The offending cell.
        cell: Cell,
    },
    /// The party talked to an NPC. The renderer looks up the NPC's
    /// dialogue binding in the map record and opens a window; while it is
    /// open, it stops sending direction inputs (the engine is not modal).
    Interact {
        /// Index into the current map's NPC list.
        npc_index: usize,
        /// The faced cell that was hit.
        cell: Cell,
    },
    /// Confirm pressed with nothing in talk range — the cartridge answers
    /// with the leader's "Nothing here" line.
    InteractNothing {
        /// Which way the party was facing.
        facing: Direction,
    },
}

/// The game shell: pack data, the current engine map, and the field state.
pub struct Runtime {
    data: GameData,
    map: FieldMap,
    state: FieldState,
}

impl Runtime {
    /// Starts on `map_id` at `spawn`, facing `facing`.
    pub fn new(
        data: GameData,
        map_id: u16,
        spawn: Cell,
        facing: Direction,
        step_frames: StepFrames,
    ) -> Result<Runtime, BridgeError> {
        let record = data
            .map(psiv_data::MapId(map_id))
            .ok_or(BridgeError::NotPacked(map_id))?;
        let map = field_map(record)?;
        let state = FieldState::new(&map, spawn, facing, step_frames)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        Ok(Runtime { data, map, state })
    }

    /// The currently loaded map.
    #[must_use]
    pub fn map_id(&self) -> MapId {
        self.map.id()
    }

    /// The engine map, for the renderer's collision/NPC queries.
    #[must_use]
    pub fn map(&self) -> &FieldMap {
        &self.map
    }

    /// The loaded pack, for presentation-layer queries (sprite sheets, the
    /// current map's record). Read-only; the runtime owns all mutation.
    #[must_use]
    pub fn data(&self) -> &GameData {
        &self.data
    }

    /// The current map's record, for presentation-layer queries (NPC sprite
    /// bindings and the like).
    #[must_use]
    pub fn map_record(&self) -> Option<&psiv_data::MapRecord> {
        self.data.map(psiv_data::MapId(self.map.id().0))
    }

    /// The current map's composed-render path, exactly as the pack declares
    /// it (relative to the pack root). The renderer must never invent pack
    /// filenames; the pack names its own files.
    #[must_use]
    pub fn map_png(&self) -> Option<&str> {
        self.data
            .map(psiv_data::MapId(self.map.id().0))
            .map(|record| record.png.as_str())
    }

    /// The field state, for the renderer's position/interpolation queries.
    #[must_use]
    pub fn state(&self) -> &FieldState {
        &self.state
    }

    /// Advances one tick and resolves any map change.
    pub fn tick(&mut self, input: Input) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        for effect in self.state.tick(&self.map, input) {
            match effect {
                Effect::StepCompleted { cell } => events.push(RuntimeEvent::StepCompleted { cell }),
                Effect::Warp {
                    trigger,
                    target_map,
                    target_cell,
                    facing,
                    ..
                } => match self.change_map(target_map, target_cell, facing) {
                    Ok(()) => events.push(RuntimeEvent::MapChanged {
                        map: target_map,
                        trigger,
                    }),
                    Err(BridgeError::NotPacked(id)) => {
                        events.push(RuntimeEvent::UnpackedTarget { map: MapId(id) });
                    }
                    // Any other bridge failure on a packed map is a defect the
                    // pack's own validation should have caught; surface it the
                    // same way rather than panicking mid-game.
                    Err(_) => events.push(RuntimeEvent::UnpackedTarget { map: target_map }),
                },
                Effect::WarpUnmapped { cell } => events.push(RuntimeEvent::WarpUnmapped { cell }),
                Effect::Interact { npc_index, cell } => {
                    events.push(RuntimeEvent::Interact { npc_index, cell });
                }
                Effect::InteractNothing { facing } => {
                    events.push(RuntimeEvent::InteractNothing { facing });
                }
            }
        }
        events
    }

    fn change_map(
        &mut self,
        target: MapId,
        cell: Cell,
        facing: Direction,
    ) -> Result<(), BridgeError> {
        let record = self
            .data
            .map(psiv_data::MapId(target.0))
            .ok_or(BridgeError::NotPacked(target.0))?;
        let map = field_map(record)?;
        self.state
            .enter_map(&map, cell, facing)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
        self.map = map;
        Ok(())
    }
}
