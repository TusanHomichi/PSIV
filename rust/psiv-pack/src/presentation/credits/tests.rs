use super::*;
use crate::test_support::retail_rom;

#[test]
fn retail_credits_calls_establish_base_words_uploads_and_record_addresses() {
    let Some(rom) = retail_rom("credits call contracts") else {
        return;
    };
    let r = rom.bytes();
    // Independently assemble the six LEA / MOVE.W #d0 / LEA / JSR EniDecomp
    // calls. This catches treating $100 and $2200 as output sizes.
    let mut at = 0x79DD2;
    for (source, base, destination) in [
        (0x1DF09Cu32, 0x100u16, 0xFFFF1000u32),
        (0x1DF17C, 0x100, 0xFFFF1380),
        (0x1DF294, 0x100, 0xFFFF1700),
        (0x1DF3E0, 0x100, 0xFFFF1A80),
        (0x1DF3E0, 0x100, 0xFFFF1E00),
        (0x1DF4C2, 0x2200, 0xFFFF2000),
    ] {
        let mut instruction = vec![0x41, 0xF9]; // lea absolute.l,a0
        instruction.extend(source.to_be_bytes());
        instruction.extend([0x30, 0x3C]); // move.w #base,d0
        instruction.extend(base.to_be_bytes());
        instruction.extend([0x43, 0xF9]); // lea absolute.l,a1
        instruction.extend(destination.to_be_bytes());
        instruction.extend([0x4E, 0xB9, 0, 4, 0x1A, 0x90]);
        assert_eq!(&r[at..at + instruction.len()], instruction);
        at += instruction.len();
    }
    // EniDecomp begins MOVEM / MOVEA.W d0,a3: caller's base is retained;
    // its two header words each use ADDA.W a3, not an output-count loop.
    assert!(crate::rom::matches_pin(
        r,
        0x41A90,
        (
            6,
            "3d8d76aa8b7f6a187eee69be516927672a26286df8941b65138e97b13b840b89"
        ),
    ));
    assert!(crate::rom::matches_pin(
        r,
        0x41AA8,
        (
            8,
            "b0a990e4192e89e1ebab92f61c02dc33ca3fd82c6028401549f1f600ea777511"
        ),
    ));
    for (at, pointer) in [(0x7A346, 0x1FEE26u32), (0x7A452, 0x1FEB42)] {
        assert_eq!(&r[at..at + 2], &[0x41, 0xF9]);
        assert_eq!(&r[at + 2..at + 6], pointer.to_be_bytes());
    }
    let a = extract(&rom).unwrap();
    assert_eq!(
        a.art
            .iter()
            .map(|a| (a.tiles.len(), a.consumed))
            .collect::<Vec<_>>(),
        [(2784, 1994), (2144, 1121), (928, 499)]
    );
    assert_eq!(
        a.mappings
            .iter()
            .map(|m| (m.words.len(), m.consumed))
            .collect::<Vec<_>>(),
        [(448, 224), (448, 280), (448, 332), (448, 226), (448, 106)]
    );
}

