//! Aiedo's side events driven with pads from hand-built saves (#81): Rocky's
//! owner, and the bail quest from the first word with the sisters to the
//! walk home to Uzo. They need the repository pack and skip with a message
//! without one.

mod common;

use common::pack;
use psiv_campaign::driver::Driver;
use psiv_core::{
    ActorRef, BespokeKind, CharId, Direction, Flag, GameState, RetailLocation, RetailSave,
    WanderSpeed,
};
use psiv_runtime::{Button, Pad, Session};

const AIEDO_PRISON: u16 = 0x05C;
const ROCKY_HOUSE: u16 = 0x065;
const UZO_HOUSE_3: u16 = 0x0E4;
const AIEDO_HOUSE_3: u16 = 0x060;
const AIEDO: u16 = 0x054;
/// The admirer on Aiedo's street (object 5, `$FFFFC440`, dialogue 7).
const ADMIRER: usize = 5;

/// The guard, `Field_Obj_Secondary`.
const GUARD: usize = 0;
/// The sister at `$FFFFC340`.
const SISTER: usize = 1;
/// Rocky's owner, at `$FFFFC340`.
const OWNER: usize = 1;

/// A Chaz/Alys party in Aiedo (`World_Index` 0) on `map` at `(x, y)`, with
/// `money` meseta and the event flags `flags`: before Zio (`$42` clear), while
/// Alys travels with Chaz.
fn party_at(map: u16, x: u16, y: u16, money: u32, flags: &[u16]) -> Option<Session> {
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

/// `AiedoHouse3`'s man (object 0, routine `loc_48F36`, facing down at
/// (26,28)) fires `Event_AiedoManSavings` on every talk: talked to from the
/// right, he ends facing the party.
#[test]
fn the_man_looking_for_his_savings_turns_to_the_party() {
    let Some(session) = party_at(AIEDO_HOUSE_3, 27, 30, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    let flags = driver.runtime().game().snapshot();
    driver.talk(0).expect("he speaks");
    assert_eq!(driver.scenes_ended(), scenes + 1, "the scene ran");
    let leader = psiv_campaign::driver::standing_cell(driver.runtime());
    let man = driver.runtime().map().npcs()[0].cell;
    let facing = driver.runtime().map().npcs()[0].facing;
    assert_eq!(
        man.neighbor(facing),
        Some(leader),
        "object 0 faces the leader at {leader:?}"
    );
    assert_eq!(driver.runtime().game().snapshot(), flags, "nothing written");
}

/// Before Zio (`$42` clear) the man on Aiedo's street flatters Alys and runs
/// off north (`Event_AiedoManCrushOnAlys`): the scene ends with his slot
/// cleared and nothing else written. Once Zio has been fought, entry 7's
/// `$FA $42 $01` reads entry 8 instead and he stays.
#[test]
fn the_admirer_runs_off_the_screen() {
    let Some(session) = party_at(AIEDO, 47, 77, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let flags = driver.runtime().game().snapshot();
    let scenes = driver.scenes_ended();
    // About 900 frames with the text; a man who never leaves the screen
    // would hold the scene for good, so the talk is bounded.
    driver.set_budget(3_000);
    driver.talk(ADMIRER).expect("he speaks and runs");
    assert_eq!(driver.scenes_ended(), scenes + 1, "the scene ran out");
    assert!(
        !driver.runtime().map().npcs()[ADMIRER].active,
        "his slot is clear"
    );
    assert_eq!(driver.runtime().game().snapshot(), flags, "nothing written");

    // Negative control: after Zio.
    let Some(session) = party_at(AIEDO, 47, 77, 0, &[0x42]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    driver.talk(ADMIRER).expect("he only talks");
    assert_eq!(driver.scenes_ended(), scenes, "no scene");
    assert!(driver.runtime().map().npcs()[ADMIRER].active);
}

/// While `Event_AiedoManCrushOnAlys` waits on his routine (`$BC`), the
/// renderer's view of the admirer is the routine's: the scene's cast, frozen
/// where the scene started, yields him to [`Runtime::object_motions`], which
/// walks him up the street one cell per `WanderSpeed::Selector2` step with
/// the step interpolated, facing up, until his cleared slot drops him.
///
/// [`Runtime::object_motions`]: psiv_runtime::Runtime::object_motions
#[test]
fn the_admirer_draws_his_routine_walk() {
    let Some(session) = party_at(AIEDO, 47, 77, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    driver.set_budget(3_000);
    driver.talk_to(ADMIRER).expect("he speaks");
    let start = driver.runtime().map().npcs()[ADMIRER].cell;
    let mut drawn_y = Vec::new();
    let mut cells = Vec::new();
    while driver.runtime().scene_active() {
        let runtime = driver.runtime();
        if runtime.running_object_routine() == Some(ADMIRER) {
            let cast = runtime.scene_actors();
            let draws = runtime.scene_actor_draws();
            assert!(
                cast.iter().any(|a| a.actor == ActorRef::Npc(ADMIRER)),
                "the cast still holds him"
            );
            assert!(
                draws.iter().all(|a| a.actor != ActorRef::Npc(ADMIRER)),
                "the frozen cast does not draw him"
            );
            assert_eq!(draws.len(), cast.len() - 1, "the rest of the cast draws");
            let motions = runtime.object_motions();
            if runtime.map().npcs()[ADMIRER].active {
                let motion = motions
                    .iter()
                    .find(|m| m.npc_index == ADMIRER)
                    .expect("his routine places him");
                assert_eq!(motion.facing, Direction::Up);
                assert_eq!(motion.cell.x, start.x, "straight up the street");
                drawn_y.push(i32::from(motion.cell.y) * 16 + motion.offset_16ths.1);
                cells.push(motion.cell);
            } else {
                assert!(motions.iter().all(|m| m.npc_index != ADMIRER));
            }
            driver.neutral(1).expect("the routine runs");
        } else {
            driver.tap(Button::Speak).expect("the text runs");
        }
    }
    assert!(!driver.runtime().map().npcs()[ADMIRER].active, "slot clear");
    assert!(
        driver
            .runtime()
            .object_motions()
            .iter()
            .all(|m| m.npc_index != ADMIRER),
        "a cleared slot draws nothing"
    );
    assert!(drawn_y.len() > 1, "the wait drew him");
    assert!(
        drawn_y.windows(2).all(|w| w[1] <= w[0]),
        "he only walks up: {drawn_y:?}"
    );
    assert!(
        drawn_y
            .windows(2)
            .any(|w| w[0] - w[1] > 0 && w[0] - w[1] < 16),
        "steps interpolate: {drawn_y:?}"
    );
    // Every whole step between two cell changes lasts the routine's speed.
    let changes: Vec<usize> = cells
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] != w[1])
        .map(|(i, _)| i + 1)
        .collect();
    assert!(changes.len() >= 2, "he walks several cells: {cells:?}");
    let BespokeKind::Departing { speed, .. } =
        psiv_core::scene_routine_kind(0xBC).expect("$BC is transcribed")
    else {
        panic!("$BC departs");
    };
    assert_eq!(speed, WanderSpeed::Selector2);
    for pair in changes.windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            usize::from(speed.frames()),
            "one cell per step at the routine's speed: {changes:?}"
        );
    }
}

const HUNTERS_GUILD: u16 = 0x057;
/// Below the counter: interaction area 0 is the cell the leader faces from here.
const RECEPTION: (u16, u16) = (29, 26);

fn press(driver: &mut Driver, button: Button) {
    driver.tick(Pad::new(button)).expect("a frame");
    driver.neutral(1).expect("a frame");
}

fn guild_session(flags: &[u16]) -> Option<Driver> {
    let session = party_at(HUNTERS_GUILD, RECEPTION.0, RECEPTION.1, 1000, flags)?;
    let mut driver = Driver::new(session, None);
    press(&mut driver, Button::Up);
    Some(driver)
}

/// Speak until `done`, within a bound: each press pages the text.
fn speak_until(driver: &mut Driver, done: impl Fn(&Driver) -> bool) {
    for _ in 0..600 {
        if done(driver) {
            return;
        }
        press(driver, Button::Speak);
        driver.neutral(6).expect("a frame");
    }
    panic!(
        "no progress at the guild's counter: board {:?}, text {:?}, frames {}",
        driver.session().job_board_view(),
        driver.runtime().dialogue_view().map(|view| view.lines),
        driver.frames()
    );
}

fn cursor_is_up(driver: &Driver) -> bool {
    driver
        .session()
        .job_board_view()
        .is_some_and(|view| view.cursor_lit && !driver.runtime().dialogue_open())
}

/// The whole counter, on the pad: the welcome, the list, the cursor down to
/// the second job, the offer and its YES, the wait for a last press.
#[test]
fn the_guild_takes_a_job_on_the_pad() {
    let Some(mut driver) = guild_session(&[0x44]) else {
        return;
    };
    speak_until(&mut driver, cursor_is_up);
    let view = driver.session().job_board_view().unwrap().clone();
    assert_eq!(view.rows.len(), 8);
    assert!(view.rows[0].pending, "the first job's flag is not set");
    assert!(!view.rows[1].pending, "the second job is offered");
    press(&mut driver, Button::Down);
    assert_eq!(driver.session().job_board_view().unwrap().cursor, 1);
    press(&mut driver, Button::Speak);
    assert_eq!(driver.session().job_board_view().unwrap().stamped, Some(1));
    driver.answer(true).expect("YES");
    assert!(flag(&driver, 0x1E), "EventFlag_TinkerbellsDog");
    for _ in 0..200 {
        if driver.session().job_board_view().is_none() {
            break;
        }
        press(&mut driver, Button::Speak);
    }
    assert!(driver.session().job_board_view().is_none());
    assert!(!driver.runtime().scene_active());
}

/// Cancel at the cursor: the goodbye, nothing taken. The text stays up with
/// the window closed to the pad until the last press.
#[test]
fn cancel_at_the_guild_takes_nothing() {
    let Some(mut driver) = guild_session(&[0x44]) else {
        return;
    };
    speak_until(&mut driver, cursor_is_up);
    press(&mut driver, Button::Cancel);
    speak_until(&mut driver, |driver| {
        driver.session().job_board_view().is_none()
    });
    assert!(!flag(&driver, 0x1E));
}

/// A job's reward, from the flags: the line and the money.
#[test]
fn the_guild_pays_a_finished_job() {
    let Some(mut driver) = guild_session(&[0x1E, 0x24]) else {
        return;
    };
    speak_until(&mut driver, |driver| flag(driver, 0x25));
    assert_eq!(driver.runtime().game().money(), 3000);
}

/// Back to the title with the cursor up: the board and its retained text go.
#[test]
fn the_title_takes_the_board_down() {
    let Some(mut session) = party_at(HUNTERS_GUILD, RECEPTION.0, RECEPTION.1, 1000, &[0x44]) else {
        return;
    };
    let tap = |session: &mut Session, button| {
        session.frame(Pad::new(button));
        for _ in 0..7 {
            session.frame(Pad::NEUTRAL);
        }
    };
    tap(&mut session, Button::Up);
    for _ in 0..600 {
        if session.job_board_view().is_some_and(|view| view.cursor_lit) {
            break;
        }
        tap(&mut session, Button::Speak);
    }
    assert!(session.job_board_view().is_some_and(|view| view.cursor_lit));
    session.start_title();
    assert!(session.job_board_view().is_none());
    assert!(session.runtime().dialogue_view().is_none());
}

/// The welcome up to its last frame of text, the frame before it closes.
fn welcome_ending() -> Option<Session> {
    let mut session = party_at(HUNTERS_GUILD, RECEPTION.0, RECEPTION.1, 1000, &[0x44])?;
    session.frame(Pad::new(Button::Up));
    for _ in 0..7 {
        session.frame(Pad::NEUTRAL);
    }
    session.frame(Pad::new(Button::Speak));
    for _ in 0..6000 {
        let Some(view) = session.runtime().dialogue_view() else {
            session.frame(Pad::NEUTRAL);
            continue;
        };
        if view.dismissable && view.page_end == Some(psiv_data::PageEnd::End) {
            return Some(session);
        }
        let turn = view.dismissable && view.waiting;
        session.frame(Pad::NEUTRAL);
        if turn {
            session.frame(Pad::new(Button::Speak));
        }
    }
    panic!("the welcome never reached its last page");
}

fn hold_updates(session: &mut Session, entry: Pad) -> u64 {
    let before = session.runtime().map_updates().calls;
    session.frame(entry);
    for _ in 0..80 {
        if session
            .job_board_view()
            .is_some_and(|view| !view.rows.is_empty())
        {
            return session.runtime().map_updates().calls - before;
        }
        session.frame(Pad::NEUTRAL);
    }
    panic!("the Hold never ended");
}

/// The welcome's own `jsr DMAPlane_A_VInt` after `RunText` (`.showjobs`): the
/// frame the text closes on runs no update and reads no pad, so the wait
/// starts on the frame after it. `pad` is that frame's.
fn past_the_welcomes_vint(pad: Pad) -> Option<Session> {
    let mut session = welcome_ending()?;
    let before = session.runtime().map_updates().calls;
    session.frame(pad);
    assert!(!session.runtime().dialogue_open(), "the text closed");
    assert_eq!(session.runtime().map_updates().calls, before);
    Some(session)
}

/// `DoMapUpdate_JoypadLoop(#$3B)` through the session: 60 updates on a
/// timeout, the entry frame's among them.
#[test]
fn a_guild_hold_runs_sixty_map_updates() {
    let Some(mut session) = past_the_welcomes_vint(Pad::NEUTRAL) else {
        return;
    };
    assert_eq!(hold_updates(&mut session, Pad::NEUTRAL), 60);
}

/// A press on the welcome's VInt frame is before the loop's test: the wait runs whole.
#[test]
fn a_press_on_the_welcomes_vint_does_not_end_the_guilds_hold() {
    let Some(mut session) = past_the_welcomes_vint(Pad::new(Button::Start)) else {
        return;
    };
    assert_eq!(hold_updates(&mut session, Pad::NEUTRAL), 60);
}

/// The loop tests the pad before it updates: a press on the entry frame leaves
/// with none.
#[test]
fn a_press_entering_a_guild_hold_runs_no_map_update() {
    let Some(mut session) = past_the_welcomes_vint(Pad::NEUTRAL) else {
        return;
    };
    assert_eq!(hold_updates(&mut session, Pad::new(Button::Start)), 0);
}

const STRIP_CLUB: u16 = 0x05A;
/// `TempEveFlag_StrippersDance`.
const STRIPPERS_DANCE: Flag = Flag::temp(0x1B);
/// The customer, `Field_Obj_Secondary`.
const CUSTOMER: usize = 0;

/// Holds `direction` until the leader stands `cells` cells further on, or
/// the frames run out.
fn walk(driver: &mut Driver, direction: Direction, cells: u16) {
    let start = psiv_campaign::driver::standing_cell(driver.runtime());
    let (dx, dy) = direction.delta();
    let goal = (
        i32::from(start.x) + dx * i32::from(cells),
        i32::from(start.y) + dy * i32::from(cells),
    );
    for _ in 0..u32::from(cells) * 40 {
        let at = psiv_campaign::driver::standing_cell(driver.runtime());
        if (i32::from(at.x), i32::from(at.y)) == goal
            && !psiv_campaign::driver::is_stepping(driver.runtime())
        {
            return;
        }
        driver
            .tick(psiv_campaign::driver::dir_pad(direction))
            .expect("the walk");
    }
    panic!("the leader did not reach {goal:?} from {start:?}");
}

/// Stepping up to the stage (cell row 37, `y = $240`, x `$1E0..$270`, trigger `$79`) brings
/// the dancers out (`Event_StrippersAppearing`): the customer takes routine
/// `$34C`, which turns him up with line `$39` and back down with `$38` every
/// 256 frames, and `TempEveFlag_StrippersDance` is set. Leaving through the
/// guild clears it (`Event_ExitingStripClub`, trigger `$7A`).
#[test]
fn the_dancers_come_out_at_the_stage_and_leaving_resets_them() {
    let Some(session) = party_at(STRIP_CLUB, 32, 40, 0, &[]) else {
        return;
    };
    let mut driver = Driver::new(session, None);
    let scenes = driver.scenes_ended();
    let record_line = driver.runtime().npc_dialogue_id(CUSTOMER);
    assert_eq!(
        record_line,
        Some(0x3B),
        "the record's line before the dance"
    );
    walk(&mut driver, Direction::Up, 2);
    assert!(
        !driver.runtime().scene_active(),
        "cell row 38 is below the trigger"
    );
    // One more step up: the trigger fires on row 37 and the scene runs.
    driver.set_budget(2_000);
    for _ in 0..1_000 {
        if driver.scenes_ended() > scenes {
            break;
        }
        let pad = if driver.runtime().scene_active() {
            psiv_runtime::Pad::NEUTRAL
        } else {
            psiv_campaign::driver::dir_pad(Direction::Up)
        };
        driver.tick(pad).expect("the dance");
    }
    assert_eq!(driver.scenes_ended(), scenes + 1, "the scene ran");
    assert!(driver.runtime().game().is_set(STRIPPERS_DANCE));
    let customer = driver.runtime().map().npcs()[CUSTOMER];
    assert_eq!(customer.id.0, 0x34C);
    driver.neutral(1).expect("one field frame");
    assert_eq!(
        driver.runtime().map().npcs()[CUSTOMER].facing,
        Direction::Up
    );
    assert_eq!(driver.runtime().npc_dialogue_id(CUSTOMER), Some(0x39));
    driver.neutral(256).expect("his timer wraps");
    assert_eq!(
        driver.runtime().map().npcs()[CUSTOMER].facing,
        Direction::Down
    );
    assert_eq!(driver.runtime().npc_dialogue_id(CUSTOMER), Some(0x38));

    // Out through the doorway at (34..35, 46): down, right past the customer,
    // down.
    walk(&mut driver, Direction::Down, 6);
    walk(&mut driver, Direction::Right, 2);
    for _ in 0..400 {
        if driver.runtime().map_id().0 == HUNTERS_GUILD
            && !driver.runtime().game().is_set(STRIPPERS_DANCE)
        {
            break;
        }
        driver
            .tick(psiv_campaign::driver::dir_pad(Direction::Down))
            .expect("out to the guild");
    }
    assert_eq!(driver.runtime().map_id().0, HUNTERS_GUILD);
    assert!(!driver.runtime().game().is_set(STRIPPERS_DANCE), "$95 ran");
}
