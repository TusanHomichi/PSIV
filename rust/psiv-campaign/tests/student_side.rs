//! The guild's missing student driven with pads from hand-built saves (#81):
//! the dorm owner, the student in the church, the old man and the Perolymate,
//! and the walk home. They need the repository pack and skip with a message
//! without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const PIATA_DORM: u16 = 0x018;
const KADARY_CHURCH: u16 = 0x069;
const KADARY_INN: u16 = 0x071;
/// `EventFlag_MissingStudent`: the guild's job is taken.
const JOB_TAKEN: u16 = 0x26;
const DORM_OWNER: u16 = 0x27;
const STUDENT_SICK: u16 = 0x28;
const PEROLYMATE: u16 = 0x29;
const STUDENT_RECOVERED: u16 = 0x2A;
const ITEM_PEROLYMATE: u8 = 0x93;

/// `PiataDorm` object 0, `Field_Obj_Secondary`.
const OWNER: usize = 0;
/// `PiataDorm` object 4, shown once she is home.
const STUDENT_HOME: usize = 4;
/// `KadaryChurch` object 4, `$FFFFC400`.
const STUDENT_IN_CHURCH: usize = 4;
/// `KadaryInn` object 1, `$FFFFC340`.
const OLD_MAN: usize = 1;
/// `KadaryInn` object 3, the invisible block by the bed: the student's own
/// object (2) is not talkable, and the block carries her dialogue.
const BED_BLOCK: usize = 3;

fn party_at(map: u16, x: u16, y: u16, flags: &[u16], items: &[u8]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), None, None, None, None]);
    for flag in flags {
        game.set(Flag::event(*flag)).expect("flag in range");
    }
    for item in items {
        game.inventory_mut().add(*item).expect("room in the bag");
    }
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

fn holds(driver: &Driver, item: u8) -> bool {
    driver.runtime().game().inventory().slots().contains(&item)
}

/// With the job taken the owner tells the party about her (`$76`).
#[test]
fn the_dorm_owner_speaks_once_the_job_is_taken() {
    let Some(session) = party_at(PIATA_DORM, 45, 30, &[JOB_TAKEN], &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OWNER).expect("the owner speaks");
    assert!(flag(&driver, DORM_OWNER), "Event_PiataDormOwner");

    // Negative control: without the job she only talks.
    let Some(session) = party_at(PIATA_DORM, 45, 30, &[], &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(OWNER).expect("the owner speaks");
    assert!(!flag(&driver, DORM_OWNER));
}

/// The whole job from the church to the dorm: she collapses (`$77`), the old
/// man refuses an empty bag and takes the medicine (`$78`), and she goes home
/// (`$79`).
#[test]
fn the_student_is_found_treated_and_taken_home() {
    let flags = [JOB_TAKEN, DORM_OWNER];
    let Some(session) = party_at(KADARY_CHURCH, 25, 34, &flags, &[ITEM_PEROLYMATE]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(6_000);
    driver.talk(STUDENT_IN_CHURCH).expect("she speaks");
    assert!(flag(&driver, STUDENT_SICK), "Event_MissingStudentFound");
    assert_eq!(driver.runtime().map_id().0, KADARY_INN);

    driver.talk(BED_BLOCK).expect("she is too ill to answer");
    assert!(!flag(&driver, STUDENT_RECOVERED), "not before the medicine");

    driver.talk(OLD_MAN).expect("the old man speaks");
    assert!(flag(&driver, PEROLYMATE), "Event_OldManNearMissingStudent");
    assert!(!holds(&driver, ITEM_PEROLYMATE), "he took it");

    driver.talk(BED_BLOCK).expect("she speaks");
    assert!(
        flag(&driver, STUDENT_RECOVERED),
        "Event_MissingStudentInBed"
    );
    assert_eq!(driver.runtime().map_id().0, PIATA_DORM);
    assert!(
        driver.runtime().map().npcs()[STUDENT_HOME].active,
        "`MapDataMan_PiataDorm` keeps her once `$2A` is set"
    );
}

/// Without the owner's word the student only talks (`$77`'s first arm), and
/// the party stays in the church.
#[test]
fn the_student_stays_put_until_the_owner_has_spoken() {
    let Some(session) = party_at(KADARY_CHURCH, 25, 34, &[JOB_TAKEN, 0x65], &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(3_000);
    driver.talk(STUDENT_IN_CHURCH).expect("she speaks");
    assert!(!flag(&driver, STUDENT_SICK));
    assert_eq!(driver.runtime().map_id().0, KADARY_CHURCH);
}

/// The old man with an empty bag writes nothing (`$78`'s absent arm).
#[test]
fn the_old_man_needs_the_medicine() {
    let flags = [JOB_TAKEN, DORM_OWNER, STUDENT_SICK];
    let Some(session) = party_at(KADARY_INN, 36, 32, &flags, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(OLD_MAN).expect("the old man speaks");
    assert_eq!(driver.scenes_ended(), scenes + 1, "the scene ran");
    assert!(!flag(&driver, PEROLYMATE));
}
