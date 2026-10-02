//! The text loop, held against the cartridge, one rule at a time.
//!
//! Every test drives [`TextFlow`] over segments written by hand, one per rule
//! of `RunText_CharacterLoop`. The corpus half -- all 2,736 retail entries
//! against the extractor's own pagination -- is in `tests_corpus`.
//!
//! No engine types are touched here: the loop is pure, and a [`GameState`] is
//! all it needs to read its branches.

use super::{Opening, TextFlow};
use psiv_core::{Flag, GameState};
use psiv_data::{ActionKind, Ctrl, DialogueEntry, DialogueSet, FlagScope, PageEnd, Segment};
use std::path::PathBuf;

/// A fresh game state: no event flags set, which is the state the extractor's
/// own pagination was decoded in.
fn no_flags() -> GameState {
    GameState::new()
}

/// The local runtime pack, for the rules that are about retail data rather
/// than about a synthetic entry.
pub(super) fn pack_dir() -> PathBuf {
    match std::env::var_os("PSIV_RUNTIME_PACK") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("runtime-pack"),
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

pub(super) fn entry(segments: Vec<Segment>) -> DialogueEntry {
    DialogueEntry {
        id: 0,
        text: String::new(),
        segments,
        pages: Vec::new(),
    }
}

pub(super) fn text(run: &str) -> Segment {
    Segment::Text(run.to_owned())
}

pub(super) fn wait() -> Segment {
    Segment::Control(Ctrl::Wait {
        code: 0xFD,
        operands: Vec::new(),
    })
}

pub(super) fn newline() -> Segment {
    Segment::Control(Ctrl::Newline {
        code: 0xFC,
        operands: Vec::new(),
    })
}

pub(super) fn close() -> Segment {
    Segment::Control(Ctrl::Close {
        code: 0xF7,
        operands: Vec::new(),
    })
}

pub(super) fn portrait(id: u8) -> Segment {
    Segment::Control(Ctrl::Portrait {
        code: 0xF4,
        operands: vec![id],
        id,
        position: None,
    })
}

pub(super) fn delay(frames: u16) -> Segment {
    Segment::Control(Ctrl::Delay {
        code: 0xF9,
        operands: vec![frames as u8],
        frames,
    })
}

pub(super) fn yes_no() -> Segment {
    Segment::Control(Ctrl::YesNo {
        code: 0xF5,
        operands: vec![1, 2],
        yes_entry: 1,
        no_entry: 2,
    })
}

pub(super) fn flag_check(flag: u8, then_entry: u16) -> Segment {
    Segment::Control(Ctrl::FlagCheck {
        code: 0xFA,
        operands: vec![flag, then_entry as u8],
        flag,
        scope: FlagScope::EventFlag,
        then_entry,
    })
}

pub(super) fn event(id: u16) -> Segment {
    Segment::Control(Ctrl::Event {
        code: 0xF6,
        operands: vec![0, id as u8],
        id,
    })
}

pub(super) fn open(entry: &DialogueEntry) -> TextFlow {
    match TextFlow::open(entry) {
        Opening::Window(flow) => *flow,
        Opening::Jump(next) => panic!("unexpected preamble jump to {next}"),
        Opening::Event(id) => panic!("expected a window, got event {id}"),
        Opening::Silent => panic!("expected a window, got silence"),
    }
}

/// Every page the entry shows, driven the way a player drives it: run out any
/// delay, read the page, press accept.
pub(super) fn pages(entry: &DialogueEntry) -> Vec<(Vec<String>, PageEnd)> {
    let game = no_flags();
    let Opening::Window(mut flow) = TextFlow::open(entry) else {
        return Vec::new();
    };
    let mut pages = Vec::new();
    while flow.is_open() {
        while flow.take_pending_action().is_some() {
            flow.resume_after_action(&game);
        }
        let mut guard = 0;
        while flow.is_holding() {
            flow.tick(&game);
            while flow.take_pending_action().is_some() {
                flow.resume_after_action(&game);
            }
            guard += 1;
            assert!(guard < 1_000, "a delay that never ends");
        }
        let Some(end) = flow.page_end() else { break };
        pages.push((flow.lines().to_vec(), end));
        // Static pagination stops at a choice; branch coverage answers each
        // alternative separately instead of inventing a default answer.
        if end == PageEnd::Choice {
            break;
        }
        flow.advance(&game);
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
    let game = no_flags();
    let entry = entry(vec![text("One"), wait(), text("Two")]);
    let mut flow = open(&entry);
    assert_eq!(flow.lines(), ["One"]);
    assert!(flow.is_waiting());
    assert!(flow.is_open());

    flow.advance(&game);
    assert_eq!(flow.lines(), ["Two"]);
    assert!(!flow.is_waiting(), "the last page shows no arrow");
    assert_eq!(flow.page_end(), Some(PageEnd::End));
    assert!(flow.is_open());

    flow.advance(&game);
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
fn scene_close_preserves_cursor_and_refreshes_flags_before_resuming() {
    // The resumed chunk's `$FA` reads the live bank: flag 7 is set here, and
    // the flow takes the branch the moment the resume pumps it.
    let mut game = no_flags();
    game.set(Flag::event(7)).unwrap();
    let entry = entry(vec![
        portrait(2),
        text("first"),
        wait(),
        text("pause"),
        close(),
        flag_check(7, 3),
        text("unchanged"),
    ]);
    let mut flow = open(&entry);
    assert!(
        !flow.pause_for_scene(),
        "ordinary wait stays inside the scene call"
    );
    flow.advance(&game);
    assert_eq!(flow.lines(), &["pause"]);
    assert!(flow.pause_for_scene());
    assert!(!flow.is_open());
    assert_eq!(
        flow.lines(),
        &["pause"],
        "next text cannot run before resume"
    );
    let Opening::Window(mut resumed) = flow.resume_scene(&game) else {
        panic!("saved middle of entry");
    };
    assert_eq!(resumed.take_jump(), Some(3));
    assert_eq!(
        resumed.portrait(),
        None,
        "the previous portrait was destroyed"
    );
}

#[test]
fn scene_resume_after_entry_terminator_opens_the_following_entry() {
    let game = no_flags();
    let mut entry = entry(vec![text("finished")]);
    entry.id = 18;
    let mut flow = open(&entry);
    assert!(flow.pause_for_scene());
    assert!(matches!(flow.resume_scene(&game), Opening::Jump(19)));
}

#[test]
fn after_igglanova_dialogue_has_three_scene_calls_and_no_lost_pages() {
    let dir = pack_dir();
    if !dir.join("dialogue/trees.json").is_file() {
        return;
    }
    let game = no_flags();
    let set = DialogueSet::load(&dir).unwrap();
    let entry = set.entry(33, 18).unwrap();
    let mut flow = open(entry);
    let mut chunks = vec![Vec::new()];
    loop {
        chunks.last_mut().unwrap().push(flow.lines().join("\n"));
        let end = flow.page_end();
        if flow.pause_for_scene() {
            if end == Some(PageEnd::End) {
                break;
            }
            let Opening::Window(next) = flow.resume_scene(&game) else {
                panic!("F7 must resume inside the entry");
            };
            flow = *next;
            chunks.push(Vec::new());
        } else {
            flow.advance(&game);
        }
        assert!(chunks.len() <= 3);
    }
    assert_eq!(chunks.iter().map(Vec::len).collect::<Vec<_>>(), [2, 4, 17]);
    assert_eq!(chunks[1][0], "What? But we destroyed the\nmonster...");
    assert_eq!(chunks[2][0], "What?");
    assert_eq!(chunks[2].last().unwrap(), "Hey!\nDon't leave me here!");
    assert_eq!(
        chunks.concat(),
        entry
            .pages
            .iter()
            .map(|p| p.lines.join("\n"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_portrait_shows_and_hides() {
    let game = no_flags();
    let entry = entry(vec![
        portrait(6),
        text("Rika"),
        wait(),
        portrait(0),
        text("gone"),
    ]);
    let mut flow = open(&entry);
    assert_eq!(flow.portrait(), Some(6));
    flow.advance(&game);
    assert_eq!(flow.portrait(), None, "$F4 id 0 hides the window");
}

#[test]
fn a_delay_holds_the_message_for_its_frames() {
    let game = no_flags();
    let entry = entry(vec![text("wait"), delay(3), text("for it")]);
    let mut flow = open(&entry);
    assert!(flow.is_holding());
    assert_eq!(flow.lines(), ["wait"]);
    assert_eq!(flow.page_end(), None, "a delay is not a page break");
    assert!(!flow.tick(&game));
    assert!(!flow.tick(&game));
    assert!(flow.tick(&game), "the third tick runs the message on");
    assert_eq!(flow.lines(), ["waitfor it"]);
    assert_eq!(flow.page_end(), Some(PageEnd::End));
}

#[test]
fn a_choice_requires_an_answer_and_branches_relative_to_its_entry() {
    let game = no_flags();
    let mut entry = entry(vec![text("Well?"), yes_no(), text("after choice")]);
    entry.id = 24;
    let mut flow = open(&entry);
    assert_eq!(flow.page_end(), Some(PageEnd::Choice));
    flow.advance(&game);
    assert!(flow.is_open());
    assert!(flow.has_choice());
    assert!(flow.answer_choice(false, &game));
    assert_eq!(flow.take_jump(), Some(26));
    assert!(
        !flow.answer_choice(true, &game),
        "an answer is consumed once"
    );
}

#[test]
fn choice_zero_continues_inside_entry_and_full_line_choices_keep_the_operands() {
    let game = no_flags();
    let choice = Segment::Control(Ctrl::YesNo {
        code: 0xF5,
        operands: vec![0, 1],
        yes_entry: 0,
        no_entry: 1,
    });
    let entry = entry(vec![
        portrait(2),
        text(&"Q".repeat(32)),
        choice,
        text("Yes response"),
    ]);
    let mut flow = open(&entry);
    assert!(
        flow.has_choice(),
        "choice immediately after column 32 is still a choice"
    );
    assert!(flow.answer_choice(true, &game));
    assert_eq!(flow.lines(), ["Yes response"]);
    assert_eq!(flow.portrait(), Some(2));
    assert_eq!(flow.page_end(), Some(PageEnd::End));
    let mut flow = open(&entry);
    flow.answer_choice(false, &game);
    assert_eq!(flow.take_jump(), Some(1));
}

#[test]
fn chaz_house_answers_reach_different_retail_responses_and_keep_alys_portrait() {
    let dir = pack_dir();
    if !dir.join("dialogue/trees.json").is_file() {
        return;
    }
    let game = no_flags();
    let set = DialogueSet::load(&dir).unwrap();
    for yes in [true, false] {
        let mut flow = open(set.entry(10, 49).unwrap());
        for _ in 0..10 {
            if flow.has_choice() {
                break;
            }
            flow.advance(&game);
        }
        assert!(flow.answer_choice(yes, &game));
        if let Some(next) = flow.take_jump() {
            assert_eq!(next, 50);
            flow.continue_at(set.entry(10, next).unwrap(), &game);
        }
        assert_eq!(flow.portrait(), Some(2));
        assert_eq!(
            flow.lines().join("\n"),
            if yes {
                "OK, let's be up bright\nand early tomorrow!"
            } else {
                "Well, shall we keep going\na little longer?"
            }
        );
    }
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
fn an_action_stops_at_its_retail_byte_position() {
    let game = no_flags();
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
    assert_eq!(flow.lines(), [""]);
    assert!(flow.action_ready(0));
    assert_eq!(
        flow.take_pending_action(),
        Some(super::DialogueAction::LoadPanel(387))
    );
    flow.resume_after_action(&game);
    assert_eq!(flow.lines(), ["after the panel"]);
}

#[test]
fn an_action_after_text_waits_for_the_typewriter() {
    let game = no_flags();
    let entry = entry(vec![
        text("before"),
        Segment::Control(Ctrl::Action {
            code: 0xF2,
            operands: vec![0, 0x30],
            action: ActionKind::LoadPanel,
            action_id: 0,
            sound: None,
            panel: Some(0x30),
            flag: None,
        }),
        text("after"),
    ]);
    let mut flow = open(&entry);
    assert_eq!(flow.lines(), ["before"]);
    assert!(!flow.action_ready(0));
    assert!(flow.action_ready(6));
    assert_eq!(
        flow.take_pending_action(),
        Some(super::DialogueAction::LoadPanel(0x30))
    );
    flow.resume_after_action(&game);
    assert_eq!(flow.lines(), ["beforeafter"]);
}
