//! `psiv-campaign verify-continue`: what a native CONTINUE's resave may change.
//!
//! Each resave is the slot writer's own file (`RetailSlot::encode` plus `TR`,
//! as `Runtime::slot_bytes` writes it) from the decoded source, one row
//! further down, for the three kinds of source: an `SY`-marked native slot, a
//! native slot written before the marker (#129), and a cartridge image. Each
//! control is one way a run, or the writer, can go wrong.

use std::path::PathBuf;
use std::process::Command;

use psiv_campaign::verify_continue::{sha256_hex, verify};
use psiv_core::{
    RETAIL_HEADER_LOGICAL_BYTES, RETAIL_HEADER_PHYSICAL_BYTES, RETAIL_PAYLOAD_BYTES,
    RetailLocation, RetailSave, RetailSlot, StateSnapshot,
};
use serde_json::{Value, json};

// Logical header offsets (`docs/camp/SAVE_FORMAT.md`).
const SAVE_SLOT: usize = 0x12;
const CHECKSUM: usize = 0x14;
const ROSTER_MASK: usize = 0x20;
const DUNGEON_EXIT: usize = 0x22;
const SAVED_PARTY: usize = 0x26;
const STANDING_Y: usize = 0x2E;
// Logical payload offsets.
const LOCATION_Y: usize = 0x308;
const MONEY: usize = 0x338;
const CHARACTER: usize = 0x400;
const CHARACTER_STRIDE: usize = 0x80;

fn header_byte(bytes: &mut [u8], offset: usize) -> &mut u8 {
    &mut bytes[1 + offset * 2]
}

fn payload_byte(bytes: &mut [u8], offset: usize) -> &mut u8 {
    &mut bytes[RETAIL_HEADER_PHYSICAL_BYTES + 1 + offset * 2]
}

fn set_header_word(bytes: &mut [u8], offset: usize, value: u16) {
    let [high, low] = value.to_be_bytes();
    *header_byte(bytes, offset) = high;
    *header_byte(bytes, offset + 1) = low;
}

fn set_payload_word(bytes: &mut [u8], offset: usize, value: u16) {
    let [high, low] = value.to_be_bytes();
    *payload_byte(bytes, offset) = high;
    *payload_byte(bytes, offset + 1) = low;
}

/// Restamps the checksum of the slot the header names, after a payload edit.
fn restamp(bytes: &mut [u8]) {
    let slot = usize::from(bytes[1 + SAVE_SLOT * 2 + 2]);
    let sum = (0..RETAIL_PAYLOAD_BYTES).fold(0_u16, |sum, offset| {
        sum.wrapping_add(u16::from(*payload_byte(bytes, offset)))
    });
    set_header_word(bytes, CHECKSUM + slot * 2, sum);
}

/// A logical payload with the given character records seated: each gets a
/// level and HP, fields the codec reads and writes back.
fn payload(seated: &[usize]) -> [u8; RETAIL_PAYLOAD_BYTES] {
    let mut payload = [0; RETAIL_PAYLOAD_BYTES];
    payload[0x304..0x306].copy_from_slice(&0x13_u16.to_be_bytes());
    payload[0x306..0x308].copy_from_slice(&(38_u16 * 16).to_be_bytes());
    payload[MONEY + 3] = 0x41;
    for &index in seated {
        let record = CHARACTER + index * CHARACTER_STRIDE;
        payload[record + 0x09] = 1 + index as u8;
        payload[record + 0x0F] = 20;
        payload[record + 0x11] = 25;
    }
    payload
}

fn save(seated: &[usize], standing_row: u16) -> RetailSave {
    RetailSave {
        snapshot: StateSnapshot::from_retail_payload(&payload(seated)).unwrap(),
        location: RetailLocation {
            map_index: 0x13,
            char_x: 38 * 16,
            char_y: standing_row * 16,
            ..RetailLocation::default()
        },
    }
}

