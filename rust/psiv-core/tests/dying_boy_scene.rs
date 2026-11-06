//! Scene outcomes for Torinco's dying boy (#81): `$86`
//! (`docs/scenes/163_DyingBoy.md`).

mod common;

use common::map_with;
use psiv_core::{
    ActorRef, Cell, CharId, Direction, EventIndex, Flag, GameState, PresentationOp, SceneEffect,
    SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const CULVERS: u16 = 0xA9;
const ALIS_SWORD: u16 = 0xAA;
const ITEM_ALIS_SWORD: u8 = 0x8A;
const CHAZ: CharId = CharId(0);
const RUNE: CharId = CharId(3);
const RIKA: CharId = CharId(5);
const DEMI: CharId = CharId(6);
const WREN: CharId = CharId(7);
const RAJA: CharId = CharId(8);

/// A 40-cell open room.
fn room() -> psiv_core::FieldMap {
    let row = ".".repeat(40);
    let rows: Vec<&str> = (0..40).map(|_| row.as_str()).collect();
    map_with(&rows, vec![], vec![])
}

/// A state with `party` in slot order, `flags` set and `items` carried.
fn state(party: &[CharId], flags: &[u16], items: &[u8]) -> GameState {
    let mut state = GameState::new();
    let mut slots = [None; 5];
    for (slot, who) in party.iter().enumerate() {
        slots[slot] = Some(*who);
    }
    state.set_party(slots);
    for flag in flags {
        state.set(Flag::event(*flag)).unwrap();
    }
    for item in items {
        state.inventory_mut().add(*item).unwrap();
    }
    state
}

/// Every effect of `$86`, with each tick's number, answering each window at
/// once. The party stands in a row below the bed.
fn play(state: &mut GameState) -> (Vec<(usize, SceneEffect)>, SceneRunner) {
    let scene = scene_for(EventIndex(0x86)).expect("$86 registered");
    let mut cast: Vec<ScriptedActor> = (0..5)
        .map(|slot| {
            ScriptedActor::new(
                ActorRef::PartyMember(slot),
                Cell::new(24 + slot as u16, 31),
                Direction::Up,
            )
        })
        .collect();
    cast.push(ScriptedActor::new(
        ActorRef::Npc(2),
        Cell::new(24, 28),
        Direction::Down,
    ));
    let mut runner = SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap());
    let map = room();
    let mut log = Vec::new();
    let mut input = SceneInput::None;
    for tick in 0..5_000 {
        let effects = runner.tick(&map, state, input);
        input = SceneInput::None;
        for effect in &effects {
            match effect {
                SceneEffect::DialogueOpen(_) | SceneEffect::DialogueResume => {
                    input = SceneInput::DialogueClosed;
                }
                SceneEffect::Faulted(fault) => panic!("$86: {fault:?}"),
                _ => {}
            }
        }
        log.extend(effects.into_iter().map(|e| (tick, e)));
        if runner.is_finished() {
            return (log, runner);
        }
    }
    panic!("$86 did not finish");
}

