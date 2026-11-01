use super::driver::command_vocabulary;
use super::records::voice_table;
use super::*;

fn walk(track: &[u8]) -> Result<Decoded, PackError> {
    decode_track(track, 0, track.len(), "t", 0)
}

#[test]
fn notes_take_a_delay_only_below_0x80() {
    // note+delay, note, note+delay, end
    let decoded = walk(&[0x81, 0x10, 0x82, 0x83, 0x04, 0xF2]).unwrap();
    assert_eq!(decoded.states, 4);
    assert_eq!(decoded.covered.len(), 6);
    assert!(decoded.has_track_end);
    assert_eq!(decoded.command_counts, BTreeMap::from([(0xF2, 1)]));
}

#[test]
fn a_loop_walks_both_ways_and_a_call_returns() {
    let track = [
        0xF7, 0x00, 0x02, 0xFF, 0xFC, // 0: loop to 4-4 = 0, or on to 5
        0xF8, 0x00, 0x02, // 5: gosub 7+2 = 9, returning to 8
        0xF2, // 8: end
        0xF9, // 9: return
    ];
    let decoded = walk(&track).unwrap();
    assert_eq!(decoded.states, 4);
    assert_eq!(decoded.max_call_depth, 1);
    assert_eq!(
        decoded.command_counts,
        BTreeMap::from([(0xF2, 1), (0xF7, 1), (0xF8, 1), (0xF9, 1)])
    );
    assert!(decoded.has_track_end);
}

#[test]
fn the_walk_refuses_what_the_driver_cannot_play() {
    for (track, message) in [
        (&[0xF9][..], "t: RETURN without GOSUB at 0x000000"),
        (
            &[0xFF, 0x01, 0x00],
            "t: undocumented FF meta 0x01 at 0x000001",
        ),
        (
            &[0xF6, 0x00, 0x10],
            "t: 0xF6 at 0x000000 targets outside track at 0x000012",
        ),
        (
            &[0xF6, 0xFF, 0xF0],
            "t: 0xF6 at 0x000000 targets outside track at 0x-0000E",
        ),
        (
            &[0xF0, 0x00, 0x00],
            "t: 0xF0 at 0x000000 truncates at 0x000003",
        ),
        (&[0x81], "t: note at 0x000000 lacks lookahead"),
        (&[0xEF], "t: EF at 0x000000 lacks instrument"),
        (
            &[0xF8, 0xFF, 0xFE],
            "t: GOSUB stack exceeds static safety limit",
        ),
    ] {
        assert_eq!(walk(track).err().map(|e| e.0).as_deref(), Some(message));
    }
}

#[test]
fn envelopes_end_at_a_control_byte_but_not_inside_chg_mult() {
    assert_eq!(
        envelope_span(&[1, 0x84, 0x81, 2, 0x83], 0, 5, "modulation", 0).unwrap(),
        (0, 5, 0x83)
    );
    assert_eq!(
        envelope_span(&[1, 0x84, 0x81], 0, 3, "volume", 0).unwrap(),
        (0, 3, 0x81)
    );
    assert_eq!(
        envelope_span(&[1, 2], 0, 2, "volume", 4).unwrap_err().0,
        "volume envelope 4 has no terminal control byte"
    );
    assert_eq!(
        envelope_span(&[0x84], 0, 1, "modulation", 1).unwrap_err().0,
        "modulation envelope 1 truncates CHG_MULT"
    );
}

#[test]
fn a_voice_table_pads_with_zeros_and_covers_every_used_voice() {
    let mut rom = vec![7; 2 * VOICE_SIZE];
    rom.extend([0, 0]);
    let (count, padding, voices) = voice_table(&rom, 0, rom.len(), "r", &[]).unwrap();
    assert_eq!((count, padding, voices.len()), (2, 2, 2));
    assert_eq!(voices[1].rom_offset, "0x000019");

    let used = Decoded {
        fm_instruments: BTreeSet::from([2]),
        ..Decoded::default()
    };
    assert_eq!(
        voice_table(&rom, 0, rom.len(), "r", &[used]).unwrap_err().0,
        "r: used FM voice index exceeds the full voice table"
    );
    *rom.last_mut().unwrap() = 1;
    assert_eq!(
        voice_table(&rom, 0, rom.len(), "r", &[]).unwrap_err().0,
        "r: nonzero bytes after the FM voice table"
    );
}

#[test]
fn another_image_fails_the_first_anchor() {
    let error = verify_provenance(&vec![0; 0x100000]).unwrap_err().0;
    assert!(
        error.starts_with("sound provenance anchor mismatch at 0x0D0008: expected "),
        "{error}"
    );
}

#[test]
fn the_vocabulary_documents_every_primary_command_once() {
    let opcodes: Vec<u8> = COMMAND_SPECS.iter().map(|s| s.opcode).collect();
    assert_eq!(opcodes, (0xE0..=0xFF).collect::<Vec<u8>>());
    let vocabulary = command_vocabulary();
    assert_eq!(vocabulary["primary"]["0xF7"]["relative_word_at"], 3);
    assert_eq!(vocabulary["primary"]["0xFF"]["variable"], true);
    assert_eq!(vocabulary["primary"]["0xE0"].get("variable"), None);
    assert_eq!(vocabulary["total_documented_entries"], 33);
}