#[test]
fn retail_credits_preserve_all_raw_records_attributes_and_terminators() {
    let Some(rom) = retail_rom("credits address coverage") else {
        return;
    };
    let a = extract(&rom).unwrap();
    let r = rom.bytes();
    for palette in a.palettes.iter().chain([&a.fade]) {
        let bytes: Vec<_> = palette.words.iter().flat_map(|w| w.to_be_bytes()).collect();
        let at = palette.rom_addr as usize;
        assert_eq!(bytes, r[at..at + bytes.len()]);
    }
    for record in a.headers.iter().chain(&a.names) {
        let at = record.rom_addr as usize;
        assert_eq!(record.bytes, r[at..at + record.bytes.len()]);
        assert_eq!(record.bytes.last(), Some(&0xFF));
        assert!(!record.bytes[..record.bytes.len() - 1].contains(&0xFF));
    }
    for table in &a.commands {
        let at = table.rom_addr as usize;
        assert_eq!(table.bytes, r[at..at + table.bytes.len()]);
        assert_eq!(&table.bytes[table.bytes.len() - 2..], &[0xFF, 0]);
    }
    // Empty records are meaningful ordinals, not omitted text.
    assert_eq!(a.names[0].bytes, [0xFF]);
    assert_eq!(a.headers[0].rom_addr, 0x1FEE26);
    assert_eq!(
        a.names.last().unwrap().rom_addr + a.names.last().unwrap().bytes.len() as u32,
        0x1FEE25
    );
    assert_eq!(
        a.headers.last().unwrap().rom_addr + a.headers.last().unwrap().bytes.len() as u32,
        0x1FF066
    );
    assert!(
        a.mappings
            .iter()
            .flat_map(|m| &m.words)
            .any(|w| w & 0x1800 == 0x1800)
    );
    assert!(a.mappings[4].words.iter().all(|w| (w >> 13) & 3 == 1));
    // First Enigma command: header literal + caller's base; first incremental
    // run later in the stream and H/V inline branches are covered by decoder
    // synthetic vectors. Attribute bits must survive unchanged here.
    for m in &a.mappings {
        let literal = word(r, m.rom_addr as usize + 4).unwrap();
        assert_eq!(m.words[0], literal.wrapping_add(m.base_tile));
    }
}

#[test]
fn credits_emission_loads_as_the_consumers_type() {
    let Some(rom) = retail_rom("credits typed loading") else {
        return;
    };
    let root = crate::test_support::scratch("credits");
    build(&rom, &root).unwrap();
    let a = EndingCreditsAssets::load(&root, crate::rom::EXPECTED_SHA256).unwrap();
    assert_eq!(a, extract(&rom).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

// Independent MSB-first format interpreter, deliberately without the retail
// decoder's rolling 16-bit window or refill/rotation implementation.
fn bit_mapping(bytes: &[u8], base: u16) -> Vec<u16> {
    let mut bit = 48;
    let mut take = |count: usize| -> u16 {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | u16::from((bytes[bit / 8] >> (7 - bit % 8)) & 1);
            bit += 1;
        }
        value
    };
    let mut incremental = u16::from_be_bytes([bytes[2], bytes[3]]).wrapping_add(base);
    let literal = u16::from_be_bytes([bytes[4], bytes[5]]).wrapping_add(base);
    let mut words = Vec::new();
    loop {
        let mode = if take(1) == 0 { take(1) } else { 4 + take(2) };
        let count = take(4);
        if mode == 7 && count == 15 {
            return words;
        }
        let mut value = 0;
        for i in 0..=count {
            if mode == 0 {
                words.push(incremental);
                incremental = incremental.wrapping_add(1);
            } else if mode == 1 {
                words.push(literal);
            } else {
                if i == 0 || mode == 7 {
                    let mut attributes = base;
                    for (flag, word) in [(2, 0x1000), (1, 0x800)] {
                        if bytes[1] & flag != 0 && take(1) != 0 {
                            attributes |= word;
                        }
                    }
                    value = take(bytes[0] as usize).wrapping_add(attributes);
                }
                words.push(value);
                if mode == 5 {
                    value = value.wrapping_add(1);
                }
                if mode == 6 {
                    value = value.wrapping_sub(1);
                }
            }
        }
        assert!(words.len() <= 448, "independent stream did not terminate");
    }
}

#[test]
fn independent_credits_bit_examples_preserve_every_tile_attribute() {
    let Some(rom) = retail_rom("credits independent bit examples") else {
        return;
    };
    let a = extract(&rom).unwrap();
    for mapping in &a.mappings {
        let at = mapping.rom_addr as usize;
        assert_eq!(
            mapping.words,
            bit_mapping(&rom.bytes()[at..], mapping.base_tile)
        );
    }
    let mut changed = a.mappings[0].words.clone();
    changed[0] ^= 0x800;
    assert_ne!(
        changed,
        bit_mapping(&rom.bytes()[0x1DF09C..], 0x100),
        "attribute negative control"
    );
    assert_ne!(
        a.mappings[4].words,
        bit_mapping(&rom.bytes()[0x1DF4C2..], 0),
        "base negative control"
    );
}

#[test]
fn referenced_text_matches_independently_encoded_local_reference_records() {
    let Some(rom) = retail_rom("credits referenced text") else {
        return;
    };
    let reference = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reference/ps4disasm/ps4.asm"
    ));
    if !reference.is_file() {
        eprintln!("local disassembly absent; independent credits text comparison skipped");
        return;
    }
    let asm = std::fs::read_to_string(reference).unwrap();
    let a = extract(&rom).unwrap();
    for (label, records, header) in [
        ("CreditTextNames:", &a.names, false),
        ("CreditTextHeaders:", &a.headers, true),
    ] {
        // Last definition is the western revision, not the Japanese branch.
        let section = asm.rsplit_once(label).unwrap().1;
        let mut expected = Vec::new();
        for line in section.lines().skip(1) {
            let line = line.trim();
            if line.starts_with("even") || line == "charset" {
                break;
            }
            let Some(body) = line.strip_prefix("dc.b") else {
                continue;
            };
            for token in body.split(',').map(str::trim) {
                if let Some(hex) = token.strip_prefix('$') {
                    expected.push(u8::from_str_radix(hex, 16).unwrap());
                } else if token.starts_with('"') {
                    for ch in token.trim_matches('"').bytes() {
                        expected.push(if header {
                            match ch {
                                b' ' => 0,
                                b'-' => 0x9A,
                                b'A'..=b'Z' => ch - b'A' + 0x80,
                                _ => panic!("header charset"),
                            }
                        } else {
                            match ch {
                                b' ' => 0,
                                b'.' => 0x35,
                                b'A'..=b'Z' => ch - b'A' + 1,
                                b'a'..=b'z' => ch - b'a' + 27,
                                _ => panic!("name charset"),
                            }
                        });
                    }
                } else {
                    panic!("unexpected credits directive");
                }
            }
        }
        let actual: Vec<_> = records
            .iter()
            .flat_map(|r| r.bytes.iter().copied())
            .collect();
        assert_eq!(actual, expected);
        expected[0] ^= 1;
        assert_ne!(actual, expected, "independent text negative control");
    }
}