/// The slot writer's file, as `Runtime::slot_bytes` writes it.
fn written(save: &RetailSave, slot: usize, exit: u8) -> Vec<u8> {
    RetailSlot::encode(save, slot)
        .unwrap()
        .with_dungeon_exit(exit)
        .as_bytes()
        .to_vec()
}

/// A native slot written before the `SY` marker: the writer's header
/// without it, and the standing row itself in `$F408`.
fn legacy(seated: &[usize], standing_row: u16) -> Vec<u8> {
    let mut bytes = written(&save(seated, standing_row), 0, 0);
    set_header_word(&mut bytes, STANDING_Y, 0);
    set_payload_word(&mut bytes, LOCATION_Y, standing_row * 16);
    restamp(&mut bytes);
    bytes
}

/// A slot file as the cartridge leaves it: `InitSRAM` zeroes the header
/// (ps4.asm:134737), and `$F408` is the object Y, one row above standing.
fn cartridge(seated: &[usize], standing_row: u16) -> Vec<u8> {
    let mut bytes = written(&save(seated, standing_row), 0, 0);
    for offset in ROSTER_MASK..RETAIL_HEADER_LOGICAL_BYTES {
        *header_byte(&mut bytes, offset) = 0;
    }
    bytes
}

/// What the writer saves into slot 2 after CONTINUE from `source` and one
/// step down.
fn resave(source: &[u8], exit: u8) -> Vec<u8> {
    let slot = usize::from(source[1 + SAVE_SLOT * 2 + 2]);
    let mut loaded = RetailSlot::from_bytes(source, slot)
        .unwrap()
        .decode()
        .unwrap();
    loaded.location.char_y += 16;
    written(&loaded, 1, exit)
}

fn receipt(row: i64) -> Value {
    let state = json!({"map": 19, "world": 0, "flags": [], "money": 0x41, "leader": 0,
        "inventory": [], "party_status": [], "party_resources": []});
    let at = |row: i64| {
        let mut state = state.clone();
        state["cell"] = json!([38, row]);
        state
    };
    json!({"complete": true, "observations": [
        {"label": "continued", "state": at(row)},
        {"label": "moved", "state": at(row + 1)},
        {"label": "resaved", "state": at(row + 1)},
    ]})
}

fn check(source: &[u8], resaved: &[u8]) -> Result<(), String> {
    verify(source, &sha256_hex(source), resaved, &receipt(18)).map(|_| ())
}

#[track_caller]
fn fails(source: &[u8], resaved: &[u8], message: &str) {
    let error = check(source, resaved).expect_err("the resave must fail");
    assert!(error.contains(message), "{error:?} lacks {message:?}");
}

#[test]
fn a_marked_source_resaves_one_object_row_down() {
    let source = written(&save(&[0, 1], 18), 0, 3);
    let resaved = resave(&source, 3);
    let report = verify(&source, &sha256_hex(&source), &resaved, &receipt(18)).unwrap();
    assert_eq!(report.header_changes, [0x13, 0x14, 0x15, 0x16, 0x17]);
    let changes: Vec<usize> = report.payload_changes.iter().map(|c| c.0).collect();
    assert_eq!(changes, [0x309], "object row 17 to 18");
}

#[test]
fn a_legacy_source_resaves_in_the_retail_convention() {
    // The legacy word is the standing row; the marked resave after one step
    // holds the same word, since it is now the object row.
    let source = legacy(&[0, 1], 18);
    let resaved = resave(&source, 0);
    let report = verify(&source, &sha256_hex(&source), &resaved, &receipt(18)).unwrap();
    assert!(report.payload_changes.is_empty());
    assert!(report.header_changes.contains(&STANDING_Y));
}

#[test]
fn a_zero_roster_legacy_source_is_still_legacy() {
    // A session without battle files seats no roster: a zero mask, but `TR`
    // marks the file native.
    let source = legacy(&[], 18);
    check(&source, &resave(&source, 0)).unwrap();
}

