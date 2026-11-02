//! Motavia's side events driven with pads from hand-built saves (#81): Uzo's
//! mother, Sekreas in Rappy Cave and the patients of Meese's clinic. They need
//! the repository pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Direction, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const UZO_HOUSE_3: u16 = 0x0E4;
const RAPPY_CAVE: u16 = 0x0ED;
const MEESE_CLINIC: u16 = 0x133;

fn party_at(map: u16, x: u16, y: u16, flags: &[u16]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([Some(CharId(0)), Some(CharId(1)), None, None, None]);
    for flag in flags {
        game.set(Flag::event(*flag)).expect("flag in range");
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

/// One step up, stopping as soon as a scene takes over.
fn step_up(driver: &mut Driver) {
    for _ in 0..40 {
        if driver.runtime().scene_active() {
            return;
        }
        let at = psiv_campaign::driver::standing_cell(driver.runtime());
        if at.y < 24 && !psiv_campaign::driver::is_stepping(driver.runtime()) {
            return;
        }
        driver
            .tick(psiv_campaign::driver::dir_pad(Direction::Up))
            .expect("the step");
    }
    panic!("the step did not finish");
}

/// With the guild's job taken (`EventFlag_StainInLife`, `$78`) the mother's
/// entry reaches `$F6 $00 $7F`; `Event_LostGirlsMother` sets the flag
/// `Event_GirlPrison` waits on.
#[test]
fn the_mother_asks_after_her_daughters() {
    let Some(session) = party_at(UZO_HOUSE_3, 27, 32, &[0x78]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(1).expect("the mother speaks");
    assert!(flag(&driver, 0x79), "Event_LostGirlsMother");
    let leader = psiv_campaign::driver::standing_cell(driver.runtime());
    let mother = &driver.runtime().map().npcs()[1];
    assert_eq!(mother.cell.neighbor(mother.facing), Some(leader));
}

/// With the king fought (`EventFlag_KingRappy`, `$AF`) trigger `$73` fires on
/// the next step: the scene ends with the leader at `($3E0,$110)`, the flag
/// set and the trigger closed.
#[test]
fn sekreas_explains_after_the_fight() {
    let Some(session) = party_at(RAPPY_CAVE, 62, 24, &[0xAF]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    step_up(&mut driver);
    assert!(driver.runtime().scene_active(), "trigger $73 fires");
    driver.settle(false).expect("the scene ends");
    assert_eq!(driver.scenes_ended(), scenes + 1);
    assert!(flag(&driver, 0xBA), "Event_KingRappyDefeated");
    assert_eq!(
        psiv_campaign::driver::standing_cell(driver.runtime()),
        psiv_core::Cell::new(62, 18)
    );

    // Negative control: before the fight the same step fires nothing.
    let Some(session) = party_at(RAPPY_CAVE, 62, 24, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    step_up(&mut driver);
    assert!(!driver.runtime().scene_active());
    assert!(!flag(&driver, 0xBA));
}

/// The bed at `(18,38)`: talking to its block from `($110,$260)` fires the
/// event, and the patient ends facing the leader.
#[test]
fn the_clinic_patient_answers_from_her_table_cell() {
    let Some(session) = party_at(MEESE_CLINIC, 17, 39, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(20).expect("the block's entry fires the event");
    assert_eq!(driver.scenes_ended(), scenes + 1);
    assert_eq!(driver.runtime().map().npcs()[19].facing, Direction::Left);
}
