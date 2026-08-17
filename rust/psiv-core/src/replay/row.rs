//! One replayed frame, in the oracle's shape.
//!
//! [`FrameSample`] is what the driver reads out of the engine; [`ReplayRow`] is
//! that frozen into the log's own field names and formats, so a row can be
//! compared or written without borrowing the engine it came from.

use crate::field::FieldState;
use crate::geom::Direction;
use crate::state::{GameState, PARTY_SLOTS};
use crate::trigger::PixelPos;

use super::tape::Buttons;

/// One object slot's state, in the oracle's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectSample {
    /// `$8000 | pack id`, or 0 for an empty slot.
    pub id: u16,
    /// `facing_dir`: 0 down, 4 up, 8 right, `$C` left.
    pub facing: u16,
    /// The wander countdown, or 0 for an object that does not wander.
    pub timer: i16,
    /// `x_step_duration`, `$1000` down to 0 in `$80` steps.
    pub x_dur: u16,
    /// `y_step_duration`.
    pub y_dur: u16,
    /// `curr_x_pos` integer word.
    pub x_px: i32,
    /// `curr_y_pos` integer word.
    pub y_px: i32,
    /// `x_move_boundary`, the leash offset.
    pub x_bnd: u8,
    /// `y_move_boundary`.
    pub y_bnd: u8,
    /// `offscreen_flag` (`$12`): 1 when `FieldObj_OnScreenTest` rejected the
    /// object last frame, which is the frame its whole update was skipped.
    ///
    /// This is the visibility gate's own output, so comparing it tests the
    /// camera directly rather than through the objects it freezes.
    pub offscreen: u8,
}

impl ObjectSample {
    /// The value of one of this slot's columns.
    #[must_use]
    pub fn field(&self, kind: &str) -> Option<String> {
        let value = match kind {
            "id" => format!("{:04X}", self.id),
            "facing" => self.facing.to_string(),
            "timer" => self.timer.to_string(),
            "xdur" => self.x_dur.to_string(),
            "ydur" => self.y_dur.to_string(),
            "x_px" => self.x_px.to_string(),
            "y_px" => self.y_px.to_string(),
            "xbnd" => self.x_bnd.to_string(),
            "ybnd" => self.y_bnd.to_string(),
            "off" => self.offscreen.to_string(),
            _ => return None,
        };
        Some(value)
    }
}

/// A frame's worth of engine state in the oracle's own shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRow {
    /// Oracle frame number.
    pub frame: u32,
    /// Mark, on a step's first frame.
    pub mark: Option<String>,
    /// Buttons, in tape spelling.
    pub buttons: String,
    /// `Field_Map_Index`.
    pub map_index: u16,
    /// `Camera_X_Pos_FG`, integer pixels.
    pub cam_x: i32,
    /// `Camera_Y_Pos_FG`, integer pixels.
    pub cam_y: i32,
    /// `Camera_X_Step_Counter_FG`, the 16.16 longword.
    pub cam_step_x: i32,
    /// `Camera_Y_Step_Counter_FG`.
    pub cam_step_y: i32,
    /// `Camera_X_Pos_BG`, integer pixels.
    pub cam_x_bg: i32,
    /// `Camera_Y_Pos_BG`, integer pixels.
    pub cam_y_bg: i32,
    /// `Camera_X_Step_Counter_BG`, the 16.16 longword.
    pub cam_step_x_bg: i32,
    /// `Camera_Y_Step_Counter_BG`.
    pub cam_step_y_bg: i32,
    /// `$EC24`, global plane selector.
    pub gate_ec24: u8,
    /// `$EC25`, FG driver gate.
    pub gate_ec25: u8,
    /// `facing_dir`: 0 down, 4 up, 8 right, `$C` left.
    pub c1_facing: u16,
    /// `x_step_duration` in cartridge units.
    pub c1_x_step_dur: u16,
    /// `y_step_duration` in cartridge units.
    pub c1_y_step_dur: u16,
    /// `curr_x_pos` integer word.
    pub c1_x_px: i32,
    /// `curr_y_pos` integer word.
    pub c1_y_px: i32,
    /// `dest_x_pos`.
    pub c1_dest_x: i32,
    /// `dest_y_pos`.
    pub c1_dest_y: i32,
    /// The second party member's facing.
    pub c2_facing: u16,
    /// The second party member's X.
    pub c2_x_px: i32,
    /// The second party member's Y.
    pub c2_y_px: i32,
    /// Standing collision type.
    pub coll_standing: u8,
    /// Previous standing collision type.
    pub coll_saved_standing: u8,
    /// Neighbour collision types.
    pub coll_left: u8,
    /// Neighbour collision types.
    pub coll_up: u8,
    /// Neighbour collision types.
    pub coll_right: u8,
    /// Neighbour collision types.
    pub coll_down: u8,
    /// Slots 1-4 packed big-endian, as the oracle logs the longword.
    pub party_slots: u32,
    /// The five slot bytes.
    pub party: [u8; PARTY_SLOTS],
    /// Event-flag bank, in the oracle's eight longwords.
    pub eflags: [u32; 8],
    /// Extended event flags, first longword.
    pub ext_eflags_00: u32,
    /// Chest flags, first longword.
    pub chest_flags_00: u32,
    /// Temp event flags, first word.
    pub temp_eflags_00: u16,
    /// Town flags, first longword.
    pub town_flags_00: u32,
    /// The secondary object slots, index-aligned with the map's NPC list.
    pub objects: Vec<ObjectSample>,
}

