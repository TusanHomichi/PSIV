//! The text loop, held against the cartridge.
//!
//! Two kinds of test. The synthetic ones drive [`TextFlow`] over segments
//! written by hand, one per rule of `RunText_CharacterLoop`. The corpus test
//! replays all 2,736 retail entries and demands that the flow's pages are
//! *exactly* the pages `psiv_tools.dialogue_pack.paginate` wrote into the pack
//! -- an independent transcription of the same routine, from the ROM. If the
//! two ever disagree, one of them is wrong about the cartridge.
//!
//! No Godot types are touched here: the node is untestable headless, the loop
//! it drives is not.

use super::{Opening, TextFlow};
use psiv_data::{Ctrl, DialogueEntry, DialogueSet, FlagScope, PageEnd, Segment};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn entry(segments: Vec<Segment>) -> DialogueEntry {
    DialogueEntry {
        id: 0,
        text: String::new(),
        segments,
        pages: Vec::new(),
    }
}

fn text(run: &str) -> Segment {
    Segment::Text(run.to_owned())
}

fn wait() -> Segment {
    Segment::Control(Ctrl::Wait {
        code: 0xFD,
        operands: Vec::new(),
    })
}

fn newline() -> Segment {
    Segment::Control(Ctrl::Newline {
        code: 0xFC,
        operands: Vec::new(),
    })
}

fn close() -> Segment {
    Segment::Control(Ctrl::Close {
        code: 0xF7,
        operands: Vec::new(),
    })
}

fn portrait(id: u8) -> Segment {
    Segment::Control(Ctrl::Portrait {
        code: 0xF4,
        operands: vec![id],
        id,
        position: None,
    })
}

fn delay(frames: u16) -> Segment {
    Segment::Control(Ctrl::Delay {
        code: 0xF9,
        operands: vec![frames as u8],
        frames,
    })
}

fn yes_no() -> Segment {
    Segment::Control(Ctrl::YesNo {
        code: 0xF5,
        operands: vec![1, 2],
        yes_entry: 1,
        no_entry: 2,
    })
}

fn flag_check(flag: u8, then_entry: u16) -> Segment {
    Segment::Control(Ctrl::FlagCheck {
        code: 0xFA,
        operands: vec![flag, then_entry as u8],
        flag,
        scope: FlagScope::EventFlag,
        then_entry,
    })
}

fn event(id: u16) -> Segment {
    Segment::Control(Ctrl::Event {
        code: 0xF6,
        operands: vec![0, id as u8],
        id,
    })
}

fn open(entry: &DialogueEntry) -> TextFlow {
    match TextFlow::open(entry) {
        Opening::Window(flow) => *flow,
        Opening::Jump(next) => panic!("unexpected preamble jump to {next}"),
        Opening::Event(id) => panic!("expected a window, got event {id}"),
        Opening::Silent => panic!("expected a window, got silence"),
    }
}

/// Every page the entry shows, driven the way a player drives it: run out any
/// delay, read the page, press accept.
fn pages(entry: &DialogueEntry) -> Vec<(Vec<String>, PageEnd)> {
    let Opening::Window(mut flow) = TextFlow::open(entry) else {
        return Vec::new();
    };
    let mut pages = Vec::new();
    while flow.is_open() {
        let mut guard = 0;
        while flow.is_holding() {
            flow.tick();
            guard += 1;
            assert!(guard < 1_000, "a delay that never ends");
        }
        let Some(end) = flow.page_end() else { break };
        pages.push((flow.lines().to_vec(), end));
        flow.advance();
        assert!(pages.len() < 1_000, "a message that never ends");
    }
    pages
}

// ---------------------------------------------------------------------------
// The rules, one at a time
// ---------------------------------------------------------------------------

