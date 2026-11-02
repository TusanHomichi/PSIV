//! Synthetic routines, no ROM data.

use super::*;

const TEST: i64 = 0x100;
const CLEAR: i64 = 0x200;
const KOS: i64 = 0x300;

fn run(routine: &[u8]) -> Result<Vec<Path>, String> {
    let tests = HashMap::from([(TEST, "event_flags")]);
    let clears = HashMap::from([(CLEAR, "extended_event_flags")]);
    let at = 0x1000usize;
    let mut rom = vec![0u8; at];
    rom.extend_from_slice(routine);
    let mut decoder = Decoder {
        rom: &rom,
        start: at as i64,
        tests: &tests,
        clears: &clears,
        kos: KOS,
        paths: Vec::new(),
    };
    decoder.run().map(|()| decoder.paths)
}

fn jsr(target: i64) -> Vec<u8> {
    let mut out = vec![0x4E, 0xB9];
    out.extend_from_slice(&(target as u32).to_be_bytes());
    out
}

const RTS: [u8; 2] = [0x4E, 0x75];

fn routine(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn kinds(path: &Path) -> Vec<&str> {
    path.writes.iter().map(|write| write.kind).collect()
}

#[test]
fn beq_is_the_clear_arm_and_each_path_carries_its_gate() {
    // move.w #$34,d0 / jsr test / beq.s +4 / clr.w $C300.w / rts
    let bytes = routine(&[
        &[0x30, 0x3C, 0x00, 0x34],
        &jsr(TEST),
        &[0x67, 0x04, 0x42, 0x78, 0xC3, 0x00],
        &RTS,
    ]);
    let paths = run(&bytes).unwrap();
    assert_eq!(paths.len(), 2);
    // The branch target first, then the fall-through.
    assert!(!paths[0].gates[0].required);
    assert!(paths[0].writes.is_empty());
    assert!(paths[1].gates[0].required);
    assert_eq!(kinds(&paths[1]), ["object_despawn"]);
}

#[test]
fn a_flag_tested_twice_drops_the_impossible_arm() {
    // Both arms of the first test reach a second test of the same flag. Each
    // keeps one arm, and the kept arm's gate isn't listed twice.
    let test = routine(&[&[0x30, 0x3C, 0x00, 0x34], &jsr(TEST)]);
    let bytes = routine(&[
        &test,
        &[0x67, 0x00, 0x00, 0x02],
        &test,
        &[0x67, 0x00, 0x00, 0x02],
        &RTS,
    ]);
    let paths = run(&bytes).unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|path| path.gates.len() == 1));
}

#[test]
fn a_known_counter_runs_a_dbf_loop_and_an_unknown_one_falls_through() {
    // moveq #1,d4 / clr.w $C300.w / dbf d4,-6 / rts
    let bytes = routine(&[
        &[0x78, 0x01, 0x42, 0x78, 0xC3, 0x00, 0x51, 0xCC, 0xFF, 0xFA],
        &RTS,
    ]);
    let paths = run(&bytes).unwrap();
    assert_eq!(kinds(&paths[0]), ["object_despawn", "object_despawn"]);
    let bytes = routine(&[&[0x51, 0xCC, 0xFF, 0xFA], &RTS]);
    assert_eq!(run(&bytes).unwrap()[0].deferred, ["loop"]);
}

#[test]
fn object_stores_land_on_slots_by_field() {
    // lea $C340.w,a2; move.w #$28,(a2); clr.w (a2); move.b #9,$14(a2);
    // move.b #1,3(a2); rts
    let bytes = routine(&[
        &[0x45, 0xF8, 0xC3, 0x40],
        &[0x34, 0xBC, 0x00, 0x28],
        &[0x42, 0x52],
        &[0x15, 0x7C, 0x00, 0x09, 0x00, 0x14],
        &[0x15, 0x7C, 0x00, 0x01, 0x00, 0x03],
        &RTS,
    ]);
    let path = &run(&bytes).unwrap()[0];
    assert_eq!(
        kinds(path),
        ["object_rewrite", "object_despawn", "object_dialogue"]
    );
    assert_eq!(path.deferred, ["write at +0x3 of object slot 1"]);
    assert_eq!(
        path.writes[0].detail,
        Json::object()
            .with("object_index", 1usize)
            .with("object_id", 0x28u16)
    );
    // A word store of zero is a despawn too.
    let bytes = routine(&[&[0x45, 0xF8, 0xC3, 0x00, 0x34, 0xBC, 0x00, 0x00], &RTS]);
    assert_eq!(kinds(&run(&bytes).unwrap()[0]), ["object_despawn"]);
}

#[test]
fn a_store_through_the_layout_pointer_is_a_layout_write() {
    // moveq #5,d1; moveq #6,d2; moveq #1,d3; jsr GetMapLayoutOffset;
    // move.b #7,-$1000(a1); rts
    let bytes = routine(&[
        &[0x72, 0x05, 0x74, 0x06, 0x76, 0x01],
        &jsr(GET_MAP_LAYOUT_OFFSET),
        &[0x13, 0x7C, 0x00, 0x07, 0xF0, 0x00],
        &RTS,
    ]);
    let path = &run(&bytes).unwrap()[0];
    assert_eq!(
        path.writes[0].layout,
        Some((
            LayoutTarget {
                plane: Plane::Bg,
                chunk_x: 5,
                chunk_y: 6
            },
            -0x1000,
            Some(7)
        ))
    );
}

