//! The column table: every column the oracle log can carry, and whether the
//! engine models it.
//!
//! This is the comparator's contract with the oracle. A column named here and
//! marked [`Coverage::Modelled`] is one the engine promises a value for and
//! that a diff will compare; a [`Coverage::NotModelled`] column carries the
//! reason it is exempt. Nothing may be silently absent — that is how a false
//! pass happens, and it happened once already.

/// Whether the engine models an oracle column, and if not, why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// The engine produces this and it is compared.
    Modelled,
    /// The engine does not produce it. The reason is emitted in the header so
    /// a blank column is never mistaken for a zero.
    NotModelled(&'static str),
}

/// One oracle CSV column and its coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// The oracle's column name.
    pub name: &'static str,
    /// Whether this crate can produce it.
    pub coverage: Coverage,
}

const fn modelled(name: &'static str) -> Column {
    Column {
        name,
        coverage: Coverage::Modelled,
    }
}

const fn missing(name: &'static str, why: &'static str) -> Column {
    Column {
        name,
        coverage: Coverage::NotModelled(why),
    }
}

/// Every column the oracle logs, with what this engine can say about it.
///
/// Declared rather than silently zeroed: a column the engine does not model is
/// emitted empty and named in the output header, so a diff cannot mistake
/// "no opinion" for "zero".
pub const COLUMNS: &[Column] = &[
    modelled("frame"),
    modelled("mark"),
    modelled("buttons"),
    missing("game_mode", "no boot/menu state machine"),
    missing("game_mode_routine", "no boot/menu state machine"),
    missing(
        "routine_exit_flags",
        "dispatcher plumbing, not engine state",
    ),
    modelled("map_index"),
    missing("map_index_2", "the previous-map word is bridge state"),
    missing("world_index", "planet id is pack data, not engine state"),
    missing("event_index", "the runtime owns event dispatch"),
    missing(
        "field_move_flags",
        "Char_Move_Flags is scene/presentation state",
    ),
    missing("step_offset", "FieldObj_Step_Offset is a speed selector"),
    missing("joy_held", "raw joypad bytes are the host's"),
    missing("joy_pressed", "raw joypad bytes are the host's"),
    missing("btn_held_frames", "input repeat timers are the host's"),
    missing("btn_held_timer", "input repeat timers are the host's"),
    modelled("c1_facing"),
    missing("c1_mappings_idx", "sprite animation is the renderer's"),
    modelled("c1_x_step_dur"),
    modelled("c1_y_step_dur"),
    modelled("c1_x_px"),
    modelled("c1_y_px"),
    missing(
        "c1_x_sub",
        "sub-pixel fraction; the engine steps whole pixels",
    ),
    missing(
        "c1_y_sub",
        "sub-pixel fraction; the engine steps whole pixels",
    ),
    modelled("c1_dest_x"),
    modelled("c1_dest_y"),
    modelled("c2_facing"),
    modelled("c2_x_px"),
    modelled("c2_y_px"),
    modelled("coll_standing"),
    modelled("coll_saved_standing"),
    missing("coll_shop", "written by the interaction probe, not stored"),
    modelled("coll_left"),
    modelled("coll_up"),
    modelled("coll_right"),
    modelled("coll_down"),
    // The camera, now that the oracle logs it. These are the gate's inputs, so
    // comparing them localises a visibility divergence to the camera itself
    // rather than to the objects it froze.
    modelled("cam_x_fg_px"),
    modelled("cam_y_fg_px"),
    modelled("cam_step_x_fg"),
    modelled("cam_step_y_fg"),
    missing(
        "cam_x_fg",
        "the 16.16 longword; the engine compares its pixel view instead",
    ),
    missing("cam_y_fg", "the 16.16 longword"),
    missing(
        "cam_x_bg",
        "no BG plane camera; FG and BG agree in field control",
    ),
    missing("cam_y_bg", "no BG plane camera"),
    missing("cam_x_bg_px", "no BG plane camera"),
    missing("cam_y_bg_px", "no BG plane camera"),
    missing("cam_step_x_bg", "no BG plane camera"),
    missing("cam_step_y_bg", "no BG plane camera"),
    missing("map_row_size_fg", "a VDP plane dimension, not engine state"),
    missing("map_col_size_fg", "a VDP plane dimension"),
    missing("map_row_size_bg", "a VDP plane dimension"),
    missing("map_col_size_bg", "a VDP plane dimension"),
    missing(
        "gate_ec24",
        "unnamed RAM; plane select is a hypothesis, not a fact",
    ),
    missing(
        "gate_ec25",
        "unnamed RAM; camera-driver enable is a hypothesis",
    ),
    missing(
        "c1_x_step_const",
        "velocity is derived from successive positions",
    ),
    missing(
        "c1_y_step_const",
        "velocity is derived from successive positions",
    ),
    missing("window_index", "windows are the renderer's"),
    missing("window_saved_index", "windows are the renderer's"),
    missing("windows_opened", "windows are the renderer's"),
    missing("window_init_flag", "windows are the renderer's"),
    missing("window_render_mode", "windows are the renderer's"),
    missing("window_option_idx", "windows are the renderer's"),
    missing("win_char_num", "windows are the renderer's"),
    missing("arrow_offscreen", "windows are the renderer's"),
    missing("arrow_x_px", "windows are the renderer's"),
    missing("arrow_y_px", "windows are the renderer's"),
    missing(
        "interaction_evt_flag",
        "interaction routing is the runtime's",
    ),
    missing(
        "interaction_evt_type",
        "interaction routing is the runtime's",
    ),
    modelled("party_slots"),
    modelled("party_slot_1"),
    modelled("party_slot_2"),
    modelled("party_slot_3"),
    modelled("party_slot_4"),
    modelled("party_slot_5"),
    missing("saved_char_x", "written by the battle/reload path"),
    missing("saved_char_y", "written by the battle/reload path"),
    modelled("eflags_00"),
    modelled("eflags_04"),
    modelled("eflags_08"),
    modelled("eflags_0C"),
    modelled("eflags_10"),
    modelled("eflags_14"),
    modelled("eflags_18"),
    modelled("eflags_1C"),
    modelled("ext_eflags_00"),
    modelled("chest_flags_00"),
    modelled("temp_eflags_00"),
    modelled("town_flags_00"),
    missing("rng_seed", "the cartridge RNG is not ported yet"),
];

