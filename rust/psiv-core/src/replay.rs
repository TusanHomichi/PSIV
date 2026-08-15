//! Tape replay: driving the engine from the oracle's input tapes and emitting
//! its CSV columns, so field behaviour can be diffed frame by frame against
//! real hardware.
//!
//! Everything here is pure — tapes parse from `&str`, rows format to `String`,
//! oracle logs parse from `&str`. The file handling lives in the thin driver
//! that calls this, which keeps the comparator's contract inside the
//! deterministic core where it can be unit-tested without fixtures.
//!
//! # The alignment contract
//!
//! An oracle tape starts at **power-on** and the engine cannot: there is no
//! title screen, no `Event_GameStart` execution, no boot path. The first
//! several thousand frames of every tape are therefore unreplayable by
//! construction, not by omission.
//!
//! So a replay declares where the engine picks the tape up:
//!
//! - **By mark** (preferred): `--align-mark settle` starts the engine on the
//!   frame the oracle logged that mark. Marks are stable across tape edits in
//!   a way raw frame numbers are not.
//! - **By frame**: `--align-frame 6456` for a tape with no usable mark.
//!
//! From that frame on, engine frame *n* is compared against oracle frame *n*.
//! The engine's starting state comes from the pack's `game_start` record — the
//! epilogue of the opening scene, which is exactly what the player is handed
//! when it ends — and the comparator asserts that state matches the oracle's
//! row at the alignment frame before it compares anything else. A mismatch
//! there means the pack and the tape disagree about where the game begins, and
//! nothing after it would be meaningful.
//!
//! Frames before the alignment point are skipped, not replayed.

use core::fmt;
use std::collections::BTreeMap;

use crate::field::{FieldState, Input};
use crate::geom::Direction;
use crate::state::{GameState, PARTY_SLOTS};
use crate::trigger::PixelPos;

/// The four directions plus the four Mega Drive buttons, as a tape spells them.
///
/// The mapping is not the obvious one and the oracle's own notes call it a
/// trap: Genesis **C** is `ButtonSpeak` (talk), **B** is `ButtonCancel` (inert
/// in field control), **A** is `ButtonCamp`. Bit order here is the tape's
/// letter order `UDLRABCS`, not the joypad's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Buttons(u8);

impl Buttons {
    /// Nothing held.
    pub const NONE: Buttons = Buttons(0);

    const UP: u8 = 1 << 0;
    const DOWN: u8 = 1 << 1;
    const LEFT: u8 = 1 << 2;
    const RIGHT: u8 = 1 << 3;
    /// Genesis A — `ButtonCamp`.
    const A: u8 = 1 << 4;
    /// Genesis B — `ButtonCancel`, inert in field control.
    const B: u8 = 1 << 5;
    /// Genesis C — `ButtonSpeak`, the talk button.
    const C: u8 = 1 << 6;
    const START: u8 = 1 << 7;

    /// Parses a tape's button field: `.` for nothing, else letters from
    /// `UDLRABCS`.
    ///
    /// # Errors
    ///
    /// [`TapeError::BadButton`] for any other character.
    pub fn parse(text: &str) -> Result<Buttons, TapeError> {
        if text == "." {
            return Ok(Buttons::NONE);
        }
        let mut bits = 0;
        for ch in text.chars() {
            bits |= match ch {
                'U' => Buttons::UP,
                'D' => Buttons::DOWN,
                'L' => Buttons::LEFT,
                'R' => Buttons::RIGHT,
                'A' => Buttons::A,
                'B' => Buttons::B,
                'C' => Buttons::C,
                'S' => Buttons::START,
                other => return Err(TapeError::BadButton(other)),
            };
        }
        Ok(Buttons(bits))
    }

