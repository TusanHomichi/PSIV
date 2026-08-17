//! Land Rover-class field rules and the battle member built from a saved car.
//!
//! The numbers in this module are deliberately boring. They are the retail
//! tables, transcribed once, with the bridge responsible for maps, audio and
//! rendering. The field routine is `FieldObj_LandRover` and its callers at
//! `ps4.asm:93123-93207`; movement is `VehicleMove` at
//! `ps4.asm:93478-93599`; the four footprint samplers are
//! `UpdateVehicleCollision` at `ps4.asm:90694-90810`; the selector tables are
//! `loc_45C46` at `ps4.asm:91065-91094` and `ps4.asm:91222-91280`.

use crate::battle::{
    ELEMENT_SLOTS, EQUIPMENT_SLOTS, PartyMember, SKILL_SLOTS, StatPair, StatTriple, Stats,
    TECHNIQUE_SLOTS,
};
use crate::collision::CollisionType;
use crate::field::Input;
use crate::geom::{Cell, Direction};
use crate::map::{FieldMap, MapId, WarpTrigger};
use crate::state::VehicleRecord;

/// The retail vehicle selector range. Zero means the party is on foot.
pub const VEHICLE_INDEX_MIN: u16 = 1;
/// The last selector in `Saved_Vehicle_Stats`.
pub const VEHICLE_INDEX_MAX: u16 = 3;
/// The retail movement table's normal selector, `FieldObj_Step_Offset`.
pub const DEFAULT_STEP_OFFSET: u8 = 1;

/// The vehicle's fixed data, from `VehicleData` at `ps4.asm:321152`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleProfile {
    /// The persisted selector value.
    pub index: u16,
    /// The retail label used by debug and battle presentation.
    pub name: &'static str,
    /// The sprite-pack sheet id.
    pub sheet_id: &'static str,
    /// Static strength.
    pub strength: u8,
    /// Static mental power.
    pub mental: u8,
    /// Static agility.
    pub agility: u8,
    /// Static dexterity.
    pub dexterity: u8,
    /// Static physical attack.
    pub attack: u16,
    /// Static physical defence.
    pub defence: u16,
    /// Static mental defence.
    pub mental_defence: u16,
    /// Static element properties.
    pub properties: [u8; ELEMENT_SLOTS],
    /// Retail skill availability mask.
    pub default_skill_mask: u8,
    /// Retail starting skill uses, before save changes.
    pub default_skill_uses: [u8; SKILL_SLOTS],
}

const VEHICLE_PROFILES: [VehicleProfile; 3] = [
    VehicleProfile {
        index: 1,
        name: "LAND ROVER",
        sheet_id: "LandRover",
        strength: 64,
        mental: 0,
        agility: 60,
        dexterity: 70,
        attack: 200,
        defence: 80,
        mental_defence: 50,
        properties: [1, 2, 1, 1, 1, 2, 3, 0, 0, 0, 0, 0, 0, 2],
        default_skill_mask: 0x03,
        default_skill_uses: [8, 8, 0, 0, 0, 0, 0, 0],
    },
    VehicleProfile {
        index: 2,
        name: "ICE DIGGER",
        sheet_id: "IceDigger",
        strength: 80,
        mental: 0,
        agility: 48,
        dexterity: 70,
        attack: 250,
        defence: 80,
        mental_defence: 50,
        properties: [1, 2, 1, 1, 1, 2, 3, 0, 0, 0, 0, 0, 0, 2],
        default_skill_mask: 0x50,
        default_skill_uses: [8, 4, 0, 0, 0, 0, 0, 0],
    },
    VehicleProfile {
        index: 3,
        name: "HYDROFOIL",
        sheet_id: "Hydrofoil",
        strength: 255,
        mental: 0,
        agility: 96,
        dexterity: 70,
        attack: 150,
        defence: 70,
        mental_defence: 40,
        properties: [1, 2, 1, 1, 1, 2, 3, 0, 0, 0, 0, 0, 0, 2],
        default_skill_mask: 0x0C,
        default_skill_uses: [4, 2, 0, 0, 0, 0, 0, 0],
    },
];