#[test]
fn a_cartridge_image_gains_the_writers_mask_exit_and_marker() {
    for seated in [&[0, 1][..], &[0, 4, 10], &[]] {
        let source = cartridge(seated, 18);
        let resaved = resave(&source, 0);
        let report = verify(&source, &sha256_hex(&source), &resaved, &receipt(18)).unwrap();
        let mask = seated.iter().fold(0_u16, |mask, &index| mask | 1 << index);
        assert_eq!(
            RetailSlot::from_bytes(&resaved, 1).unwrap().roster_mask(),
            mask,
            "{seated:?}"
        );
        assert!(report.header_changes.contains(&DUNGEON_EXIT));
        assert!(report.header_changes.contains(&STANDING_Y));
    }
}

#[test]
fn a_cartridge_resave_must_carry_exactly_the_seated_records() {
    let source = cartridge(&[0, 4], 18);
    // 0b1_0001 is what the writer stores; each of these is not.
    for mask in [0b1_0000, 0b0_0001, 0b1_0011, 0b1_0001 | 0x0800, 0x8011, 0] {
        let mut resaved = resave(&source, 0);
        set_header_word(&mut resaved, ROSTER_MASK, mask);
        fails(&source, &resaved, "is not the source's seats");
    }
    // An empty cartridge roster may not gain a seat.
    let empty = cartridge(&[], 18);
    let mut resaved = resave(&empty, 0);
    set_header_word(&mut resaved, ROSTER_MASK, 1);
    fails(&empty, &resaved, "is not the source's seats");
}

#[test]
fn a_cartridge_resave_needs_the_writers_exit_and_nothing_else() {
    let source = cartridge(&[0], 18);
    let mut missing = resave(&source, 0);
    for offset in DUNGEON_EXIT..DUNGEON_EXIT + 4 {
        *header_byte(&mut missing, offset) = 0;
    }
    fails(&source, &missing, "dungeon exit is not the writer's");
    let mut malformed = resave(&source, 5);
    *header_byte(&mut malformed, DUNGEON_EXIT + 3) = 5;
    fails(&source, &malformed, "dungeon exit is not the writer's");
    let mut bridge = resave(&source, 0);
    *header_byte(&mut bridge, SAVED_PARTY) = b'P';
    fails(
        &source,
        &bridge,
        "non-checksum header state changed at 0x26",
    );
}

#[test]
fn a_native_source_keeps_its_mask_and_exit() {
    // A zero-roster legacy slot is native by its `TR`: it is no cartridge
    // image, so its resave may not rewrite the exit or invent a mask either.
    for source in [
        written(&save(&[0, 1], 18), 0, 3),
        legacy(&[0, 1], 18),
        legacy(&[], 18),
    ] {
        let exit = RetailSlot::from_bytes(&source, 0)
            .unwrap()
            .dungeon_exit()
            .unwrap();
        fails(
            &source,
            &resave(&source, exit + 1),
            "non-checksum header state changed at 0x24",
        );
        let mut mask = resave(&source, exit);
        set_header_word(&mut mask, ROSTER_MASK, 0b111);
        fails(&source, &mask, "non-checksum header state changed at 0x21");
    }
}

#[test]
fn a_resave_without_the_marker_fails() {
    for source in [
        written(&save(&[0], 18), 0, 0),
        legacy(&[0], 18),
        cartridge(&[0], 18),
    ] {
        let mut resaved = resave(&source, 0);
        set_header_word(&mut resaved, STANDING_Y, 0);
        fails(&source, &resaved, "the resave lacks the SY marker");
    }
}

#[test]
fn a_resave_that_did_not_step_one_row_fails() {
    for source in [
        written(&save(&[0], 18), 0, 0),
        legacy(&[0], 18),
        cartridge(&[0], 18),
    ] {
        for delta in [0_u16, 32] {
            let mut resaved = resave(&source, 0);
            let word = u16::from_be_bytes([
                *payload_byte(&mut resaved, LOCATION_Y),
                *payload_byte(&mut resaved, LOCATION_Y + 1),
            ]);
            set_payload_word(&mut resaved, LOCATION_Y, word - 16 + delta);
            restamp(&mut resaved);
            fails(&source, &resaved, "the step is not one row down");
        }
    }
}