    /// The tape spelling, for round-tripping into a log's `buttons` column.
    #[must_use]
    pub fn to_tape(self) -> String {
        if self.0 == 0 {
            return ".".to_string();
        }
        let mut out = String::new();
        for (bit, ch) in [
            (Buttons::UP, 'U'),
            (Buttons::DOWN, 'D'),
            (Buttons::LEFT, 'L'),
            (Buttons::RIGHT, 'R'),
            (Buttons::A, 'A'),
            (Buttons::B, 'B'),
            (Buttons::C, 'C'),
            (Buttons::START, 'S'),
        ] {
            if self.0 & bit != 0 {
                out.push(ch);
            }
        }
        out
    }

    /// The direction held, if any.
    ///
    /// A tape may in principle hold two; the engine takes one per tick, so the
    /// order here is a declared tie-break rather than a discovered rule. No
    /// retail field tape holds two at once.
    #[must_use]
    pub const fn direction(self) -> Option<Direction> {
        if self.0 & Buttons::UP != 0 {
            Some(Direction::Up)
        } else if self.0 & Buttons::DOWN != 0 {
            Some(Direction::Down)
        } else if self.0 & Buttons::LEFT != 0 {
            Some(Direction::Left)
        } else if self.0 & Buttons::RIGHT != 0 {
            Some(Direction::Right)
        } else {
            None
        }
    }

    /// Whether the talk button is held. Genesis **C**, not B.
    #[must_use]
    pub const fn talk(self) -> bool {
        self.0 & Buttons::C != 0
    }

    /// The engine input for this frame.
    ///
    /// Talk wins over a direction, matching the cartridge: the two are read
    /// through different paths and `FieldControls_GetInput` hands the frame to
    /// the interaction routine before the movement code runs.
    #[must_use]
    pub const fn to_input(self) -> Input {
        if self.talk() {
            return Input::Action;
        }
        match self.direction() {
            Some(dir) => Input::Direction(dir),
            None => Input::Neutral,
        }
    }
}

/// Why a tape would not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapeError {
    /// A button letter outside `UDLRABCS`.
    BadButton(char),
    /// A frame count that was not a positive integer.
    BadFrameCount(String),
    /// A line with too few fields.
    Malformed(String),
    /// `end` without a `repeat`, or a `repeat` inside a `repeat`.
    BadRepeat(String),
    /// A `repeat` block that never closed.
    UnclosedRepeat,
}

impl fmt::Display for TapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapeError::BadButton(ch) => write!(f, "{ch:?} is not one of UDLRABCS"),
            TapeError::BadFrameCount(text) => write!(f, "{text:?} is not a frame count"),
            TapeError::Malformed(line) => write!(f, "malformed tape line: {line:?}"),
            TapeError::BadRepeat(line) => write!(f, "bad repeat structure at {line:?}"),
            TapeError::UnclosedRepeat => write!(f, "a repeat block was never closed"),
        }
    }
}

impl core::error::Error for TapeError {}

/// One step: a button state held for some frames, optionally labelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeStep {
    /// How many consecutive frames, at least 1.
    pub frames: u32,
    /// What is held.
    pub buttons: Buttons,
    /// A label, recorded on the step's first frame.
    pub mark: Option<String>,
}

/// One frame of a replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeFrame {
    /// 1-based, counting from power-on exactly as the oracle's logs do.
    pub number: u32,
    /// The mark, on a step's first frame only.
    pub mark: Option<String>,
    /// What is held.
    pub buttons: Buttons,
}

/// A parsed tape.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tape {
    steps: Vec<TapeStep>,
}

impl Tape {
    /// Parses tape text.
    ///
    /// Blank lines and `#` comments are ignored; `repeat <n>` / `end` blocks
    /// expand in place and do not nest.
    ///
    /// # Errors
    ///
    /// See [`TapeError`].
    pub fn parse(text: &str) -> Result<Tape, TapeError> {
        let mut steps = Vec::new();
        let mut repeat: Option<(u32, Vec<TapeStep>)> = None;

        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            if let Some(count) = line.strip_prefix("repeat ") {
                if repeat.is_some() {
                    return Err(TapeError::BadRepeat(line.to_string()));
                }
                let count = count
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| TapeError::BadFrameCount(count.trim().to_string()))?;
                repeat = Some((count, Vec::new()));
                continue;
            }
            if line == "end" {
                let Some((count, body)) = repeat.take() else {
                    return Err(TapeError::BadRepeat(line.to_string()));
                };
                for _ in 0..count {
                    steps.extend(body.iter().cloned());
                }
                continue;
            }