#[test]
fn text_fills_the_window_and_waits_where_the_data_says() {
    let entry = entry(vec![
        text("Are you a hunter?"),
        wait(),
        text("Are you here to exterminate"),
        newline(),
        text("the monsters?"),
    ]);
    assert_eq!(
        pages(&entry),
        vec![
            (vec!["Are you a hunter?".to_owned()], PageEnd::Wait),
            (
                vec![
                    "Are you here to exterminate".to_owned(),
                    "the monsters?".to_owned()
                ],
                PageEnd::End
            ),
        ]
    );
}

#[test]
fn a_page_is_shown_before_it_is_advanced() {
    let entry = entry(vec![text("One"), wait(), text("Two")]);
    let mut flow = open(&entry);
    assert_eq!(flow.lines(), ["One"]);
    assert!(flow.is_waiting());
    assert!(flow.is_open());

    flow.advance();
    assert_eq!(flow.lines(), ["Two"]);
    assert!(!flow.is_waiting(), "the last page shows no arrow");
    assert_eq!(flow.page_end(), Some(PageEnd::End));
    assert!(flow.is_open());

    flow.advance();
    assert!(!flow.is_open(), "accept on the last page closes the window");
}

#[test]
fn a_line_breaks_by_itself_at_thirty_two_characters() {
    // No $FC anywhere: the column counter wraps and the line counter takes it.
    let entry = entry(vec![text(&"A".repeat(40))]);
    let (lines, end) = pages(&entry).remove(0);
    assert_eq!(lines, ["A".repeat(32), "A".repeat(8)]);
    assert_eq!(end, PageEnd::End);
}

#[test]
fn a_full_window_pages_mid_run() {
    // 65 characters: two full lines, then one more glyph with nowhere to go.
    let entry = entry(vec![text(&"B".repeat(65))]);
    assert_eq!(
        pages(&entry),
        vec![
            (vec!["B".repeat(32), "B".repeat(32)], PageEnd::Full),
            (vec!["B".to_owned()], PageEnd::End),
        ]
    );
}

#[test]
fn a_newline_landing_on_a_wrap_is_swallowed() {
    // The routine checks for $FC at the wrap so an explicit newline there does
    // not cost a second line break.
    let entry = entry(vec![text(&"C".repeat(32)), newline(), text("tail")]);
    assert_eq!(
        pages(&entry),
        vec![(vec!["C".repeat(32), "tail".to_owned()], PageEnd::End)]
    );
}

#[test]
fn a_wait_after_a_full_window_does_not_wait_twice() {
    // interrupt() swallows a following $FD: one page, one press.
    let entry = entry(vec![text(&"D".repeat(64)), wait(), text("next")]);
    assert_eq!(
        pages(&entry),
        vec![
            (vec!["D".repeat(32), "D".repeat(32)], PageEnd::Full),
            (vec!["next".to_owned()], PageEnd::End),
        ]
    );
}

#[test]
fn close_ends_the_page_and_the_next_one_opens_fresh() {
    let entry = entry(vec![text("bye"), close(), text("more")]);
    assert_eq!(
        pages(&entry),
        vec![
            (vec!["bye".to_owned()], PageEnd::Close),
            (vec!["more".to_owned()], PageEnd::End),
        ]
    );
}

#[test]
fn a_portrait_shows_and_hides() {
    let entry = entry(vec![
        portrait(6),
        text("Rika"),
        wait(),
        portrait(0),
        text("gone"),
    ]);
    let mut flow = open(&entry);
    assert_eq!(flow.portrait(), Some(6));
    flow.advance();
    assert_eq!(flow.portrait(), None, "$F4 id 0 hides the window");
}

#[test]
fn a_delay_holds_the_message_for_its_frames() {
    let entry = entry(vec![text("wait"), delay(3), text("for it")]);
    let mut flow = open(&entry);
    assert!(flow.is_holding());
    assert_eq!(flow.lines(), ["wait"]);
    assert_eq!(flow.page_end(), None, "a delay is not a page break");
    assert!(!flow.tick());
    assert!(!flow.tick());
    assert!(flow.tick(), "the third tick runs the message on");
    assert_eq!(flow.lines(), ["waitfor it"]);
    assert_eq!(flow.page_end(), Some(PageEnd::End));
}

