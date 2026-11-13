//! `restore` before a boss and the recovery after a battle, driven through the
//! camp's own menus: REVER the fallen first, RIMPA the paralyzed next, then
//! cure the living; and never a caster the camp refuses. The cases need the
//! runtime pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_campaign::route::NameOrId;
use psiv_core::battle::status;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

/// `Technique_REVER`: technique 36, battle effect 21, costs 12 TP.
const REVER: u8 = 36;
/// `Technique_RES`: technique 24, battle effect 18, costs 3 TP.
const RES: u8 = 24;
/// `Technique_RIMPA`: technique 35, battle effect 20 (cure paralysis), costs
/// 5 TP.
const RIMPA: u8 = 35;

const DEZOLIS: u16 = 0x001;
/// Below Dezolis warp 11, the Gumbious entrance's footprint.
const BELOW_GUMBIOUS: (u16, u16) = (125, 73);

/// Chaz knows REVER and RES with `chaz_tp` TP; Rika is fallen and Rune is at
/// half HP. Only Chaz knows a technique, so his TP alone decides whether the
/// revive can be paid for. The late-arc flags keep an earlier trigger from
/// firing on the first frame.
fn party(chaz_tp: u16) -> Option<Session> {
    party_of(chaz_tp, false)
}

/// As [`party`], with a fallen Wren (an android) ahead of Rika in party order.
fn party_with_fallen_android(chaz_tp: u16) -> Option<Session> {
    party_of(chaz_tp, true)
}

fn party_of(chaz_tp: u16, android: bool) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party(if android {
        [
            Some(CharId(0)),
            Some(CharId(7)),
            Some(CharId(5)),
            Some(CharId(3)),
            None,
        ]
    } else {
        [
            Some(CharId(0)),
            Some(CharId(5)),
            Some(CharId(3)),
            None,
            None,
        ]
    });
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
    if android {
        let wren = game.roster_mut().get_mut(CharId(7)).expect("Wren");
        wren.curr_hp = 0;
        wren.status |= status::DEAD;
        wren.profession = psiv_core::battle::PROFESSION_ANDROID;
    }
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

#[test]
fn restore_skips_a_fallen_android_instead_of_halting() {
    // The camp answers NO EFFECT to REVER on an android; only an inn or a
    // Repair Kit mends one. Wren is first in party order, ahead of Rika.
    let Some(session) = party_with_fallen_android(100) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let profession = driver
        .runtime()
        .camp_state()
        .party
        .iter()
        .find(|m| m.name == "Wren")
        .expect("Wren is in the camp party")
        .profession
        .clone();
    assert_eq!(profession.to_ascii_lowercase(), "android");
    assert!(
        driver.restore().expect("the camp casts"),
        "Rika is revived and Rune cured"
    );
    let decisions = driver.take_decisions();
    assert!(
        decisions
            .iter()
            .any(|line| line.contains("casts technique 36") && line.contains("on Rika")),
        "{decisions:?}"
    );
    assert!(
        !decisions.iter().any(|line| line.contains("on Wren")),
        "no revive is aimed at the android: {decisions:?}"
    );
    assert_eq!(hp(&driver, "Wren").0, 0, "{decisions:?}");
}

/// Chaz, Rika and Rune at full HP with no technique and no TP, on the same
/// spot as [`party_of`], changed by `setup` before the save loads.
fn party_set_up(setup: impl FnOnce(&mut GameState)) -> Option<Session> {
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
    for id in [CharId(0), CharId(5), CharId(3)] {
        let member = game.roster_mut().get_mut(id).expect("a party member");
        member.techniques.fill(0);
        member.curr_tp = 0;
        member.curr_hp = member.max_hp;
        member.status = 0;
    }
    setup(&mut game);
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

/// Gives `who` the techniques `known` and 100 TP.
fn teach(game: &mut GameState, who: CharId, known: &[u8]) {
    let member = game.roster_mut().get_mut(who).expect("a party member");
    member.techniques[..known.len()].copy_from_slice(known);
    member.curr_tp = 100;
}

/// Leaves `who` living at `percent` of maximum HP with `bits` of status.
fn hurt(game: &mut GameState, who: CharId, percent: u16, bits: u8) {
    let member = game.roster_mut().get_mut(who).expect("a party member");
    member.curr_hp = (member.max_hp * percent / 100).max(1);
    member.status |= bits;
}

/// The camp party member `name`'s status byte.
fn status_of(driver: &Driver, name: &str) -> u8 {
    driver
        .runtime()
        .camp_state()
        .party
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("{name} is in the camp party"))
        .status
}

const CHAZ: CharId = CharId(0);
const RIKA: CharId = CharId(5);
const RUNE: CharId = CharId(3);