/// The cartridge's `facing_dir` value for a direction.
#[must_use]
pub const fn facing_value(facing: Direction) -> u16 {
    match facing {
        Direction::Down => 0x0,
        Direction::Up => 0x4,
        Direction::Right => 0x8,
        Direction::Left => 0xC,
    }
}

fn be_u32(bytes: &[u8]) -> u32 {
    let mut out = 0;
    for (index, byte) in bytes.iter().take(4).enumerate() {
        out |= u32::from(*byte) << (24 - index * 8);
    }
    out
}

/// Everything one replayed frame reads out of the engine.
///
/// A struct rather than a long argument list, because the caller is a driver
/// assembling these fields one by one and positional camera tuples are easy
/// to get subtly wrong.
#[derive(Debug, Clone, Copy)]
pub struct FrameSample<'a> {
    /// Oracle frame number.
    pub frame: u32,
    /// The tape mark on this frame, if any.
    pub mark: Option<&'a str>,
    /// What the tape holds.
    pub buttons: Buttons,
    /// `Field_Map_Index`.
    pub map_index: u16,
    /// The party leader.
    pub state: &'a FieldState,
    /// The second party member's facing and position, when there is one.
    pub follower: Option<(Direction, PixelPos)>,
    /// The collision type of the cell being stood on.
    pub standing: u8,
    /// The previous frame's standing collision, for the saved-standing column.
    pub previously_standing: u8,
    /// Collision of the cells left/up/right/down, in that order — the order
    /// `UpdateCharacterCollision` caches them.
    pub neighbours: [u8; 4],
    /// Persistent state.
    pub game: &'a GameState,
    /// The map's objects, in slot order.
    pub objects: &'a [ObjectSample],
    /// FG `Camera_*_Pos` in pixels and `Camera_*_Step_Counter` in 16.16.
    pub camera: (i32, i32, i32, i32),
    /// BG `Camera_*_Pos` in pixels and `Camera_*_Step_Counter` in 16.16.
    pub camera_bg: (i32, i32, i32, i32),
    /// The oracle's camera gate columns `$EC24` and `$EC25`.
    pub camera_gates: (u8, u8),
}

impl ReplayRow {
    /// Builds a row from one frame's engine state.
    #[must_use]
    pub fn from_sample(sample: FrameSample<'_>) -> ReplayRow {
        let FrameSample {
            frame,
            mark,
            buttons,
            map_index,
            state,
            follower,
            standing,
            previously_standing,
            neighbours,
            game,
            objects,
            camera,
            camera_bg,
            camera_gates,
        } = sample;
        let (cam_x, cam_y, cam_step_x, cam_step_y) = camera;
        let (cam_x_bg, cam_y_bg, cam_step_x_bg, cam_step_y_bg) = camera_bg;
        let (gate_ec24, gate_ec25) = camera_gates;
        let at = PixelPos::from_cell(state.cell());
        let (dx, dy) = state.render_offset_16ths();
        let (x_dur, y_dur) = state.step_durations_8_8();
        let destination = state.step_destination().map_or(at, PixelPos::from_cell);
        let snapshot = game.snapshot();
        let mut eflags = [0u32; 8];
        for (index, slot) in eflags.iter_mut().enumerate() {
            *slot = be_u32(&snapshot.event_flags[index * 4..]);
        }

        ReplayRow {
            frame,
            mark: mark.map(str::to_string),
            buttons: buttons.to_tape(),
            map_index,
            cam_x,
            cam_y,
            cam_step_x,
            cam_step_y,
            cam_x_bg,
            cam_y_bg,
            cam_step_x_bg,
            cam_step_y_bg,
            gate_ec24,
            gate_ec25,
            c1_facing: facing_value(state.facing()),
            c1_x_step_dur: x_dur,
            c1_y_step_dur: y_dur,
            c1_x_px: at.x + dx,
            c1_y_px: at.y + dy,
            c1_dest_x: destination.x,
            c1_dest_y: destination.y,
            c2_facing: follower.map_or(0, |(facing, _)| facing_value(facing)),
            c2_x_px: follower.map_or(0, |(_, at)| at.x),
            c2_y_px: follower.map_or(0, |(_, at)| at.y),
            coll_standing: standing,
            coll_saved_standing: previously_standing,
            coll_left: neighbours[0],
            coll_up: neighbours[1],
            coll_right: neighbours[2],
            coll_down: neighbours[3],
            party_slots: be_u32(&snapshot.party),
            party: snapshot.party,
            eflags,
            // These three column names come from the disassembly's labels, and
            // two of those labels are wrong about their address. What each
            // column actually reads:
            //
            //   ext_eflags_00   $F120 -- the event bank's upper half, which is
            //                   also where chest flags live
            //   chest_flags_00  $F140 -- the temp bank, despite the name
            //   temp_eflags_00  $F156 -- byte 22 of that same $F140 bank
            //
            // The names are the oracle's and stay as they are; the mapping is
            // what had to be corrected.
            ext_eflags_00: be_u32(&snapshot.event_flags[32..]),
            chest_flags_00: be_u32(&snapshot.temp_flags),
            temp_eflags_00: (u16::from(snapshot.temp_flags[22]) << 8)
                | u16::from(snapshot.temp_flags[23]),
            town_flags_00: be_u32(&snapshot.town_flags),
            objects: objects.to_vec(),
        }
    }

