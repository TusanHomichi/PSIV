//! The op counts the scene documents state, held to the registry.
//!
//! A scene document names the constant that holds its ops and how many there
//! are: `` `LE_ROOF_AGAIN` (78 ops) `` on a `**Data:**` line or in a table
//! row. Every such statement under `docs/scenes` must equal that scene's
//! `ops.len()`, so a change that grows a scene fails until its document says
//! so (#632). The constant is tied to its scene by the `SCENES` list in
//! `scenes/mod.rs`, read in order beside the registry itself. A constant two
//! modules share (`RAJA_SICK`) is settled by the scene name the same line
//! gives in backticks.
//!
//! Counts written in prose ("~78 ops", "14 ops.") are not read: they carry no
//! constant to tie them to. `Event_GameStart` and `Event_MeetingHahn`, the two
//! scenes that diverge from their documents on purpose, state theirs that way
//! (`tests.rs`, `every_scene_matches_its_docs_op_count`).

use super::documented_ranges::repo;
use super::{SCENES, Scene};

/// One `` `CONST` (N ops) `` statement.
#[derive(Debug)]
struct Statement {
    /// The document's repository path and line, `PATH:LINE`.
    at: String,
    line: String,
    constant: String,
    ops: usize,
}

/// The constant names of `SCENES`, in order, read off `mod.rs`'s list.
fn registry_constants(source: &str) -> Vec<String> {
    let start = source
        .find("pub static SCENES: &[Scene] = &[")
        .expect("scenes/mod.rs declares SCENES");
    let body = &source[start..];
    let body = &body[body.find('\n').expect("SCENES spans lines") + 1..];
    let body = &body[..body.find("\n];").expect("SCENES ends with `];`")];
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(|line| {
            let path = line.strip_suffix(',').unwrap_or(line);
            path.rsplit("::").next().unwrap_or(path).to_owned()
        })
        .collect()
}

fn is_constant(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Every `` `CONST` (N ops) `` (also `(1 op)` and `(N typed ops)`) in `text`.
fn statements(file: &str, text: &str) -> Vec<Statement> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some(offset) = line[from..].find("` (") {
            let close = from + offset;
            from = close + 3;
            let Some(open) = line[..close].rfind('`') else {
                continue;
            };
            let constant = &line[open + 1..close];
            if !is_constant(constant) {
                continue;
            }
            let after = &line[close + 3..];
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                continue;
            }
            let rest = &after[digits..];
            if ![" ops)", " op)", " typed ops)"]
                .iter()
                .any(|unit| rest.starts_with(unit))
            {
                continue;
            }
            found.push(Statement {
                at: format!("{file}:{}", index + 1),
                line: line.to_owned(),
                constant: constant.to_owned(),
                ops: after[..digits].parse().expect("digits parse"),
            });
        }
    }
    found
}

/// The scene a statement names, or why it names none.
fn scene_of<'a>(
    statement: &Statement,
    constants: &[String],
    scenes: &'a [Scene],
) -> Result<&'a Scene, String> {
    let candidates: Vec<&Scene> = constants
        .iter()
        .zip(scenes)
        .filter(|(constant, _)| **constant == statement.constant)
        .map(|(_, scene)| scene)
        .collect();
    match candidates.as_slice() {
        [] => Err(format!(
            "{}: `{}` is no registered scene",
            statement.at, statement.constant
        )),
        [only] => Ok(only),
        several => {
            let named: Vec<&&Scene> = several
                .iter()
                .filter(|scene| statement.line.contains(&format!("`{}`", scene.name)))
                .collect();
            match named.as_slice() {
                [only] => Ok(only),
                _ => Err(format!(
                    "{}: `{}` is {} scenes; name one in backticks on the line",
                    statement.at,
                    statement.constant,
                    several.len()
                )),
            }
        }
    }
}

/// Every statement that names no scene or gives the wrong count.
fn disagreements(statements: &[Statement], constants: &[String], scenes: &[Scene]) -> Vec<String> {
    statements
        .iter()
        .filter_map(|statement| match scene_of(statement, constants, scenes) {
            Err(problem) => Some(problem),
            Ok(scene) => (scene.ops.len() != statement.ops).then(|| {
                format!(
                    "{}: `{}` ({} ops), but {} has {}",
                    statement.at,
                    statement.constant,
                    statement.ops,
                    scene.name,
                    scene.ops.len()
                )
            }),
        })
        .collect()
}

