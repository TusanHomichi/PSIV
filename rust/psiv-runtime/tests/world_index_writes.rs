//! Every cartridge write to `World_Index` has a cited native owner (#126).
//!
//! The world byte decides the RYUKA/TELEPIPE town lists and the ship's
//! destination list, so a write the runtime misses strands a later save on the
//! wrong planet. `Runtime::set_world_index` (`src/travel.rs`) is the one
//! runtime writer, and its documentation cites every write the scenes and the
//! destination menu make; the new-game initializer's own write is the pack's
//! `world_index` start field (`psiv-pack/src/game_start/mod.rs`).
//!
//! [`WRITES`] is the US disassembly's list. Each one must be cited by an
//! owner, and with the local disassembly present the list must be exactly the
//! writes it holds, so a newly found write cannot be dropped silently.

use std::path::Path;

/// The disassembly lines that write `World_Index`: the new-game initializer,
/// the destination menu and `Cutscene_SpaceshipSabotage`'s copy of it, then
/// `Cutscene_CrashLaanding`, `Cutscene_DarkForce1Defeated`, the Eclipse
/// Torch's return, `Cutscene_GumbiousBishop` and `Cutscene_Reunion`.
const WRITES: [usize; 8] = [
    88670, 133677, 155599, 155847, 156396, 156940, 157065, 157791,
];

const OWNERS: [&str; 2] = [
    include_str!("../src/travel.rs"),
    include_str!("../../psiv-pack/src/game_start/mod.rs"),
];

/// Whether `text` cites `line` as `ps4.asm:N` or a continuing `:N`.
fn cites(text: &str, line: usize) -> bool {
    let needle = format!(":{line}");
    text.match_indices(&needle).any(|(at, _)| {
        let before = &text[..at];
        let after = text[at + needle.len()..].chars().next();
        let continues = before.ends_with("ps4.asm")
            || !before
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '$' || c == '_');
        continues && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// An instruction that writes `World_Index`: its last operand, unless the
/// instruction only compares or tests, or the one operand of a clear.
fn writes_world_index(line: &str) -> bool {
    let code = line.split(';').next().unwrap_or_default().trim();
    let Some((mnemonic, operands)) = code.split_once(char::is_whitespace) else {
        return false;
    };
    let operands = operands.trim();
    if ["cmp", "tst", "btst"]
        .iter()
        .any(|m| mnemonic.starts_with(m))
    {
        return false;
    }
    let destination = if mnemonic.starts_with("clr") {
        operands
    } else {
        match operands.rsplit_once(',') {
            Some((_, last)) => last.trim(),
            None => return false,
        }
    };
    destination.starts_with("(World_Index)")
}

#[test]
fn every_world_index_write_is_cited_by_its_native_owner() {
    let missing: Vec<usize> = WRITES
        .into_iter()
        .filter(|&line| !OWNERS.iter().any(|text| cites(text, line)))
        .collect();
    assert_eq!(missing, [], "World_Index writes no native owner cites");
}

#[test]
fn the_pattern_reads_destinations_only() {
    for line in [
        "\tmove.b\t#3, (World_Index).w",
        "\tclr.w\t(World_Index).w",
        "\tmove.b\t(a0,d0.w), (World_Index).w",
    ] {
        assert!(writes_world_index(line), "{line}");
    }
    for line in [
        "\tmove.b\t(World_Index).w, d0",
        "\tcmpi.b\t#2, (World_Index).w",
        "\tbtst\t#0, (World_Index).w",
        "\ttst.b\t(World_Index).w",
        "\tmove.b\t(World_Index).w, ($FFFFEC29).w",
        "\tmove.b\t#1, (World_Index).w ; a comment, (World_Index)",
    ] {
        let expected = line.contains("; a comment");
        assert_eq!(writes_world_index(line), expected, "{line}");
    }
    assert!(cites("(`ps4.asm:133677`)", 133677));
    assert!(cites("`:155599` in", 155599));
    assert!(!cites(&format!("ps4.asm:{}0", 133_677), 133_677));
    assert!(!cites("$F:133677", 133677));
}

#[test]
fn the_list_is_the_disassemblys_writes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/ps4disasm/ps4.asm");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("the disassembly is a local input; skipping");
        return;
    };
    let text = String::from_utf8_lossy(&bytes);
    let found: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| writes_world_index(line))
        .map(|(index, _)| index + 1)
        .collect();
    assert_eq!(found, WRITES);
}
