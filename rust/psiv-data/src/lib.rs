//! Schema layer: typed access to the runtime pack. See docs/RUNTIME_DESIGN.md.
//!
//! The runtime pack is what `python -m psiv_tools pack <rom> runtime-pack/`
//! emits: a manifest plus one JSON record per field map. This crate is the only
//! thing in the workspace that knows that format. It deserializes it, validates
//! it, and hands out a [`GameData`] that later layers can treat as already
//! correct.
//!
//! No game logic lives here. `psiv-core` does not depend on this crate; the
//! bridge between them is built on top of both.
//!
//! # Fail-closed
//!
//! Every defect is a hard error. A pack with a version we do not recognise, a
//! missing map file, an undefined collision nibble, a warp pointing at a map
//! that is neither packed nor declared unpacked, an NPC standing outside the
//! map -- all of these stop the load rather than degrading into a half-loaded
//! world. Validation runs on every load and cannot be switched off, because a
//! runtime that tolerates a broken pack stops being a test oracle for the
//! cartridge.
//!
//! ```no_run
//! use psiv_data::{CellPos, GameData, MapId};
//! use std::path::Path;
//!
//! let data = GameData::load(Path::new("runtime-pack"))?;
//! let piata = data.map(MapId(0x010)).expect("Piata is packed");
//!
//! // A doorway is walkable: type 1 does not block, the warp fires on entry.
//! let door = piata.warps[0].source.pos();
//! assert!(!piata.blocks_at(door));
//! assert!(piata.warps_at(door).next().is_some());
//! # Ok::<(), psiv_data::DataError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod collision;
mod error;
mod game_data;
mod ids;
mod manifest;
mod map;
mod sprites;

pub use collision::{Collision, CollisionGrid, CollisionType, Plane, UndefinedCollisionType};
pub use error::DataError;
pub use game_data::GameData;
pub use ids::{MapId, RomHash};
pub use manifest::{Manifest, MapEntry, PACK_FORMAT_VERSION, RomInfo, SkippedMap};
pub use map::{
    COLLISION_CELL_PIXELS, Cell, CellPos, CellRect, ContentsType, DIALOGUE_TREE_COUNT, Dimensions,
    Direction, Facing, Flags, MapRecord, MapRef, Music, Npc, RangeRef, SpriteFacing, SpriteRef, TransitionTable,
    Treasure, Warp, WarpSource,
};
pub use sprites::{Sequence, SequenceFrame, Sheet, SheetFile};