#[test]
fn replace_clear_and_other_calls_are_told_apart() {
    // lea $123456.l,a0 / lea $A000.w,a1 / jsr Kos
    let bytes = routine(&[
        &[0x41, 0xF9, 0x00, 0x12, 0x34, 0x56, 0x43, 0xF8, 0xA0, 0x00],
        &jsr(KOS),
        &[0x70, 0x0B],
        &jsr(CLEAR),
        &jsr(0x400),
        &RTS,
    ]);
    let path = &run(&bytes).unwrap()[0];
    assert_eq!(kinds(path), ["layout_replace", "flag_clear"]);
    assert_eq!(
        path.writes[0].detail,
        Json::object()
            .with("plane", "fg")
            .with("source", "0x123456")
    );
    assert_eq!(path.deferred, ["call to 0x000400"]);
}

#[test]
fn a_position_compare_walks_both_arms_as_a_condition() {
    // cmpi.w #$1234,$10(a3) / bls.s +0 / rts
    let bytes = routine(&[
        &[0x0C, 0x6B, 0x12, 0x34, 0x00, 0x10, 0x63, 0x02, 0x72, 0x00],
        &RTS,
    ]);
    let paths = run(&bytes).unwrap();
    assert_eq!(paths.len(), 2);
    assert_eq!(
        paths[0].conditions,
        ["position compare at 0x001000: #0x1234 against $10(a3)"]
    );
}

#[test]
fn a_byte_immediate_loses_its_high_half() {
    // move.b #$1234,d0, a form retail only uses with a clear high byte, then
    // a word store of d0.
    let bytes = routine(&[
        &[0x10, 0x3C, 0x12, 0x34],
        &[0x45, 0xF8, 0xC3, 0x00, 0x34, 0x80],
        &RTS,
    ]);
    assert_eq!(
        run(&bytes).unwrap()[0].writes[0].detail,
        Json::object()
            .with("object_index", 0usize)
            .with("object_id", 0x34u16)
    );
}

#[test]
fn d7_set_at_the_return_aborts_the_list() {
    assert!(run(&routine(&[&[0x7E, 0x01], &RTS])).unwrap()[0].aborts);
    assert!(!run(&routine(&[&[0x7E, 0x00], &RTS])).unwrap()[0].aborts);
    assert!(!run(&RTS).unwrap()[0].aborts);
}

#[test]
fn the_table_read_makes_a_constant_and_an_unknown_load_forgets() {
    // lea $1000.w,a0 / moveq #1,d1 / move.b (a0,d1.w),d0 reads the routine's
    // own second byte, $F8. Storing d0 shows it.
    let bytes = routine(&[
        &[0x41, 0xF8, 0x10, 0x00, 0x72, 0x01, 0x10, 0x30, 0x10, 0x00],
        &[0x45, 0xF8, 0xC3, 0x00, 0x34, 0x80],
        &RTS,
    ]);
    let path = &run(&bytes).unwrap()[0];
    assert_eq!(kinds(path), ["object_rewrite"]);
    assert_eq!(
        path.writes[0].detail,
        Json::object()
            .with("object_index", 0usize)
            .with("object_id", 0xF8u16)
    );
    // move.w $1234.w,d0 forgets d0; a store from it then fails.
    let bytes = routine(&[
        &[0x70, 0x01, 0x30, 0x38, 0x12, 0x34],
        &[0x45, 0xF8, 0xC3, 0x00, 0x34, 0x80],
        &RTS,
    ]);
    assert!(run(&bytes).unwrap_err().contains("sources d0"));
}

#[test]
fn the_reader_refuses_what_it_cannot_read() {
    let refused = |bytes: &[u8], wanted: &str| {
        let error = run(bytes).unwrap_err();
        assert!(error.contains(wanted), "{error}");
    };
    refused(&[0x4E, 0x71], "opcode 0x4E71 is outside");
    refused(&[0x4E, 0x71], "+0x0:");
    refused(&[0x67, 0x00, 0x00, 0x00], "beq with no flag test");
    refused(&[0x66, 0x00, 0x00, 0x00], "bne with no flag test");
    refused(&[0x63, 0x00, 0x00, 0x00], "a bls with no compare");
    refused(&[0x52, 0x41], "addq/subq on d1");
    refused(&jsr(TEST), "a flag test with no id");
    refused(&jsr(CLEAR), "a flag clear with no id");
    refused(
        &jsr(GET_MAP_LAYOUT_OFFSET),
        "GetMapLayoutOffset with d1 unset",
    );
    refused(&jsr(KOS), "KosDecomp with an unrecognised");
    refused(&[0x42, 0x52], "a store through a2 before");
    refused(&[0x43, 0xE8, 0x00, 0x00], "not a plain address");
    // bra.s to itself never reaches an rts.
    refused(&[0x60, 0xFE], "no rts within 4000");
}

#[test]
fn too_many_forks_are_refused() {
    // Seven distinct flags double the paths each time, past 64.
    let mut bytes = Vec::new();
    for flag in 0..7u8 {
        bytes.extend_from_slice(&[0x30, 0x3C, 0x00, flag]);
        bytes.extend(jsr(TEST));
        bytes.extend_from_slice(&[0x67, 0x00, 0x00, 0x02]);
    }
    bytes.extend_from_slice(&RTS);
    assert_eq!(run(&bytes).unwrap_err(), "more than 64 paths");
}

/// `MapDataMan_RanchOwner`'s shape: position words are writes, not deferred.
#[test]
fn position_words_are_writes_of_their_own() {
    let bytes = routine(&[
        &[0x45, 0xF8, 0xC3, 0x00],
        &[0x35, 0x7C, 0x01, 0x10, 0x00, 0x30],
        &[0x35, 0x7C, 0x02, 0xD0, 0x00, 0x34],
        &RTS,
    ]);
    let path = &run(&bytes).unwrap()[0];
    assert_eq!(kinds(path), ["object_x", "object_y"]);
    assert!(path.deferred.is_empty());
}