/// How many secondary object slots the oracle logs.
///
/// Slot `i` is `npcs[i]` of the packed map record, and the logged `id` is
/// `$8000 | pack id` — the high bit is the object-loaded marker.
pub const OBJECT_SLOTS: usize = 32;

/// The eleven columns each object slot contributes, and whether the engine
/// models them.
pub const OBJECT_COLUMN_KINDS: [(&str, Coverage); 12] = [
    ("off", Coverage::Modelled),
    ("id", Coverage::Modelled),
    (
        "rflags",
        Coverage::NotModelled("render flags beyond bit 3 are presentation state"),
    ),
    ("facing", Coverage::Modelled),
    (
        "map_idx",
        Coverage::NotModelled("the object's map slot is bridge bookkeeping"),
    ),
    ("timer", Coverage::Modelled),
    ("xdur", Coverage::Modelled),
    ("ydur", Coverage::Modelled),
    ("x_px", Coverage::Modelled),
    ("y_px", Coverage::Modelled),
    ("xbnd", Coverage::Modelled),
    ("ybnd", Coverage::Modelled),
];

/// The high bit the oracle's object id carries.
pub const OBJECT_ID_LOADED: u16 = 0x8000;

/// The oracle's name for one object column, e.g. `o07_timer`.
#[must_use]
pub fn object_column(slot: usize, kind: &str) -> String {
    format!("o{slot:02}_{kind}")
}

/// Every object column, in the oracle's order.
#[must_use]
pub fn object_columns() -> Vec<Column> {
    let mut out = Vec::with_capacity(OBJECT_SLOTS * OBJECT_COLUMN_KINDS.len());
    for slot in 0..OBJECT_SLOTS {
        for (kind, coverage) in OBJECT_COLUMN_KINDS {
            out.push(Column {
                name: Box::leak(object_column(slot, kind).into_boxed_str()),
                coverage,
            });
        }
    }
    out
}

/// The names this engine emits values for.
#[must_use]
pub fn modelled_columns() -> Vec<&'static str> {
    COLUMNS
        .iter()
        .filter(|c| c.coverage == Coverage::Modelled)
        .map(|c| c.name)
        .collect()
}

