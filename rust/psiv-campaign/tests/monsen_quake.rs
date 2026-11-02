//! The earthquake at Monsen (#81) on the pack, on hand-built saves: the talk
//! and the walk-out, with Rune in the party and without. Skips without the
//! local pack.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{Cell, CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::Session;

const MONSEN: u16 = 0x073;
const INFORMANT: usize = 6;
const CHAZ: CharId = CharId(0);
const RUNE: CharId = CharId(3);
const GRYZ: CharId = CharId(4);
const RIKA: CharId = CharId(5);
const DEMI: CharId = CharId(6);

fn party_at(party: [Option<CharId>; 5], flags: &[u16]) -> Option<Session> {
    let set = pack()?;
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party(party);
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
                    map_index: MONSEN,
                    char_x: 47 * 16,
                    char_y: 36 * 16,
                },
            })
            .expect("the map loads"),
    )
}

fn plays_through(party: [Option<CharId>; 5]) {
    let Some(session) = party_at(party, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(20_000);
    driver.talk(INFORMANT).expect("the talk and the quake");
    assert!(
        driver.runtime().game().is_set(Flag::event(0x60)),
        "EventFlag_PlateSystem"
    );
    assert_eq!(
        psiv_campaign::driver::standing_cell(driver.runtime()),
        Cell::new(0x31, 0x23),
        "the leader's last walk, to ($310,$220)"
    );
    // The shake's frames cancel and the camera pans back to him: `($310,$220)`
    // less `$98`/`$58`, with the walk's last two pixels of X still on it (#291).
    let (x, y) = driver.runtime().camera().position();
    assert_eq!(y, 0x1C8);
    assert!((0x278..=0x27A).contains(&x), "{x:#X}");
}

/// Tree 12 entry `$0A`: `$65`, `$61` and `$60` clear reach `$F6 $2C`.
#[test]
fn the_quake_plays_through_with_rune() {
    plays_through([Some(CHAZ), Some(GRYZ), Some(RIKA), Some(DEMI), Some(RUNE)]);
}

#[test]
fn the_quake_plays_through_without_rune() {
    plays_through([Some(CHAZ), Some(GRYZ), Some(RIKA), Some(DEMI), None]);
}

/// Negative control: with `$60` set he has another line, and no scene.
#[test]
fn it_happens_once() {
    let party = [Some(CHAZ), Some(GRYZ), Some(RIKA), Some(DEMI), None];
    let Some(session) = party_at(party, &[0x60]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let before = driver.scenes_ended();
    driver.talk(INFORMANT).expect("he only talks");
    assert_eq!(driver.scenes_ended(), before);
}