/// Returns the static record selected by `$F43C`, or `None` for on-foot and
/// out-of-range values.
#[must_use]
pub fn profile(index: u16) -> Option<&'static VehicleProfile> {
    VEHICLE_PROFILES
        .iter()
        .find(|profile| profile.index == index)
}

/// Returns the saved vehicle as the one battle fighter retail creates.
///
/// Retail's `loc_78EE` writes `Obj_Fighters = 1` and replaces the ordinary
/// party with fighter id `Vehicle_Index + $B` (`ps4.asm:11408-11414`). The
/// saved record supplies the HP and skill uses here. The HP read is an
/// intentional project correction: the retail entry path reloads static HP,
/// but PSIV's save lane promises that the three vehicle records round-trip and
/// the runtime must therefore carry their current HP into the battle surface.
#[must_use]
pub fn battle_member(index: u16, record: VehicleRecord) -> Option<PartyMember> {
    let profile = profile(index)?;
    let mut skills = [0; SKILL_SLOTS];
    for (slot, skill) in skills.iter_mut().enumerate() {
        if record.skill_mask & (1 << slot) != 0 {
            *skill = (slot as u8) + 1;
        }
    }
    Some(PartyMember {
        character: (0x0B_u16 + index) as u8,
        name: profile.name.to_owned(),
        stats: Stats {
            name_bytes: [0; 6],
            profession: 0,
            level: 0,
            experience: 0,
            curr_hp: record.current_hp,
            max_hp: record.max_hp,
            curr_tp: 0,
            max_tp: 0,
            status: 0,
            strength: StatTriple::uniform(profile.strength),
            mental: StatTriple::uniform(profile.mental),
            agility: StatTriple::uniform(profile.agility),
            dexterity: StatTriple::uniform(profile.dexterity),
            attack: StatPair::uniform(profile.attack),
            defence: StatPair::uniform(profile.defence),
            mental_defence: StatPair::uniform(profile.mental_defence),
            element_props: profile.properties,
            element_shadow: profile.properties,
            weapon_elements: [0; 2],
            equipment: [0; EQUIPMENT_SLOTS],
            techniques: [0; TECHNIQUE_SLOTS],
            skills,
            curr_skill_uses: record.current_skill_uses,
            max_skill_uses: record.max_skill_uses,
            physical_prop_save: profile.properties[0],
            enemy_id: 0,
            gain_exp_flag: false,
        },
    })
}

/// What a mounted field tick asks the runtime to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleEffect {
    /// A 32-pixel vehicle step ended.
    StepCompleted {
        /// The new standing cell.
        cell: Cell,
    },
    /// A map transition fired after the vehicle came to rest.
    Warp {
        /// The cell that fired it.
        from: Cell,
        /// The transition table that supplied it.
        trigger: WarpTrigger,
        /// Destination map.
        target_map: MapId,
        /// Destination cell.
        target_cell: Cell,
        /// Destination facing.
        facing: Direction,
    },
    /// A map-change tile had no corresponding transition record.
    WarpUnmapped {
        /// The map-change cell.
        cell: Cell,
    },
    /// The Action edge successfully requested dismount.
    Dismount,
    /// The Action edge requested dismount on a non-empty vehicle cell.
    DismountBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VehicleStep {
    direction: Direction,
    to: Cell,
    frame: u8,
    frames: u8,
}

/// Mounted field state on the retail 32-pixel vehicle grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleState {
    index: u16,
    cell: Cell,
    facing: Direction,
    step_offset: u8,
    previous_standing: u8,
    step: Option<VehicleStep>,
    action_latched: bool,
}

