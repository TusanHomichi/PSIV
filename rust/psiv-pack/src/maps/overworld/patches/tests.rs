use super::*;

fn decoder(routine: &[u8], plane: Plane) -> (Vec<u8>, usize, Plane) {
    // The hook sits above GetMapLayoutChunk, so `bsr.w` displacements are
    // real ones.
    let at = GET_MAP_LAYOUT_CHUNK_BG + 0x100;
    let mut rom = vec![0; at];
    rom.extend_from_slice(routine);
    (rom, at, plane)
}

fn decode(routine: &[u8], plane: Plane) -> Result<Vec<LayoutPatch>, PackError> {
    let (rom, at, plane) = decoder(routine, plane);
    PatchDecoder {
        rom: &rom,
        plane,
        page: 3,
        routine: at,
        pos: at,
    }
    .decode()
}

/// `move.w #flag,d0 / jsr EventFlags_Test / beq.s end`, then `body`, `rts`.
fn gated(flag: u16, body: &[u8]) -> Vec<u8> {
    let mut out = vec![0x30, 0x3C];
    out.extend_from_slice(&flag.to_be_bytes());
    out.extend_from_slice(&[0x4E, 0xB9]);
    out.extend_from_slice(&EVENT_FLAGS_TEST.to_be_bytes());
    out.extend_from_slice(&[0x67, body.len() as u8]);
    out.extend_from_slice(body);
    out.extend_from_slice(&RTS.to_be_bytes());
    out
}

/// `move.w #column,d0 / move.w #row,d1 / bsr.w GetMapLayoutChunk`, for a
/// routine starting at the test's address and `prefix` bytes in.
fn locate(column: u16, row: u16, target: usize, prefix: usize) -> Vec<u8> {
    let mut out = vec![0x30, 0x3C];
    out.extend_from_slice(&column.to_be_bytes());
    out.extend_from_slice(&[0x32, 0x3C]);
    out.extend_from_slice(&row.to_be_bytes());
    out.extend_from_slice(&[0x61, 0x00]);
    let from = GET_MAP_LAYOUT_CHUNK_BG + 0x100 + prefix + out.len();
    let displacement = target as i64 - from as i64;
    out.extend_from_slice(&(displacement as i16).to_be_bytes());
    out
}

// The gate's own 12 bytes: move.w, flag, jsr, address, beq.
const GATE: usize = 12;

#[test]
fn a_hook_with_nothing_gated_is_no_patches() {
    assert_eq!(decode(&RTS.to_be_bytes(), Plane::Fg).unwrap(), []);
}

#[test]
fn a_store_resolves_to_its_plane_and_row() {
    // Row 40 is window row 8 at column 5. move.l #imm,(a1), then a byte
    // store on the other plane: $F000 from BG's $B000 reaches FG's $A000.
    let mut body = locate(5, 40, GET_MAP_LAYOUT_CHUNK_BG, GATE);
    body.extend_from_slice(&[0x22, 0xBC, 1, 2, 3, 4]);
    body.extend_from_slice(&[0x13, 0x7C, 0xEE, 0x09]);
    body.extend_from_slice(&0xF000u16.to_be_bytes());
    let patches = decode(&gated(0x66, &body), Plane::Bg).unwrap();
    let [patch] = patches.as_slice() else {
        panic!("{patches:?}");
    };
    assert_eq!(patch.event_flag, 0x66);
    assert_eq!(patch.page, 3);
    assert_eq!(
        patch.writes,
        [
            PatchWrite {
                plane: Plane::Bg,
                chunk_x: 5,
                chunk_y: 40,
                chunk_ids: vec![1, 2, 3, 4],
                displacement: 0,
            },
            PatchWrite {
                plane: Plane::Fg,
                chunk_x: 5,
                chunk_y: 40,
                chunk_ids: vec![9],
                displacement: -0x1000,
            },
        ]
    );
}

#[test]
fn a_row_move_takes_the_nearest_representative() {
    // Row 32 is window row 0; $F80 lands on window row 31 of the same
    // plane, which is one row back: the real row is 31, not 63.
    let mut body = locate(0, 32, GET_MAP_LAYOUT_CHUNK_FG, GATE);
    body.extend_from_slice(&[0x13, 0x7C, 0x00, 0x07]);
    body.extend_from_slice(&0x0F80u16.to_be_bytes());
    let patches = decode(&gated(0x34, &body), Plane::Fg).unwrap();
    let write = &patches[0].writes[0];
    assert_eq!(write.plane, Plane::Fg);
    assert_eq!((write.chunk_x, write.chunk_y), (0, 31));
    // $1000 from window row 1 lands on the same row of the other plane.
    let mut body = locate(0, 33, GET_MAP_LAYOUT_CHUNK_FG, GATE);
    body.extend_from_slice(&[0x13, 0x7C, 0x00, 0x07]);
    body.extend_from_slice(&0x1000u16.to_be_bytes());
    let patches = decode(&gated(0x34, &body), Plane::Fg).unwrap();
    let write = &patches[0].writes[0];
    assert_eq!(write.plane, Plane::Bg);
    assert_eq!((write.chunk_x, write.chunk_y), (0, 33));
}

