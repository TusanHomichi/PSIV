//! Tape 25 is local evidence; changes are compared without committed CRAM.
use super::*;
use crate::test_support::retail_rom;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

fn capture() -> Option<Value> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/layouts/title/palette_cycle.json");
    match std::fs::read(&path) {
        Ok(b) => Some(serde_json::from_slice(&b).unwrap()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local tape 25 CRAM capture absent: title capture comparisons skipped");
            None
        }
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}
fn run<'a>(
    rom: &'a [u8],
    lag: &'a [(&'a str, &'a [(u32, u32)])],
    held: &'a [(u32, u32)],
) -> Result<Boot<'a>, PackError> {
    let mut b = Boot::new(rom);
    b.lag = lag;
    b.start_held = held;
    b.sega()?;
    b.title()?;
    Ok(b)
}
fn sample(b: &Boot<'_>, shift: u32) -> Value {
    let mut previous = None;
    let mut frames = serde_json::Map::new();
    for frame in FRAMES {
        let p = b.palette_at(frame + shift).unwrap();
        let changed: Vec<_> = previous.map_or_else(Vec::new, |old: [u16; 64]| {
            (0..64).filter(|&i| old[i] != p[i]).collect()
        });
        let lines: serde_json::Map<String, Value> = (0..4)
            .map(|i| (i.to_string(), serde_json::json!(p[i * 16..i * 16 + 16])))
            .collect();
        frames.insert(
            frame.to_string(),
            serde_json::json!({"lines":lines,"changed_entries_from_previous":changed}),
        );
        previous = Some(p);
    }
    Value::Object(frames)
}
fn mismatches(actual: &Value, expected: &Value) -> Vec<u32> {
    FRAMES
        .into_iter()
        .filter(|f| actual[f.to_string()] != expected[f.to_string()])
        .collect()
}

#[test]
fn tape_start_edges_and_sample_frames_match_the_committed_input() {
    let text = include_str!("../../../../../oracle/tapes/25_title_boot.tape");
    let mut frame = 0;
    let mut held = Vec::new();
    for line in text.lines() {
        let words: Vec<_> = line.split('#').next().unwrap().split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let n: u32 = words[0].parse().unwrap();
        for _ in 0..n {
            frame += 1;
            if words[1].contains('S') {
                held.push(frame);
            }
        }
    }
    assert_eq!(
        held,
        START_HELD
            .into_iter()
            .flat_map(|(from, to)| from..to)
            .collect::<Vec<_>>()
    );
    assert!(FRAMES.windows(2).all(|p| p[0] < p[1]));
    assert!(FRAMES.last().unwrap() <= &frame);
}

#[test]
fn derived_cycle_shape_and_prompt_offset_and_fail_closed_paths() {
    let Some(rom) = retail_rom("title sequencing") else {
        return;
    };
    let (d, frames) = derive(rom.bytes()).unwrap();
    let d = Value::from(&d);
    assert_eq!(d["line_count"], 4);
    assert_eq!(d["colors_per_line"], 16);
    assert_eq!(
        d["frames"]["25"]["changed_entries_from_previous"],
        serde_json::json!([])
    );
    assert_eq!(frames.iter().map(|(f, _)| *f).collect::<Vec<_>>(), FRAMES);
    for (f, words) in frames {
        for i in 0..4 {
            assert_eq!(
                d["frames"][f.to_string()]["lines"][i.to_string()],
                serde_json::json!(words[i * 16..i * 16 + 16])
            );
        }
    }
    let source = include_str!("../../../../psiv-godot/src/title.rs");
    let offset: u32 = source
        .split_once("const TITLE_MAIN_FRAME_OFFSET: u32 = ")
        .unwrap()
        .1
        .split(';')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let prompt_offsets = |b: &Boot<'_>| {
        let first = b.entries["TitleRoutine_PressStartButton"];
        let last = b.entries["loc_42C0E"];
        (first + 1..=last)
            .map(|k| b.vint_frames[k as usize] - k)
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        prompt_offsets(&run(rom.bytes(), &LAG, &START_HELD).unwrap()),
        [offset].into()
    );
    let delayed = [(0, 50), (1, 2)];
    let mut lag = LAG;
    lag[1].1 = &delayed;
    assert_eq!(
        prompt_offsets(&run(rom.bytes(), &lag, &START_HELD).unwrap()),
        [offset + 1].into()
    );
    let mut wrong = rom.bytes().to_vec();
    wrong[0x41114..0x41116].copy_from_slice(&0x0C78u16.to_be_bytes());
    assert!(derive(&wrong).is_err());
    assert!(run(rom.bytes(), &LAG, &START_HELD[..1]).is_err());
}

#[test]
fn cycle_and_every_original_negative_control_match_local_tape_25() {
    let Some(rom) = retail_rom("title CRAM capture") else {
        return;
    };
    let Some(c) = capture() else { return };
    let expected = &c["frames"];
    let derived = Value::from(&derive(rom.bytes()).unwrap().0);
    assert_eq!(derived["frames"], *expected);
    assert_eq!(derived["line_count"], c["line_count"]);
    assert_eq!(derived["colors_per_line"], c["colors_per_line"]);
    let cases: Vec<(Vec<(usize, u16)>, Vec<u32>)> = vec![
        (vec![(0x41116, 6)], vec![75, 100, 125, 150, 175]),
        // Shine's byte at $411C1 (preserve the adjacent byte).
        (
            vec![(0x411C0, u16::from_be_bytes([rom.bytes()[0x411C0], 2]))],
            vec![100, 125],
        ),
        (
            vec![(0x1D2A1E, 2)],
            vec![300, 400, 401, 450, 500, 550, 600, 650],
        ),
        (
            (0..32).map(|i| (0x42CE6 + i * 2, 2)).collect(),
            vec![500, 550, 600],
        ),
        (vec![(0x296304, 2)], vec![650]),
        (vec![(0x427E8, 0x0CCC)], vec![401, 450, 500, 550, 600, 650]),
    ];
    for (edits, expected_frames) in cases {
        let mut data = rom.bytes().to_vec();
        for (at, word) in edits {
            data[at..at + 2].copy_from_slice(&word.to_be_bytes());
        }
        assert_eq!(
            mismatches(&Value::from(&derive(&data).unwrap().0)["frames"], expected),
            expected_frames
        );
    }
    let delayed = [(0, 11), (1, 4)];
    let mut lag = LAG;
    lag[0].1 = &delayed;
    assert_eq!(
        mismatches(
            &sample(&run(rom.bytes(), &lag, &START_HELD).unwrap(), 0),
            expected
        ),
        [25, 50]
    );
    let b = run(rom.bytes(), &LAG, &START_HELD).unwrap();
    assert_eq!(
        mismatches(&sample(&b, 1), expected),
        [25, 50, 75, 100, 125, 400, 401, 600]
    );
    assert_eq!(
        mismatches(
            &sample(
                &run(rom.bytes(), &LAG, &[(402, 406), START_HELD[1]]).unwrap(),
                0
            ),
            expected
        ),
        [401, 450]
    );
}
