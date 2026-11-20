//! Unit tests and negative controls for `Cutscene_Ending`'s three-stage
//! credits engine (`$079D0C..$07A0A5`). All inputs are genuinely synthetic
//! fixtures with no ROM bytes, retail command tables, or dialogue text.

use psiv_core::{CreditsPlane, PresentationOp};
use psiv_data::{
    CreditsArt, CreditsCommands, CreditsFontUpload, CreditsMapping, CreditsPalette, CreditsText,
    EndingCreditsAssets,
};

use super::{
    EndingCredits, INITIAL_BG_Y, PALETTE_HEADER_START, PALETTE_LINE_4_START, PALETTE_TEXT_SLOT,
};
use crate::cram::Cram;

fn synthetic_assets() -> EndingCreditsAssets {
    let mapping = |rom_addr: u32, base_tile: u16, dests: Vec<u32>, tag: u16| CreditsMapping {
        rom_addr,
        consumed: 1,
        base_tile,
        ram_destinations: dests,
        columns: 16,
        rows: 28,
        words: (0..448u16)
            .map(|i| base_tile + ((tag + i) & 0x3F))
            .collect(),
    };
    let fade_words: Vec<u16> = (0..8u16)
        .flat_map(|row| {
            let v = row * 0x0222;
            [v, v, v, v, v]
        })
        .collect();
    let headers: Vec<CreditsText> = (0..12u8)
        .map(|i| CreditsText {
            rom_addr: 0x1FEE26 + u32::from(i) * 3,
            bytes: vec![0x80 + i, 0x81, 0xFF],
        })
        .collect();
    let names: Vec<CreditsText> = (0..32u8)
        .map(|i| CreditsText {
            rom_addr: 0x1FEB42 + u32::from(i) * 3,
            bytes: vec![1 + (i % 26), 2, 0xFF],
        })
        .collect();
    // Synthetic schedules covering every CreditsJumpTable opcode (0..=12)
    // without embedding the retail command tables from ROM.
    let stage12_schedule = vec![
        0x00, 0x00, // header
        0x01, 0x05, // fade in header
        0x02, 0x01, // name line 1
        0x03, 0x02, // name line 2
        0x04, 0x03, // name line 3
        0x05, 0x04, // name line 4
        0x06, 0x06, // fade in names
        0x10, 0x07, // fade out header
        0x14, 0x08, // fade out names
        0xFF, 0x00,
    ];
    let stage3_schedule = vec![
        0x00, 0x09, // offset header
        0x01, 0x05, // fade in header
        0x08, 0x07, // fade out header
        0x0C, 0x0A, // second-line header
        0x0D, 0x0B, // third-line header
        0x0E, 0x0C, // final fade-in & latch bit 7
        0xFF, 0x00,
    ];
    EndingCreditsAssets {
        version: psiv_data::CREDITS_VERSION,
        source_rom_sha256: "0".repeat(64),
        art: vec![
            CreditsArt {
                rom_addr: 0x1DE278,
                compression: "kosinski".into(),
                consumed: 1,
                vram_destination: 0x2000,
                tiles: vec![0; 87 * 32],
            },
            CreditsArt {
                rom_addr: 0x1DEA48,
                compression: "kosinski".into(),
                consumed: 1,
                vram_destination: 0x4000,
                tiles: vec![0; 67 * 32],
            },
            CreditsArt {
                rom_addr: 0x2A3350,
                compression: "nemesis".into(),
                consumed: 1,
                vram_destination: 0xF800,
                tiles: vec![0; 29 * 32],
            },
        ],
        font_upload: CreditsFontUpload {
            rom_addr: 0x7A26C,
            destination_tile: 0x7C0,
            art_rom_addr: 0x2A3350,
            terminator: 0xFFFF,
        },
        mappings: vec![
            mapping(0x1DF09C, 0x100, vec![0xFFFF_1000], 0x00),
            mapping(0x1DF17C, 0x100, vec![0xFFFF_1380], 0x10),
            mapping(0x1DF294, 0x100, vec![0xFFFF_1700], 0x20),
            mapping(0x1DF3E0, 0x100, vec![0xFFFF_1A80, 0xFFFF_1E00], 0x30),
            mapping(0x1DF4C2, 0x2200, vec![0xFFFF_2000], 0x05),
        ],
        palettes: vec![
            CreditsPalette {
                rom_addr: 0x1DF53A,
                start_word: 0,
                words: vec![0x0246; 32],
            },
            CreditsPalette {
                rom_addr: 0x1DF57A,
                start_word: 48,
                words: vec![0x08EE; 16],
            },
        ],
        commands: vec![
            CreditsCommands {
                rom_addr: 0x7A522,
                bytes: stage12_schedule,
            },
            CreditsCommands {
                rom_addr: 0x7A548,
                bytes: stage3_schedule,
            },
        ],
        headers,
        names,
        fade: CreditsPalette {
            rom_addr: 0x7A6DE,
            start_word: 50,
            words: fade_words,
        },
    }
}

