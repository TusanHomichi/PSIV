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
// Retail records are integers: a float anywhere here is a decoding bug, so the compiler denies it.
#![deny(clippy::float_arithmetic)]

mod battle;
mod camera;
mod collision;
mod dialogue;
mod error;
mod game_data;
mod ids;
mod manifest;
mod map;
mod new_game;
mod sound;
mod sprites;
mod travel;

pub use battle::{
    AbilitiesFile, Ability, BATTLE_DIRECTORY, BattleFiles, ELEMENT_SLOTS, EffectTable,
    EncounterGroup, EncounterGroups, EnemiesFile, Enemy, EnemyAi, EnemyAnimation,
    EnemyAnimationCensus, EnemyAnimationDispatch, EnemyAnimationEvidence,
    EnemyAnimationFrameSequence, EnemyAnimationSfxWrite, EnemyAnimationSource,
    EnemyAnimationTableSource, EnemyAnimationsFile, EnemyAttack, EnemyStats, Formation,
    FormationEnemy, FormationsFile, Level, LevelStats, LevelTable, LevelsFile, MapBinding, NamedId,
    NamedMapRef, PositionGrid, Property, Rewards,
};
pub use camera::{MapScroll, ScrollCounters};
// The seating path: `battle/characters.json` and `battle/equipment.json`.
pub use battle::{
    Character, CharactersFile, DerivedStat, ElementRef, Equipment, EquipmentBonuses, EquipmentFile,
    EquipmentKindRef, EquipmentType, Equipped, Initialized, Loadout, SkillSlots, StatusPortrait,
    TechniqueSlots, WeaponElements,
};
pub use collision::{Collision, CollisionGrid, CollisionType, Plane, UndefinedCollisionType};
pub use dialogue::*;
pub use error::DataError;
pub use game_data::GameData;
pub use ids::{MapId, RomHash};
pub use manifest::{
    GameStartFacing, GameStartMap, GameStartSummary, Manifest, MapEntry, PACK_FORMAT_VERSION,
    RomInfo, SkippedMap,
};
pub use map::{
    COLLISION_CELL_PIXELS, Cell, CellPos, CellRect, ContentsType, DIALOGUE_TREE_COUNT, Dimensions,
    Direction, EffectGate, EffectPath, EffectWrite, Facing, Flags, InteractionArea,
    InteractionFlagType, InteractionSource, LayoutVariant, MapEffect, MapRecord, MapRef, Music,
    Npc, OverworldLayoutPatch, OverworldLayoutWrite, OverworldPatch, OverworldPatchFlag,
    OverworldPatchTile, PaletteAffectedSprites, PaletteEffect, PaletteSpriteReplacement, RangeRef,
    ResolvedCell, SpriteFacing, SpriteRef, TransitionTable, Treasure, VariantCollision,
    VariantPlane, VehicleBattleLayout, Warp, WarpSource,
};
pub use new_game::NewGame;
pub use sound::{SoundFiles, SoundRecord, SoundTrackKind, SoundTrackRecord};
pub use sprites::{Sequence, SequenceFrame, Sheet, SheetFile, VehiclePaletteVariant};
pub use travel::{DungeonDestination, PlaceEntry, TownDestination, TravelData, TravelFile};
