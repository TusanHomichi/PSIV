//! #51: the battle command windows open and close as the cartridge draws
//! them, two cells a frame, and read no button meanwhile
//! (`docs/battle/BATTLE_COMMAND_UI.md`). Split from `session_battle.rs`.

#[allow(dead_code)]
mod battle_session;

use battle_session::*;
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{Button, CommandMenuView, MenuPage, MenuView, Pad, Session};

/// Neutral frames while the window grows: the width each one drew, and the
/// first frame it was whole.
fn grow(session: &mut Session) -> (Vec<u8>, CommandMenuView) {
    let mut widths = Vec::new();
    for _ in 0..16 {
        let menu = commands_of(&tick(session, Pad::NEUTRAL));
        match menu.growing {
            Some(growing) => widths.push(growing.width),
            None => return (widths, menu),
        }
    }
    panic!("the window never finished growing: {widths:?}");
}

/// #51: `Battle_CreateWindow` (`ps4.asm:1657`) grows a window two cells a
/// frame, centred, draws it whole once the width reaches the window's, and
/// only then does the window's routine read a button. Tape 32: TECH accepted
/// at f25165 (`Battle_Routine_2` 7), the window `8007` growing f25166-f25170
/// and whole at f25171, `Battle_TechWindow` (8) from f25172; the video shows
/// 2, 4, 6, 8 and 10 cells at f25167-f25171 and the list at f25172.
#[test]
fn the_strip_and_the_technique_window_grow_before_they_read_a_button() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    let mut session = Session::start(GameData::load(pack).expect("pack loads"))
        .with_battles(files)
        .field()
        .expect("the pack boots");
    let opening = session.debug_battle_window(TAPE_07_FORMATION, "top");
    assert!(opening.fault.is_none(), "{:?}", opening.fault);
    wait_for_menu(&mut session);

    // COMD at A (tape 32: f25001): the main options still show, then close
    // 6, 4, 2 cells wide (`Battle_DestroyWindow`; video f25003-f25005).
    let pressed = view(&tick(&mut session, Pad::new(Button::Speak)))
        .menu
        .clone();
    assert!(
        matches!(pressed, Some(MenuView::Top { width: None, .. })),
        "{pressed:?}"
    );
    let mut closing = Vec::new();
    for _ in 0..3 {
        match view(&tick(&mut session, Pad::NEUTRAL)).menu.clone() {
            Some(MenuView::Top {
                width: Some(width), ..
            }) => closing.push(width),
            other => panic!("the main options are closing: {other:?}"),
        }
    }
    assert_eq!(closing, [6, 4, 2]);
    // A+4 they are gone and the 16-cell strip starts; its setup frame (A+5)
    // draws nothing either, and its first two cells are A+6's (f25007).
    for frame in ["gone", "setup"] {
        let opened = commands_of(&tick(&mut session, Pad::NEUTRAL));
        let growing = opened.growing.expect("the strip opens growing");
        assert_eq!(
            (growing.window, growing.width, growing.full_width),
            (MenuPage::Actions, 0, 16),
            "{frame}"
        );
        assert!(opened.strip.is_none());
    }
    let (widths, strip) = grow(&mut session);
    assert_eq!(widths, [2, 4, 6, 8, 10, 12, 14]);
    let drawn = strip.strip.expect("the whole strip, icons and all");
    assert_eq!(drawn.age, 0, "the cursor object is created on this frame");

    // Right to TECH, then accept: the 12-cell window grows over the strip.
    tick(&mut session, Pad::new(Button::Right));
    tick(&mut session, Pad::NEUTRAL);
    let accepted = commands_of(&tick(&mut session, Pad::new(Button::Speak)));
    assert_eq!(accepted.page, MenuPage::Techniques);
    let growing = accepted.growing.expect("the list opens growing");
    assert_eq!((growing.width, growing.full_width), (0, 12));
    assert!(accepted.strip.is_some() && accepted.list.is_none());
    let mut widths = Vec::new();
    for frame in 0..5 {
        // A Down pressed while the window grows is not read.
        let pad = if frame == 2 {
            Pad::new(Button::Down)
        } else {
            Pad::NEUTRAL
        };
        let menu = commands_of(&tick(&mut session, pad));
        let growing = menu.growing.expect("still growing");
        widths.push((growing.width, growing.left_offset()));
        assert!(menu.list.is_none());
    }
    assert_eq!(widths, [(2, 5), (4, 4), (6, 3), (8, 2), (10, 1)]);
    let whole_frame = tick(&mut session, Pad::NEUTRAL);
    let whole = commands_of(&whole_frame);
    assert!(whole.growing.is_none());
    assert!(
        !view(&whole_frame).cursor_red,
        "the list's routine has not run: its row is blue"
    );
    assert_eq!(
        whole.list.expect("the list, whole").cursor,
        0,
        "the Down was not read"
    );
    let moved_frame = tick(&mut session, Pad::new(Button::Down));
    assert!(
        view(&moved_frame).cursor_red,
        "the routine runs and a held button shows red"
    );
    let moved = commands_of(&moved_frame);
    assert_eq!(
        moved.list.expect("the list").cursor,
        1,
        "the next frame reads it"
    );
}