const STAGE1: PresentationOp = PresentationOp::EndingCreditsStage {
    stage: 1,
    scroll_delay: 0x80,
    foreground_step: 0,
    background_step: 0x2000,
    source_ram: 0xFFFF_1000,
    plane: CreditsPlane::B,
    commands_rom_addr: 0x0007_A522,
};

const STAGE2: PresentationOp = PresentationOp::EndingCreditsStage {
    stage: 2,
    scroll_delay: 0x100,
    foreground_step: 0x2000,
    background_step: 0x1000,
    source_ram: 0xFFFF_2000,
    plane: CreditsPlane::A,
    commands_rom_addr: 0x0007_A522,
};

const STAGE3: PresentationOp = PresentationOp::EndingCreditsStage {
    stage: 3,
    scroll_delay: 0x180,
    foreground_step: 0x2000,
    background_step: 0x1000,
    source_ram: 0xFFFF_2000,
    plane: CreditsPlane::A,
    commands_rom_addr: 0x0007_A548,
};

fn run_stage(
    credits: &mut EndingCredits,
    op: PresentationOp,
    frame: &mut u16,
    assets: &EndingCreditsAssets,
    cram: &mut Cram,
    cycle: &mut u8,
) -> usize {
    *frame = frame.wrapping_add(1);
    let mut ticks = 1;
    if credits.begin_stage(op, *frame, assets, cram, cycle) {
        return 0;
    }
    loop {
        *frame = frame.wrapping_add(1);
        if credits.tick(*frame, assets, cram) {
            return ticks;
        }
        ticks += 1;
        assert!(ticks < 20_000, "stage {op:?} did not terminate");
    }
}

#[test]
fn init_seats_bg_camera_and_copies_initial_plane_b_from_1a80() {
    let assets = synthetic_assets();
    let credits = EndingCredits::new(&assets);
    let view = credits.view();
    assert_eq!(view.bg_y_scroll, (INITIAL_BG_Y >> 16) as u16);
    assert_eq!(view.fg_y_scroll, 0);
    assert!(!view.column_scroll);
    // Row 0..27 columns 0..16 come from mapping 3 ($FFFF1A80); columns 16..64 stay zero.
    for row in 0..28 {
        assert_eq!(
            &view.plane_b[row * 64..row * 64 + 16],
            &assets.mappings[3].words[row * 16..(row + 1) * 16],
            "row {row}"
        );
        assert!(
            view.plane_b[row * 64 + 16..(row + 1) * 64]
                .iter()
                .all(|&w| w == 0)
        );
    }
    assert!(view.plane_a.iter().all(|&w| w == 0));
}

