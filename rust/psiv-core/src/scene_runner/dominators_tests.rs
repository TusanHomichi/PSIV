//! The Dominators' aftermath (`$91`): the ring around the terminal, the
//! barrier's palette rows and the fifth member's gap, split from
//! `mechanics_tests.rs` to keep both under the size limit.

use super::mechanics_tests::{
    CHAZ, DEMI, GRYZ, HAHN, KYRA, Played, RAJA, RIKA, RUNE, SETH, WREN, big_map, cast_at,
    no_layout, play, state_with,
};
use super::*;
use crate::Flag;

fn dominators(members: &[u8]) -> (Played, GameState) {
    let map = big_map();
    let mut state = state_with(members);
    let played = play(
        0x91,
        &mut state,
        &map,
        cast_at(members.len(), Cell::new(46, 46)),
        &no_layout,
    );
    (played, state)
}

/// The walk targets the scene sets, in order, as `(character, cell)`.
fn go_targets(played: &Played) -> Vec<(u8, Cell)> {
    played
        .effects
        .iter()
        .filter_map(|e| match e {
            SceneEffect::ActorMoveStarted {
                actor: ActorRef::Character(CharId(who)),
                to,
            } => Some((*who, *to)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_dominators_aftermath_seats_the_party_and_the_fifth_member_goes_to_the_gap() {
    // Pixel targets divided by 16, plus the one-row standing shift.
    let chaz = Cell::new(0x2C0 / 16, 0x2E0 / 16 + 1);
    let rune = Cell::new(0x2E0 / 16, 0x2E0 / 16 + 1);
    let rika = Cell::new(0x300 / 16, 0x2E0 / 16 + 1);
    let wren = Cell::new(0x2E0 / 16, 0x2B0 / 16 + 1);
    let gap = Cell::new(0x2F0 / 16, 0x2E0 / 16 + 1);
    for fifth in [HAHN, GRYZ, DEMI, RAJA, KYRA, SETH] {
        let members = [CHAZ, RUNE, RIKA, WREN, fifth];
        let (played, state) = dominators(&members);
        assert_eq!(played.faults(), vec![], "fifth {fifth}");
        assert_eq!(
            go_targets(&played)[..5],
            [
                (CHAZ, chaz),
                (RUNE, rune),
                (RIKA, rika),
                (WREN, wren),
                (fifth, gap)
            ],
            "fifth {fifth}"
        );
        // The dialogue runs to its yield, then resumes after the palette.
        assert_eq!(played.dialogues(), vec![5]);
        assert!(played.effects.contains(&SceneEffect::DialogueResume));
        assert!(state.is_set(Flag::event(0xBB)), "DaughterShutDown");
        // When the first window opens, after the 120-frame walk, each
        // member stands where it was sent: the ring around the terminal.
        let ring = &played.at_dialogue[0];
        for (slot, cell) in [chaz, rune, rika, wren, gap].into_iter().enumerate() {
            let seat = ring
                .iter()
                .find(|(actor, _)| *actor == ActorRef::PartyMember(slot))
                .unwrap();
            assert_eq!(seat.1, cell, "fifth {fifth} slot {slot}");
        }
        // The leader then walks to ($2E0,$2F0), the last thing it moves.
        assert_eq!(
            played.leader().cell,
            Cell::new(0x2E0 / 16, 0x2F0 / 16 + 1),
            "fifth {fifth}"
        );
    }
}

#[test]
fn the_dominators_barrier_writes_line_zero_words_twelve_and_thirteen() {
    use crate::PresentationOp;

    let (played, _) = dominators(&[CHAZ, RUNE, RIKA, WREN]);
    let palette: Vec<_> = played
        .effects
        .iter()
        .filter_map(|effect| match effect {
            SceneEffect::Presentation {
                op: SceneOp::Presentation { op },
            } if matches!(
                op,
                PresentationOp::SetPaletteWords { .. }
                    | PresentationOp::PaletteCycleFromTable { .. }
            ) =>
            {
                Some(*op)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        palette,
        [
            PresentationOp::SetPaletteWords {
                offset: 0x18 / 2,
                first: 0x06CE,
                second: 0x0CE0
            },
            PresentationOp::PaletteCycleFromTable {
                source_rom_addr: 0x0007_339A,
                offset: 0x18 / 2,
                second_skip: 8,
                steps: 4,
                frame_delay: 16,
            },
        ]
    );
    assert_eq!(palette[1].palette_loop_frames(), Some(64));
}

#[test]
fn the_dominators_aftermath_with_four_members_never_names_a_fifth() {
    let (played, _) = dominators(&[CHAZ, RUNE, RIKA, WREN]);
    assert_eq!(played.faults(), vec![]);
    assert_eq!(
        go_targets(&played).len(),
        4,
        "no fifth member, no fifth walk"
    );
}