impl VehicleState {
    /// Creates a mounted state at the supplied standing cell.
    #[must_use]
    pub fn new(map: &FieldMap, index: u16, cell: Cell, facing: Direction) -> Option<Self> {
        profile(index)?;
        let cell = map.normalize(cell)?;
        Some(Self {
            index,
            cell,
            facing,
            step_offset: DEFAULT_STEP_OFFSET,
            previous_standing: standing_collision(map, cell),
            step: None,
            action_latched: false,
        })
    }

    /// The selected vehicle index (`$F43C`).
    #[must_use]
    pub const fn index(self) -> u16 {
        self.index
    }

    /// The current 32-pixel-grid standing cell.
    #[must_use]
    pub const fn cell(self) -> Cell {
        self.cell
    }

    /// The current facing.
    #[must_use]
    pub const fn facing(self) -> Direction {
        self.facing
    }

    /// Whether the vehicle is between standing cells.
    #[must_use]
    pub const fn is_moving(self) -> bool {
        self.step.is_some()
    }

    /// Retail's `FieldObj_Step_Offset`, masked to its three legal bits.
    #[must_use]
    pub const fn step_offset(self) -> u8 {
        self.step_offset & 3
    }

    /// Changes the step selector used by the next movement. The field routine
    /// only gives Hydrofoil a second table block; selector 2 on Hydrofoil is
    /// outside the retail movement table and is rejected here.
    pub fn set_step_offset(&mut self, value: u8) {
        self.step_offset = value & 3;
    }

    /// The interpolated screen offset in pixels, in the same sign convention
    /// as the field renderer. A 32-pixel step is one vehicle movement unit.
    #[must_use]
    pub fn render_offset_16ths(self) -> (i32, i32) {
        let Some(step) = self.step else {
            return (0, 0);
        };
        let distance = (i32::from(step.frame) * 32) / i32::from(step.frames);
        match step.direction {
            Direction::Up => (0, -distance),
            Direction::Down => (0, distance),
            Direction::Left => (-distance, 0),
            Direction::Right => (distance, 0),
        }
    }

    /// Retail's 8.8 movement constants for the current selector.
    ///
    /// Block 0 is `$0200` (16 frames), block 1 `$0400` (8 frames), and block
    /// 2 `$0800` (4 frames), each over 32 pixels (`ps4.asm:94079`).
    #[must_use]
    pub fn step_timing(self) -> Option<(u8, u16)> {
        let mut block = self.step_offset();
        if self.index == 3 {
            block = block.saturating_add(1);
        }
        match block {
            0 => Some((16, 0x0200)),
            1 => Some((8, 0x0400)),
            2 => Some((4, 0x0800)),
            _ => None,
        }
    }

    /// Ticks input through vehicle movement, collision, transitions and
    /// dismount. One call represents one field frame.
    pub fn tick(&mut self, map: &FieldMap, input: Input) -> Vec<VehicleEffect> {
        let mut effects = Vec::new();
        let action = input.is_action();
        let action_edge = action && !self.action_latched;
        self.action_latched = action;

        if let Some(mut step) = self.step {
            step.frame = step.frame.saturating_add(1);
            if step.frame >= step.frames {
                self.cell = step.to;
                self.facing = step.direction;
                self.step = None;
                effects.push(VehicleEffect::StepCompleted { cell: self.cell });
                effects.extend(self.transition_effect(map));
            } else {
                self.step = Some(step);
            }
            return effects;
        }

        if action_edge {
            if dismount_allowed(map, self.cell) {
                effects.push(VehicleEffect::Dismount);
            } else {
                effects.push(VehicleEffect::DismountBlocked);
            }
            return effects;
        }

        let Some(direction) = input.direction() else {
            effects.extend(self.transition_effect(map));
            return effects;
        };
        self.facing = direction;
        let Some((frames, _constant)) = self.step_timing() else {
            return effects;
        };
        let Some(to) = map
            .neighbor(self.cell, direction)
            .and_then(|anchor| map.neighbor(anchor, direction))
        else {
            return effects;
        };
        if !can_enter(self.index, map, self.cell, direction) {
            return effects;
        }
        self.step = Some(VehicleStep {
            direction,
            to,
            frame: 1,
            frames,
        });
        effects
    }