/// The scalar modelled columns plus every modelled object column.
#[must_use]
pub fn all_modelled_columns() -> Vec<&'static str> {
    let mut out = modelled_columns();
    out.extend(
        object_columns()
            .into_iter()
            .filter(|c| c.coverage == Coverage::Modelled)
            .map(|c| c.name),
    );
    out
}

/// The header a replay CSV opens with: the modelled columns, preceded by
/// comment lines naming every column the engine does not model and why.
#[must_use]
pub fn csv_header() -> String {
    let mut out = String::from("# psiv-core replay; columns below are engine-produced\n");
    for column in COLUMNS {
        if let Coverage::NotModelled(why) = column.coverage {
            out.push_str(&format!("# not modelled: {} - {}\n", column.name, why));
        }
    }
    for column in object_columns() {
        if let Coverage::NotModelled(why) = column.coverage {
            out.push_str(&format!("# not modelled: {} - {}\n", column.name, why));
        }
    }
    out.push_str(&all_modelled_columns().join(","));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_table_covers_the_oracles_log_exactly() {
        // Every column the oracle harness can emit, pinned so a new RAM-map
        // column cannot slip through unclassified: this table has to name each
        // one and say whether the engine models it.
        //
        // Membership is asserted, not order. The harness emits column *groups*
        // selected per run (`--groups core,pos,collision,objects,camera`), so
        // the order and even the presence of a group varies between logs while
        // the classification obligation does not. Comparing sorted names keeps
        // the check meaningful without failing every time the oracle re-logs a
        // tape with a different group set.
        const ORACLE_HEADER: &str = "frame,mark,buttons,game_mode,game_mode_routine,routine_exit_flags,map_index,map_index_2,world_index,event_index,field_move_flags,step_offset,joy_held,joy_pressed,btn_held_frames,btn_held_timer,c1_facing,c1_mappings_idx,c1_x_step_dur,c1_y_step_dur,c1_x_px,c1_y_px,c1_x_sub,c1_y_sub,c1_dest_x,c1_dest_y,c2_facing,c2_x_px,c2_y_px,coll_standing,coll_saved_standing,coll_shop,coll_left,coll_up,coll_right,coll_down,cam_y_fg,cam_y_fg_px,cam_x_fg,cam_x_fg_px,cam_y_bg,cam_y_bg_px,cam_x_bg,cam_x_bg_px,cam_step_x_fg,cam_step_y_fg,cam_step_x_bg,cam_step_y_bg,map_row_size_fg,map_col_size_fg,map_row_size_bg,map_col_size_bg,gate_ec24,gate_ec25,c1_x_step_const,c1_y_step_const,window_index,window_saved_index,windows_opened,window_init_flag,window_render_mode,window_option_idx,win_char_num,arrow_offscreen,arrow_x_px,arrow_y_px,interaction_evt_flag,interaction_evt_type,party_slots,party_slot_1,party_slot_2,party_slot_3,party_slot_4,party_slot_5,saved_char_x,saved_char_y,eflags_00,eflags_04,eflags_08,eflags_0C,eflags_10,eflags_14,eflags_18,eflags_1C,ext_eflags_00,chest_flags_00,temp_eflags_00,town_flags_00,rng_seed";

        let mut names: Vec<&str> = COLUMNS.iter().map(|c| c.name).collect();
        let mut oracle: Vec<&str> = ORACLE_HEADER.split(',').collect();
        names.sort_unstable();
        oracle.sort_unstable();
        assert_eq!(names, oracle, "column table drifted from the oracle log");

        let modelled = modelled_columns();
        assert!(modelled.contains(&"c1_x_px"));
        assert!(modelled.contains(&"c1_x_step_dur"));
        assert!(!modelled.contains(&"rng_seed"));
        assert!(!modelled.contains(&"game_mode"));
    }

    #[test]
    fn the_header_names_every_unmodelled_column() {
        let header = csv_header();
        for column in COLUMNS {
            if let Coverage::NotModelled(why) = column.coverage {
                assert!(
                    header.contains(column.name) && header.contains(why),
                    "{} should be declared with its reason",
                    column.name
                );
            }
        }
        assert!(
            header
                .lines()
                .last()
                .unwrap()
                .starts_with("frame,mark,buttons,"),
            "the last header line is the column row"
        );
    }
}