/// #51: the frame a list is drawn whole is `Battle_CreateWindow`'s, not the
/// list routine's, so its selected row is blue whatever the blink last
/// showed; the routine runs the next frame and decides it
/// (`Battle_UpdateRedCursor2`, `ps4.asm:1572`). The strip fixture starts the
/// blink red, so a stale value would show. TECH is covered above; this is
/// SKILL and ITEM.
#[test]
fn the_skill_and_item_windows_show_no_red_cursor_until_their_routine_runs() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    for (icon, page) in [(2, MenuPage::Skills), (3, MenuPage::Items)] {
        let mut session = Session::start(GameData::load(pack).expect("pack loads"))
            .with_battles(files.clone())
            .field()
            .expect("the pack boots");
        let opening =
            session.debug_battle_window(TAPE_07_FORMATION, &format!("strip,cursor={icon}"));
        assert!(opening.fault.is_none(), "{:?}", opening.fault);
        tick(&mut session, Pad::NEUTRAL);
        let accepted = commands_of(&tick(&mut session, Pad::new(Button::Speak)));
        assert_eq!(accepted.page, page);
        let mut frames = 0;
        let whole = loop {
            let frame = tick(&mut session, Pad::NEUTRAL);
            if commands_of(&frame).growing.is_none() {
                break frame;
            }
            frames += 1;
            assert!(frames < 16, "{page:?} never finished growing");
        };
        assert!(commands_of(&whole).list.is_some(), "{page:?} is whole");
        assert!(
            !view(&whole).cursor_red,
            "{page:?}: the whole frame's row is blue"
        );
        let next = tick(&mut session, Pad::new(Button::Down));
        assert!(
            view(&next).cursor_red,
            "{page:?}: the routine runs and a held button shows red"
        );
    }
}

/// Each frame's opening or closing window: its page, its width (`None` once
/// whole) and the acting fighter.
fn window_frames(session: &mut Session, frames: usize) -> Vec<(MenuPage, Option<u8>, Option<u8>)> {
    (0..frames)
        .map(|_| {
            let menu = commands_of(&tick(session, Pad::NEUTRAL));
            (menu.page, menu.growing.map(|g| g.width), menu.actor)
        })
        .collect()
}

/// #51: `Battle_DestroyWindow` (`ps4.asm:1678`) narrows a window two cells a
/// frame and moves on the frame it is gone. Tape 37 (tape 32, then every way
/// the windows close), with A the press:
/// - B in the technique list (f25505): 10, 8, 6, 4 and 2 cells at A+1..A+5
///   (video f25507-f25511), the strip again at A+6, read from A+7;
/// - DEFEND (f25605): the strip 14 down to 2 at A+1..A+7, gone at A+8, and
///   the next member's strip's first two cells at A+9 (video f25615);
/// - B on that strip (f25689): the same, back to the previous member;
/// - SANER (f25853): the list 10..2 at A+1..A+5, the strip whole at A+6, then
///   14..2 at A+7..A+13, gone at A+14, the next strip at A+15 (video f25869).
#[test]
fn the_windows_close_two_cells_a_frame_before_the_next_one_opens() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    let mut session = Session::start(GameData::load(pack).expect("pack loads"))
        .with_battles(files)
        .field()
        .expect("the pack boots");
    let opening = session.debug_battle_window(TAPE_07_FORMATION, "top");
    assert!(opening.fault.is_none(), "{:?}", opening.fault);
    wait_for_menu(&mut session);
    tap(&mut session, Button::Speak);
    tap(&mut session, Button::Right);
    let listed = commands_of(&tap(&mut session, Button::Speak));
    assert_eq!(listed.page, MenuPage::Techniques);
    let alys = listed.actor;

    let tech = MenuPage::Techniques;
    let strip = MenuPage::Actions;
    // B in the list.
    tick(&mut session, Pad::new(Button::Cancel));
    let frames = window_frames(&mut session, 6);
    let list_closing: Vec<_> = [10, 8, 6, 4, 2].map(|w| (tech, Some(w), alys)).into();
    assert_eq!(frames[..5], list_closing[..]);
    assert_eq!(frames[5], (strip, None, alys), "the strip again");
    let back = commands_of(&tick(&mut session, Pad::new(Button::Right)));
    assert_eq!(back.cursor, 2, "the strip reads the next frame's press");
    tick(&mut session, Pad::NEUTRAL);

    // Right twice more to DEFEND, then accept: the strip closes and the next
    // member's opens.
    tap(&mut session, Button::Right);
    tap(&mut session, Button::Right);
    tick(&mut session, Pad::new(Button::Speak));
    let frames = window_frames(&mut session, 9);
    let strip_closing: Vec<_> = [14, 12, 10, 8, 6, 4, 2]
        .map(|w| (strip, Some(w), alys))
        .into();
    assert_eq!(frames[..7], strip_closing[..]);
    let chaz = frames[7].2;
    assert_ne!(chaz, alys, "the next member answers");
    assert_eq!(
        frames[7..],
        [(strip, Some(0), chaz), (strip, Some(2), chaz)]
    );
    settle_windows(&mut session);
    let settled = commands_of(&tap(&mut session, Button::Cancel));
    // B on that strip: it closes and Alys's reopens.
    assert_eq!(
        settled.actor, alys,
        "Cancel goes back to the previous member"
    );

    // TECH and the first technique (SANER: no target): the list closes, the
    // strip shows whole for a frame, then closes, then the next strip opens.
    for _ in 0..3 {
        tap(&mut session, Button::Left);
    }
    let listed = commands_of(&tap(&mut session, Button::Speak));
    assert_eq!((listed.page, listed.cursor), (tech, 0));
    tick(&mut session, Pad::new(Button::Speak));
    let frames = window_frames(&mut session, 15);
    assert_eq!(frames[..5], list_closing[..]);
    assert_eq!(
        frames[5],
        (strip, None, alys),
        "the strip whole for a frame"
    );
    assert_eq!(frames[6..13], strip_closing[..]);
    assert_eq!(
        frames[13..],
        [(strip, Some(0), chaz), (strip, Some(2), chaz)]
    );
}
