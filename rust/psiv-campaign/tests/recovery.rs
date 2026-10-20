//! `restore` before a boss, driven through the camp's own menus: REVER the
//! fallen first, then cure the living to full. The cases need the runtime pack
//! and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::battle::status;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

/// `Technique_REVER`: technique 36, battle effect 21, costs 12 TP.
const REVER: u8 = 36;
/// `Technique_RES`: technique 24, battle effect 18, costs 3 TP.
const RES: u8 = 24;

const DEZOLIS: u16 = 0x001;
/// Below Dezolis warp 11, the Gumbious entrance's footprint.
const BELOW_GUMBIOUS: (u16, u16) = (125, 73);

/// Chaz knows REVER and RES with `chaz_tp` TP; Rika is fallen and Rune is at
/// half HP. Only Chaz knows a technique, so his TP alone decides whether the
/// revive can be paid for. The late-arc flags keep an earlier trigger from
/// firing on the first frame.
fn party(chaz_tp: u16) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(3)),
        None,
        None,
    ]);
    for flag in [
        0x08, 0x66, 0x68, 0x70, 0x71, 0x72, 0x80, 0x81, 0x84, 0x85, 0x88,
    ] {
        game.set(Flag::event(flag)).expect("flag in range");
    }
    for id in [CharId(5), CharId(3)] {
        let member = game.roster_mut().get_mut(id).expect("a party member");
        member.techniques.fill(0);
        member.curr_tp = 0;
    }
    let chaz = game.roster_mut().get_mut(CharId(0)).expect("Chaz");
    chaz.techniques.fill(0);
    chaz.techniques[0] = REVER;
    chaz.techniques[1] = RES;
    chaz.curr_tp = chaz_tp;
    let rika = game.roster_mut().get_mut(CharId(5)).expect("Rika");
    rika.curr_hp = 0;
    rika.status |= status::DEAD;
    let rune = game.roster_mut().get_mut(CharId(3)).expect("Rune");
    rune.curr_hp = rune.max_hp / 2;
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 1,
                    map_index_2: 0,
                    map_index: DEZOLIS,
                    char_x: BELOW_GUMBIOUS.0 * 16,
                    char_y: BELOW_GUMBIOUS.1 * 16,
                },
            })
            .expect("the map loads"),
    )
}

/// `(current HP, maximum HP)` of the camp party member `name`.
fn hp(driver: &Driver, name: &str) -> (u16, u16) {
    let camp = driver.runtime().camp_state();
    let member = camp
        .party
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("{name} is in the camp party"));
    (member.current_hp, member.max_hp)
}

#[test]
fn restore_revives_the_fallen_before_it_cures_the_living() {
    let Some(session) = party(100) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(
        driver.restore().expect("the camp casts"),
        "something was cast"
    );
    let decisions = driver.take_decisions();
    let revive = decisions.first().expect("the revive is recorded");
    assert!(revive.contains("casts technique 36"), "{decisions:?}");
    assert!(revive.contains("on Rika"), "{decisions:?}");
    assert!(hp(&driver, "Rika").0 > 0, "{decisions:?}");
    let (rune, rune_max) = hp(&driver, "Rune");
    assert_eq!(rune, rune_max, "the heal follows the revive: {decisions:?}");
}

#[test]
fn a_caster_who_cannot_pay_for_rever_heals_but_does_not_revive() {
    // REVER costs 12 TP; 11 leaves three 3-TP RES casts, so the heal still
    // runs to its stop while the revive is out of reach.
    let Some(session) = party(11) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(
        driver.restore().expect("the camp casts"),
        "RES is affordable, so the heal runs"
    );
    let decisions = driver.take_decisions();
    assert!(
        !decisions.iter().any(|line| line.contains("technique 36")),
        "no REVER was paid for: {decisions:?}"
    );
    assert_eq!(hp(&driver, "Rika").0, 0, "{decisions:?}");
    let (rune, rune_max) = hp(&driver, "Rune");
    assert_eq!(rune, rune_max, "the living are still cured: {decisions:?}");
}
