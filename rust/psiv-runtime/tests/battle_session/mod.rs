//! Shared by the battle session tests: the pack, the frame loop, and the
//! waits for the battle windows to open and close (#51).

use std::path::Path;

use psiv_runtime::{BattleView, Button, CommandMenuView, Frame, MenuView, Pad, Session};

pub const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// Tape 07's formation: two Zoran Bults at positions `$0E` and `$1A`.
pub const TAPE_07_FORMATION: u16 = 0x8A;

pub fn pack() -> Option<&'static Path> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    Some(pack)
}

/// One frame the way the shipped shell runs it: one call, the session's
/// frame, which routes its own windows.
pub fn tick(session: &mut Session, pad: Pad) -> Frame {
    session.frame(pad)
}

/// Presses one button and releases it, so the press is a fresh edge, then
/// waits out the windows the press closed and opened: `Battle_DestroyWindow`
/// and `Battle_CreateWindow` take a few frames that read no button (#51).
/// Returns the first frame after them.
pub fn tap(session: &mut Session, button: Button) -> Frame {
    tick(session, Pad::new(button));
    let mut released = tick(session, Pad::NEUTRAL);
    for _ in 0..64 {
        if !growing(&released) {
            break;
        }
        released = tick(session, Pad::NEUTRAL);
    }
    released
}

/// Whether the frame's command window is still opening.
pub fn growing(frame: &Frame) -> bool {
    matches!(
        view_of(frame).and_then(|view| view.menu.as_ref()),
        Some(MenuView::Commands(menu)) if menu.settling
    ) || matches!(
        view_of(frame).and_then(|view| view.menu.as_ref()),
        Some(MenuView::Top { width: Some(_), .. })
    )
}

/// Neutral frames until no window is opening or closing.
pub fn settle_windows(session: &mut Session) {
    for _ in 0..64 {
        if !growing(&tick(session, Pad::NEUTRAL)) {
            return;
        }
    }
    panic!("the windows never settled");
}

/// The view of a battle frame, for the caller's assertions.
pub fn view(frame: &Frame) -> &BattleView {
    frame
        .battle
        .as_ref()
        .and_then(|battle| battle.view.as_ref())
        .expect("a battle frame carries a view")
}

pub fn view_of(frame: &Frame) -> Option<&BattleView> {
    frame
        .battle
        .as_ref()
        .and_then(|battle| battle.view.as_ref())
}

/// Waits out the dwell beats until the command window is open and whole.
pub fn wait_for_menu(session: &mut Session) -> BattleView {
    for _ in 0..2_000 {
        let frame = tick(session, Pad::NEUTRAL);
        if let Some(view) = view_of(&frame)
            && view.ready
            && !growing(&frame)
        {
            return view.clone();
        }
    }
    panic!("the command window never opened");
}

/// The command menu of `frame`, which must have one.
pub fn commands_of(frame: &Frame) -> CommandMenuView {
    match view(frame).menu.clone() {
        Some(MenuView::Commands(menu)) => *menu,
        other => panic!("the command window is open: {other:?}"),
    }
}