    /// The value of one modelled column, formatted as the oracle formats it.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<String> {
        let value = match name {
            "frame" => self.frame.to_string(),
            "mark" => self.mark.clone().unwrap_or_default(),
            "buttons" => self.buttons.clone(),
            "map_index" => format!("{:04X}", self.map_index),
            "c1_facing" => self.c1_facing.to_string(),
            "c1_x_step_dur" => self.c1_x_step_dur.to_string(),
            "c1_y_step_dur" => self.c1_y_step_dur.to_string(),
            "c1_x_px" => self.c1_x_px.to_string(),
            "c1_y_px" => self.c1_y_px.to_string(),
            "c1_dest_x" => self.c1_dest_x.to_string(),
            "c1_dest_y" => self.c1_dest_y.to_string(),
            "c2_facing" => self.c2_facing.to_string(),
            "c2_x_px" => self.c2_x_px.to_string(),
            "c2_y_px" => self.c2_y_px.to_string(),
            "coll_standing" => format!("{:02X}", self.coll_standing),
            "coll_saved_standing" => format!("{:02X}", self.coll_saved_standing),
            "coll_left" => format!("{:02X}", self.coll_left),
            "coll_up" => format!("{:02X}", self.coll_up),
            "coll_right" => format!("{:02X}", self.coll_right),
            "coll_down" => format!("{:02X}", self.coll_down),
            "cam_x_fg_px" => self.cam_x.to_string(),
            "cam_y_fg_px" => self.cam_y.to_string(),
            "cam_step_x_fg" => format!("{:08X}", self.cam_step_x),
            "cam_step_y_fg" => format!("{:08X}", self.cam_step_y),
            "cam_x_bg_px" => self.cam_x_bg.to_string(),
            "cam_y_bg_px" => self.cam_y_bg.to_string(),
            "cam_step_x_bg" => format!("{:08X}", self.cam_step_x_bg),
            "cam_step_y_bg" => format!("{:08X}", self.cam_step_y_bg),
            "gate_ec24" => format!("{:02X}", self.gate_ec24),
            "gate_ec25" => format!("{:02X}", self.gate_ec25),
            "party_slots" => format!("{:08X}", self.party_slots),
            "party_slot_1" => self.party[0].to_string(),
            "party_slot_2" => self.party[1].to_string(),
            "party_slot_3" => self.party[2].to_string(),
            "party_slot_4" => self.party[3].to_string(),
            "party_slot_5" => self.party[4].to_string(),
            "eflags_00" => format!("{:08X}", self.eflags[0]),
            "eflags_04" => format!("{:08X}", self.eflags[1]),
            "eflags_08" => format!("{:08X}", self.eflags[2]),
            "eflags_0C" => format!("{:08X}", self.eflags[3]),
            "eflags_10" => format!("{:08X}", self.eflags[4]),
            "eflags_14" => format!("{:08X}", self.eflags[5]),
            "eflags_18" => format!("{:08X}", self.eflags[6]),
            "eflags_1C" => format!("{:08X}", self.eflags[7]),
            "ext_eflags_00" => format!("{:08X}", self.ext_eflags_00),
            "chest_flags_00" => format!("{:08X}", self.chest_flags_00),
            "temp_eflags_00" => format!("{:04X}", self.temp_eflags_00),
            "town_flags_00" => format!("{:08X}", self.town_flags_00),
            name => {
                // Object columns: `oNN_kind`, slot-aligned with the map's NPCs.
                let rest = name.strip_prefix('o')?;
                let (slot, kind) = rest.split_once('_')?;
                let slot: usize = slot.parse().ok()?;
                return self.objects.get(slot).and_then(|obj| obj.field(kind));
            }
        };
        Some(value)
    }

    /// The row as a CSV line over `columns`.
    #[must_use]
    pub fn to_csv(&self, columns: &[&str]) -> String {
        columns
            .iter()
            .map(|name| self.field(name).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Direction;

    #[test]
    fn facing_values_are_the_cartridges() {
        assert_eq!(facing_value(Direction::Down), 0x0);
        assert_eq!(facing_value(Direction::Up), 0x4);
        assert_eq!(facing_value(Direction::Right), 0x8);
        assert_eq!(facing_value(Direction::Left), 0xC);
    }
}
