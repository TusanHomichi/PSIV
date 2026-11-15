//! The byte loop yields at every timed `$F2`, in both text modes.

use super::*;
use crate::dialogue::tests::delay;

fn action(kind: ActionKind, id: u8, sound: Option<u8>, panel: Option<u16>) -> Segment {
    Segment::Control(Ctrl::Action {
        code: 0xF2,
        operands: vec![id],
        action: kind,
        action_id: id,
        sound,
        panel,
        flag: None,
    })
}

fn sound() -> Segment {
    action(ActionKind::LoadSound, 3, Some(0), None)
}

fn panel() -> Segment {
    action(ActionKind::LoadPanel, 0, None, Some(0x100))
}

fn actions(signals: Vec<DialogueSignal>) -> Vec<DialogueAction> {
    signals
        .into_iter()
        .filter_map(|signal| match signal {
            DialogueSignal::Action(action) => Some(action),
            _ => None,
        })
        .collect()
}

#[test]
fn each_vint_action_yields_at_its_glyph_in_both_text_loops() {
    for text3 in [false, true] {
        for timed in [
            sound(),
            action(ActionKind::LoadSound2, 4, Some(0), None),
            action(ActionKind::UpdatePalette, 6, None, None),
            action(ActionKind::PauseMusic, 8, None, None),
            action(ActionKind::ResumeMusic, 9, None, None),
            panel(),
        ] {
            let mut runner = DialogueRunner::new();
            let mut game = GameState::new();
            assert!(runner.open_resolved(
                &entry(vec![text("aaaa"), timed.clone(), set_flag(7), text("b")]),
                &game
            ));
            runner.set_text3(text3);
            for glyph in 1..=4 {
                assert!(actions(frame(&mut runner, &mut game, Pad::new(Button::Speak))).is_empty());
                assert_eq!(runner.view().unwrap().revealed, glyph);
            }
            assert!(!game.is_set(Flag::event(7)));
            assert_eq!(
                actions(frame(&mut runner, &mut game, Pad::new(Button::Speak))).len(),
                1,
                "{timed:?}"
            );
            assert_eq!(
                runner.view().unwrap().revealed,
                4,
                "the VInt draws no glyph"
            );
            assert!(
                !game.is_set(Flag::event(7)),
                "a later control must wait too"
            );
            assert_eq!(
                actions(frame(&mut runner, &mut game, Pad::new(Button::Speak))),
                [DialogueAction::SetEventFlag(7)]
            );
            assert!(game.is_set(Flag::event(7)));
            assert_eq!(runner.view().unwrap().revealed, 5);
        }
    }
}

#[test]
fn consecutive_actions_each_spend_a_frame_before_the_delay_starts() {
    let mut runner = DialogueRunner::new();
    let mut game = GameState::new();
    assert!(runner.open_resolved(
        &entry(vec![sound(), panel(), delay(5), sound(), text("x")]),
        &game
    ));
    let mut emitted = Vec::new();
    let mut first_glyph = None;
    for tick in 1..=9 {
        for action in actions(frame(&mut runner, &mut game, Pad::new(Button::Speak))) {
            emitted.push((tick, action));
        }
        if runner.view().unwrap().revealed == 1 && first_glyph.is_none() {
            first_glyph = Some(tick);
        }
    }
    assert_eq!(
        emitted,
        [
            (1, DialogueAction::LoadSound(0)),
            (2, DialogueAction::LoadPanel(0x100)),
            (8, DialogueAction::LoadSound(0))
        ]
    );
    assert_eq!(first_glyph, Some(9));
}

#[test]
fn panel_destroys_wait_only_when_the_global_stack_is_nonempty() {
    for (kind, id, all) in [
        (ActionKind::DestroyLastPanel, 1, false),
        (ActionKind::DestroyAllPanels, 2, true),
    ] {
        for initial in 0_u8..=2 {
            let mut runner = DialogueRunner::new();
            let mut game = GameState::new();
            for _ in 0..initial {
                runner.scene_panel_op(&psiv_core::SceneOp::PanelCreate { id: 1 });
            }
            assert!(runner.open_resolved(
                &entry(vec![action(kind, id, None, None), set_flag(8), text("x")]),
                &game
            ));
            let emitted = actions(frame(&mut runner, &mut game, Pad::new(Button::Speak)));
            assert_eq!(
                emitted,
                if initial == 0 {
                    vec![DialogueAction::SetEventFlag(8)]
                } else if all {
                    vec![DialogueAction::DestroyAllPanels]
                } else {
                    vec![DialogueAction::DestroyLastPanel]
                }
            );
            assert_eq!(game.is_set(Flag::event(8)), initial == 0);
            assert_eq!(
                runner.panels,
                if all { 0 } else { initial.saturating_sub(1) }
            );
            if initial > 0 {
                frame(&mut runner, &mut game, Pad::new(Button::Speak));
                assert!(game.is_set(Flag::event(8)));
            }
        }
    }
}

#[test]
fn measured_panel_lag_freezes_the_loop_without_spending_counted_frames() {
    for (id, lag) in [(0x143, 4), (0x144, 11)] {
        let mut runner = DialogueRunner::new();
        let mut game = GameState::new();
        assert!(runner.open_resolved(
            &entry(vec![
                action(ActionKind::LoadPanel, 0, None, Some(id)),
                set_flag(9),
                text("x")
            ]),
            &game
        ));
        assert!(actions(frame(&mut runner, &mut game, Pad::new(Button::Speak))).is_empty());
        assert_eq!(runner.lag, lag);
        for _ in 0..lag {
            assert!(runner.lag_frame());
            assert!(actions(frame(&mut runner, &mut game, Pad::NEUTRAL)).is_empty());
            assert!(!game.is_set(Flag::event(9)));
            assert_eq!(runner.view().unwrap().revealed, 0);
        }
        assert!(!runner.lag_frame());
        assert_eq!(
            actions(frame(&mut runner, &mut game, Pad::new(Button::Speak))),
            [
                DialogueAction::LoadPanel(id),
                DialogueAction::SetEventFlag(9)
            ]
        );
        assert_eq!(runner.view().unwrap().revealed, 1);
    }
}

#[test]
fn blanking_vram_does_not_reset_the_retail_panel_counter() {
    // `InitVRAMAndCRAMAfterFadeOut` clears the planes, not `Panel_Num`.
    let mut runner = DialogueRunner::new();
    runner.scene_panel_op(&psiv_core::SceneOp::PanelCreate { id: 1 });
    runner.scene_panel_op(&psiv_core::SceneOp::InitVramAndCram);
    assert_eq!(runner.panels, 1);
    runner.scene_panel_op(&psiv_core::SceneOp::PanelDestroyLast);
    assert_eq!(runner.panels, 0);
}

#[test]
fn each_window_returns_the_retail_panel_count_including_retained_pixels() {
    use psiv_core::DialogueWindow;
    for (window, left) in [
        (DialogueWindow::Standard, 0),
        (DialogueWindow::Retained, 0),
        (DialogueWindow::Cutscene, 0),
        (DialogueWindow::Cutscene5, 0),
        (DialogueWindow::Cutscene3, 1),
        (DialogueWindow::EndingIntro, 1),
        (DialogueWindow::Ending, 1),
    ] {
        let mut runner = DialogueRunner::new();
        runner.scene_panel_op(&psiv_core::SceneOp::PanelCreate { id: 1 });
        runner.return_panels(Some(window));
        assert_eq!(runner.panels, left, "{window:?}");
    }
}
