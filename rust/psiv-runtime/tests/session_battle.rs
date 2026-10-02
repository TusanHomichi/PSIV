//! The first Academy Basement battle, fought with pad presses only.
//!
//! This is the S3 acceptance in one file: a field state on the basement floor,
//! an encounter rolled by walking, and then nothing but joypad bytes — menu
//! navigation, an ATTACK, a technique with a target, a confirm wait — through
//! to victory, the rewards landing in game state, and the return to the field.
//!
//! The formation is tape 07's: two Zoran Bults at positions `$0E` and `$1A`
//! (`docs/oracle/BATTLE_ORACLE_REPLAY.md`; `runtime-pack/battle/formations.json`
//! id 138, formation `$8A`), the party the certified `battle-0x88` capture
//! shows. The AcademyBasement floor's own encounter group carries it (eight of
//! its thirty-two entries are `$8A`), and the walk below is pinned — cell, one
//! idle frame, then up and down the corridor — so the first landing rolls it:
//! `Rng2` mixes `Main_Frame_Count`, so the frame the roll happens on decides
//! which entry the group table pays out.
//!
//! Two negative controls live here as well, because a menu can be wrong in two
//! ways that never fail a happy path: a pad that never confirms must leave the
//! battle running, and cancelling out of a target list must return to the
//! command window without spending a turn.

use std::path::Path;

use psiv_core::battle::Side;
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave, StepFrames};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{
    BattleView, Button, Frame, MenuPage, MenuView, Pad, Runtime, RuntimeEvent, SceneDialogueOpen,
    Session,
};

const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

/// `AcademyBasement`, the first basement floor.
const BASEMENT: u16 = 0x15;
/// The pinned walk's start: open floor, away from the floor's two doorways at
/// `(28..29, 13)` and `(16..17, 20..21)`.
const BASEMENT_SPAWN: Cell = Cell { x: 20, y: 10 };
/// Tape 07's formation: two Zoran Bults at positions `$0E` and `$1A`.
const TAPE_07_FORMATION: u16 = 0x8A;
/// `RES`, the single-target cure Chaz starts with — the technique this test
/// picks and aims, so the target cursor is exercised.
const RES: u8 = 24;

fn pack() -> Option<&'static Path> {
    let pack = Path::new(PACK);
    if !pack.join("manifest.json").is_file() {
        eprintln!("runtime pack not present; skipping");
        return None;
    }
    Some(pack)
}

/// Tape 07's post-opening party on the basement floor, battles armed.
fn basement_session(pack: &Path) -> Session {
    field_session(pack, BASEMENT, BASEMENT_SPAWN)
}

/// The same party on `map` at `cell`, battles armed.
fn field_session(pack: &Path, map: u16, cell: Cell) -> Session {
    let files = BattleFiles::load(pack).expect("battle files load");
    let data = GameData::load(pack).expect("pack loads");
    let mut initial = Runtime::new(
        data.clone(),
        map,
        cell,
        Direction::Down,
        StepFrames::default(),
    )
    .expect("the map spawns");
    initial.enable_battles(&files).expect("battles arm");
    let mut game = GameState::from_snapshot(&initial.game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(1)),
        Some(CharId(2)),
        None,
        None,
    ]);
    game.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 20;
    let mut runtime = Runtime::from_save(
        data,
        RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: cell.x * 16,
                char_y: cell.y * 16,
            },
        },
        StepFrames::default(),
    )
    .expect("the basement save loads");
    runtime.enable_battles(&files).expect("battles arm");
    Session::new(runtime)
}

/// One frame the way the shipped shell runs it: the session's frame, the
/// events a shell routes, then the window's own half of the same frame.
fn tick(session: &mut Session, pad: Pad) -> Frame {
    let frame = session.frame(pad);
    for event in &frame.events {
        match event {
            RuntimeEvent::SceneDialogue { entry } => {
                match session.runtime_mut().open_scene_dialogue(*entry, false) {
                    SceneDialogueOpen::Empty => session.runtime_mut().dialogue_closed(),
                    SceneDialogueOpen::UnknownTree => {
                        panic!("scene dialogue tree is absent from the loaded pack")
                    }
                    SceneDialogueOpen::Opened => {}
                }
            }
            RuntimeEvent::SceneDialogueResume => {
                if !session.runtime_mut().resume_scene_dialogue(false) {
                    session.runtime_mut().dialogue_closed();
                }
            }
            RuntimeEvent::SceneChoiceRequested => {
                session.runtime_mut().open_scene_choice();
            }
            _ => {}
        }
    }
    session.window_tick();
    frame
}

