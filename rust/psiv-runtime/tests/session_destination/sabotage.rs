//! Sabotage's one-row menu uses the same fade boundary as the shared menu.
use super::*;

#[test]
fn both_sabotage_answers_fade_once_before_teardown_and_keep_the_handoff() {
    for answer in [Button::Speak, Button::Cancel] {
        let story = Story {
            flags: &[0x08, 0x70, 0x72],
            towns: &[],
            world_word: 0x035A,
        };
        let Some(mut player) = Player::at((ZELAN, 31, 46), &story) else {
            return;
        };
        player.walk_into_menu(Button::Down);
        assert_eq!(player.rows(), [WORLD_KURAN]);
        let mut fade_at = None;
        let mut destroy_at = None;
        let mut chunks_at = None;
        let mut commit_at = None;
        let mut load_at = None;
        let mut battle = None;
        let mut fades = 0;
        let mut expected = player
            .runtime()
            .data()
            .ship_menu()
            .unwrap()
            .palette_words(player.session.destination_view().unwrap().palette())
            .unwrap();
        for offset in 0..3000 {
            // After confirming the menu, close the scene's dialogue with
            // ordinary fresh Speak edges, as the player does.
            let pad = if offset == 0 {
                Pad::new(answer)
            } else if player.runtime().dialogue_view().is_some() && offset % 2 == 0 {
                Pad::new(Button::Speak)
            } else {
                Pad::NEUTRAL
            };
            let frame = player.tick(pad);
            for event in &frame.events {
                match event {
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::FadeOut,
                        ..
                    } if load_at.is_none() => {
                        fade_at = Some(offset);
                        fades += 1;
                    }
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::PanelDestroyAll,
                        ..
                    } if load_at.is_none() => destroy_at = Some(offset),
                    RuntimeEvent::ScenePresentation {
                        op:
                            SceneOp::Presentation {
                                op: psiv_core::PresentationOp::ReloadMapChunks,
                            },
                        ..
                    } if load_at.is_none() => chunks_at = Some(offset),
                    RuntimeEvent::ScenePresentation {
                        op: SceneOp::CommitDestination,
                        ..
                    } => commit_at = Some(offset),
                    RuntimeEvent::MapChanged { .. } => {
                        assert!(load_at.replace(offset).is_none(), "one map handoff");
                        if answer == Button::Cancel {
                            assert_eq!(player.runtime().map_id().0, ZELAN);
                            assert_eq!(player.runtime().previous_map_id(), 0xFFFF);
                            assert!(player.runtime().game().is_clear(Flag::event(0x71)));
                        } else {
                            assert_eq!(player.runtime().map_id().0, 0x18C);
                            assert_eq!(player.runtime().previous_map_id(), 0xFFFF);
                        }
                    }
                    RuntimeEvent::SceneBattleStarted { index, .. } => battle = Some(*index),
                    _ => {}
                }
            }
            if fade_at.is_some() && load_at.is_none() {
                assert_eq!(
                    player.session.destination_view().unwrap().phase,
                    DestinationPhase::Fading
                );
                expected = expected.map(psiv_core::palette::fade_out_step);
                assert_eq!(&player.runtime().cram().words[..32], &expected);
                assert_eq!(player.runtime().world_index(), WORLD_ZELAN);
            }
            if battle.is_some() || (load_at.is_some() && !player.runtime().scene_active()) {
                break;
            }
        }
        let fade = fade_at.expect("menu fade");
        let load = load_at.expect("map handoff");
        assert_eq!(fades, 1);
        assert_eq!(destroy_at, Some(fade + 14));
        assert_eq!(chunks_at, destroy_at);
        assert_eq!(expected, [0; 32]);
        if answer == Button::Cancel {
            assert_eq!(
                (fade, load),
                (12, 55),
                "Cancel retains the old total budget"
            );
            assert!(commit_at.is_none() && battle.is_none());
            assert_eq!(player.runtime().world_index(), WORLD_ZELAN);
        } else {
            assert_eq!(load - fade, 18, "14 fade frames plus four post-fade frames");
            assert_eq!(commit_at, Some(load));
            assert_eq!(battle, Some(8));
            assert!(player.runtime().game().is_set(Flag::event(0x71)));
            assert_eq!(player.runtime().world_index(), WORLD_KURAN);
        }
        assert_eq!(
            player.runtime().game().party_members(),
            [CharId(0), CharId(5), CharId(3)]
        );
    }
}