fn lines(log: &[(usize, SceneEffect)]) -> Vec<u16> {
    log.iter()
        .filter_map(|(_, e)| match e {
            SceneEffect::DialogueOpen(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

fn resumes(log: &[(usize, SceneEffect)]) -> usize {
    log.iter()
        .filter(|(_, e)| *e == SceneEffect::DialogueResume)
        .count()
}

fn party() -> [CharId; 5] {
    [CHAZ, RUNE, RIKA, WREN, RAJA]
}

/// `loc_72FAC`: once the sword has been used, entry `$3E` and nothing else.
#[test]
fn after_the_cure_the_boy_only_talks() {
    let mut s = state(&party(), &[CULVERS, ALIS_SWORD], &[ITEM_ALIS_SWORD]);
    let (log, _) = play(&mut s);
    assert_eq!(lines(&log), [0x3E]);
    assert!(s.inventory().slots().contains(&ITEM_ALIS_SWORD), "kept");
}

/// `loc_72FA4`: before Culvers has asked, or with no sword, entry `$3B`, and
/// the bag is untouched.
#[test]
fn before_the_father_asks_or_without_the_sword_he_only_talks() {
    let mut s = state(&party(), &[], &[ITEM_ALIS_SWORD]);
    let (log, _) = play(&mut s);
    assert_eq!(lines(&log), [0x3B]);
    assert!(s.inventory().slots().contains(&ITEM_ALIS_SWORD));

    let mut s = state(&party(), &[CULVERS], &[0x01]);
    let before = s.snapshot();
    let (log, _) = play(&mut s);
    assert_eq!(lines(&log), [0x3B]);
    assert_eq!(s.snapshot(), before);
}

/// `$072DCA..$072FA3`: the sword is taken, the party gathers, Chaz raises the
/// sword for the routine's 164 frames with his own sprite hidden, the text
/// resumes three times, Chaz ends where the leader's last walk takes him, and
/// `EventFlag_AlisSword` is set.
#[test]
fn the_sword_cures_the_boy() {
    let mut s = state(&party(), &[CULVERS], &[0x01, ITEM_ALIS_SWORD]);
    let (log, runner) = play(&mut s);
    assert_eq!(lines(&log), [0x33]);
    assert_eq!(resumes(&log), 3);
    assert!(s.is_set(Flag::event(ALIS_SWORD)));
    assert_eq!(
        s.inventory().slots()[..2],
        [0x01, 0x00],
        "the bag closes up"
    );

    let hidden = log
        .iter()
        .find(|(_, e)| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::Presentation {
                        op: PresentationOp::SetCharacterVisible { visible: false, .. }
                    }
                }
            )
        })
        .map(|(t, _)| *t)
        .expect("Chaz hidden");
    let shown = log
        .iter()
        .find(|(_, e)| {
            matches!(
                e,
                SceneEffect::Presentation {
                    op: SceneOp::Presentation {
                        op: PresentationOp::SetCharacterVisible { visible: true, .. }
                    }
                }
            )
        })
        .map(|(t, _)| *t)
        .expect("Chaz shown");
    assert_eq!(shown - hidden, 164, "the sword's frames");
    assert_eq!(
        runner.actor(ActorRef::PartyMember(0)).unwrap().cell,
        Cell::new(0x1A0 / 16, 0x1D0 / 16 + 1)
    );
}

/// `$072E06..$072E86`: Rune, Rika and Wren walk to their spots, and the fifth
/// member found (Raja, the first of Hahn..Seth present) to `($210,$1A0)`.
#[test]
fn the_party_gathers_at_the_bed() {
    let mut s = state(&party(), &[CULVERS], &[ITEM_ALIS_SWORD]);
    let (log, _) = play(&mut s);
    let started = |slot: usize| {
        log.iter().find_map(|(_, e)| match e {
            SceneEffect::ActorMoveStarted { actor, to }
                if *actor == ActorRef::Character(party()[slot])
                    || *actor == ActorRef::PartyMember(slot) =>
            {
                Some(*to)
            }
            _ => None,
        })
    };
    assert_eq!(
        started(1),
        Some(Cell::new(0x1E0 / 16, 0x1A0 / 16 + 1)),
        "Rune"
    );
    assert_eq!(
        started(2),
        Some(Cell::new(0x1F0 / 16, 0x1A0 / 16 + 1)),
        "Rika"
    );
    assert_eq!(
        started(3),
        Some(Cell::new(0x200 / 16, 0x1A0 / 16 + 1)),
        "Wren"
    );
    assert_eq!(
        started(4),
        Some(Cell::new(0x210 / 16, 0x1A0 / 16 + 1)),
        "Raja"
    );
}

/// The chain tests Hahn, Gryz, Demi, Raja, Kyra, Seth in that order and moves
/// the first present; with none of them it moves no fifth member and still
/// finishes.
#[test]
fn the_fifth_member_is_the_first_of_the_six_present() {
    let mut s = state(
        &[CHAZ, RUNE, RIKA, WREN, DEMI],
        &[CULVERS],
        &[ITEM_ALIS_SWORD],
    );
    let (log, _) = play(&mut s);
    assert!(log.iter().any(|(_, e)| matches!(e,
        SceneEffect::ActorMoveStarted { actor, to }
            if (*actor == ActorRef::Character(DEMI) || *actor == ActorRef::PartyMember(4))
                && *to == Cell::new(0x210 / 16, 0x1A0 / 16 + 1))));

    let mut s = state(&[CHAZ, RUNE, RIKA, WREN], &[CULVERS], &[ITEM_ALIS_SWORD]);
    let (log, _) = play(&mut s);
    assert!(!log.iter().any(|(_, e)| matches!(e,
        SceneEffect::ActorMoveStarted { to, .. } if *to == Cell::new(0x210 / 16, 0x1A0 / 16 + 1))));
    assert!(s.is_set(Flag::event(ALIS_SWORD)));
}