/// Presses through whatever the map opens with.
///
/// The basement floor's entry trigger runs a scene, and a scene's dialogue
/// takes the same Speak presses a player makes: one every fourth frame, which
/// is the shape the oracle tapes use.
fn settle(session: &mut Session) {
    let mut idle = 0;
    for frame in 0..20_000_u64 {
        let ready = session
            .runtime()
            .dialogue_view()
            .is_some_and(|view| view.dismissable || view.choice.is_some_and(|choice| choice.ready));
        let pad = if ready && frame % 4 == 0 {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        tick(session, pad);
        if !session.runtime().scene_active() && !session.runtime().state().is_stepping() {
            idle += 1;
            if idle == 2 {
                return;
            }
        } else {
            idle = 0;
        }
    }
    panic!("the basement's opening scene never released the field");
}

/// Walks up and down the basement floor until an encounter rolls.
///
/// Returns the formation the tables rolled, with the battle already started by
/// the frame that rolled it.
fn walk_until_encounter(session: &mut Session) -> u16 {
    let mut direction = Direction::Down;
    for _ in 0..40_000 {
        let pad = if session.runtime().state().is_stepping() {
            Pad::NEUTRAL
        } else {
            Pad::new(match direction {
                Direction::Up => Button::Up,
                Direction::Down => Button::Down,
                Direction::Left => Button::Left,
                Direction::Right => Button::Right,
            })
        };
        let frame = tick(session, pad);
        for event in &frame.events {
            if let RuntimeEvent::EncounterRolled { formation } = event {
                return *formation;
            }
        }
        if frame
            .events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::StepCompleted { .. }))
        {
            direction = direction.opposite();
        }
    }
    panic!("no encounter rolled on the basement floor");
}