#[test]
fn other_changes_fail() {
    let source = written(&save(&[0], 18), 0, 0);
    let mut header = resave(&source, 0);
    *header_byte(&mut header, 0x30) = 1;
    fails(
        &source,
        &header,
        "non-checksum header state changed at 0x30",
    );
    let mut gameplay = resave(&source, 0);
    *payload_byte(&mut gameplay, MONEY + 3) = 0x42;
    restamp(&mut gameplay);
    fails(&source, &gameplay, "unexpected gameplay changes: 0x33b");
    let mut unused = resave(&source, 0);
    unused[RETAIL_HEADER_PHYSICAL_BYTES] = 1;
    fails(&source, &unused, "unused interleaved bytes changed");
    let mut checksum = resave(&source, 0);
    *header_byte(&mut checksum, CHECKSUM + 3) ^= 1;
    fails(&source, &checksum, "the resave is not a valid slot");
    assert!(
        verify(
            &source,
            &sha256_hex(b"other"),
            &resave(&source, 0),
            &receipt(18)
        )
        .unwrap_err()
        .contains("Continue modified the source save")
    );
}

#[test]
fn the_receipt_must_show_one_harmless_step() {
    let source = written(&save(&[0], 18), 0, 0);
    let resaved = resave(&source, 0);
    let run = |receipt: Value| verify(&source, &sha256_hex(&source), &resaved, &receipt);
    let mut unfinished = receipt(18);
    unfinished["complete"] = json!(false);
    assert!(run(unfinished).unwrap_err().contains("did not finish"));
    let mut stood = receipt(18);
    stood["observations"][1]["state"]["cell"] = json!([38, 18]);
    assert!(run(stood).unwrap_err().contains("not one cell down"));
    let mut spent = receipt(18);
    spent["observations"][2]["state"]["money"] = json!(0);
    assert!(run(spent).unwrap_err().contains("changed money"));
    let mut missing = receipt(18);
    missing["observations"].as_array_mut().unwrap().pop();
    assert!(run(missing).unwrap_err().contains("lacks"));
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_command_writes_its_report_and_exits_by_the_verdict() {
    let dir = scratch("verify-continue-cli");
    let source = cartridge(&[0], 18);
    std::fs::write(dir.join("source.sram"), &source).unwrap();
    std::fs::write(dir.join("resaved.sram"), resave(&source, 0)).unwrap();
    std::fs::write(dir.join("stepless.sram"), &source).unwrap();
    std::fs::write(dir.join("receipt.json"), receipt(18).to_string()).unwrap();
    let run = |resaved: &str, out: &str| {
        Command::new(env!("CARGO_BIN_EXE_psiv-campaign"))
            .arg("verify-continue")
            .args(["--source", dir.join("source.sram").to_str().unwrap()])
            .args(["--source-sha256", &sha256_hex(&source)])
            .args(["--resaved", dir.join(resaved).to_str().unwrap()])
            .args(["--receipt", dir.join("receipt.json").to_str().unwrap()])
            .args(["--out", dir.join(out).to_str().unwrap()])
            .output()
            .unwrap()
    };
    let pass = run("resaved.sram", "report/out.json");
    assert_eq!(pass.status.code(), Some(0), "{pass:?}");
    let report: Value =
        serde_json::from_slice(&std::fs::read(dir.join("report/out.json")).unwrap()).unwrap();
    assert_eq!(report["complete"], json!(true));
    assert_eq!(report["source_sha256"], json!(sha256_hex(&source)));
    let fail = run("stepless.sram", "fail.json");
    assert_eq!(fail.status.code(), Some(1));
    assert!(!dir.join("fail.json").exists());
    let usage = run("absent.sram", "usage.json");
    assert_eq!(usage.status.code(), Some(2));
}