            let mut fields = line.split_whitespace();
            let (Some(frames), Some(buttons)) = (fields.next(), fields.next()) else {
                return Err(TapeError::Malformed(line.to_string()));
            };
            let frames = frames
                .parse::<u32>()
                .map_err(|_| TapeError::BadFrameCount(frames.to_string()))?;
            if frames == 0 {
                return Err(TapeError::BadFrameCount(frames.to_string()));
            }
            let step = TapeStep {
                frames,
                buttons: Buttons::parse(buttons)?,
                mark: fields.next().map(str::to_string),
            };
            match repeat.as_mut() {
                Some((_, body)) => body.push(step),
                None => steps.push(step),
            }
        }

        if repeat.is_some() {
            return Err(TapeError::UnclosedRepeat);
        }
        Ok(Tape { steps })
    }

    /// The steps, expanded.
    #[must_use]
    pub fn steps(&self) -> &[TapeStep] {
        &self.steps
    }

    /// Total frames.
    #[must_use]
    pub fn frame_count(&self) -> u32 {
        self.steps.iter().map(|step| step.frames).sum()
    }

    /// Every frame, numbered from 1 as the oracle numbers `retro_run()` calls.
    #[must_use]
    pub fn frames(&self) -> Vec<TapeFrame> {
        let mut out = Vec::with_capacity(self.frame_count() as usize);
        let mut number = 1;
        for step in &self.steps {
            for index in 0..step.frames {
                out.push(TapeFrame {
                    number,
                    mark: if index == 0 { step.mark.clone() } else { None },
                    buttons: step.buttons,
                });
                number += 1;
            }
        }
        out
    }

    /// The frame a mark lands on.
    #[must_use]
    pub fn mark_frame(&self, mark: &str) -> Option<u32> {
        let mut number = 1;
        for step in &self.steps {
            if step.mark.as_deref() == Some(mark) {
                return Some(number);
            }
            number += step.frames;
        }
        None
    }
}

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
/// assembling these field by field and a positional call of nine would be easy
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
    /// `Camera_*_Pos_FG` in pixels and `Camera_*_Step_Counter_FG` in 16.16.
    pub camera: (i32, i32, i32, i32),
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
        } = sample;
        let (cam_x, cam_y, cam_step_x, cam_step_y) = camera;
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
            ext_eflags_00: be_u32(&snapshot.event_flags[32..]),
            chest_flags_00: be_u32(&snapshot.chest_flags),
            // The oracle reads this at $F156, which is byte 22 of the $F140
            // bank ($F156 - $F140 = $16). Retail has no separate temp bank, so
            // the column is a window into the chest bytes rather than an array
            // of its own.
            temp_eflags_00: (u16::from(snapshot.chest_flags[22]) << 8)
                | u16::from(snapshot.chest_flags[23]),
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

/// The outcome of a comparison: what disagreed, and over which columns.
#[derive(Debug, Clone)]
pub struct DiffReport {
    /// Every disagreement, ordered by frame then column.
    pub divergences: Vec<Divergence>,
    /// Columns both sides carried, so a clean result over them means something.
    pub compared: Vec<&'static str>,
    /// Columns the engine emits that the oracle log does not carry. These were
    /// not compared, and reporting them as agreement is how a false pass
    /// happens — a whole column group once vanished into a silent `continue`
    /// and the run announced itself clean over columns it had never read.
    pub unavailable: Vec<&'static str>,
}

