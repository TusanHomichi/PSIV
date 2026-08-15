//! The replay loop end to end, on synthetic data.
//!
//! Proves the comparator's machinery — tape to inputs, engine to rows, rows
//! against a log — without needing the pack or the oracle's own files. The
//! real diff against hardware is the `psiv-replay` binary's job.

mod common;

use common::map_with;
use psiv_core::{
    Buttons, COLUMNS, Cell, Coverage, Direction, FieldState, FrameSample, GameState, OracleLog,
    ReplayRow, StepFrames, Tape, csv_header, facing_value, modelled_columns,
};

/// Runs a tape through a bare `FieldState` on an open map and collects rows.
fn replay(tape_text: &str, from: u32) -> Vec<ReplayRow> {
    let map = map_with(
        &["........", "........", "........", "........"],
        vec![],
        vec![],
    );
    let mut state = FieldState::new(
        &map,
        Cell::new(1, 1),
        Direction::Down,
        StepFrames::default(),
    )
    .expect("valid placement");
    let game = GameState::new();
    let tape = Tape::parse(tape_text).expect("valid tape");

    let mut rows = Vec::new();
    let mut previous_standing = 0;
    for frame in tape.frames() {
        if frame.number < from {
            continue;
        }
        state.tick(&map, frame.buttons.to_input());
        let standing = map.collision_at(state.cell()).map_or(0, |t| t.to_raw());
        rows.push(ReplayRow::from_sample(FrameSample {
            frame: frame.number,
            mark: frame.mark.as_deref(),
            buttons: frame.buttons,
            map_index: 0x13,
            state: &state,
            follower: None,
            standing,
            previously_standing: previous_standing,
            neighbours: [0; 4],
            game: &game,
            objects: &[],
        }));
        previous_standing = standing;
    }
    rows
}

#[test]
fn a_tape_drives_the_engine_and_the_rows_track_the_walk() {
    // Eight frames of RIGHT is exactly one cell at the default step rate.
    let rows = replay("8 R walk\n8 .\n", 1);
    assert_eq!(rows.len(), 16);

    // The countdown ladder, in cartridge units, only on the walked axis.
    let durations: Vec<(u16, u16)> = rows[..8]
        .iter()
        .map(|r| (r.c1_x_step_dur, r.c1_y_step_dur))
        .collect();
    assert_eq!(
        durations,
        vec![
            (0x0E00, 0),
            (0x0C00, 0),
            (0x0A00, 0),
            (0x0800, 0),
            (0x0600, 0),
            (0x0400, 0),
            (0x0200, 0),
            (0, 0),
        ]
    );

    // Position advances two pixels a frame and lands a whole cell along.
    let xs: Vec<i32> = rows[..8].iter().map(|r| r.c1_x_px).collect();
    assert_eq!(xs, vec![18, 20, 22, 24, 26, 28, 30, 32]);
    assert_eq!(
        rows[15].c1_x_px, 32,
        "and stays there once the tape releases"
    );
    assert_eq!(rows[0].c1_facing, facing_value(Direction::Right));
}

#[test]
fn the_mark_lands_on_the_first_frame_of_its_step() {
    let rows = replay("4 R walk\n4 D turn\n", 1);
    assert_eq!(rows[0].mark.as_deref(), Some("walk"));
    assert_eq!(rows[1].mark, None);
    assert_eq!(rows[4].mark.as_deref(), Some("turn"));
}

#[test]
fn replaying_from_an_alignment_frame_skips_everything_before_it() {
    // The whole tape is 16 frames; picking it up at 9 replays the last 8.
    let all = replay("8 R\n8 R\n", 1);
    let aligned = replay("8 R\n8 R\n", 9);
    assert_eq!(all.len(), 16);
    assert_eq!(aligned.len(), 8);
    assert_eq!(aligned[0].frame, 9, "frame numbers stay the oracle's");
}

#[test]
fn a_clean_run_diffs_against_a_matching_log() {
    let rows = replay("8 R walk\n", 1);

    // Build a log that agrees with the engine, in the oracle's shape.
    let columns = modelled_columns();
    let mut text = String::from("# synthetic\n");
    text.push_str(&columns.join(","));
    text.push('\n');
    for row in &rows {
        text.push_str(&row.to_csv(&columns));
        text.push('\n');
    }

    let log = OracleLog::parse(&text);
    assert_eq!(log.len(), rows.len());
    assert!(
        log.diff(&rows, &[]).is_empty(),
        "a log built from these rows must diff clean"
    );
}

#[test]
fn a_divergence_is_reported_with_its_frame_and_column() {
    let rows = replay("8 R walk\n", 1);
    let columns = modelled_columns();
    let mut text = String::from("# synthetic\n");
    text.push_str(&columns.join(","));
    text.push('\n');
    for (index, row) in rows.iter().enumerate() {
        let mut line = row.to_csv(&columns);
        if index == 3 {
            // Corrupt one x position on one frame.
            line = line.replace(&row.c1_x_px.to_string(), "999");
        }
        text.push_str(&line);
        text.push('\n');
    }

    let divergences = OracleLog::parse(&text).diff(&rows, &[]);
    assert!(!divergences.is_empty(), "the corruption must be caught");
    assert_eq!(divergences[0].frame, 4, "the frame it happened on");
    assert_eq!(divergences[0].oracle, "999");

    // And a caller can exclude a column it has a documented reason to ignore.
    let skipped = OracleLog::parse(&text).diff(&rows, &["c1_x_px", "c1_dest_x"]);
    assert!(skipped.is_empty(), "skipping is explicit, never implicit");
}

#[test]
fn the_header_declares_unmodelled_columns_rather_than_zeroing_them() {
    let header = csv_header();
    let unmodelled: Vec<&str> = COLUMNS
        .iter()
        .filter(|c| matches!(c.coverage, Coverage::NotModelled(_)))
        .map(|c| c.name)
        .collect();

    assert!(!unmodelled.is_empty());
    for name in &unmodelled {
        assert!(
            header.contains(&format!("# not modelled: {name} -")),
            "{name} should be declared with a reason"
        );
    }
    // None of them appear in the emitted column row.
    let row = header.lines().last().unwrap();
    for name in &unmodelled {
        assert!(
            !row.split(',').any(|c| c == *name),
            "{name} must not be emitted as a value column"
        );
    }
}

#[test]
fn the_talk_button_drives_an_interaction_not_a_step() {
    // Genesis C is ButtonSpeak. A tape holding it must not move the party.
    let rows = replay("1 C talk\n1 .\n", 1);
    assert_eq!(rows[0].c1_x_px, 16, "confirm does not walk");
    assert_eq!(rows[0].c1_x_step_dur, 0);
    assert_eq!(Buttons::parse("C").unwrap().to_tape(), "C");
    assert_eq!(rows[0].buttons, "C");
}
