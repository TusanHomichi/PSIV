//! The late arc's direct interactions driven with pads from hand-built saves
//! on the real maps (#83): the route reaches none of these side talks, and
//! Kuran's chest and the Meese clinic sit past most of the game. They need the
//! repository pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const ZOSA: u16 = 0x126;
const MEESE_CLINIC: u16 = 0x134;
const MYST_VALE_2: u16 = 0x159;
const MYST_VALE_5: u16 = 0x15C;
const INNER_SANCTUARY_B1: u16 = 0x16F;
const KURAN_F1_2: u16 = 0x193;

/// The keeper, `$FFFFC700`.
const KEEPER: usize = 16;
/// The invisible block in front of Raja's bed (`$FFFFC400` is Raja himself):
/// the talk lands on it, and it carries his dialogue.
const RAJA: usize = 5;
/// `EventFlag_RajaSick`: the clinic holds Raja only from then on, until the
/// reunion (`MapDataMan` entry `$7E`).
const RAJA_SICK: u16 = 0x94;
/// `EventFlag_ElsydeonCave`, `$D8`: Rune waits in the Inner Sanctuary from then
/// until Elsydeon (entry `$55`).
const ELSYDEON_CAVE: u16 = 0xD8;

/// Chaz, Rune, Rika and Wren on `map` at `(x, y)` with `money` and `flags`.
fn party_at(map: u16, x: u16, y: u16, money: u32, flags: &[u16]) -> Option<(Session, GameState)> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(3)),
        Some(CharId(5)),
        Some(CharId(7)),
        None,
    ]);
    for flag in flags {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    game.set_money(money);
    let session = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 1,
                map_index_2: 0,
                map_index: map,
                char_x: x * 16,
                char_y: y * 16,
            },
        })
        .expect("the map loads");
    Some((session, game))
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

/// The guards speak and `EventFlag_MuskCats` (`$90`) is set; from then on they
/// answer entry `$1D`, which carries no event.
#[test]
fn the_musk_cats_let_the_party_through() {
    let Some((session, _)) = party_at(MYST_VALE_2, 18, 24, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(0).expect("the guard speaks");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_MuskCatsGuarding");
    assert!(flag(&driver, 0x90));
    driver.talk(0).expect("the guard speaks again");
    assert_eq!(driver.scenes_ended(), scenes + 1, "entry $1D fires nothing");
}

/// The chief hands over the Silver Tusk; the talk lands on the invisible block
/// in front of him (object 8), which carries his dialogue. Entry `$2C`'s `$FA $91` then skips the
/// event on the second talk, so a second tusk never comes.
#[test]
fn the_elder_gives_the_tusk_once() {
    let Some((session, _)) = party_at(MYST_VALE_5, 29, 26, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(8).expect("the chief speaks");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_MuskCatElder");
    assert!(flag(&driver, 0x91));
    let tusks = |driver: &Driver| {
        let game = driver.runtime().game();
        game.inventory()
            .slots()
            .iter()
            .filter(|&&i| i == 0x53)
            .count()
    };
    assert_eq!(tusks(&driver), 1);
    driver.talk(8).expect("the chief speaks again");
    assert_eq!(
        driver.scenes_ended(),
        scenes + 1,
        "no scene the second time"
    );
    assert_eq!(tusks(&driver), 1);
}

/// A YES with the money: the fee goes, and `EventFlag_Penguin` is up.
#[test]
fn the_keeper_lets_the_penguin_walk_with_the_party() {
    let Some((session, _)) = party_at(ZOSA, 13, 41, 600, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(KEEPER).expect("the keeper asks");
    driver.answer(true).expect("YES");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_PenguinOwner");
    assert_eq!(driver.runtime().game().money(), 100);
    assert!(flag(&driver, 0x8A), "EventFlag_Penguin");
    assert!(!flag(&driver, 0x8C), "EventFlag_PenguinNoMoney");
    // Entry $15, the override: a further talk fires no scene and takes nothing.
    driver.talk(KEEPER).expect("the keeper warns");
    assert_eq!(driver.scenes_ended(), scenes + 1);
    assert_eq!(driver.runtime().game().money(), 100);
}

/// The other two endings.
#[test]
fn the_keeper_takes_nothing_from_a_short_purse_or_a_no() {
    for (money, yes, short) in [(499, true, true), (600, false, false)] {
        let Some((session, _)) = party_at(ZOSA, 13, 41, money, &[]) else {
            return;
        };
        let mut driver = Driver::new(session, None);
        driver.talk(KEEPER).expect("the keeper asks");
        driver.answer(yes).expect("the answer");
        assert_eq!(driver.runtime().game().money(), money);
        assert!(!flag(&driver, 0x8A));
        assert_eq!(flag(&driver, 0x8C), short);
    }
}

/// Raja's talk runs and turns him to the party, on either side of Dark Force 2.
#[test]
fn raja_answers_from_his_bed() {
    for flags in [&[RAJA_SICK][..], &[RAJA_SICK, 0x9E][..]] {
        let Some((session, _)) = party_at(MEESE_CLINIC, 24, 30, 0, flags) else {
            return;
        };
        let mut driver = Driver::new(session, None);
        let scenes = driver.scenes_ended();
        driver.talk(RAJA).expect("Raja speaks");
        assert_eq!(driver.scenes_ended(), scenes + 1, "Event_RajaSick");
    }
}

/// Rune's line, then `Event_Recovery` fills the party.
#[test]
fn rune_heals_the_party() {
    let Some((session, _)) = party_at(INNER_SANCTUARY_B1, 31, 30, 0, &[ELSYDEON_CAVE]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(0).expect("Rune speaks");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_RuneHealingChaz");
    let game = driver.runtime().game();
    for member in game.party_members() {
        let stats = game.roster().get(member).expect("seated");
        assert_eq!(stats.curr_hp, stats.max_hp, "{member:?}");
        assert_eq!(stats.curr_tp, stats.max_tp, "{member:?}");
    }
}

/// The chest fires `Event_Hijammer` through interaction area `$0D`, and the
/// gate's chest flag closes it behind the scene.
#[test]
fn wren_finds_hijammer_on_kuran() {
    let Some((session, _)) = party_at(KURAN_F1_2, 14, 18, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    let items = driver.runtime().game().inventory().occupied();
    driver.open_chest(0).expect("the chest opens");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_Hijammer");
    assert!(flag(&driver, 0x5E), "EventFlag_Hijammer");
    let game = driver.runtime().game();
    assert!(
        game.is_set(Flag::chest(0x5E)),
        "the gate recorded the chest"
    );
    assert_eq!(game.roster().get(CharId(7)).expect("Wren").skills[4], 0x20);
    assert_eq!(
        game.inventory().occupied(),
        items,
        "its Dagger is not awarded"
    );
}