/// One frame's disagreement between engine and oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// The oracle frame it happened on.
    pub frame: u32,
    /// Which column.
    pub column: String,
    /// What the engine said.
    pub engine: String,
    /// What the hardware said.
    pub oracle: String,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frame {}: {} engine={} oracle={}",
            self.frame, self.column, self.engine, self.oracle
        )
    }
}

/// An oracle log, indexed by frame.
///
/// Both indexes are built at parse time. A linear scan per lookup is fine at
/// thirty-odd columns and quadratic at three hundred — the object group made
/// that difference the gap between a second and an afternoon.
#[derive(Debug, Clone, Default)]
pub struct OracleLog {
    rows: Vec<Vec<String>>,
    by_frame: BTreeMap<u32, usize>,
    by_column: BTreeMap<String, usize>,
}

impl OracleLog {
    /// Parses an oracle CSV: `#` comment lines, then a header row, then data.
    #[must_use]
    pub fn parse(text: &str) -> OracleLog {
        let mut header = Vec::new();
        let mut rows = Vec::new();
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let fields: Vec<String> = line.split(',').map(str::to_string).collect();
            if header.is_empty() {
                header = fields;
            } else {
                rows.push(fields);
            }
        }
        let by_column = header
            .iter()
            .enumerate()
            .map(|(index, name)| (name.clone(), index))
            .collect();
        let frame_index = header.iter().position(|name| name == "frame");
        let by_frame = frame_index
            .map(|frame_index| {
                rows.iter()
                    .enumerate()
                    .filter_map(|(row, fields)| {
                        let frame = fields.get(frame_index)?.parse::<u32>().ok()?;
                        Some((frame, row))
                    })
                    .collect()
            })
            .unwrap_or_default();
        OracleLog {
            rows,
            by_frame,
            by_column,
        }
    }

    /// The value of `column` on `frame`, if the log has both.
    #[must_use]
    pub fn get(&self, frame: u32, column: &str) -> Option<&str> {
        let column = *self.by_column.get(column)?;
        let row = *self.by_frame.get(&frame)?;
        self.rows.get(row)?.get(column).map(String::as_str)
    }

    /// How many data rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the log has no data rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Compares engine rows against this log over the modelled columns.
    ///
    /// `skip` names columns to leave out of the comparison — the caller's
    /// escape hatch for a column known to diverge for a documented reason,
    /// rather than this module quietly excluding it.
    ///
    /// Returns divergences in frame order, first one first.
    #[must_use]
    pub fn diff(&self, rows: &[ReplayRow], skip: &[&str]) -> Vec<Divergence> {
        self.compare(rows, skip).divergences
    }

    /// The full comparison: what diverged, and what was actually looked at.
    ///
    /// A column both sides carry is compared; one the oracle log does not carry
    /// is *unavailable*, not clean. Keeping the two apart is the difference
    /// between a verdict and a false pass — an engine column the log never had
    /// would otherwise vanish into a silent `continue` and be counted as
    /// agreement.
    #[must_use]
    pub fn compare(&self, rows: &[ReplayRow], skip: &[&str]) -> DiffReport {
        let mut divergences = Vec::new();
        let mut compared = Vec::new();
        let mut unavailable = Vec::new();
        for column in all_modelled_columns() {
            if skip.contains(&column) || column == "mark" || column == "frame" {
                continue;
            }
            let mut seen = false;
            for row in rows {
                let (Some(engine), Some(oracle)) = (row.field(column), self.get(row.frame, column))
                else {
                    continue;
                };
                seen = true;
                if engine != oracle {
                    divergences.push(Divergence {
                        frame: row.frame,
                        column: column.to_string(),
                        engine,
                        oracle: oracle.to_string(),
                    });
                }
            }
            if seen {
                compared.push(column);
            } else {
                unavailable.push(column);
            }
        }
        divergences.sort_by(|a, b| a.frame.cmp(&b.frame).then(a.column.cmp(&b.column)));
        DiffReport {
            divergences,
            compared,
            unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_parse_and_round_trip() {
        assert_eq!(Buttons::parse(".").unwrap(), Buttons::NONE);
        assert_eq!(Buttons::parse(".").unwrap().to_tape(), ".");
        let held = Buttons::parse("DC").unwrap();
        assert_eq!(held.direction(), Some(Direction::Down));
        assert!(held.talk());
        assert_eq!(held.to_tape(), "DC");
        assert_eq!(Buttons::parse("X"), Err(TapeError::BadButton('X')));
    }

    #[test]
    fn genesis_c_is_the_talk_button_not_b() {
        // The oracle's own notes call this a trap: C is ButtonSpeak, B is
        // ButtonCancel and does nothing in field control.
        assert_eq!(Buttons::parse("C").unwrap().to_input(), Input::Action);
        assert_eq!(Buttons::parse("B").unwrap().to_input(), Input::Neutral);
        assert_eq!(Buttons::parse("A").unwrap().to_input(), Input::Neutral);
    }

    #[test]
    fn talk_wins_over_a_direction() {
        assert_eq!(Buttons::parse("RC").unwrap().to_input(), Input::Action);
        assert_eq!(
            Buttons::parse("R").unwrap().to_input(),
            Input::Direction(Direction::Right)
        );
    }

    #[test]
    fn a_tape_expands_repeat_blocks() {
        let tape = Tape::parse(
            "# a comment\n\
             2 . start\n\
             repeat 3\n\
             1 D\n\
             2 .\n\
             end\n\
             1 R done\n",
        )
        .unwrap();

        assert_eq!(tape.steps().len(), 1 + 3 * 2 + 1);
        assert_eq!(tape.frame_count(), 2 + 3 * 3 + 1);
        assert_eq!(tape.mark_frame("start"), Some(1));
        assert_eq!(tape.mark_frame("done"), Some(12));
        assert_eq!(tape.mark_frame("nope"), None);
    }

    #[test]
    fn frames_number_from_one_and_carry_marks_on_the_first_frame_only() {
        let tape = Tape::parse("3 D walk\n1 .\n").unwrap();
        let frames = tape.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].number, 1);
        assert_eq!(frames[0].mark.as_deref(), Some("walk"));
        assert_eq!(frames[1].mark, None, "only the first frame is marked");
        assert_eq!(frames[3].buttons, Buttons::NONE);
    }

    #[test]
    fn malformed_tapes_are_rejected() {
        assert!(matches!(
            Tape::parse("0 D\n"),
            Err(TapeError::BadFrameCount(_))
        ));
        assert!(matches!(Tape::parse("D\n"), Err(TapeError::Malformed(_))));
        assert!(matches!(Tape::parse("end\n"), Err(TapeError::BadRepeat(_))));
        assert!(matches!(
            Tape::parse("repeat 2\n1 .\n"),
            Err(TapeError::UnclosedRepeat)
        ));
        assert!(matches!(
            Tape::parse("repeat 2\nrepeat 2\n1 .\nend\nend\n"),
            Err(TapeError::BadRepeat(_))
        ));
    }

    #[test]
    fn facing_values_are_the_cartridges() {
        assert_eq!(facing_value(Direction::Down), 0x0);
        assert_eq!(facing_value(Direction::Up), 0x4);
        assert_eq!(facing_value(Direction::Right), 0x8);
        assert_eq!(facing_value(Direction::Left), 0xC);
    }

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

    #[test]
    fn an_oracle_log_is_indexed_by_frame_and_column() {
        let log = OracleLog::parse(
            "# provenance\n\
             frame,mark,buttons,c1_x_px\n\
             1,start,.,768\n\
             2,,D,770\n",
        );
        assert_eq!(log.len(), 2);
        assert_eq!(log.get(1, "c1_x_px"), Some("768"));
        assert_eq!(log.get(2, "buttons"), Some("D"));
        assert_eq!(log.get(3, "c1_x_px"), None);
        assert_eq!(log.get(1, "nonesuch"), None);
    }
}