fn constants() -> Vec<String> {
    let source = std::fs::read_to_string(repo().join("rust/psiv-core/src/scenes/mod.rs"))
        .expect("read scenes/mod.rs");
    let constants = registry_constants(&source);
    assert_eq!(
        constants.len(),
        SCENES.len(),
        "the SCENES list read off mod.rs is the registry: {constants:?}"
    );
    constants
}

fn doc(name: &str) -> (String, String) {
    let file = format!("docs/scenes/{name}");
    let text = std::fs::read_to_string(repo().join(&file)).expect("read a scene doc");
    (file, text)
}

fn all_statements() -> Vec<Statement> {
    let mut files: Vec<_> = std::fs::read_dir(repo().join("docs/scenes"))
        .expect("docs/scenes")
        .map(|entry| entry.expect("a docs/scenes entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name.ends_with(".md"))
        .collect();
    files.sort();
    files
        .iter()
        .flat_map(|name| {
            let (file, text) = doc(name);
            statements(&file, &text)
        })
        .collect()
}

#[test]
fn every_scene_doc_op_count_matches_the_registry() {
    let statements = all_statements();
    // Most scene documents state their count this way; a parser that stopped
    // reading them would otherwise pass on nothing.
    assert!(
        statements.len() >= 130,
        "only {} `CONST` (N ops) statements read",
        statements.len()
    );
    let wrong = disagreements(&statements, &constants(), SCENES);
    assert!(wrong.is_empty(), "scene doc op counts: {wrong:#?}");
}

/// The issue's negative control: each of the two Data lines #629 left stale,
/// put back to its stale count, fails the check, and names that line.
#[test]
fn a_stale_data_line_fails_the_check() {
    let constants = constants();
    for (name, current, stale) in [
        (
            "52_LeRoofAgain.md",
            "`LE_ROOF_AGAIN` (78 ops)",
            "`LE_ROOF_AGAIN` (72 ops)",
        ),
        (
            "68_FindingAirCastle.md",
            "`FINDING_AIR_CASTLE` (55 ops)",
            "`FINDING_AIR_CASTLE` (50 ops)",
        ),
    ] {
        let (file, text) = doc(name);
        assert!(text.contains(current), "{file} states {current}");
        assert!(disagreements(&statements(&file, &text), &constants, SCENES).is_empty());
        let reverted = text.replacen(current, stale, 1);
        let wrong = disagreements(&statements(&file, &reverted), &constants, SCENES);
        assert_eq!(wrong.len(), 1, "{file}: {wrong:?}");
        assert!(wrong[0].starts_with(&format!("{file}:7: ")), "{wrong:?}");
    }
}

/// The other ways a statement can go wrong: a constant no scene has, a
/// shared constant the line does not settle, and the unit spellings the
/// documents use.
#[test]
fn statements_read_every_unit_and_refuse_what_names_no_scene() {
    let constants = constants();
    let read = statements(
        "synthetic",
        "`A_B` (1 op), `C` (396 typed ops) and `D_2` (12 ops); `x` (3 ops), `E` (~4 ops), `F` (5 bytes)",
    );
    let read: Vec<(&str, usize)> = read.iter().map(|s| (s.constant.as_str(), s.ops)).collect();
    assert_eq!(read, [("A_B", 1), ("C", 396), ("D_2", 12)]);

    let unknown = statements("synthetic", "`NO_SUCH_SCENE` (3 ops)");
    let wrong = disagreements(&unknown, &constants, SCENES);
    assert_eq!(
        wrong,
        ["synthetic:1: `NO_SUCH_SCENE` is no registered scene"]
    );

    // `RAJA_SICK` is both `Event_RajaSick` (11) and `Cutscene_RajaSick`.
    let bare = statements("synthetic", "`RAJA_SICK` (11 ops)");
    let wrong = disagreements(&bare, &constants, SCENES);
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert!(wrong[0].contains("is 2 scenes"), "{wrong:?}");
    let named = statements("synthetic", "`Event_RajaSick` | `RAJA_SICK` (11 ops)");
    assert!(disagreements(&named, &constants, SCENES).is_empty());
    let miscounted = statements("synthetic", "`Event_RajaSick` | `RAJA_SICK` (54 ops)");
    assert_eq!(disagreements(&miscounted, &constants, SCENES).len(), 1);
}