    fn transition_effect(&mut self, map: &FieldMap) -> Vec<VehicleEffect> {
        let mut effects = Vec::new();
        let raw = standing_collision(map, self.cell);
        if raw == 1 {
            if self.previous_standing != 1 {
                self.previous_standing = raw;
                if let Some(warp) = map.warp_at(self.cell, WarpTrigger::MapChange) {
                    effects.push(VehicleEffect::Warp {
                        from: self.cell,
                        trigger: warp.trigger,
                        target_map: warp.target_map,
                        target_cell: warp.target_cell,
                        facing: warp.facing,
                    });
                } else {
                    effects.push(VehicleEffect::WarpUnmapped { cell: self.cell });
                }
            }
            return effects;
        }
        self.previous_standing = raw;
        if !CollisionType::from_raw(raw).is_blocking()
            && let Some(warp) = map.warp_at(self.cell, WarpTrigger::NormalGround)
        {
            effects.push(VehicleEffect::Warp {
                from: self.cell,
                trigger: warp.trigger,
                target_map: warp.target_map,
                target_cell: warp.target_cell,
                facing: warp.facing,
            });
        }
        effects
    }

    /// Re-anchors the vehicle after the bridge loads a destination map.
    pub fn enter_map(&mut self, map: &FieldMap, cell: Cell, facing: Direction) -> bool {
        let Some(cell) = map.normalize(cell) else {
            return false;
        };
        self.cell = cell;
        self.facing = facing;
        self.previous_standing = standing_collision(map, cell);
        self.step = None;
        self.action_latched = false;
        true
    }
}

/// The raw collision value under the vehicle's four 16-pixel footprint cells.
#[must_use]
pub fn standing_collision(map: &FieldMap, cell: Cell) -> u8 {
    footprint_max(map, cell, &[(0, -1), (1, -1), (0, 0), (1, 0)])
}

/// The raw collision values sampled in the requested vehicle direction.
#[must_use]
pub fn directional_collision(map: &FieldMap, cell: Cell, direction: Direction) -> u8 {
    let offsets: &[(i32, i32)] = match direction {
        Direction::Right => &[(2, -1), (3, -1), (2, 0), (3, 0)],
        Direction::Left => &[(-2, -1), (-1, -1), (-2, 0), (-1, 0)],
        Direction::Down => &[(0, 1), (1, 1), (0, 2), (1, 2)],
        Direction::Up => &[(0, -3), (1, -3), (0, -2), (1, -2)],
    };
    footprint_max(map, cell, offsets)
}

/// Returns whether `raw` is passable for a selected vehicle.
#[must_use]
pub const fn can_cross(index: u16, raw: u8) -> bool {
    match index {
        // Land Rover: only 0, 1 and sand (`$A`) are passable.
        1 => matches!(raw, 0 | 1 | 0xA),
        // Ice Digger: 0, 1, sand and ice (`$B`).
        2 => matches!(raw, 0 | 1 | 0xA | 0xB),
        // Hydrofoil: 0, 1, water (`9`) and sand (`$A`).
        3 => matches!(raw, 0 | 1 | 9 | 0xA),
        _ => false,
    }
}

/// Returns whether the whole four-cell vehicle footprint can move one unit.
#[must_use]
pub fn can_enter(index: u16, map: &FieldMap, cell: Cell, direction: Direction) -> bool {
    can_cross(index, directional_collision(map, cell, direction))
}

