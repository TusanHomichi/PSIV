use super::*;
use crate::battle::{SliceRolls, StatPair, StatTriple, Stats};

#[test]
fn status_names_decode_the_saved_font_bytes_and_preserve_spaces() {
    let mut stats = stats(1, 0);
    stats.name_bytes = [8, 57, 64, 70, 0xFE, 0];
    assert_eq!(stats.display_name(), "Hahn");
    stats.name_bytes = [1, 0, 27, 0x53, 0xFE, 0];
    assert_eq!(stats.display_name(), "A 0.");
}

fn game(ids: &[u8]) -> GameState {
    let mut game = GameState::default();
    let mut slots = [None; 5];
    for (slot, id) in ids.iter().enumerate() {
        slots[slot] = Some(CharId(*id));
        game.roster_mut().seat(CharId(*id), stats(1, 0)).unwrap();
        let stats = game.roster_mut().get_mut(CharId(*id)).unwrap();
        stats.curr_hp = 10;
        stats.max_hp = 20;
    }
    game.set_party(slots);
    game
}

#[test]
fn poison_uses_the_shared_four_step_phase_and_resets_on_load() {
    let mut game = game(&[0, 1]);
    let mut clock = FieldStatusClock::default();
    let mut rolls = SliceRolls::new(&[7]);
    for _ in 0..3 {
        assert_eq!(
            clock.step(&mut game, false, &mut rolls),
            FieldStatusResult::default()
        );
    }
    game.roster_mut().get_mut(CharId(1)).unwrap().status = status::POISONED;
    assert!(clock.step(&mut game, true, &mut rolls).flash_red);
    assert_eq!(game.roster().get(CharId(1)).unwrap().curr_hp, 9);
    clock.reset();
    for _ in 0..3 {
        assert!(!clock.step(&mut game, true, &mut rolls).flash_red);
    }
    assert!(clock.step(&mut game, true, &mut rolls).flash_red);
    assert_eq!(rolls.drawn(), 0);
}

#[test]
fn poison_death_preserves_other_bits_and_does_not_flash_for_the_dead() {
    let mut game = game(&[1, 0]);
    for who in [CharId(1), CharId(0)] {
        let stats = game.roster_mut().get_mut(who).unwrap();
        stats.curr_hp = 1;
        stats.status = status::POISONED | status::PARALYZED | status::TECH_SEALED;
    }
    let mut clock = FieldStatusClock {
        poison: 3,
        paralysis: 24,
    };
    let mut rolls = SliceRolls::new(&[0]);
    let result = clock.step(&mut game, true, &mut rolls);
    assert_eq!(result.fallen, [CharId(1), CharId(0)]);
    assert!(result.perished);
    assert!(!result.flash_red);
    assert_eq!(
        game.roster().get(CharId(1)).unwrap().status,
        status::DEAD | status::TECH_SEALED
    );
    assert_eq!(
        rolls.drawn(),
        0,
        "death clears paralysis before its cure roll"
    );
}

#[test]
fn paralysis_draws_only_steps_six_through_twenty_four_and_does_not_restore_stats() {
    let mut game = game(&[0, 1]);
    for who in [CharId(0), CharId(1)] {
        let stats = game.roster_mut().get_mut(who).unwrap();
        stats.status = status::PARALYZED | status::TECH_SEALED;
        stats.agility.battle = 1;
    }
    let mut clock = FieldStatusClock::default();
    let mut rolls = SliceRolls::new(&[7]);
    for _ in 0..5 {
        clock.step(&mut game, false, &mut rolls);
    }
    assert_eq!(rolls.drawn(), 0);
    for _ in 6..25 {
        clock.step(&mut game, false, &mut rolls);
    }
    assert_eq!(rolls.drawn(), 38);
    clock.step(&mut game, false, &mut rolls);
    assert_eq!(rolls.drawn(), 38);
    for who in [CharId(0), CharId(1)] {
        let stats = game.roster().get(who).unwrap();
        assert_eq!(stats.status, status::TECH_SEALED);
        assert_eq!(stats.agility.battle, 1);
    }
    game.roster_mut().get_mut(CharId(0)).unwrap().status |= status::PARALYZED;
    clock.paralysis = 255;
    clock.step(&mut game, false, &mut rolls);
    assert_ne!(
        game.roster().get(CharId(0)).unwrap().status & status::PARALYZED,
        0
    );
    clock.paralysis = 5;
    let mut cure = SliceRolls::new(&[8]);
    clock.step(&mut game, false, &mut cure);
    assert_eq!(
        game.roster().get(CharId(0)).unwrap().status & status::PARALYZED,
        0
    );
    assert_eq!(cure.drawn(), 1);
}

#[test]
fn androids_heal_one_hp_without_clearing_shutdown_or_reaching_past_an_empty_slot() {
    let mut game = game(&[0, 6, 7]);
    game.roster_mut().get_mut(CharId(0)).unwrap().profession = 5;
    let demi = game.roster_mut().get_mut(CharId(6)).unwrap();
    demi.curr_hp = 0;
    demi.status = status::ANDROID_DEAD;
    let mut clock = FieldStatusClock::default();
    let mut rolls = SliceRolls::new(&[0]);
    assert!(!clock.step(&mut game, true, &mut rolls).perished);
    assert_eq!(game.roster().get(CharId(0)).unwrap().curr_hp, 10);
    assert_eq!(game.roster().get(CharId(6)).unwrap().curr_hp, 1);
    assert_eq!(
        game.roster().get(CharId(6)).unwrap().status,
        status::ANDROID_DEAD
    );
    assert_eq!(game.roster().get(CharId(7)).unwrap().curr_hp, 11);
    game.set_party([Some(CharId(7)), None, Some(CharId(6)), None, None]);
    game.roster_mut().get_mut(CharId(7)).unwrap().curr_hp = 20;
    clock.step(&mut game, true, &mut rolls);
    assert_eq!(game.roster().get(CharId(6)).unwrap().curr_hp, 1);
    assert_eq!(game.roster().get(CharId(7)).unwrap().curr_hp, 20);
}

#[test]
fn zero_hp_poison_wraps_like_a_word_subtract() {
    let mut game = game(&[0]);
    let stats = game.roster_mut().get_mut(CharId(0)).unwrap();
    stats.curr_hp = 0;
    stats.status = status::POISONED;
    let mut clock = FieldStatusClock {
        poison: 3,
        paralysis: 0,
    };
    let result = clock.step(&mut game, true, &mut SliceRolls::new(&[7]));
    assert_eq!(game.roster().get(CharId(0)).unwrap().curr_hp, u16::MAX);
    assert!(result.flash_red);
    assert!(result.fallen.is_empty());
}

fn stats(level: u16, experience: u32) -> Stats {
    Stats {
        name_bytes: [0; 6],
        profession: 0,
        level,
        experience,
        curr_hp: 20,
        max_hp: 25,
        curr_tp: 10,
        max_tp: 10,
        status: 0,
        strength: StatTriple::uniform(8),
        mental: StatTriple::uniform(6),
        agility: StatTriple::uniform(7),
        dexterity: StatTriple::uniform(5),
        attack: StatPair::default(),
        defence: StatPair::default(),
        mental_defence: StatPair::default(),
        element_props: [0; 14],
        element_shadow: [0; 14],
        equipment: [0; 4],
        techniques: [0; 16],
        skills: [0; 8],
        curr_skill_uses: [0; 8],
        max_skill_uses: [0; 8],
        enemy_id: 0,
        gain_exp_flag: false,
        weapon_elements: Default::default(),
        physical_prop_save: 0,
    }
}