#[test]
fn the_decoder_refuses_what_it_has_not_read() {
    let refused = |routine: &[u8], wanted: &str| {
        let error = decode(routine, Plane::Fg).unwrap_err();
        assert!(error.0.contains(wanted), "{}", error.0);
        assert!(error.0.starts_with("FG page 3 hook at 0x"), "{}", error.0);
    };
    refused(&[0x4E, 0x71], "is not `move.w #imm,d0` or `rts`");
    let mut wrong_jsr = gated(0x66, &[]);
    wrong_jsr[9] ^= 1;
    refused(&wrong_jsr, "EventFlags_Test");
    let mut no_branch = gated(0x66, &[0x4E, 0x71]);
    no_branch[11] = 0;
    refused(&no_branch, "expected a short `beq`");
    refused(
        &gated(0x66, &[0x4E, 0x71]),
        "outside this decoder's vocabulary",
    );
    refused(
        &gated(0x66, &[0x22, 0xBC, 0, 0, 0, 0]),
        "before GetMapLayoutChunk",
    );
    // A store reading d0 needs the moveq, and the bsr forgets it.
    let mut stale = vec![0x70, 0x00];
    stale.extend(locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE + 2));
    stale.extend_from_slice(&[0x12, 0x80]);
    refused(&gated(0x66, &stale), "without a preceding `moveq #0,d0`");
    // A bsr somewhere else.
    refused(
        &gated(0x66, &locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG + 2, GATE)),
        "is not GetMapLayoutChunkFG/BG",
    );
    // A bsr before the coordinates.
    let from = GET_MAP_LAYOUT_CHUNK_BG + 0x100 + GATE + 2;
    let displacement = GET_MAP_LAYOUT_CHUNK_FG as i64 - from as i64;
    let mut early = vec![0x61, 0x00];
    early.extend_from_slice(&(displacement as i16).to_be_bytes());
    refused(&gated(0x66, &early), "without both coordinates");
    // A long store that runs past its row.
    let mut past = locate(126, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE);
    past.extend_from_slice(&[0x22, 0xBC, 0, 0, 0, 0]);
    refused(&gated(0x66, &past), "runs past the row");
    // A displacement off both regions.
    let mut off = locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE);
    off.extend_from_slice(&[0x13, 0x7C, 0, 1, 0x20, 0x00]);
    refused(&gated(0x66, &off), "outside both Map_Layout regions");
    // A block that overruns its beq: the store's immediate crosses it.
    let mut over = locate(0, 0, GET_MAP_LAYOUT_CHUNK_FG, GATE);
    over.extend_from_slice(&[0x12, 0xBC]);
    refused(&gated(0x66, &over), "overruns its `beq` target");
}

#[test]
fn only_the_two_world_maps_take_the_paged_path() {
    assert!(is_overworld(0) && is_overworld(1));
    assert!(!is_overworld(2));
    let error = read_page_table(&[], 2, Plane::Fg).unwrap_err();
    assert!(error.0.contains("only [0, 1]"), "{}", error.0);
}

#[test]
fn a_rom_without_the_opcodes_is_refused_by_name() {
    let error = check_code_sites(&vec![0; 0x60000]).unwrap_err();
    assert!(
        error
            .0
            .starts_with("FG page table selector at 0x053A44: expected 45f9"),
        "{}",
        error.0
    );
}

#[test]
fn a_later_patch_wins_and_other_planes_are_left_alone() {
    let write = |plane, chunk_id| PatchWrite {
        plane,
        chunk_x: 1,
        chunk_y: 0,
        chunk_ids: vec![chunk_id, 9],
        displacement: 0,
    };
    let patch = |writes| LayoutPatch {
        plane: Plane::Fg,
        page: 0,
        routine: 0,
        event_flag: 0,
        writes,
    };
    let layout = Layout {
        plane: Plane::Fg,
        width_chunks: WIDTH_CHUNKS,
        height_chunks: HEIGHT_CHUNKS,
        cells: vec![0; WIDTH_CHUNKS * HEIGHT_CHUNKS],
        blob: Blob {
            rom_offset: 0,
            compressed_length: 0,
            decompressed_length: 0,
            sha256: String::new(),
        },
        anomaly: None,
    };
    let first = patch(vec![write(Plane::Fg, 5)]);
    let second = patch(vec![write(Plane::Fg, 6), write(Plane::Bg, 7)]);
    let patched = apply_patches(&layout, &[&first, &second]);
    assert_eq!(&patched.cells[..4], &[0, 6, 9, 0]);
}
