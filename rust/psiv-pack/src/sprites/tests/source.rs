//! The reference fork is optional and is used only for the spans that the
//! retained Python tests proved equal to US retail; never trust its addresses.
use super::*;

fn source() -> Option<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm/ps4.asm");
    match std::fs::read(&path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local disassembly absent: sprite source tests skipped");
            None
        }
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}

fn labelled_bytes(text: &str, label: &str) -> Vec<u8> {
    let rest = text.split_once(&format!("\n{label}:")).unwrap().1;
    let mut bytes = Vec::new();
    for line in rest.lines() {
        let line = line.split(';').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let Some(values) = line.strip_prefix("dc.b") else {
            break;
        };
        bytes.extend(
            values
                .split(',')
                .map(|t| u8::from_str_radix(t.trim().trim_start_matches('$'), 16).unwrap()),
        );
    }
    bytes
}

#[test]
fn shared_mappings_party_headers_and_speed_words_agree_with_the_listing() {
    let Some(r) = retail() else { return };
    let Some(text) = source() else { return };
    let labels = [
        "IdleDown",
        "WalkDown_1",
        "WalkDown_2",
        "IdleLeft",
        "WalkLeft_1",
        "WalkLeft_2",
        "IdleUp",
        "WalkUp_1",
        "WalkUp_2",
        "IdleRight",
        "WalkRight_1",
        "WalkRight_2",
    ];
    let expected: Vec<_> = labels
        .iter()
        .flat_map(|l| labelled_bytes(&text, &format!("Mappings_Char{l}")))
        .collect();
    assert_eq!(expected.len(), 96);
    assert_eq!(&r.rom.bytes()[0x482a4..0x48304], expected);
    for sprite in &r.party {
        for (_, name) in FACINGS {
            let capitalized = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
            let expected = labelled_bytes(
                &text,
                &format!("SprMapsData_{}{capitalized}", sprite.symbol),
            );
            assert_eq!(expected.len(), 2);
            let at = sprite.sheet.sequences[&format!("walk_{name}")].sequence_offset;
            assert_eq!(&r.rom.bytes()[at..at + 2], expected);
        }
    }
    let table = text.split_once("\nFieldObj_MovementsTbl:").unwrap().1;
    let words: Vec<_> = table
        .lines()
        .filter_map(|line| {
            let values = line.trim().strip_prefix("dc.w")?;
            let (_, second) = values.split_once(',')?;
            second.trim().strip_prefix('$')?.get(..4).map(str::to_owned)
        })
        .collect();
    for (n, expected) in ["0100", "0200", "0400"].iter().enumerate() {
        assert_eq!(&words[n * 16 + 2], expected);
    }
    assert!(text.contains("andi.w\t#7, d1"));
}

#[test]
fn every_matched_reference_init_block_agrees_on_interaction() {
    let Some(r) = retail() else { return };
    let Some(text) = source() else { return };
    let lines: Vec<_> = text.lines().collect();
    let mut bodies = std::collections::BTreeMap::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(label) = line.strip_suffix(':') else {
            continue;
        };
        if !(label.starts_with("FieldObj_") || label.starts_with("loc_")) {
            continue;
        }
        if lines.get(i + 1) != Some(&"\tbset\t#7, (a4)") {
            continue;
        }
        let Some(target) = lines
            .get(i + 2)
            .and_then(|l| l.trim().strip_prefix("bne.s"))
            .map(str::trim)
        else {
            continue;
        };
        let tail = &lines[i + 3..];
        let end = tail
            .iter()
            .position(|l| {
                if target.starts_with('+') {
                    l.starts_with('+')
                } else {
                    l.starts_with(&format!("{target}:"))
                }
            })
            .unwrap_or(tail.len().min(100));
        bodies.insert(label, &tail[..end]);
    }
    let mut checked = 0;
    for routine in &r.routines {
        let field_label = format!("FieldObj_{}", routine.symbol);
        let Some(body) = bodies
            .get(field_label.as_str())
            .or_else(|| bodies.get(routine.symbol))
        else {
            continue;
        };
        let has = |op| {
            body.iter().any(|line| {
                ["$2", "render_flags"]
                    .iter()
                    .any(|field| line.starts_with(&format!("\t{op}\t#3, {field}(a4)")))
            })
        };
        assert_eq!(has("bset"), routine.interactable, "{}", routine.symbol);
        if !has("bset") {
            assert_eq!(
                has("bclr"),
                routine.interactable_source.contains("bclr"),
                "{}",
                routine.symbol
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 215);
}