#[test]
fn stage_one_clears_header_and_name_palettes_and_runs_schedule() {
    let assets = synthetic_assets();
    let mut credits = EndingCredits::new(&assets);
    let mut cram = Cram::default();
    cram.write_slice(0, &assets.palettes[0].words);
    cram.write_slice(PALETTE_LINE_4_START, &assets.palettes[1].words);
    cram.write(PALETTE_TEXT_SLOT, 0x0EEE);
    let mut cycle = 7u8;
    let mut frame = 0xA0DFu16;

    // First tick of stage 1 clears Palette_Line_4, word 47 and cycle_index,
    // stages header 0 on Plane A and waits for LoadWindowTiles's DMA frame.
    frame = frame.wrapping_add(1);
    assert!(!credits.begin_stage(STAGE1, frame, &assets, &mut cram, &mut cycle));
    assert_eq!(&cram.view().words[PALETTE_LINE_4_START..64], &[0; 16]);
    assert_eq!(cram.view().words[PALETTE_TEXT_SLOT], 0);
    assert_eq!(cycle, 0);
    assert_eq!(credits.view().cred_text_a, 0);
    assert_eq!(
        credits.view().plane_a[5 * 64 + 17],
        0,
        "not visible before DMA"
    );

    // Second tick completes LoadWindowTiles's DMAPlane_A_VInt and bumps CredText_A.
    frame = frame.wrapping_add(1);
    assert!(!credits.tick(frame, &assets, &mut cram));
    assert_eq!(credits.view().cred_text_a, 1);
    assert_eq!(credits.view().plane_a[5 * 64 + 17], 0xE7C0);
    assert_eq!(credits.view().plane_a[5 * 64 + 18], 0xE7C1);

    // Run to the end of synthetic stage 1 (two 64-count cycles).
    let mut saw_header_fade = false;
    let mut saw_name_fade = false;
    let mut saw_row_copy = false;
    let initial_row1 = credits.view().plane_b[64..80].to_vec();
    loop {
        frame = frame.wrapping_add(1);
        if credits.tick(frame, &assets, &mut cram) {
            break;
        }
        if cram.view().words[PALETTE_HEADER_START] != 0 {
            saw_header_fade = true;
        }
        if cram.view().words[PALETTE_TEXT_SLOT] != 0 {
            saw_name_fade = true;
        }
        if credits.view().plane_b[64..80] != initial_row1 {
            saw_row_copy = true;
        }
    }
    assert!(
        saw_header_fade,
        "CreditsFadeInFirstBlock must brighten line 4"
    );
    assert!(
        saw_name_fade,
        "CreditsFadeInSecondBlock must brighten word 47"
    );
    assert!(
        saw_row_copy,
        "bit 19 must replace Plane B row 1 from $FFFF1000"
    );
    assert_eq!(credits.view().text_counter, 0x80);
    assert_eq!(credits.view().row_counter, 0x10);
    assert_eq!(credits.view().bg_y_scroll, 0xFFA0);
    // Counter 0x00 and 0x40 each fire opcode 0 eight times (plus final 0x80 tick).
    assert_eq!(credits.view().cred_text_a, 0x10);
    assert_eq!(credits.view().cred_text_b, 0x10);
    assert!(!credits.view().column_scroll);
}

