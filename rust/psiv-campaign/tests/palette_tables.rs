//! Scene table records expanded from the pack, without a source copy of the
//! cartridge's colours. These are table/scene-contract tests, not play evidence.

mod common;

use psiv_core::{EventIndex, PresentationOp, SceneOp, palette, scene_for};

fn table(source: u32) -> Option<Vec<u16>> {
    let dir = common::pack_dir();
    let Ok(text) = std::fs::read_to_string(dir.join("presentation/panels.json")) else {
        eprintln!("runtime pack not present at {}; skipping", dir.display());
        return None;
    };
    let manifest: serde_json::Value = serde_json::from_str(&text).expect("panels.json parses");
    let record = manifest["palettes"]
        .as_array()
        .expect("palettes")
        .iter()
        .find(|r| r["rom_addr"] == format!("0x{source:06X}"))
        .expect("the pack carries the scene's table");
    let hex = record["raw_hex"].as_str().expect("raw_hex");
    let words: Vec<_> = (0..hex.len() / 4)
        .map(|i| u16::from_str_radix(&hex[4 * i..4 * i + 4], 16).expect("hex"))
        .collect();
    assert_eq!(words.len(), record["words"].as_u64().unwrap() as usize);
    Some(words)
}

#[test]
fn vahals_barrier_expands_four_rows_from_the_pack() {
    let scene = scene_for(EventIndex(0x91)).expect("registered");
    let (source, offset, skip, steps, delay) = scene
        .ops
        .iter()
        .find_map(|op| match op {
            SceneOp::Presentation {
                op:
                    PresentationOp::PaletteCycleFromTable {
                        source_rom_addr,
                        offset,
                        second_skip,
                        steps,
                        frame_delay,
                    },
            } => Some((
                *source_rom_addr,
                *offset,
                *second_skip,
                *steps,
                *frame_delay,
            )),
            _ => None,
        })
        .expect("the barrier loop");
    assert_eq!(
        (source, offset, skip, steps, delay),
        (0x07339A, 12, 8, 4, 16)
    );
    let Some(words) = table(source) else { return };
    assert_eq!(words.len(), 9);
    assert!(
        words[4] & 0x8000 != 0,
        "the first series ends in a terminator"
    );
    let writes = palette::palette_cycle_writes(&words, skip, steps).expect("all reads fit");
    assert_eq!(writes.len(), 4);
    for (row, pair) in writes.iter().enumerate() {
        assert_eq!(*pair, (words[row], words[row + 5]), "row {row}");
    }
    // The initial immediate pair and the first table row are separate writes
    // in the cartridge, even though their colours match.
    let initial: Vec<_> = scene
        .ops
        .iter()
        .filter_map(|op| match op {
            SceneOp::Presentation {
                op: PresentationOp::SetPaletteWords { first, second, .. },
            } => Some((*first, *second)),
            _ => None,
        })
        .collect();
    assert_eq!(initial, [writes[0]]);
    assert_eq!(
        palette::palette_cycle_writes(&words[..8], skip, steps),
        None
    );
}

#[test]
fn zios_barrier_expands_three_series_from_the_pack() {
    let scene = scene_for(EventIndex(0x30)).expect("registered");
    let (source, destination, steps, delay) = scene
        .ops
        .iter()
        .find_map(|op| match op {
            SceneOp::Presentation {
                op:
                    PresentationOp::PaletteRampFromTable {
                        source_rom_addr,
                        destination_ram,
                        steps,
                        frame_delay,
                    },
            } => Some((*source_rom_addr, *destination_ram, *steps, *frame_delay)),
            _ => None,
        })
        .expect("the barrier ramp");
    assert_eq!(
        (source, destination, steps, delay),
        (0x06EE16, 0xFFFF_FB60, 7, 30)
    );
    let Some(words) = table(source) else { return };
    assert_eq!(words.len(), 21);
    let writes = palette::palette_ramp_writes(&words, steps).expect("all reads fit");
    assert_eq!(writes.len(), 7);
    for (row, triple) in writes.iter().enumerate() {
        assert_eq!(
            *triple,
            [words[row], words[row + 7], words[row + 14]],
            "row {row}"
        );
    }
    assert_eq!(palette::palette_ramp_writes(&words[..20], steps), None);
}