#[test]
fn recovery_never_asks_a_paralyzed_caster() {
    // Chaz alone knows RES and has the TP, but is paralyzed: the camp refuses
    // him (`loc_5FF98`), so there is nobody to cast and nothing is tried.
    // The #483 trace asked him 24 times and took every refusal for a cast.
    let Some(session) = party_set_up(|game| {
        teach(game, CHAZ, &[RES]);
        hurt(game, CHAZ, 100, status::PARALYZED);
        hurt(game, RIKA, 30, 0);
    }) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let cast = driver.recover().expect("nothing halts");
    let decisions = driver.take_decisions();
    assert!(!cast, "nobody the camp accepts can cast: {decisions:?}");
    assert!(
        !decisions.iter().any(|line| line.contains("casts")),
        "{decisions:?}"
    );
    assert!(driver.needs_refuge(), "a paralyzed member cannot train on");
}

#[test]
fn recovery_heals_from_the_same_caster_when_he_can_act() {
    // The control for the case above: the same party with Chaz free of
    // paralysis cures Rika, so the refusal, not the setup, is what stops it.
    let Some(session) = party_set_up(|game| {
        teach(game, CHAZ, &[RES]);
        hurt(game, RIKA, 30, 0);
    }) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.recover().expect("the camp casts"));
    let decisions = driver.take_decisions();
    assert!(
        decisions
            .first()
            .is_some_and(|line| line.contains("Chaz casts technique 24 on Rika")),
        "{decisions:?}"
    );
    let (rika, rika_max) = hp(&driver, "Rika");
    assert!(rika * 100 >= rika_max * 70, "{decisions:?}");
    assert!(!driver.needs_refuge(), "{decisions:?}");
}

#[test]
fn recovery_frees_a_paralyzed_healer_who_then_heals() {
    // Rika knows RES but is paralyzed; Chaz knows only RIMPA. The cure for
    // paralysis comes first, and it makes Rika the healer for Rune.
    let Some(session) = party_set_up(|game| {
        teach(game, CHAZ, &[RIMPA]);
        teach(game, RIKA, &[RES]);
        hurt(game, RIKA, 100, status::PARALYZED);
        hurt(game, RUNE, 30, 0);
    }) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.recover().expect("the camp casts"));
    let decisions = driver.take_decisions();
    assert!(
        decisions
            .first()
            .is_some_and(|line| line.contains("Chaz casts technique 35 on Rika")),
        "{decisions:?}"
    );
    assert_eq!(
        status_of(&driver, "Rika") & status::PARALYZED,
        0,
        "{decisions:?}"
    );
    assert!(
        decisions
            .iter()
            .any(|line| line.contains("Rika casts technique 24 on Rune")),
        "{decisions:?}"
    );
    let (rune, rune_max) = hp(&driver, "Rune");
    assert!(rune * 100 >= rune_max * 70, "{decisions:?}");
    assert!(!driver.needs_refuge(), "{decisions:?}");
}

#[test]
fn recovery_raises_the_fallen_between_battles() {
    // The patrol's refuge is a walk through the dungeon's encounters; a
    // member a living caster can REVER is raised in the camp instead, and
    // then cured like the rest.
    let Some(session) = party(100) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.needs_refuge(), "Rika is down before the camp");
    assert!(driver.recover().expect("the camp casts"));
    let decisions = driver.take_decisions();
    assert!(
        decisions
            .first()
            .is_some_and(|line| line.contains("casts technique 36 on Rika")),
        "{decisions:?}"
    );
    let (rika, rika_max) = hp(&driver, "Rika");
    assert!(rika * 100 >= rika_max * 70, "{decisions:?}");
    assert!(!driver.needs_refuge(), "{decisions:?}");
}

#[test]
fn a_cast_the_camp_refuses_halts_the_objective() {
    // A route's `use_technique` from a paralyzed caster: the camp answers
    // CASTER IS PARALYZED and spends nothing, which is no cast.
    let Some(session) = party_set_up(|game| {
        teach(game, CHAZ, &[RES]);
        hurt(game, CHAZ, 100, status::PARALYZED);
        hurt(game, RIKA, 30, 0);
    }) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let halt = driver
        .use_technique(
            &NameOrId::Name("Chaz".into()),
            &NameOrId::Id(u16::from(RES)),
            &NameOrId::Name("Rika".into()),
        )
        .expect_err("the refusal is no cast");
    assert!(
        format!("{halt:?}").contains("CASTER IS PARALYZED"),
        "{halt:?}"
    );
}

#[test]
fn a_paralyzed_member_at_full_hp_needs_the_refuge() {
    // Paralysis takes the member's every battle turn (`NO_TURN`) until some
    // 25 field steps pass without a map load; a patrol with no cure left
    // takes its refuge for it as for a fallen member.
    let Some(session) = party_set_up(|game| hurt(game, RUNE, 100, status::PARALYZED)) else {
        return;
    };
    let driver = Driver::new(session, None);
    assert!(driver.needs_refuge());
    let Some(session) = party_set_up(|_| {}) else {
        return;
    };
    assert!(
        !Driver::new(session, None).needs_refuge(),
        "the same party unparalyzed trains on"
    );
}