/// One frame of the test's own press policy, driven only by the view.
///
/// It is deliberately the simplest thing that fights: COMD, ATTACK, the first
/// living enemy, and any face button on a waiting page. The `RES` cast, the
/// menu navigation and the cancel below are pressed by hand.
fn fight_press(view: &BattleView) -> Pad {
    if let Some(beat) = view.current {
        return if beat.waits_for_confirm {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
    }
    if !view.ready {
        return Pad::NEUTRAL;
    }
    match view.menu.as_ref() {
        Some(MenuView::Top { .. }) => Pad::new(Button::Speak),
        Some(MenuView::Commands(menu)) => match menu.page {
            MenuPage::Actions if menu.cursor == 0 => Pad::new(Button::Speak),
            MenuPage::Actions => Pad::new(Button::Up),
            MenuPage::Targets(_) if menu.cursor == 0 => Pad::new(Button::Speak),
            MenuPage::Targets(_) => Pad::new(Button::Up),
            _ => Pad::new(Button::Cancel),
        },
        Some(MenuView::VehicleSkills { .. }) | None => Pad::NEUTRAL,
    }
}

/// Presses one button and releases it, so the press is a fresh edge.
fn tap(session: &mut Session, button: Button) -> Frame {
    let frame = tick(session, Pad::new(button));
    tick(session, Pad::NEUTRAL);
    frame
}

/// The view of a battle frame, for the caller's assertions.
fn view(frame: &Frame) -> &BattleView {
    frame
        .battle
        .as_ref()
        .and_then(|battle| battle.view.as_ref())
        .expect("a battle frame carries a view")
}

fn view_of(frame: &Frame) -> Option<&BattleView> {
    frame
        .battle
        .as_ref()
        .and_then(|battle| battle.view.as_ref())
}

/// Waits out the dwell beats until the command window is open.
fn wait_for_menu(session: &mut Session) -> BattleView {
    for _ in 0..2_000 {
        let frame = tick(session, Pad::NEUTRAL);
        if let Some(view) = view_of(&frame)
            && view.ready
        {
            return view.clone();
        }
    }
    panic!("the command window never opened");
}

fn living_enemies(session: &Session) -> Vec<u8> {
    session
        .runtime()
        .battle_roster()
        .into_iter()
        .flat_map(|roster| roster.living(Side::Enemy))
        .map(|fighter| fighter.id.get())
        .collect()
}

#[test]
fn the_first_basement_battle_is_fought_with_pad_presses() {
    let Some(pack) = pack() else {
        return;
    };
    let mut session = basement_session(pack);
    settle(&mut session);
    let money_before = session.runtime().game().money();
    let experience_before: u64 = session
        .runtime()
        .game()
        .party_members()
        .into_iter()
        .filter_map(|who| session.runtime().game().roster().get(who))
        .map(|stats| u64::from(stats.experience))
        .sum();

    // The pin: this state's landing frames roll tape 07's formation.
    tick(&mut session, Pad::NEUTRAL);
    let formation = walk_until_encounter(&mut session);
    println!("basement encounter: formation {formation:#05x}");
    assert_eq!(
        formation, TAPE_07_FORMATION,
        "the basement's first roll in this state is tape 07's own formation"
    );
    assert!(
        session.battle_active(),
        "the frame that rolled the encounter started the battle"
    );
    let cell_at_roll = session.runtime().state().cell();

    // The battle opens on its own `Started` beat: no menu, no command yet.
    let opening = tick(&mut session, Pad::NEUTRAL);
    let opening = view(&opening);
    assert_eq!(opening.enemies.len(), 2, "two Zoran Bults");
    assert!(!opening.ready, "the opening beat owns the frame");

    // The menu opens when the opening beat's dwell runs out, on the top menu:
    // SPEAK presses COMD.
    let top = wait_for_menu(&mut session);
    assert_eq!(top.menu, Some(MenuView::Top { cursor: 0 }));
    assert_eq!(top.cursor, 0);

    // COMD opens the command window on the first actor's ATTACK row.
    let commands = tap(&mut session, Button::Speak);
    let Some(MenuView::Commands(menu)) = view(&commands).menu.clone() else {
        panic!("COMD opens the command window: {:?}", view(&commands).menu);
    };
    assert_eq!(menu.page, MenuPage::Actions);
    assert_eq!(menu.cursor, 0, "ATTACK is the first row");
    assert_eq!(menu.actor, Some(1), "the first party slot answers first");
    assert_eq!(menu.character, Some(0), "Chaz");

    // Menu navigation: one step down to TECH, one step back up.
    let down = tap(&mut session, Button::Down);
    let Some(MenuView::Commands(menu)) = view(&down).menu.clone() else {
        panic!("the command window stays open");
    };
    assert_eq!(menu.cursor, 1, "TECH is the second row");
    assert_eq!(menu.rows[1].label, "TECH");
    let up = tap(&mut session, Button::Up);
    let Some(MenuView::Commands(menu)) = view(&up).menu.clone() else {
        panic!("the command window stays open");
    };
    assert_eq!(menu.cursor, 0, "and back to ATTACK");

    // A technique with a target: TECH -> RES -> a party member.
    tap(&mut session, Button::Down);
    let techniques = tap(&mut session, Button::Speak);
    let Some(MenuView::Commands(menu)) = view(&techniques).menu.clone() else {
        panic!("TECH opens the technique list");
    };
    assert_eq!(menu.page, MenuPage::Techniques);
    let res = menu
        .techniques
        .iter()
        .position(|technique| technique.id == RES)
        .expect("Chaz knows RES");
    assert!(menu.techniques[res].available, "RES is affordable");
    let mut row = menu.cursor;
    while row != res {
        let button = if row < res { Button::Down } else { Button::Up };
        let frame = tap(&mut session, button);
        let Some(MenuView::Commands(inner)) = view(&frame).menu.clone() else {
            panic!("the technique list stays open");
        };
        row = inner.cursor;
    }
    let targets = tap(&mut session, Button::Speak);
    let Some(MenuView::Commands(menu)) = view(&targets).menu.clone() else {
        panic!("a single-target technique asks for a target");
    };
    assert!(
        matches!(menu.page, MenuPage::Targets(_)),
        "RES is single-target: {:?}",
        menu.page
    );
    assert!(!menu.targets.is_empty(), "a party target list");
    let aim = tap(&mut session, Button::Speak);
    let Some(MenuView::Commands(menu)) = view(&aim).menu.clone() else {
        panic!("the second actor's window opens");
    };
    assert_eq!(menu.actor, Some(2), "Chaz's order is committed");
    assert_eq!(menu.page, MenuPage::Actions);

    // The rest of the battle: ATTACK with the first living enemy, any face
    // button on a waiting page.
    let mut confirms = 0;
    let mut victory_page = false;
    for _ in 0..200_000 {
        if !session.battle_active() {
            break;
        }
        let pad = view_of(&tick(&mut session, Pad::NEUTRAL)).map_or(Pad::NEUTRAL, fight_press);
        if pad == Pad::NEUTRAL {
            continue;
        }
        let frame = if pad == Pad::new(Button::Speak) {
            // Release between presses: a held button is not a second press.
            tap(&mut session, Button::Speak)
        } else {
            tick(&mut session, pad)
        };
        let Some(view) = view_of(&frame) else {
            continue;
        };
        if view.current.is_some_and(|beat| beat.waits_for_confirm) {
            confirms += 1;
        }
        if view.message_kind == psiv_runtime::MessageKind::VictoryRewards {
            victory_page = true;
        }
    }

    assert!(!session.battle_active(), "the battle ends");
    assert!(confirms > 0, "the results pages waited for a confirm");
    assert!(victory_page, "the victory rewards page was shown");
    assert!(
        !session.runtime().battle_active(),
        "no battle owns the runtime"
    );
    assert_eq!(
        session.runtime().map_id().0,
        BASEMENT,
        "the field is back on the basement floor"
    );
    assert_eq!(
        session.runtime().state().cell(),
        cell_at_roll,
        "and the fight did not move the party"
    );

    // Rewards land in game state, and the party kept the fight.
    let money_after = session.runtime().game().money();
    assert!(
        money_after > money_before,
        "the victory paid meseta: {money_before} -> {money_after}"
    );
    let experience_after: u64 = session
        .runtime()
        .game()
        .party_members()
        .into_iter()
        .filter_map(|who| session.runtime().game().roster().get(who))
        .map(|stats| u64::from(stats.experience))
        .sum();
    assert!(
        experience_after > experience_before,
        "the victory paid experience: {experience_before} -> {experience_after}"
    );

    // The field takes the next frame back: a field frame with input, no battle.
    let field = tick(&mut session, Pad::NEUTRAL);
    assert!(field.battle.is_none(), "the battle released the frame");
    assert!(
        field.field_input.is_some(),
        "the field is ticked again, not starved"
    );
}

/// Negative control: a pad that never confirms leaves the battle running.
///
/// The command surface has no timeout — the cartridge's own menu loops wait
/// for a press — so a neutral pad must leave the battle exactly where it was:
/// no order, no round, nobody hit.
#[test]
fn a_pad_that_never_confirms_leaves_the_battle_running() {
    let Some(pack) = pack() else {
        return;
    };
    let mut session = basement_session(pack);
    settle(&mut session);
    tick(&mut session, Pad::NEUTRAL);
    assert_eq!(walk_until_encounter(&mut session), TAPE_07_FORMATION);
    let hp: Vec<u16> = session
        .runtime()
        .battle_party()
        .iter()
        .map(|member| member.stats.curr_hp)
        .collect();

    // Six hundred neutral frames: more than the opening beat's dwell, and
    // nothing else.
    let mut ready = false;
    for _ in 0..600 {
        let frame = tick(&mut session, Pad::NEUTRAL);
        if let Some(view) = view_of(&frame) {
            ready |= view.ready;
        }
    }
    assert!(
        session.battle_active(),
        "a pad that never confirms leaves the battle running"
    );
    assert!(ready, "and the command window is still waiting for one");
    println!("after 600 neutral frames: battle running, command window ready, party {hp:?}");
    let after: Vec<u16> = session
        .runtime()
        .battle_party()
        .iter()
        .map(|member| member.stats.curr_hp)
        .collect();
    assert_eq!(
        hp, after,
        "no round ran, so nobody was hit while the window waited"
    );
}

/// Negative control: cancelling out of a target list returns to the menu
/// without issuing an order.
#[test]
fn cancelling_out_of_a_target_list_returns_to_the_menu() {
    let Some(pack) = pack() else {
        return;
    };
    let mut session = basement_session(pack);
    settle(&mut session);
    tick(&mut session, Pad::NEUTRAL);
    assert_eq!(walk_until_encounter(&mut session), TAPE_07_FORMATION);
    wait_for_menu(&mut session);
    tap(&mut session, Button::Speak);
    let attack = tap(&mut session, Button::Speak);
    assert!(matches!(
        view(&attack).menu.as_ref(),
        Some(MenuView::Commands(menu)) if matches!(menu.page, MenuPage::Targets(_))
    ));
    let enemies_before = living_enemies(&session);

    let cancelled = tap(&mut session, Button::Cancel);
    let Some(MenuView::Commands(menu)) = view(&cancelled).menu.clone() else {
        panic!("the command window is back");
    };
    assert_eq!(
        menu.page,
        MenuPage::Actions,
        "cancel returns to the Actions page, not out of the menu"
    );
    assert_eq!(menu.cursor, 0);
    assert_eq!(menu.actor, Some(1), "the same actor still answers");
    assert_eq!(
        living_enemies(&session),
        enemies_before,
        "no order was issued, so no enemy was attacked"
    );
    println!(
        "cancel from the target list: page {:?}, cursor {}, actor {:?}, enemies {enemies_before:?}",
        menu.page, menu.cursor, menu.actor
    );
}
