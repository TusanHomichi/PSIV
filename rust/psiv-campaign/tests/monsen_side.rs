//! Monsen's side events (#81) on the pack, on hand-built saves: the campaign
//! route does not take them. Skips without the local pack.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::{Button, Session};

const MONSEN_HOUSE_4: u16 = 0x078;
const MONSEN_CAVE: u16 = 0x0EC;
/// The mother, `Field_Obj_Secondary`.
const MOTHER: usize = 0;
/// Tallas, `$C380`.
const TALLAS: usize = 2;

/// A Chaz/Alys party on `map` at cell `(x, y)` with the event flags `flags`.
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

/// Presses Speak through the lines until `done`.
fn run_until(driver: &mut Driver, done: impl Fn(&Driver) -> bool) {
    driver.set_budget(8_000);
    for _ in 0..1_000 {
        if done(driver) {
            return;
        }
        driver.tap(Button::Speak).expect("a press");
        driver.neutral(6).expect("the scene runs");
    }
    panic!("the scene did not get there");
}

/// Tree 12 entry `$1A`'s chain: `EventFlag_FissureOfFear` (`$2C`, the guild's
/// job) sends the mother to `$F6 $7A`, `$2F` to `$F6 $7B`, neither to her plain
/// line.
#[test]
fn tallass_mother_asks_then_thanks() {
    let Some(session) = party_at(MONSEN_HOUSE_4, 30, 33, &[0x2C]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(MOTHER).expect("she speaks");
    assert!(flag(&driver, 0x2D), "Event_TallasMom");

    let Some(session) = party_at(MONSEN_HOUSE_4, 30, 33, &[0x2C, 0x2D, 0x2F]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(MOTHER).expect("she speaks");
    assert!(flag(&driver, 0x3C), "Event_TallasMomAfterRescue");

    // Negative control: before the job she has neither scene.
    let Some(session) = party_at(MONSEN_HOUSE_4, 30, 33, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.talk(MOTHER).expect("she speaks");
    assert!(!flag(&driver, 0x2D));
    assert!(!flag(&driver, 0x3C));
}

/// Map trigger `$71`: arriving in the cave with `$2E` clear runs the line and
/// sets it, and it does not run again.
#[test]
fn landing_in_the_cave_speaks_once() {
    let Some(session) = party_at(MONSEN_CAVE, 80, 40, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    run_until(&mut driver, |d| d.scenes_ended() > scenes);
    assert!(flag(&driver, 0x2E), "Event_InsideMonsenHole");
    driver.neutral(30).expect("no second scene");
    assert_eq!(driver.scenes_ended(), scenes + 1);
    assert!(!driver.runtime().scene_active());
}

/// Map trigger `$72`: with the Fract Ooze beaten, Tallas speaks, walks out and
/// is gone, and `$3E` is set. Without `$2F` nothing runs.
#[test]
fn tallas_walks_out_after_the_fract_ooze() {
    let Some(session) = party_at(MONSEN_CAVE, 100, 48, &[0x2E, 0x2F]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    assert!(driver.runtime().map().npcs()[TALLAS].active);
    let scenes = driver.scenes_ended();
    run_until(&mut driver, |d| d.scenes_ended() > scenes);
    assert!(flag(&driver, 0x3E), "Event_TallasRescued");
    assert!(!driver.runtime().map().npcs()[TALLAS].active);

    let Some(session) = party_at(MONSEN_CAVE, 100, 48, &[0x2E]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.neutral(30).expect("nothing fires");
    assert!(!flag(&driver, 0x3E));
    assert!(driver.runtime().map().npcs()[TALLAS].active);
}
