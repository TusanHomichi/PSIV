//! Isolated measured preemptive interval, separate from campaign evidence.
use super::*;
use psiv_core::battle::{Command, Lcg41, Side};

fn session() -> Option<Session> {
    let mut session = Session::new(super::player_tests::runtime()?);
    let start = session.debug_battle_window(0x8A, "preemptive");
    assert!(start.fault.is_none());
    assert!(
        start.advance_enemy_overlays,
        "origin loop; the loader owns its prior call"
    );
    let view = start.view.unwrap();
    assert_eq!(view.opening_transient_width, Some(0));
    assert!(!view.ready);
    assert_eq!(
        session.runtime.battle_pending_priority(),
        Some(Priority::Preemptive)
    );
    Some(session)
}

/// A frame-end count/seed sample describes the loop just run; the view's
/// plane writes reach cartridge video on the following frame. Thus this
/// view at offset 9 compares to f24850, and the poll is offset 60.
#[test]
fn measured_preemptive_interval_pins_every_seed_hold_and_visible_window() {
    let Some(mut session) = session() else { return };
    let mut seed = Lcg41::new(0x2C5A9843);
    let mut counted = 0;
    for offset in 1..=60 {
        let held = offset == 1 || offset == 56;
        let frame = session.frame(Pad::NEUTRAL).battle.unwrap();
        if !held {
            counted += 1;
            seed.step();
        }
        assert_eq!(frame.advance_enemy_overlays, !held, "offset {offset}");
        assert_eq!(session.runtime.frames, 23949 + counted, "offset {offset}");
        assert_eq!(session.runtime.rng_seed(), seed.seed(), "offset {offset}");
        assert_eq!(
            session.runtime.battle_pending_priority(),
            Some(Priority::Preemptive)
        );
        let view = frame.view.unwrap();
        let transient = match offset {
            1..=8 => Some(2 * offset),
            9..=45 => Some(18),
            46..=53 => Some(2 * (54 - offset)),
            54..=58 => Some(0),
            _ => None,
        };
        assert_eq!(view.opening_transient_width, transient, "offset {offset}");
        assert_eq!(
            view.message == "Surprise Attack!",
            (9..=45).contains(&offset)
        );
        assert_eq!(view.ready, offset >= 59);
        if offset < 55 {
            assert!(view.menu.is_none());
        } else if offset < 59 {
            let width = [0, 2, 4, 6][usize::from(offset - 55)];
            assert!(matches!(view.menu, Some(MenuView::Top { width: Some(w), .. }) if w == width));
        } else {
            assert!(matches!(view.menu, Some(MenuView::Top { width: None, .. })));
        }
    }
    assert_eq!(counted, 58);
    assert_eq!(seed.seed(), 0x7C0F6933, "US f24900 endpoint");
}

#[test]
fn preemptive_opening_ignores_edges_and_consumes_held_buttons_before_first_poll() {
    for button in Button::ALL {
        for held in [false, true] {
            let Some(mut session) = session() else { return };
            for offset in 1..=59 {
                let pad = if held || offset % 2 == 1 {
                    Pad::new(button)
                } else {
                    Pad::NEUTRAL
                };
                let view = session.frame(pad).battle.unwrap().view.unwrap();
                assert_eq!(view.cursor, 0, "{button:?} at {offset}");
                assert!(!matches!(view.menu, Some(MenuView::Commands(_))));
            }
            // The press on the last growing/full frame is consumed. Keeping
            // it held into the first poll cannot move or accept the cursor.
            let view = session
                .frame(Pad::new(button))
                .battle
                .unwrap()
                .view
                .unwrap();
            assert_eq!(view.cursor, 0);
            assert!(matches!(view.menu, Some(MenuView::Top { width: None, .. })));
            let released = session.frame(Pad::NEUTRAL).battle.unwrap().view.unwrap();
            assert!(
                matches!(released.menu, Some(MenuView::Top { width: None, .. })),
                "held press must not start closure"
            );
            let view = session
                .frame(Pad::new(button))
                .battle
                .unwrap()
                .view
                .unwrap();
            if button == Button::Down {
                assert_eq!(view.cursor, 1);
            }
            if button == Button::Up {
                assert_eq!(view.cursor, 2);
            }
            if matches!(button, Button::Speak | Button::Camp) {
                assert!(matches!(view.menu, Some(MenuView::Top { width: None, .. })));
                // The accepted frame still shows the whole options window;
                // its first two-cell close is visible on the next call.
                let closing = session.frame(Pad::NEUTRAL).battle.unwrap().view.unwrap();
                assert!(matches!(
                    closing.menu,
                    Some(MenuView::Top { width: Some(6), .. })
                ));
            }
        }
    }
    // Fresh confirm and direction edges on offset 60 are accepted.
    for button in [Button::Down, Button::Speak, Button::Camp] {
        let Some(mut session) = session() else { return };
        for _ in 1..=59 {
            session.frame(Pad::NEUTRAL);
        }
        let view = session
            .frame(Pad::new(button))
            .battle
            .unwrap()
            .view
            .unwrap();
        assert_eq!(view.cursor, usize::from(button == Button::Down));
        if button != Button::Down {
            assert!(matches!(view.menu, Some(MenuView::Top { width: None, .. })));
        }
        assert_eq!(session.runtime.rng_seed(), 0x7C0F6933);
        if button != Button::Down {
            let closing = session.frame(Pad::NEUTRAL).battle.unwrap().view.unwrap();
            assert!(matches!(
                closing.menu,
                Some(MenuView::Top { width: Some(6), .. })
            ));
        }
    }
}

#[test]
fn preemptive_first_round_keeps_only_party_actors_then_clears_priority() {
    let Some(mut session) = session() else { return };
    for _ in 1..=60 {
        session.frame(Pad::NEUTRAL);
    }
    let events = session
        .runtime
        .battle_round(&RoundOrders::Commands(vec![Command::Defend; 5]))
        .unwrap();
    let order = events
        .iter()
        .find_map(|e| match e {
            BattleEvent::RoundBegan { order, .. } => Some(order),
            _ => None,
        })
        .unwrap();
    assert_eq!(order.len(), 3);
    assert!(order.iter().all(|id| id.side() == Side::Party));
    assert_eq!(
        session.runtime.battle_pending_priority(),
        Some(Priority::Normal)
    );
}
