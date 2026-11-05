//! Scene outcomes for the late arc's six direct interactions (#83). Each case
//! reads the branch off the retail body (`docs/scenes/180_LateArcInteractions.md`).

mod common;

use common::map_with;
use psiv_core::battle::Lcg41;
use psiv_core::{
    ActorRef, Cell, CharId, DialogueId, Direction, EventIndex, Flag, GameState, SceneEffect,
    SceneFault, SceneInput, SceneOp, SceneRunner, ScriptedActor, StepFrames, scene_for,
};

const PENGUIN: Flag = Flag::event(0x8A);
const PENGUIN_NO_MONEY: Flag = Flag::event(0x8C);
const MUSK_CATS: Flag = Flag::event(0x90);
const SILVER_TUSK: Flag = Flag::event(0x91);
const DARK_FORCE_2: Flag = Flag::event(0x9E);
const HIJAMMER: Flag = Flag::event(0x5E);
const ITEM_SILVER_TUSK: u8 = 0x53;

/// A run of `event` with the cast the scenes name.
struct Play {
    runner: SceneRunner,
    log: Vec<SceneEffect>,
}

impl Play {
    fn new(event: u16) -> Play {
        let scene =
            scene_for(EventIndex(event)).unwrap_or_else(|| panic!("${event:04X} unregistered"));
        let mut cast = vec![ScriptedActor::new(
            ActorRef::PartyMember(0),
            Cell::new(2, 3),
            Direction::Up,
        )];
        cast.extend(
            (0..20).map(|n| ScriptedActor::new(ActorRef::Npc(n), Cell::new(1, 1), Direction::Down)),
        );
        Play {
            runner: SceneRunner::new(scene.ops, cast, StepFrames::new(8).unwrap()),
            log: Vec::new(),
        }
    }

    /// Ticks with `input` once, then until the scene blocks on input or ends.
    fn run(&mut self, state: &mut GameState, input: SceneInput) {
        let map = map_with(&["....", "....", "....", "...."], vec![], vec![]);
        self.log.extend(self.runner.tick(&map, state, input));
        for _ in 0..400 {
            self.log
                .extend(self.runner.tick(&map, state, SceneInput::None));
        }
    }

