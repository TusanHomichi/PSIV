//! Pack-to-engine conversion: bridge errors, map construction, and record helpers.

use std::fmt;

use crate::effects::EffectOutcome;
use psiv_core::{
    BespokeFlag, BespokeKind, BespokeRandom, BespokeSet, Cell, CollisionGrid, Direction, FieldMap,
    FollowTarget, GameState, Leash, MapId, Npc, NpcId, PATTERN_48F36, PATTERN_49128,
    PATTERN_ESPER_GUARD, PATTERN_MUSK_GUARD, PATTERN_TYPE5, PATTERN_TYPE17, PATTERN_TYPE35,
    PATTERN_TYPE36, WanderKind, WanderSet, WanderSpeed, Warp, WarpTrigger,
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

/// A placed object with no art and no dialogue entry is collision-only.
///
/// Invisible objects with a nonzero dialogue id are real hidden triggers in
/// the pack, so `sprite_reason` alone is not a talk filter. Conversely, an
/// artless object with dialogue id zero has nothing for the runtime's dialogue
/// consumer to open. Keep this derived from the pack's existing fields rather
/// than teaching the core about field-object symbols.
fn dialogue_probe_eligible(npc: &psiv_data::Npc) -> bool {
    npc.sprite_reason.is_none() || npc.dialogue_id != 0
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
    field_map_retaining_objects(record, outcome, &[])
}

/// LoadTreasureChests follows map NPC allocation. Keep live lids on a battle
/// or chunk refresh; a real map entry reconstructs them from the saved flags.
pub(super) fn attach_chests(
    map: &mut FieldMap,
    record: &MapRecord,
    game: &GameState,
    objects: &[Npc],
    outcome: &EffectOutcome,
) -> Result<(), BridgeError> {
    let chests = record
        .treasure_chests
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let range = |what| BridgeError::OutOfRange(format!("chest {index}: {what}"));
            let contents = match record.contents_type {
                psiv_data::ContentsType::Item => psiv_core::ChestContents::Item(
                    record
                        .item_id
                        .and_then(|id| u8::try_from(id).ok())
                        .filter(|id| *id != 0)
                        .ok_or_else(|| range("item"))?,
                ),
                psiv_data::ContentsType::Meseta => {
                    psiv_core::ChestContents::Meseta(record.meseta.ok_or_else(|| range("meseta"))?)
                }
            };
            Ok(psiv_core::Chest {
                cell: cell_u16(record.x_cell, record.y_cell, "chest")?,
                flag: u8::try_from(record.chest_flag).map_err(|_| range("flag"))?,
                contents,
                white: record.white_chest,
                index,
            })
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    let base = map.npcs().len();
    map.with_chests(chests, |chest| {
        objects.get(base + chest.index).map_or_else(
            || game.chest_is_open(chest),
            |npc| npc.facing == Direction::Up,
        )
    })
    .map_err(|e| BridgeError::Rejected(e.to_string()))?;
    for index in base..map.npcs().len() {
        let active =
            objects.get(index).is_none_or(|npc| npc.active) && !outcome.despawns.contains(&index);
        map.set_npc_active(index, active)
            .map_err(|e| BridgeError::Rejected(e.to_string()))?;
    }
    Ok(())
}

/// Battle return skips LoadMapObjects, but still runs MapDataManager. Retain
/// the live object records before applying its new despawns and type writes.
pub(super) fn field_map_retaining_objects(
    record: &MapRecord,
    outcome: Option<&EffectOutcome>,
    objects: &[Npc],
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
        let mut object = objects.get(index).copied().unwrap_or_else(|| {
            Npc::with_offset(NpcId(npc.object_id), cell, offset, facing)
                .with_camera_bypass(npc.camera_bypass)
                .with_interactable(npc.interactable)
                .with_talkable(dialogue_probe_eligible(npc))
        });
        let object_id = outcome
            .and_then(|o| {
                o.rewrites
                    .iter()
                    .find(|(i, _)| *i == index)
                    .map(|(_, id)| *id)
            })
            .unwrap_or(object.id.0);
        object.id = NpcId(object_id);
        object.active &= outcome.is_none_or(|o| !o.despawns.contains(&index));
        npcs.push(object);
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

/// The wandering objects on a map: the packed routines whose random calls have
/// been transcribed in `docs/field/NPC_WANDER.md`. Fixed-position and bespoke
/// routines remain out of this list; they are not silently approximated as
/// generic walkers.
pub(super) fn build_wander(map: &FieldMap, record: &MapRecord) -> Result<WanderSet, BridgeError> {
    let objects: Vec<(usize, WanderKind)> = record
        .npcs
        .iter()
        .enumerate()
        .filter_map(|(i, npc)| {
            if !map.npcs().get(i).is_some_and(|object| object.active) {
                return None;
            }
            let kind = match npc.symbol.as_deref() {
                Some("NPCType2") => WanderKind::Type2,
                Some("NPCType3") => WanderKind::Type3,
                Some("NPCType4") => WanderKind::Type4,
                Some("NPCType28") => WanderKind::Type28,
                Some("loc_490B8") => WanderKind::StoreWoman,
                Some("loc_49746") => WanderKind::ClinicWoman,
                Some("Penguin") => WanderKind::Penguin,
                Some("Butterfly") => WanderKind::Butterfly,
                Some("MuskCat") => WanderKind::MuskCat,
                Some("Xanafalgue") => WanderKind::Xanafalgue,
                _ => return None,
            };
            Some((i, kind))
        })
        .collect();
    WanderSet::build(map, &objects).map_err(|e| BridgeError::Rejected(e.to_string()))
}

const fn leash(x_max: u8, y_max: u8, x: u8, y: u8) -> Leash {
    Leash { x_max, y_max, x, y }
}

const fn fixed(command: u8, speed: WanderSpeed, leash: Leash) -> BespokeKind {
    BespokeKind::Fixed {
        command,
        speed,
        leash,
    }
}

const fn pattern(commands: &'static [u8], speed: WanderSpeed, leash: Leash) -> BespokeKind {
    BespokeKind::Pattern {
        commands,
        speed,
        leash,
    }
}

/// Converts every post-wave-7 routine into an explicit core classification.
///
/// The match is intentionally symbol-based. Object ids are the jump-table
/// offsets, but the pack already gives us the disassembly symbol and that is
/// the stable provenance key used by the census.
fn bespoke_kind(symbol: Option<&str>) -> Option<BespokeKind> {
    use BespokeKind::{
        FlaggedFollow, FlaggedPattern, FlaggedRandom, Follow, PresentationOnly, Random,
        SceneDriven, StaticAnimation,
    };
    Some(match symbol? {
        // Deterministic field walkers and fixed attempts.
        "NPCType5" => pattern(PATTERN_TYPE5, WanderSpeed::Selector1, leash(2, 2, 0, 2)),
        "NPCType6" => FlaggedFollow {
            flag: BespokeFlag::PrincipalConfession,
            before: 2,
            target: FollowTarget::LeaderGuard,
            follow_when_set: false,
            speed: WanderSpeed::Selector2,
            leash: leash(2, 0, 1, 0),
        },
        "NPCType7" => Follow {
            target: FollowTarget::LeaderRanch,
            speed: WanderSpeed::Selector1,
            leash: leash(1, 0, 1, 0),
        },
        "NPCType8" => FlaggedRandom {
            flag: BespokeFlag::IgglanovaZema,
            kind: BespokeRandom::Igglanova,
            speed: WanderSpeed::Selector0,
            leash: leash(16, 16, 8, 8),
        },
        "NPCType9" => fixed(1, WanderSpeed::Selector0, leash(16, 0, 8, 0)),
        "NPCType10" => fixed(3, WanderSpeed::Selector0, leash(0, 16, 0, 8)),
        "NPCType11" => Random {
            kind: BespokeRandom::SequenceChoice,
            speed: WanderSpeed::Selector1,
            leash: leash(11, 1, 4, 1),
        },
        "NPCType12" => Random {
            kind: BespokeRandom::Mouse,
            speed: WanderSpeed::Selector1,
            leash: leash(16, 16, 8, 8),
        },
        "NPCType13" => Random {
            kind: BespokeRandom::FilteredCardinal,
            speed: WanderSpeed::Selector0,
            leash: leash(4, 0, 0, 0),
        },
        "NPCType14" => StaticAnimation,
        "NPCType16" => fixed(2, WanderSpeed::Selector0, leash(16, 0, 8, 0)),
        "NPCType17" => pattern(PATTERN_TYPE17, WanderSpeed::Selector1, leash(4, 4, 0, 0)),
        "NPCType18" => fixed(3, WanderSpeed::Selector0, Leash::ZERO),
        "NPCType19" => fixed(1, WanderSpeed::Selector0, Leash::ZERO),
        "NPCType20" | "NPCType21" | "NPCType22" | "NPCType23" | "NPCType24" | "NPCType25"
        | "NPCType26" | "NPCType29" => StaticAnimation,
        "NPCType27" => fixed(2, WanderSpeed::Selector1, leash(16, 0, 8, 0)),
        "NPCType30" => fixed(4, WanderSpeed::Selector0, leash(2, 16, 2, 8)),
        "NPCType31" => fixed(2, WanderSpeed::Selector0, leash(16, 0, 8, 0)),
        "NPCType33" => fixed(3, WanderSpeed::Selector0, leash(0, 16, 0, 8)),
        "NPCType34" => fixed(4, WanderSpeed::Selector0, leash(0, 16, 0, 8)),
        "NPCType35" => pattern(PATTERN_TYPE35, WanderSpeed::Selector1, leash(6, 4, 0, 0)),
        "NPCType36" => pattern(PATTERN_TYPE36, WanderSpeed::Selector1, leash(6, 4, 2, 0)),
        "Mouse" => Random {
            kind: BespokeRandom::Mouse,
            speed: WanderSpeed::Selector0,
            leash: leash(16, 16, 8, 8),
        },
        "Prisoner" => Random {
            kind: BespokeRandom::Prisoner,
            speed: WanderSpeed::Selector0,
            leash: leash(1, 0, 0, 0),
        },
        "Rocky" => pattern(PATTERN_TYPE5, WanderSpeed::Selector0, leash(4, 4, 2, 2)),
        "SmallWhiteDuck" | "SmallBrownDuck" => Random {
            kind: BespokeRandom::FilteredCardinal,
            speed: WanderSpeed::Selector0,
            leash: leash(32, 32, 16, 16),
        },
        "loc_48F36" => pattern(PATTERN_48F36, WanderSpeed::Selector0, leash(4, 0, 1, 0)),
        "loc_48F96" => fixed(2, WanderSpeed::Selector0, Leash::ZERO),
        "loc_48FF4" => fixed(4, WanderSpeed::Selector0, leash(0, 16, 0, 8)),
        "loc_49128" => pattern(PATTERN_49128, WanderSpeed::Selector0, leash(16, 4, 1, 0)),
        "loc_49192" => Follow {
            target: FollowTarget::PreviousObject,
            speed: WanderSpeed::Selector0,
            leash: leash(32, 32, 16, 16),
        },
        "loc_497A8" => fixed(4, WanderSpeed::Selector0, leash(4, 4, 4, 2)),
        "loc_4980A" => fixed(3, WanderSpeed::Selector0, leash(4, 4, 0, 2)),
        "loc_4986C" => fixed(2, WanderSpeed::Selector0, leash(1, 0, 0, 0)),
        "loc_498CA" => fixed(2, WanderSpeed::Selector0, Leash::ZERO),
        "NPCHahnNearBasement" => pattern(&[3, 4], WanderSpeed::Selector1, leash(1, 0, 1, 0)),
        "Pana" => fixed(2, WanderSpeed::Selector0, leash(16, 0, 8, 0)),
        "Juza" => fixed(2, WanderSpeed::Selector0, Leash::ZERO),
        "StrayRocky" => fixed(3, WanderSpeed::Selector0, leash(2, 0, 0, 0)),
        "EsperGuard" => FlaggedPattern {
            flag: BespokeFlag::EspMansionGuards,
            before: 2,
            after: PATTERN_ESPER_GUARD,
            speed: WanderSpeed::Selector0,
            leash: leash(16, 0, 8, 0),
        },
        "InnerEsperGuards" => FlaggedPattern {
            flag: BespokeFlag::InnerSanctuary,
            before: 1,
            after: PATTERN_ESPER_GUARD,
            speed: WanderSpeed::Selector0,
            leash: leash(16, 0, 8, 0),
        },
        "MuskCatGuard" => FlaggedPattern {
            flag: BespokeFlag::MuskCats,
            before: 2,
            after: PATTERN_MUSK_GUARD,
            speed: WanderSpeed::Selector0,
            leash: leash(16, 0, 8, 0),
        },
        "FellowPenguin" => BespokeKind::FlaggedFollow {
            flag: BespokeFlag::Penguin,
            before: 2,
            target: FollowTarget::PartyTail,
            follow_when_set: true,
            speed: WanderSpeed::Selector0,
            leash: leash(4, 4, 2, 4),
        },
        "loc_496C6" => Follow {
            target: FollowTarget::ObjectAt(9),
            speed: WanderSpeed::Selector0,
            leash: leash(32, 32, 16, 16),
        },

        // Input, mode, and cast-owned actors. Their input is scene authority,
        // so inventing a field AI here would be a cross-lane semantic bug.
        "AlysAngerTower"
        | "DemiSpaceportWaiting"
        | "GryzSpaceportWaiting"
        | "HahnSpaceportWaiting"
        | "KingRappyFlyingAway"
        | "KyraSpaceportWaiting"
        | "NPCDemiSpaceport"
        | "NPCGryz"
        | "NPCGryzSpaceport"
        | "NPCHahnSpaceport"
        | "NPCKyra"
        | "NPCKyraSpaceport"
        | "NPCRajaSpaceport"
        | "NPCRika"
        | "NPCScriptMove"
        | "NPCWren"
        | "Raja"
        | "RajaSpaceportWaiting"
        | "Rika"
        | "NPCAlysPiata" => SceneDriven,

        // Fixed art/animation routines: they update position and animation
        // state, but never consume the field RNG or choose a cell direction.
        "BigFire"
        | "CaveWallPiece"
        | "DElmLars"
        | "DarkForce1"
        | "DarkForce2"
        | "DeVars"
        | "DemiTrapped"
        | "DorinChair"
        | "EclipseTorch"
        | "FractOoze"
        | "Igglanova"
        | "KingRappy"
        | "LutzMirror"
        | "LyingDownMuskCat"
        | "MuskCatChiefBottomHalf"
        | "MuskCatChiefTopHalf"
        | "NPCAlysInBed"
        | "NPCHahn"
        | "NPCRune"
        | "Pennant"
        | "PrisonDoor"
        | "ProfHoltPetrified"
        | "RajaInBed"
        | "SaLews"
        | "SandWormCarving"
        | "StudentInBed"
        | "TallasShoes"
        | "TonoeBasementDoor"
        | "TrappingRopes"
        | "ZemaRocks"
        | "GravestoneHalf"
        | "loc_49212"
        | "loc_49406"
        | "loc_49442"
        | "loc_49502"
        | "loc_49542"
        | "loc_4BDF0"
        | "loc_4BE38"
        | "loc_4BE80" => StaticAnimation,

        // These bodies have deterministic render/animation state, but their
        // movement is not a cell walk and belongs to the visual/special-object
        // lane. MileSandWorm's later draw is gated by visual animation state,
        // which this field lane deliberately does not synthesize.
        "Barrier" | "BarrierBeam1" | "BarrierBeam2" | "BarrierBeam3" | "BarrierBeam4"
        | "BigDuck" | "Blindheads" | "ChestBarrier" | "GiLeFarg" | "XeAThoulAirCastle"
        | "MileSandWorm" => PresentationOnly,
        _ => return None,
    })
}

/// Builds the explicit post-wave-7 actor set for a map.
pub(super) fn build_bespoke(map: &FieldMap, record: &MapRecord) -> Result<BespokeSet, BridgeError> {
    let objects: Vec<(usize, BespokeKind)> = record
        .npcs
        .iter()
        .enumerate()
        .filter_map(|(i, npc)| {
            if !map.npcs().get(i).is_some_and(|object| object.active) {
                return None;
            }
            bespoke_kind(npc.symbol.as_deref()).map(|kind| (i, kind))
        })
        .collect();
    BespokeSet::build(map, &objects).map_err(|e| BridgeError::Rejected(e.to_string()))
}

/// Object initialisers in the retail routine clear these two transient/story
/// bits before their first main pass. Doing it after `MapDataManager` effects
/// preserves the cartridge's order without making the save format scene-aware.
pub(super) fn clear_bespoke_entry_flags(game: &mut GameState, record: &MapRecord) {
    if record
        .npcs
        .iter()
        .any(|npc| npc.symbol.as_deref() == Some("FellowPenguin"))
    {
        let _ = game.clear(psiv_core::Flag::event(0x8A));
    }
    if record
        .npcs
        .iter()
        .any(|npc| npc.symbol.as_deref() == Some("EsperGuard"))
    {
        let _ = game.clear(psiv_core::Flag::temp(0x1A));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bespoke_leashes_preserve_disassembly_word_byte_order() {
        let cases = [
            (
                "NPCType5",
                Leash {
                    x_max: 2,
                    y_max: 2,
                    x: 0,
                    y: 2,
                },
            ),
            (
                "loc_48F36",
                Leash {
                    x_max: 4,
                    y_max: 0,
                    x: 1,
                    y: 0,
                },
            ),
        ];

        for (symbol, expected) in cases {
            assert_eq!(bespoke_kind(Some(symbol)).unwrap().leash(), expected);
        }
    }
}