#[test]
fn a_choice_ends_the_message_and_says_so() {
    let entry = entry(vec![text("Well?"), yes_no(), text("unreachable in v1")]);
    let mut flow = open(&entry);
    assert_eq!(flow.page_end(), Some(PageEnd::Choice));
    let log = flow.drain_log();
    assert!(log.iter().any(|line| line.contains("yes_no")), "{log:?}");
    flow.advance();
    assert!(!flow.is_open());
}

#[test]
fn the_preamble_is_walked_before_anything_is_shown() {
    let entry = entry(vec![
        flag_check(218, 3),
        flag_check(52, 2),
        text("not-set branch"),
    ]);
    let mut flow = open(&entry);
    assert_eq!(flow.lines(), ["not-set branch"]);
    let log = flow.drain_log();
    assert_eq!(log.len(), 2, "both checks are reported, never dropped");
    assert!(log[0].contains("flag 218"), "{log:?}");
}

#[test]
fn an_event_entry_opens_no_window() {
    let entry = entry(vec![flag_check(1, 2), event(48), text("never shown")]);
    match TextFlow::open(&entry) {
        Opening::Event(48) => {}
        Opening::Event(other) => panic!("wrong event {other}"),
        _ => panic!("$F6 shows no window"),
    }
}

#[test]
fn an_empty_entry_opens_no_window() {
    assert!(matches!(
        TextFlow::open(&entry(Vec::new())),
        Opening::Silent
    ));
}

#[test]
fn an_action_is_logged_and_the_message_runs_on() {
    let entry = entry(vec![
        Segment::Control(Ctrl::Action {
            code: 0xF2,
            operands: vec![1, 131],
            action: psiv_data::ActionKind::LoadPanel,
            action_id: 0,
            sound: None,
            panel: Some(387),
            flag: None,
        }),
        text("after the panel"),
    ]);
    let mut flow = open(&entry);
    assert_eq!(flow.lines(), ["after the panel"]);
    let log = flow.drain_log();
    assert!(log.iter().any(|line| line.contains("387")), "{log:?}");
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

#[test]
fn every_retail_entry_pages_exactly_as_the_extractor_says() {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        eprintln!(
            "skipping: no dialogue pack at {}. Build one with \
             `python -m psiv_tools pack <rom> runtime-pack/`.",
            dir.display()
        );
        return;
    }
    let set = DialogueSet::load(&dir).expect("the dialogue pack loads");

    let mut compared = 0usize;
    let mut shown = 0usize;
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            let expected: Vec<(Vec<String>, PageEnd)> = entry
                .pages
                .iter()
                .map(|page| (page.lines.clone(), page.end))
                .collect();
            assert_eq!(
                pages(entry),
                expected,
                "{} entry {} paginates differently",
                tree.label,
                entry.id
            );
            compared += 1;
            shown += expected.len();
        }
    }
    assert_eq!(compared, 2736);
    // 6,038 windows across the corpus: 4,258 $FD waits, 1,538 message ends,
    // 126 $F7 closes, 89 full windows and 27 choices.
    assert_eq!(shown, 6038);
}

#[test]
fn the_corpus_never_needs_a_third_line_or_a_wrapped_word() {
    let dir = pack_dir();
    if !dir.join("dialogue").join("trees.json").is_file() {
        return;
    }
    let set = DialogueSet::load(&dir).expect("the dialogue pack loads");
    for tree in &set.trees.trees {
        for entry in &tree.entries {
            for (lines, _) in pages(entry) {
                assert!(lines.len() <= 2, "{} entry {}", tree.label, entry.id);
                for line in lines {
                    assert!(
                        line.chars().count() <= 32,
                        "{} entry {}",
                        tree.label,
                        entry.id
                    );
                }
            }
        }
    }
}
