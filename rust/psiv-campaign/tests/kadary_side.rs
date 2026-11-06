//! Kadary's priest and believer driven with pads from hand-built saves (#81):
//! the priest faints (`$24`) and the believer writhes (`$31`). They need the
//! repository pack and skip with a message without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const KADARY: u16 = 0x068;
const KADARY_CHURCH: u16 = 0x069;
/// `EventFlag_ZioNurvus`: Zio is gone.
const ZIO_NURVUS: u16 = 0x65;
const PRIEST: usize = 9;
const BLOCK: usize = 10;
const BELIEVER: usize = 4;
const WRITHING: usize = 9;

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

/// Before Zio is beaten the priest's entry `$37` reaches `$F6 $00 $24`: he
/// faints, a solid block appears in slot 10 beside him, and both now answer
/// with `$38`, which the party reaches by talking to the block.
#[test]
fn the_priest_faints_and_his_block_answers() {
    let Some(session) = party_at(KADARY_CHURCH, 31, 24, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(4_000);
    let scenes = driver.scenes_ended();
    driver.talk(PRIEST).expect("the priest speaks");
    assert_eq!(
        driver.scenes_ended(),
        scenes + 1,
        "Event_FaintingPriest ran"
    );
    let npcs = driver.runtime().map().npcs();
    assert!(npcs.len() > BLOCK, "slot 10 written");
    assert!(
        npcs[BLOCK].active && npcs[BLOCK].interactable,
        "the block is solid"
    );
    assert_eq!(driver.runtime().npc_dialogue_id(BLOCK), Some(0x38));
    assert_eq!(driver.runtime().npc_dialogue_id(PRIEST), Some(0x38));
    driver.talk(BLOCK).expect("the block answers");
}

/// Negative control: once Zio is beaten (`$65`) entry `$37`'s `$FA $65 +2`
/// skips the event; nothing faints and no slot is written.
#[test]
fn after_zio_the_priest_only_talks() {
    let Some(session) = party_at(KADARY_CHURCH, 31, 24, &[ZIO_NURVUS]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(PRIEST).expect("the priest speaks");
    assert_eq!(driver.runtime().map().npcs().len(), 10, "no slot written");
}

/// After Zio, `MapDataMan_Kadary` shows the believer (object 4) and hides the
/// writhing one (object 9). Talking to him reaches `$F6 $00 $31`: object 9
/// comes back where he stood, and he is cleared.
#[test]
fn the_believer_writhes_where_he_stood() {
    let Some(session) = party_at(KADARY, 20, 35, &[ZIO_NURVUS]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(4_000);
    assert!(
        !driver.runtime().map().npcs()[WRITHING].active,
        "hidden at load"
    );
    let scenes = driver.scenes_ended();
    driver.talk(BELIEVER).expect("the believer speaks");
    assert_eq!(driver.scenes_ended(), scenes + 1, "Event_ZioFanatic ran");
    let npcs = driver.runtime().map().npcs();
    assert!(!npcs[BELIEVER].active, "object 4 cleared");
    assert!(npcs[WRITHING].active, "object 9 back");
    assert_eq!(npcs[WRITHING].id.0, 0x148);
    // After the scene's teardown he keeps the spot object 4 was cleared
    // from, beside the leader who talked to him, not his record's cell.
    assert_eq!(
        npcs[WRITHING].cell, npcs[BELIEVER].cell,
        "where the believer stood"
    );
    let state = driver.runtime().state();
    assert_eq!(
        driver
            .runtime()
            .map()
            .neighbor(state.cell(), state.facing()),
        Some(npcs[WRITHING].cell),
        "in front of the leader"
    );
}