    fn dialogues(&self) -> Vec<u16> {
        self.log
            .iter()
            .filter_map(|effect| match effect {
                SceneEffect::DialogueOpen(DialogueId(id)) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Plays to the end: each window closes as soon as it opens.
    fn finish(&mut self, state: &mut GameState) {
        self.run(state, SceneInput::None);
        for _ in 0..8 {
            if self.runner.is_finished() {
                break;
            }
            self.runner.set_dialogue_last_byte(None);
            self.run(state, SceneInput::DialogueClosed);
        }
        assert!(self.runner.is_finished(), "{:?}", self.log);
        assert!(
            !self
                .log
                .iter()
                .any(|e| matches!(e, SceneEffect::Faulted(_))),
            "{:?}",
            self.log
        );
    }
}

#[test]
fn the_musk_cats_speak_once_and_step_aside() {
    let mut state = GameState::new();
    let mut play = Play::new(0x49);
    play.finish(&mut state);
    assert_eq!(play.dialogues(), vec![0x1C]);
    assert!(state.is_set(MUSK_CATS));
    for npc_index in [0, 1] {
        assert!(play.log.contains(&SceneEffect::NpcDialogueSet {
            npc_index,
            dialogue_id: 0x1D,
        }));
    }
}

#[test]
fn the_elder_gives_the_tusk_when_a_slot_is_free() {
    let mut state = GameState::new();
    let mut play = Play::new(0x4A);
    play.finish(&mut state);
    assert_eq!(play.dialogues(), vec![0x2D]);
    assert!(state.inventory().contains(ITEM_SILVER_TUSK));
    assert!(state.is_set(SILVER_TUSK));
}

#[test]
fn a_full_bag_keeps_the_tusk_with_the_elder() {
    let mut state = GameState::new();
    while !state.inventory().is_full() {
        state.inventory_mut().add(1).unwrap();
    }
    let mut play = Play::new(0x4A);
    play.finish(&mut state);
    assert_eq!(play.dialogues(), vec![0x2E]);
    assert!(!state.inventory().contains(ITEM_SILVER_TUSK));
    assert!(state.is_clear(SILVER_TUSK), "he offers it again");
}

/// Entry `$12`'s question: runs the scene to the prompt, answers, and has the
/// text end on `last_byte`.
fn penguin(money: u32, yes: bool, last_byte: u8) -> (GameState, Play) {
    let mut state = GameState::new();
    state.set_money(money);
    let mut play = Play::new(0x4B);
    play.run(&mut state, SceneInput::None);
    assert_eq!(play.dialogues(), vec![0x12]);
    play.runner.set_dialogue_last_byte(Some(last_byte));
    play.run(&mut state, SceneInput::DialogueClosed);
    play.run(&mut state, SceneInput::Choice(yes));
    assert!(play.runner.is_finished(), "{:?}", play.log);
    (state, play)
}

#[test]
fn the_keeper_takes_the_fee_on_a_yes_with_the_money() {
    let (state, play) = penguin(500, true, 0x3A);
    assert_eq!(state.money(), 0);
    assert!(state.is_set(PENGUIN));
    assert!(state.is_clear(PENGUIN_NO_MONEY));
    assert!(play.log.contains(&SceneEffect::NpcDialogueSet {
        npc_index: 16,
        dialogue_id: 0x15,
    }));
    assert!(play.log.contains(&SceneEffect::ActorFaced {
        actor: ActorRef::Npc(16),
        facing: Direction::Down,
    }));
}

#[test]
fn a_short_purse_raises_the_flag_the_entry_reads_and_pays_nothing() {
    let (state, _) = penguin(499, true, b'.');
    assert_eq!(state.money(), 499);
    assert!(state.is_set(PENGUIN_NO_MONEY));
    assert!(state.is_clear(PENGUIN));
}

#[test]
fn the_keeper_compares_the_purse_unsigned() {
    // `cmpi.l #500 / bcc`: a purse past the sign bit covers the fee, where the
    // bail scene's `bmi` on the difference would call `$800001F4` short.
    for money in [0x8000_01F4, 0x8000_0000, u32::MAX] {
        let (state, _) = penguin(money, true, 0x3A);
        assert!(state.is_clear(PENGUIN_NO_MONEY), "{money:#X}");
        assert!(state.is_set(PENGUIN), "{money:#X}");
        assert_eq!(state.money(), money - 500);
    }
}

#[test]
fn a_purse_past_the_sign_bit_that_says_no_keeps_its_money() {
    let (state, _) = penguin(0x8000_01F4, false, 0x3A);
    assert_eq!(state.money(), 0x8000_01F4);
    assert!(state.is_clear(PENGUIN_NO_MONEY));
    assert!(state.is_clear(PENGUIN));
}

#[test]
fn no_pays_nothing_whatever_the_last_byte() {
    let (state, _) = penguin(9_999, false, 0x3A);
    assert_eq!(state.money(), 9_999);
    assert!(state.is_clear(PENGUIN_NO_MONEY));
    assert!(state.is_clear(PENGUIN));
}

#[test]
fn a_keeper_who_cleared_the_flag_before_clears_it_again() {
    // The flag stays from the last short talk until the purse covers the fee.
    let mut state = GameState::new();
    state.set(PENGUIN_NO_MONEY).unwrap();
    state.set_money(500);
    let mut play = Play::new(0x4B);
    play.run(&mut state, SceneInput::None);
    assert!(state.is_clear(PENGUIN_NO_MONEY));
}

/// The seed whose next [`Lcg41::step`] has `low2` as its low two bits.
fn seed_rolling(low2: u16) -> Lcg41 {
    (1..)
        .map(Lcg41::new)
        .find(|rng| rng.clone().step() & 3 == low2)
        .unwrap()
}

#[test]
fn raja_draws_his_line_from_the_low_two_bits() {
    for (low2, entry) in [(0, 0x5E), (1, 0x5F), (2, 0x5D), (3, 0x5D)] {
        let mut state = GameState::new();
        let mut play = Play::new(0x5B);
        let rng = seed_rolling(low2);
        let mut after = rng.clone();
        after.step();
        play.runner.lend_rng(rng);
        play.finish(&mut state);
        assert_eq!(play.dialogues(), vec![entry], "low two bits {low2}");
        assert_eq!(play.runner.take_rng(), Some(after), "one draw");
        assert!(play.log.contains(&SceneEffect::ActorFaced {
            actor: ActorRef::Npc(4),
            facing: Direction::Down,
        }));
    }
}

#[test]
fn after_dark_force_2_raja_draws_nothing() {
    let mut state = GameState::new();
    state.set(DARK_FORCE_2).unwrap();
    let mut play = Play::new(0x5B);
    let rng = Lcg41::new(1);
    play.runner.lend_rng(rng.clone());
    play.finish(&mut state);
    assert_eq!(play.dialogues(), vec![0x60]);
    assert_eq!(play.runner.take_rng(), Some(rng));
}

#[test]
fn a_draw_with_no_stream_lent_faults() {
    let mut state = GameState::new();
    let mut play = Play::new(0x5B);
    play.run(&mut state, SceneInput::None);
    assert!(
        play.log.contains(&SceneEffect::Faulted(SceneFault::NoRng)),
        "{:?}",
        play.log
    );
}

#[test]
fn rune_heals_through_event_recovery() {
    let recovery = scene_for(EventIndex(0x21)).unwrap().ops;
    let rune = scene_for(EventIndex(0x67)).unwrap().ops;
    assert_eq!(&rune[1..], recovery, "`jsr Event_Recovery`, whole");
    assert!(matches!(rune[0], SceneOp::RunDialogue { .. }));
    assert!(matches!(rune.last(), Some(SceneOp::RecoverStats)));
}

#[test]
fn wren_installs_hijammer() {
    let mut state = GameState::new();
    state.roster_mut().seat(CharId(7), common::stats()).unwrap();
    let mut play = Play::new(0x6D);
    play.finish(&mut state);
    assert_eq!(play.dialogues(), vec![0x0A]);
    assert!(state.is_set(HIJAMMER));
    assert_eq!(state.roster().get(CharId(7)).unwrap().skills[4], 0x20);
    assert!(play.log.contains(&SceneEffect::ActorFaced {
        actor: ActorRef::Npc(1),
        facing: Direction::Up,
    }));
    assert!(play.log.contains(&SceneEffect::DialogueResume));
    let sounds: Vec<u8> = scene_for(EventIndex(0x6D))
        .unwrap()
        .ops
        .iter()
        .filter_map(|op| match op {
            SceneOp::PlaySound { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(sounds, vec![0xE1, 0xCF, 0xCF, 0xCF]);
}