#[test]
fn stages_two_and_three_preserve_state_enable_column_scroll_and_latch_terminal_flag() {
    let assets = synthetic_assets();
    let mut credits = EndingCredits::new(&assets);
    let mut cram = Cram::default();
    cram.write_slice(0, &assets.palettes[0].words);
    cram.write_slice(PALETTE_LINE_4_START, &assets.palettes[1].words);
    let mut cycle = 0u8;
    let mut frame = 0xA0DFu16;

    let s1 = run_stage(
        &mut credits,
        STAGE1,
        &mut frame,
        &assets,
        &mut cram,
        &mut cycle,
    );
    assert!(s1 > 1000);
    let s2 = run_stage(
        &mut credits,
        STAGE2,
        &mut frame,
        &assets,
        &mut cram,
        &mut cycle,
    );
    assert!(s2 > 1000);
    assert!(credits.view().column_scroll);
    assert_eq!(credits.view().text_counter, 0x100);
    assert_eq!(credits.view().row_counter, 0x10);
    assert_eq!(credits.view().fg_y_scroll, 0x0080);
    assert_eq!(credits.view().bg_y_scroll, 0xFFE0);
    assert_eq!(credits.view().cred_text_a, 0x20);
    assert_eq!(credits.view().cred_text_b, 0x20);
    // Plane A row 0 columns 0..16 hold mapping 4 ($FFFF2000) row 0.
    assert_eq!(
        &credits.view().plane_a[0..16],
        &assets.mappings[4].words[0..16]
    );

    let s3 = run_stage(
        &mut credits,
        STAGE3,
        &mut frame,
        &assets,
        &mut cram,
        &mut cycle,
    );
    assert!(s3 > 1000);
    assert_eq!(credits.view().text_counter, 0x180);
    assert_eq!(credits.view().row_counter, 0x20);
    assert_eq!(credits.view().fg_y_scroll, 0x0100);
    assert_eq!(credits.view().bg_y_scroll, 0x0020);
    // Stage 3 runs opcodes 9, 10, 11 (8 times each, except the first tick of 0x100
    // already entered with counter 0x00 so 7 + 8 + 8 = 23 header increments).
    assert_eq!(credits.view().cred_text_a, 0x37);
    assert_eq!(credits.view().cred_text_b, 0x20);
    assert_eq!(
        credits.view().fade_control & 0xC0,
        0xC0,
        "bits 6 and 7 latched"
    );
    // Stage 3 opcodes 10 and 11 placed headers at rows $14 (col $13) and $16 (col $1A).
    assert_eq!(credits.view().plane_a[0x14 * 64 + 0x13], 0xE7C0 + 5);
    assert_eq!(credits.view().plane_a[0x16 * 64 + 0x1A], 0xE7C0 + 6);
    // Terminal fade-in left row 7 of the fade table in words 50..55.
    assert_eq!(
        &cram.view().words[PALETTE_HEADER_START..PALETTE_HEADER_START + 5],
        &assets.fade.words[35..40]
    );
    assert_eq!(cram.view().words[PALETTE_TEXT_SLOT], 0);
}

#[test]
fn negative_controls_wrong_command_table_and_missing_row_transfer_diverge() {
    let assets = synthetic_assets();
    let mut credits = EndingCredits::new(&assets);
    let mut cram = Cram::default();
    let mut cycle = 0u8;
    let mut frame = 0xA0DFu16;
    let _ = run_stage(
        &mut credits,
        STAGE1,
        &mut frame,
        &assets,
        &mut cram,
        &mut cycle,
    );
    let _ = run_stage(
        &mut credits,
        STAGE2,
        &mut frame,
        &assets,
        &mut cram,
        &mut cycle,
    );

    // Negative control 1: running stage 3 with table 1 instead of table 2
    // never latches bit 7 and never writes row $16 col $1A.
    let mut wrong_table = STAGE3;
    if let PresentationOp::EndingCreditsStage {
        commands_rom_addr, ..
    } = &mut wrong_table
    {
        *commands_rom_addr = 0x0007_A522;
    }
    let _ = run_stage(
        &mut credits,
        wrong_table,
        &mut frame,
        &assets,
        &mut cram,
        &mut cycle,
    );
    assert_eq!(credits.view().fade_control & 0x80, 0);
    assert_eq!(credits.view().plane_a[0x16 * 64 + 0x1A], 0);

    // Negative control 2: a bad scratch base leaves Plane A row 0 untransferred.
    let mut no_rows = EndingCredits::new(&assets);
    let mut bad_stage2 = STAGE2;
    if let PresentationOp::EndingCreditsStage { source_ram, .. } = &mut bad_stage2 {
        *source_ram = 0xFFFF_0000;
    }
    let mut f = 0u16;
    let _ = run_stage(
        &mut no_rows,
        bad_stage2,
        &mut f,
        &assets,
        &mut cram,
        &mut cycle,
    );
    assert!(no_rows.view().plane_a[0..16].iter().all(|&w| w == 0));
}