#[test]
fn all_three_core_stages_resolve_their_pack_inputs_and_text_ordinals() {
    use psiv_core::{EventIndex, PresentationOp, SceneOp};
    use std::collections::BTreeSet;
    let Some(rom) = retail_rom("credits stage references") else {
        return;
    };
    let a = extract(&rom).unwrap();
    let scene = psiv_core::scene_for(EventIndex(0x8021)).unwrap();
    let mut stages = 0;
    for op in scene.ops {
        if let SceneOp::Presentation {
            op:
                PresentationOp::EndingCreditsStage {
                    source_ram,
                    commands_rom_addr,
                    ..
                },
        } = op
        {
            stages += 1;
            assert!(
                a.mappings
                    .iter()
                    .any(|m| m.ram_destinations.contains(source_ram))
            );
            assert!(a.commands.iter().any(|c| c.rom_addr == *commands_rom_addr));
        }
    }
    assert_eq!(stages, 3);
    // Eight calls per TextCounter tick; header counter advances per draw,
    // name counter per fourth line. Stop on the third stage's end command.
    let (mut header, mut name) = (0usize, 0usize);
    let (mut headers, mut names) = (BTreeSet::new(), BTreeSet::new());
    'ticks: for tick in 0..0x480 {
        let table = &a.commands[usize::from(tick >= 0x400)];
        let opcode = table
            .bytes
            .as_chunks::<2>()
            .0
            .iter()
            .find(|p| usize::from(p[0]) == tick & 63)
            .map(|p| p[1]);
        if opcode == Some(12) {
            break 'ticks;
        }
        for _ in 0..8 {
            match opcode {
                Some(0 | 9..=11) => {
                    headers.insert(header >> 3);
                    header += 1;
                }
                Some(op @ 1..=4) => {
                    names.insert((name >> 3) * 4 + usize::from(op) - 1);
                    if op == 4 {
                        name += 1;
                    }
                }
                _ => (),
            }
        }
    }
    assert_eq!(headers, (0..a.headers.len()).collect());
    assert_eq!(names, (0..a.names.len()).collect());
}
