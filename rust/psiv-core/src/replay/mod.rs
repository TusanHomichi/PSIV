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
//!
//! # Layout
//!
//! Split out of a single 1225-line file. The seams are the ones the comparator
//! already had:
//!
//! - [`tape`] — the input side: buttons, steps, marks. No engine types.
//! - [`columns`] — the column table and coverage classification.
//! - [`row`] — a frame of engine state, frozen into the oracle's shape.
//! - [`oracle`] — the hardware log and the diff against it.
//!
//! Every public name is re-exported here, so `psiv_core::replay::X` and
//! `psiv_core::X` both keep working exactly as before the split.

mod columns;
mod oracle;
mod row;
mod tape;

pub use columns::{
    COLUMNS, Column, Coverage, OBJECT_COLUMN_KINDS, OBJECT_ID_LOADED, OBJECT_SLOTS,
    all_modelled_columns, csv_header, modelled_columns, object_column, object_columns,
};
pub use oracle::{DiffReport, Divergence, OracleLog};
pub use row::{FrameSample, ObjectSample, ReplayRow, facing_value};
pub use tape::{Buttons, Tape, TapeError, TapeFrame, TapeStep};
