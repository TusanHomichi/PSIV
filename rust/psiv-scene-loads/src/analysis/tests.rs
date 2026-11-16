use super::*;

fn point(name: &str, end: bool, frame: u64, cycles: u64) -> Point {
    let &(_, start, stop) = ROUTINES.iter().find(|(n, _, _)| *n == name).unwrap();
    Point {
        frame,
        cycles,
        pc: if end { stop } else { start },
    }
}
fn samples(rows: &[(u64, u16, u32, u16)]) -> Samples {
    rows.iter()
        .map(|&(frame, count, seed, panel)| (frame, Sample { count, seed, panel }))
        .collect()
}
fn frames(rows: &[(u64, u16)]) -> Samples {
    samples(
        &rows
            .iter()
            .map(|&(f, c)| (f, c, 1234, 0x10D))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn nested_decompressors_keep_their_boundaries() {
    let trace = [
        point("panel", false, 10, 500),
        point("nemesis", false, 10, 1000),
        point("nemesis", true, 12, 2000),
        point("enigma", false, 12, 2100),
        point("enigma", true, 12, 3000),
        point("panel", true, 12, 4000),
    ];
    let mut loads = spans(&trace).unwrap();
    annotate(&mut loads, &frames(&[(10, 20), (11, 20), (12, 20)])).unwrap();
    assert_eq!(loads.len(), 1);
    let c = &loads[0];
    assert_eq!(c.master_cycles, 2 * FRAME_CYCLES + 3500);
    assert_eq!(c.held_frames, [11, 12]);
    assert_eq!(c.query.as_deref(), Some("panel:10d"));
    assert_eq!(c.components[0].lag_frames, 2);
    assert_eq!(c.components[1].lag_frames, 0);
    assert_eq!(c.components[1].master_cycles, 900);
}

#[test]
fn elapsed_frames_are_not_all_lag_frames() {
    let mut loads = spans(&[
        point("load_art", false, 10, 100),
        point("nemesis", false, 10, 200),
        point("nemesis", true, 13, 200),
        point("load_art", true, 13, 400),
    ])
    .unwrap();
    annotate(
        &mut loads,
        &samples(&[
            (10, 65535, 1234, 0),
            (11, 65535, 1234, 0),
            (12, 65535, 1234, 0),
            (13, 0, 5678, 0),
        ]),
    )
    .unwrap();
    assert_eq!(loads[0].lag_frames, 2);
    assert_eq!(loads[0].query.as_deref(), Some("art:1d712c"));
}

#[test]
fn portraits_outside_a_measured_load_are_ignored() {
    assert!(
        spans(&[
            point("nemesis", false, 10, 0),
            point("nemesis", true, 12, 0)
        ])
        .unwrap()
        .is_empty()
    );
}

#[test]
fn instruction_overshoot_survives_frame_rebasing() {
    let loads = spans(&[
        point("panel", false, 10, FRAME_CYCLES + 20),
        point("panel", true, 11, 100),
    ])
    .unwrap();
    assert_eq!(loads[0].master_cycles, 80);
}

#[test]
fn missing_or_misordered_boundaries_fail_closed() {
    for (trace, message) in [
        (vec![point("panel", false, 10, 0)], "ends inside"),
        (vec![point("panel", true, 10, 0)], "without an entry"),
        (
            vec![point("panel", false, 10, 0), point("nemesis", true, 10, 30)],
            "unmatched",
        ),
        (
            vec![point("panel", false, 10, 40), point("panel", true, 10, 30)],
            "execution order",
        ),
    ] {
        assert!(spans(&trace).unwrap_err().contains(message));
    }
}

#[test]
fn a_changed_seed_invalidates_a_claimed_lag_frame() {
    let mut calls = spans(&[point("panel", false, 10, 0), point("panel", true, 11, 0)]).unwrap();
    assert!(annotate(&mut calls, &frames(&[(10, 20), (11, 20)])).is_ok());
    let mut changed = frames(&[(10, 20), (11, 20)]);
    changed.get_mut(&11).unwrap().seed = 5678;
    assert!(
        annotate(&mut calls, &changed)
            .unwrap_err()
            .contains("seed changed")
    );
}

#[test]
fn two_panel_returns_in_a_frame_cannot_borrow_the_last_id() {
    let mut loads = spans(&[
        point("panel", false, 10, 0),
        point("panel", true, 10, 100),
        point("panel", false, 10, 200),
        point("panel", true, 10, 300),
    ])
    .unwrap();
    assert!(
        annotate(&mut loads, &frames(&[(10, 20)]))
            .unwrap_err()
            .contains("RAM id is ambiguous")
    );
}

#[test]
fn decoded_inputs_must_select_the_observed_calls() {
    let mut calls = spans(&[
        point("panel", false, 10, 0),
        point("nemesis", false, 10, 1),
        point("nemesis", true, 10, 61),
        point("enigma", false, 10, 62),
        point("enigma", true, 10, 92),
        point("panel", true, 10, 100),
    ])
    .unwrap();
    let work = json!({"art":{"source":"0x123456"},"mappings":[{"source":"0x234567"}]});
    attach_work(&mut calls[0], work.clone()).unwrap();
    assert_eq!(calls[0].outside_decompressors_master_cycles, Some(10));
    assert_eq!(calls[0].components[0].input.as_ref(), Some(&work["art"]));
    assert!(
        attach_work(
            &mut calls[0],
            json!({"art":null,"mappings":work["mappings"]})
        )
        .unwrap_err()
        .contains("different calls")
    );
}

#[test]
fn a_later_panel_id_write_invalidates_identification() {
    let mut loads = spans(&[point("panel", false, 10, 0), point("panel", true, 12, 0)]).unwrap();
    assert!(
        annotate(
            &mut loads,
            &samples(&[
                (10, 20, 1234, 0x10D),
                (11, 20, 1234, 0x10D),
                (12, 20, 1234, 0x143)
            ])
        )
        .unwrap_err()
        .contains("panel id changed")
    );
}

#[test]
fn art_alone_does_not_prove_the_panel_was_measured() {
    let mut calls = spans(&[
        point("panel", false, 10, 0),
        point("panel", true, 11, 0),
        point("load_art", false, 12, 0),
        point("load_art", true, 13, 0),
    ])
    .unwrap();
    annotate(
        &mut calls,
        &frames(&[(10, 20), (11, 20), (12, 21), (13, 21)]),
    )
    .unwrap();
    coverage(46, &calls).unwrap();
    assert!(
        coverage(46, &calls[1..])
            .unwrap_err()
            .contains("missed required load")
    );
}

#[test]
fn post_return_work_is_not_charged_to_decompression() {
    let mut loads = spans(&[
        point("load_art", false, 10, 0),
        point("load_art", true, 11, 0),
    ])
    .unwrap();
    annotate(
        &mut loads,
        &frames(&[(10, 20), (11, 20), (12, 20), (13, 21)]),
    )
    .unwrap();
    assert_eq!(loads[0].held_frames, [11]);
    assert_eq!(loads[0].following_held_frames, [12]);
    assert_eq!(loads[0].following_count_resumed, Some(true));
    annotate(&mut loads, &frames(&[(10, 20), (11, 20)])).unwrap();
    assert_eq!(loads[0].following_count_resumed, Some(false));
}

#[test]
fn invalid_timestamps_or_ram_records_are_rejected() {
    assert!(
        Point {
            frame: 0,
            pc: 0,
            cycles: 0
        }
        .absolute()
        .is_err()
    );
    assert!(
        Point {
            frame: u64::MAX,
            pc: 0,
            cycles: 0
        }
        .absolute()
        .is_err()
    );
    assert!(
        analyze(Capture {
            scene: 46,
            trace: vec![Row::new()],
            frames: vec![]
        })
        .is_err()
    );
    let row = Row::from([
        ("frame".into(), "10".into()),
        ("main_frame_count".into(), "20".into()),
        ("rng_seed".into(), "0".into()),
        ("panel_index".into(), "10d".into()),
    ]);
    assert!(
        analyze(Capture {
            scene: 46,
            trace: vec![],
            frames: vec![row.clone(), row]
        })
        .unwrap_err()
        .contains("duplicate RAM")
    );
}
