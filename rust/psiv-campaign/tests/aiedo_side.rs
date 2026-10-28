//! Aiedo's side events driven with pads from hand-built saves (#81): Rocky's
//! owner, and the bail quest from the first word with the sisters to the
//! walk home to Uzo. They need the repository pack and skip with a message
//! without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const AIEDO_PRISON: u16 = 0x05C;
const ROCKY_HOUSE: u16 = 0x065;
const UZO_HOUSE_3: u16 = 0x0E4;

/// The guard, `Field_Obj_Secondary`.
const GUARD: usize = 0;
/// The sister at `$FFFFC340`.
const SISTER: usize = 1;
/// Rocky's owner, at `$FFFFC340`.
const OWNER: usize = 1;

/// A Chaz/Alys party in Aiedo (`World_Index` 0) on `map` at `(x, y)`, with
/// `money` meseta and the event flags `flags`.
fn party_at(map: u16, x: u16, y: u16, money: u32, flags: &[u16]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    // Alys found and back in Aiedo ($42, tree 11's greeting), so the town's
    // first-visit text does not run.
    for flag in [0x42].iter().chain(flags) {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    game.set_money(money);
    Some(
        Session::start(set.data.clone())
            .with_battles(set.battle.clone())
            .from_save(RetailSave {
                snapshot: game.snapshot(),
                location: RetailLocation {
                    world_index: 0,
                    map_index_2: 0,
                    map_index: map,
                    char_x: x * 16,
                    char_y: y * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn flag(driver: &Driver, id: u16) -> bool {
    driver.runtime().game().is_set(Flag::event(id))
}

/// The bail job's story so far: the sisters caught at the supermarket
/// (`EventFlag_GirlsCaught`, `$46`, which `MapDataMan_AiedoPrison` reads to
/// show them), the guild's job taken (`EventFlag_StainInLife`, `$78`) and
/// their mother spoken to (`EventFlag_MissingGirlsMom`, `$79`).
const STAIN_IN_LIFE: [u16; 3] = [0x46, 0x78, 0x79];

/// With the job taken and the mother spoken to the sister tells her story
/// (`Event_GirlPrison`, `$7A`), the guard names the bail (`Event_GirlsBail`,
/// `$7B`), the guard takes 50,000 on a YES (`Event_PayingGirlsBail`, `$7C`)
/// and the sister, freed, is walked home to Uzo (`Event_GirlsBailedOut`,
/// `$7D`).
#[test]
fn the_bail_quest_runs_from_the_cell_to_uzo() {
    let Some(session) = party_at(AIEDO_PRISON, 23, 37, 60_000, &STAIN_IN_LIFE) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(SISTER).expect("the sister speaks");
    assert!(flag(&driver, 0x7A), "Event_GirlPrison");
    driver.talk(GUARD).expect("the guard speaks");
    assert!(flag(&driver, 0x7B), "Event_GirlsBail");
    driver.talk(GUARD).expect("the guard asks for the bail");
    driver.answer(true).expect("YES");
    assert!(flag(&driver, 0x7F), "the purse covered it");
    assert!(flag(&driver, 0x7C), "Event_PayingGirlsBail");
    assert_eq!(driver.runtime().game().money(), 10_000);
    driver.talk(SISTER).expect("the sister is free");
    assert!(flag(&driver, 0x7D), "Event_GirlsBailedOut");
    driver.neutral(120).expect("the parents speak");
    assert_eq!(driver.runtime().map_id().0, UZO_HOUSE_3);
}

/// The same YES with a short purse: the guard turns the party down, nothing
/// is paid and `$7F` stays clear.
#[test]
fn a_short_purse_pays_nothing() {
    let Some(session) = party_at(
        AIEDO_PRISON,
        23,
        37,
        49_999,
        &[0x46, 0x78, 0x79, 0x7A, 0x7B],
    ) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(GUARD).expect("the guard asks for the bail");
    driver.answer(true).expect("YES");
    assert!(!flag(&driver, 0x7F));
    assert!(!flag(&driver, 0x7C));
    assert_eq!(driver.runtime().game().money(), 49_999);
}

/// NO with the money: nothing is paid.
#[test]
fn no_pays_nothing() {
    let Some(session) = party_at(
        AIEDO_PRISON,
        23,
        37,
        60_000,
        &[0x46, 0x78, 0x79, 0x7A, 0x7B],
    ) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(GUARD).expect("the guard asks for the bail");
    driver.answer(false).expect("NO");
    assert!(flag(&driver, 0x7F), "the purse covered it");
    assert!(!flag(&driver, 0x7C));
    assert_eq!(driver.runtime().game().money(), 60_000);
}

/// With the guild's dog job taken (`$1E`) Rocky's owner asks for the dog
/// (`Event_RockyOwner`, `$1F`).
#[test]
fn rockys_owner_asks_for_the_dog() {
    let Some(session) = party_at(ROCKY_HOUSE, 29, 36, 0, &[0x1E]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OWNER).expect("the owner speaks");
    assert!(flag(&driver, 0x1F), "Event_RockyOwner");

    // Negative control: without the job the owner only talks.
    let Some(session) = party_at(ROCKY_HOUSE, 29, 36, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OWNER).expect("the owner speaks");
    assert!(!flag(&driver, 0x1F));
}