/// Returns whether the vehicle may dismount on its current standing cell.
/// Retail's `loc_4587A` accepts only a zero collision result.
#[must_use]
pub fn dismount_allowed(map: &FieldMap, cell: Cell) -> bool {
    standing_collision(map, cell) == 0
}

/// Returns whether a mounted vehicle suppresses a random battle on this frame.
/// Vehicle movement uses raw 1/2 and adjacent map-change samples for grace,
/// matching `RunRandomBattles` (`ps4.asm:116840-116884`).
#[must_use]
pub fn encounter_suppressed(map: &FieldMap, cell: Cell) -> bool {
    if matches!(standing_collision(map, cell), 1 | 2) {
        return true;
    }
    Direction::ALL
        .iter()
        .any(|&direction| directional_collision(map, cell, direction) == 1)
}

fn footprint_max(map: &FieldMap, cell: Cell, offsets: &[(i32, i32)]) -> u8 {
    offsets
        .iter()
        .map(|&(dx, dy)| {
            map.normalize_signed(i32::from(cell.x) + dx, i32::from(cell.y) + dy)
                .and_then(|sample| map.collision_at(sample))
                .map(CollisionType::to_raw)
                // The cartridge wraps overworld coordinates, while an absent
                // bounded sample is safest as solid for the vehicle footprint.
                .unwrap_or(0x0F)
        })
        .max()
        .unwrap_or(0x0F)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::CollisionGrid;
    use crate::map::{FieldMap, Topology};

    fn map(cells: Vec<u8>) -> FieldMap {
        let grid = CollisionGrid::new(8, 8, cells).expect("test grid");
        FieldMap::with_topology(MapId(0), grid, vec![], vec![], Topology::Torus).expect("test map")
    }

    #[test]
    fn selectors_match_retail_vehicle_masks() {
        for raw in 0..=0x0F {
            assert_eq!(can_cross(1, raw), matches!(raw, 0 | 1 | 0xA));
            assert_eq!(can_cross(2, raw), matches!(raw, 0 | 1 | 0xA | 0xB));
            assert_eq!(can_cross(3, raw), matches!(raw, 0 | 1 | 9 | 0xA));
        }
    }

    #[test]
    fn land_rover_samples_a_two_by_two_footprint() {
        let mut cells = vec![0; 64];
        cells[3 * 8 + 3] = 0xB;
        let map = map(cells);
        assert_eq!(standing_collision(&map, Cell::new(2, 4)), 0xB);
        assert_eq!(
            directional_collision(&map, Cell::new(1, 4), Direction::Right),
            0xB
        );
    }

    #[test]
    fn movement_is_four_pixels_per_frame_at_default_selector() {
        let map = map(vec![0; 64]);
        let mut state =
            VehicleState::new(&map, 1, Cell::new(2, 2), Direction::Down).expect("vehicle");
        assert_eq!(state.step_timing(), Some((8, 0x0400)));
        state.tick(&map, Input::Direction(Direction::Right));
        assert_eq!(state.render_offset_16ths(), (4, 0));
    }

    #[test]
    fn battle_member_carries_saved_hp_mask_and_uses() {
        let record = VehicleRecord {
            current_hp: 0x0123,
            max_hp: 0x0456,
            skill_mask: 0x05,
            current_skill_uses: [1, 2, 3, 4, 5, 6, 7, 8],
            max_skill_uses: [8, 7, 6, 5, 4, 3, 2, 1],
            ..VehicleRecord::default()
        };
        let member = battle_member(1, record).expect("Land Rover");
        assert_eq!(member.character, 0x0C);
        assert_eq!(member.stats.curr_hp, 0x0123);
        assert_eq!(member.stats.max_hp, 0x0456);
        assert_eq!(member.stats.skills, [1, 0, 3, 0, 0, 0, 0, 0]);
        assert_eq!(member.stats.curr_skill_uses, record.current_skill_uses);
        assert_eq!(member.stats.max_skill_uses, record.max_skill_uses);
    }
}
