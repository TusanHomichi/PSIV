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

use psiv_core::battle::{BattleEvent, Priority, Side, status};
use psiv_core::{Cell, CharId, Direction, GameState, RetailLocation, RetailSave};
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::{
    BattleView, Button, CommandMenuView, Frame, MenuPage, MenuView, Pad, RuntimeEvent, Session,
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
/// The item the connected Zio route must keep until the first player round.
const PSYCHO_WAND: u8 = 0x39;

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

/// The same party on `map` at `cell`, battles armed — a game over an in-memory
/// save, through the session's own constructor.
fn field_session(pack: &Path, map: u16, cell: Cell) -> Session {
    let files = BattleFiles::load(pack).expect("battle files load");
    let data = GameData::load(pack).expect("pack loads");
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(1)),
        Some(CharId(2)),
        None,
        None,
    ]);
    game.roster_mut().get_mut(CharId(0)).unwrap().curr_hp = 20;
    Session::start(data)
        .with_battles(files)
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: map,
                char_x: cell.x * 16,
                char_y: cell.y * 16,
            },
        })
        .expect("the basement save loads")
}

/// A saved field position one step before the scripted Zio encounter. The
/// fixture seats ordinary party records and an item; movement, the scene,
/// battle entry and every battle round then use only `Session::frame` pads.
fn zio_ambush_session(pack: &Path) -> Session {
    let files = BattleFiles::load(pack).expect("battle files load");
    let data = GameData::load(pack).expect("pack loads");
    let initial = Session::start(data.clone())
        .with_battles(files.clone())
        .field()
        .expect("the pack boots");
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(4)),
        Some(CharId(0)),
        Some(CharId(5)),
        Some(CharId(6)),
        Some(CharId(3)),
    ]);
    game.inventory_mut().add(PSYCHO_WAND).unwrap();
    Session::start(data)
        .with_battles(files)
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0xD3,
                char_x: 30 * 16,
                char_y: 31 * 16,
            },
        })
        .expect("the Nurvus save loads")
}

/// One frame the way the shipped shell runs it: one call, the session's
/// frame, which routes its own windows.
fn tick(session: &mut Session, pad: Pad) -> Frame {
    session.frame(pad)
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
            // The strip reads Left and Right only (`Battle_CharCommand`), and a
            // member's strip reopens on the icon it left (#51).
            MenuPage::Actions => Pad::new(Button::Left),
            MenuPage::Targets(_) if menu.cursor == 0 => Pad::new(Button::Speak),
            MenuPage::Targets(_) => Pad::new(Button::Up),
            _ => Pad::new(Button::Cancel),
        },
        Some(MenuView::VehicleSkills { .. }) | None => Pad::NEUTRAL,
    }
}

/// Presses one button and releases it, so the press is a fresh edge, then
/// waits out the windows the press closed and opened: `Battle_DestroyWindow`
/// and `Battle_CreateWindow` take a few frames that read no button (#51).
/// Returns the first frame after them.
fn tap(session: &mut Session, button: Button) -> Frame {
    tick(session, Pad::new(button));
    let mut released = tick(session, Pad::NEUTRAL);
    for _ in 0..64 {
        if !growing(&released) {
            break;
        }
        released = tick(session, Pad::NEUTRAL);
    }
    released
}

/// Whether the frame's command window is still opening.
fn growing(frame: &Frame) -> bool {
    matches!(
        view_of(frame).and_then(|view| view.menu.as_ref()),
        Some(MenuView::Commands(menu)) if menu.settling
    ) || matches!(
        view_of(frame).and_then(|view| view.menu.as_ref()),
        Some(MenuView::Top { width: Some(_), .. })
    )
}

/// Neutral frames until no window is opening or closing.
fn settle_windows(session: &mut Session) {
    for _ in 0..64 {
        if !growing(&tick(session, Pad::NEUTRAL)) {
            return;
        }
    }
    panic!("the windows never settled");
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

/// Waits out the dwell beats until the command window is open and whole.
fn wait_for_menu(session: &mut Session) -> BattleView {
    for _ in 0..2_000 {
        let frame = tick(session, Pad::NEUTRAL);
        if let Some(view) = view_of(&frame)
            && view.ready
            && !growing(&frame)
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

fn item_count(session: &Session, item: u8) -> usize {
    session
        .runtime()
        .game()
        .inventory()
        .slots()
        .iter()
        .filter(|&&held| held == item)
        .count()
}

#[test]
fn zio_ambush_uses_no_command_or_item_before_the_first_player_round() {
    let Some(pack) = pack() else {
        return;
    };
    let mut session = zio_ambush_session(pack);
    let mut began = false;
    for frame_index in 0..20_000 {
        let dialogue_ready = session
            .runtime()
            .dialogue_view()
            .is_some_and(|view| view.dismissable || view.choice.is_some_and(|choice| choice.ready));
        let pad = if dialogue_ready && frame_index % 4 == 0 {
            Pad::new(Button::Speak)
        } else if !session.runtime().scene_active() && !session.runtime().state().is_stepping() {
            Pad::new(Button::Up)
        } else {
            Pad::NEUTRAL
        };
        let frame = tick(&mut session, pad);
        if let Some(events) = frame.events.iter().find_map(|event| match event {
            RuntimeEvent::SceneBattleStarted {
                index: 6, events, ..
            } => Some(events),
            _ => None,
        }) {
            assert!(events.iter().any(|event| matches!(
                event,
                BattleEvent::Started {
                    priority: Priority::Ambush,
                    ..
                }
            )));
            began = true;
            break;
        }
    }
    assert!(began, "ordinary Up and Speak pads reach event battle 6");
    assert!(session.battle_active());
    assert_eq!(item_count(&session, PSYCHO_WAND), 1, "one Psycho Wand");

    let mut saw_barrier = false;
    let mut first_player_menu = None;
    for frame_index in 0..2_000 {
        // A player can press during the surprise; those presses do not create
        // orders or spend the item while Zio owns the round.
        let pad = if frame_index % 4 == 0 {
            Pad::new(Button::Speak)
        } else {
            Pad::NEUTRAL
        };
        let frame = tick(&mut session, pad);
        let battle = view(&frame);
        saw_barrier |= battle.message == "MAG.BARRIR";
        if battle.ready {
            first_player_menu = battle.menu.clone();
            break;
        }
        assert_eq!(battle.menu, None, "the ambush has no command surface");
        assert!(battle.poses.is_empty(), "no party attack precedes its menu");
        assert_eq!(
            item_count(&session, PSYCHO_WAND),
            1,
            "the ambush cannot spend the Psycho Wand"
        );
    }
    assert!(saw_barrier, "Zio's first enemy-only action was presented");
    assert!(
        matches!(first_player_menu, Some(MenuView::Top { cursor: 0, .. })),
        "{first_player_menu:?}"
    );
    assert_eq!(item_count(&session, PSYCHO_WAND), 1);
}

#[test]
fn comd_press_with_only_sleeping_actors_enters_the_round_without_a_second_press() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    let data = GameData::load(pack).expect("pack loads");
    let initial = field_session(pack, BASEMENT, BASEMENT_SPAWN);
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    for who in [CharId(0), CharId(1), CharId(2)] {
        game.roster_mut().get_mut(who).unwrap().status = status::ASLEEP;
    }
    let mut session = Session::start(data)
        .with_battles(files)
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: BASEMENT,
                char_x: BASEMENT_SPAWN.x * 16,
                char_y: BASEMENT_SPAWN.y * 16,
            },
        })
        .expect("the sleeping party loads");
    let started = session.debug_battle(TAPE_07_FORMATION);
    assert!(started.fault.is_none());
    let top = wait_for_menu(&mut session);
    assert_eq!(
        top.menu,
        Some(MenuView::Top {
            cursor: 0,
            width: None,
            full_width: 8
        })
    );
    let roster = session.runtime().battle_roster().unwrap();
    assert_eq!(roster.living(Side::Party).count(), 3);
    assert!(
        roster
            .living(Side::Party)
            .all(|fighter| !fighter.stats.can_act()),
        "the `$6E` scan has no actor to command"
    );
    // The main options close first (`Battle_DestroyWindow`: 6, 4, 2 cells),
    // and the round goes on the frame they are gone.
    tick(&mut session, Pad::new(Button::Speak));
    for _ in 0..3 {
        let closing = tick(&mut session, Pad::NEUTRAL);
        assert!(matches!(
            view(&closing).menu,
            Some(MenuView::Top { width: Some(_), .. })
        ));
    }
    let after_comd = tick(&mut session, Pad::NEUTRAL);
    let after_comd = view(&after_comd);
    assert!(!after_comd.ready, "the single COMD press submits the round");
    assert_eq!(after_comd.menu, None, "there is no empty actor page");
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
    assert_eq!(
        top.menu,
        Some(MenuView::Top {
            cursor: 0,
            width: None,
            full_width: 8
        })
    );
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

    // Menu navigation: the strip is Left/Right only (`Battle_CharCommand`),
    // so Down does nothing; one step right to TECH, one step back left.
    let ignored = tap(&mut session, Button::Down);
    let Some(MenuView::Commands(menu)) = view(&ignored).menu.clone() else {
        panic!("the command window stays open");
    };
    assert_eq!(menu.cursor, 0, "Down is not tested by the strip");
    let down = tap(&mut session, Button::Right);
    let Some(MenuView::Commands(menu)) = view(&down).menu.clone() else {
        panic!("the command window stays open");
    };
    assert_eq!(menu.cursor, 1, "TECH is the second icon");
    assert_eq!(menu.rows[1].label, "TECH");
    let up = tap(&mut session, Button::Left);
    let Some(MenuView::Commands(menu)) = view(&up).menu.clone() else {
        panic!("the command window stays open");
    };
    assert_eq!(menu.cursor, 0, "and back to ATTACK");

    // A technique with a target: TECH -> RES -> a party member.
    tap(&mut session, Button::Right);
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

/// The command menu of `frame`, which must have one.
fn commands_of(frame: &Frame) -> CommandMenuView {
    match view(frame).menu.clone() {
        Some(MenuView::Commands(menu)) => *menu,
        other => panic!("the command window is open: {other:?}"),
    }
}

/// Neutral frames while the window grows: the width each one drew, and the
/// first frame it was whole.
fn grow(session: &mut Session) -> (Vec<u8>, CommandMenuView) {
    let mut widths = Vec::new();
    for _ in 0..16 {
        let menu = commands_of(&tick(session, Pad::NEUTRAL));
        match menu.growing {
            Some(growing) => widths.push(growing.width),
            None => return (widths, menu),
        }
    }
    panic!("the window never finished growing: {widths:?}");
}

/// #51: `Battle_CreateWindow` (`ps4.asm:1657`) grows a window two cells a
/// frame, centred, draws it whole once the width reaches the window's, and
/// only then does the window's routine read a button. Tape 32: TECH accepted
/// at f25165 (`Battle_Routine_2` 7), the window `8007` growing f25166-f25170
/// and whole at f25171, `Battle_TechWindow` (8) from f25172; the video shows
/// 2, 4, 6, 8 and 10 cells at f25167-f25171 and the list at f25172.
#[test]
fn the_strip_and_the_technique_window_grow_before_they_read_a_button() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    let mut session = Session::start(GameData::load(pack).expect("pack loads"))
        .with_battles(files)
        .field()
        .expect("the pack boots");
    let opening = session.debug_battle_window(TAPE_07_FORMATION, "top");
    assert!(opening.fault.is_none(), "{:?}", opening.fault);
    wait_for_menu(&mut session);

    // COMD at A (tape 32: f25001): the main options still show, then close
    // 6, 4, 2 cells wide (`Battle_DestroyWindow`; video f25003-f25005).
    let pressed = view(&tick(&mut session, Pad::new(Button::Speak)))
        .menu
        .clone();
    assert!(
        matches!(pressed, Some(MenuView::Top { width: None, .. })),
        "{pressed:?}"
    );
    let mut closing = Vec::new();
    for _ in 0..3 {
        match view(&tick(&mut session, Pad::NEUTRAL)).menu.clone() {
            Some(MenuView::Top {
                width: Some(width), ..
            }) => closing.push(width),
            other => panic!("the main options are closing: {other:?}"),
        }
    }
    assert_eq!(closing, [6, 4, 2]);
    // A+4 they are gone and the 16-cell strip starts; its setup frame (A+5)
    // draws nothing either, and its first two cells are A+6's (f25007).
    for frame in ["gone", "setup"] {
        let opened = commands_of(&tick(&mut session, Pad::NEUTRAL));
        let growing = opened.growing.expect("the strip opens growing");
        assert_eq!(
            (growing.window, growing.width, growing.full_width),
            (MenuPage::Actions, 0, 16),
            "{frame}"
        );
        assert!(opened.strip.is_none());
    }
    let (widths, strip) = grow(&mut session);
    assert_eq!(widths, [2, 4, 6, 8, 10, 12, 14]);
    let drawn = strip.strip.expect("the whole strip, icons and all");
    assert_eq!(drawn.age, 0, "the cursor object is created on this frame");

    // Right to TECH, then accept: the 12-cell window grows over the strip.
    tick(&mut session, Pad::new(Button::Right));
    tick(&mut session, Pad::NEUTRAL);
    let accepted = commands_of(&tick(&mut session, Pad::new(Button::Speak)));
    assert_eq!(accepted.page, MenuPage::Techniques);
    let growing = accepted.growing.expect("the list opens growing");
    assert_eq!((growing.width, growing.full_width), (0, 12));
    assert!(accepted.strip.is_some() && accepted.list.is_none());
    let mut widths = Vec::new();
    for frame in 0..5 {
        // A Down pressed while the window grows is not read.
        let pad = if frame == 2 {
            Pad::new(Button::Down)
        } else {
            Pad::NEUTRAL
        };
        let menu = commands_of(&tick(&mut session, pad));
        let growing = menu.growing.expect("still growing");
        widths.push((growing.width, growing.left_offset()));
        assert!(menu.list.is_none());
    }
    assert_eq!(widths, [(2, 5), (4, 4), (6, 3), (8, 2), (10, 1)]);
    let whole_frame = tick(&mut session, Pad::NEUTRAL);
    let whole = commands_of(&whole_frame);
    assert!(whole.growing.is_none());
    assert!(
        !view(&whole_frame).cursor_red,
        "the list's routine has not run: its row is blue"
    );
    assert_eq!(
        whole.list.expect("the list, whole").cursor,
        0,
        "the Down was not read"
    );
    let moved_frame = tick(&mut session, Pad::new(Button::Down));
    assert!(
        view(&moved_frame).cursor_red,
        "the routine runs and a held button shows red"
    );
    let moved = commands_of(&moved_frame);
    assert_eq!(
        moved.list.expect("the list").cursor,
        1,
        "the next frame reads it"
    );
}

/// #51: the frame a list is drawn whole is `Battle_CreateWindow`'s, not the
/// list routine's, so its selected row is blue whatever the blink last
/// showed; the routine runs the next frame and decides it
/// (`Battle_UpdateRedCursor2`, `ps4.asm:1572`). The strip fixture starts the
/// blink red, so a stale value would show. TECH is covered above; this is
/// SKILL and ITEM.
#[test]
fn the_skill_and_item_windows_show_no_red_cursor_until_their_routine_runs() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    for (icon, page) in [(2, MenuPage::Skills), (3, MenuPage::Items)] {
        let mut session = Session::start(GameData::load(pack).expect("pack loads"))
            .with_battles(files.clone())
            .field()
            .expect("the pack boots");
        let opening =
            session.debug_battle_window(TAPE_07_FORMATION, &format!("strip,cursor={icon}"));
        assert!(opening.fault.is_none(), "{:?}", opening.fault);
        tick(&mut session, Pad::NEUTRAL);
        let accepted = commands_of(&tick(&mut session, Pad::new(Button::Speak)));
        assert_eq!(accepted.page, page);
        let mut frames = 0;
        let whole = loop {
            let frame = tick(&mut session, Pad::NEUTRAL);
            if commands_of(&frame).growing.is_none() {
                break frame;
            }
            frames += 1;
            assert!(frames < 16, "{page:?} never finished growing");
        };
        assert!(commands_of(&whole).list.is_some(), "{page:?} is whole");
        assert!(
            !view(&whole).cursor_red,
            "{page:?}: the whole frame's row is blue"
        );
        let next = tick(&mut session, Pad::new(Button::Down));
        assert!(
            view(&next).cursor_red,
            "{page:?}: the routine runs and a held button shows red"
        );
    }
}

/// Each frame's opening or closing window: its page, its width (`None` once
/// whole) and the acting fighter.
fn window_frames(session: &mut Session, frames: usize) -> Vec<(MenuPage, Option<u8>, Option<u8>)> {
    (0..frames)
        .map(|_| {
            let menu = commands_of(&tick(session, Pad::NEUTRAL));
            (menu.page, menu.growing.map(|g| g.width), menu.actor)
        })
        .collect()
}

/// #51: `Battle_DestroyWindow` (`ps4.asm:1678`) narrows a window two cells a
/// frame and moves on the frame it is gone. Tape 37 (tape 32, then every way
/// the windows close), with A the press:
/// - B in the technique list (f25505): 10, 8, 6, 4 and 2 cells at A+1..A+5
///   (video f25507-f25511), the strip again at A+6, read from A+7;
/// - DEFEND (f25605): the strip 14 down to 2 at A+1..A+7, gone at A+8, and
///   the next member's strip's first two cells at A+9 (video f25615);
/// - B on that strip (f25689): the same, back to the previous member;
/// - SANER (f25853): the list 10..2 at A+1..A+5, the strip whole at A+6, then
///   14..2 at A+7..A+13, gone at A+14, the next strip at A+15 (video f25869).
#[test]
fn the_windows_close_two_cells_a_frame_before_the_next_one_opens() {
    let Some(pack) = pack() else {
        return;
    };
    let files = BattleFiles::load(pack).expect("battle files load");
    let mut session = Session::start(GameData::load(pack).expect("pack loads"))
        .with_battles(files)
        .field()
        .expect("the pack boots");
    let opening = session.debug_battle_window(TAPE_07_FORMATION, "top");
    assert!(opening.fault.is_none(), "{:?}", opening.fault);
    wait_for_menu(&mut session);
    tap(&mut session, Button::Speak);
    tap(&mut session, Button::Right);
    let listed = commands_of(&tap(&mut session, Button::Speak));
    assert_eq!(listed.page, MenuPage::Techniques);
    let alys = listed.actor;

    let tech = MenuPage::Techniques;
    let strip = MenuPage::Actions;
    // B in the list.
    tick(&mut session, Pad::new(Button::Cancel));
    let frames = window_frames(&mut session, 6);
    let list_closing: Vec<_> = [10, 8, 6, 4, 2].map(|w| (tech, Some(w), alys)).into();
    assert_eq!(frames[..5], list_closing[..]);
    assert_eq!(frames[5], (strip, None, alys), "the strip again");
    let back = commands_of(&tick(&mut session, Pad::new(Button::Right)));
    assert_eq!(back.cursor, 2, "the strip reads the next frame's press");
    tick(&mut session, Pad::NEUTRAL);

    // Right twice more to DEFEND, then accept: the strip closes and the next
    // member's opens.
    tap(&mut session, Button::Right);
    tap(&mut session, Button::Right);
    tick(&mut session, Pad::new(Button::Speak));
    let frames = window_frames(&mut session, 9);
    let strip_closing: Vec<_> = [14, 12, 10, 8, 6, 4, 2]
        .map(|w| (strip, Some(w), alys))
        .into();
    assert_eq!(frames[..7], strip_closing[..]);
    let chaz = frames[7].2;
    assert_ne!(chaz, alys, "the next member answers");
    assert_eq!(
        frames[7..],
        [(strip, Some(0), chaz), (strip, Some(2), chaz)]
    );
    settle_windows(&mut session);
    let settled = commands_of(&tap(&mut session, Button::Cancel));
    // B on that strip: it closes and Alys's reopens.
    assert_eq!(
        settled.actor, alys,
        "Cancel goes back to the previous member"
    );

    // TECH and the first technique (SANER: no target): the list closes, the
    // strip shows whole for a frame, then closes, then the next strip opens.
    for _ in 0..3 {
        tap(&mut session, Button::Left);
    }
    let listed = commands_of(&tap(&mut session, Button::Speak));
    assert_eq!((listed.page, listed.cursor), (tech, 0));
    tick(&mut session, Pad::new(Button::Speak));
    let frames = window_frames(&mut session, 15);
    assert_eq!(frames[..5], list_closing[..]);
    assert_eq!(
        frames[5],
        (strip, None, alys),
        "the strip whole for a frame"
    );
    assert_eq!(frames[6..13], strip_closing[..]);
    assert_eq!(
        frames[13..],
        [(strip, Some(0), chaz), (strip, Some(2), chaz)]
    );
}

/// Plays tape 07's first round with pad presses only: Alys opens TECH and
/// picks the first entry, Chaz and Hahn defend. `spec` seeds the fixture
/// (`PSIV_DEBUG_BATTLE_WINDOW`). Returns the narration lines the round showed,
/// Alys's TP when the options came back, and whether the first technique row
/// was enabled in the list.
fn alys_casts_her_first_technique(spec: &str) -> Option<(Vec<String>, u16, bool)> {
    let pack = pack()?;
    let files = BattleFiles::load(pack).expect("battle files load");
    let mut session = Session::start(GameData::load(pack).expect("pack loads"))
        .with_battles(files)
        .field()
        .expect("the pack boots");
    let opening = session.debug_battle_window(TAPE_07_FORMATION, spec);
    assert!(opening.fault.is_none(), "{:?}", opening.fault);
    wait_for_menu(&mut session);
    // COMD opens Alys's strip; Right is TECH; Speak opens the list.
    tap(&mut session, Button::Speak);
    tap(&mut session, Button::Right);
    let listed = tap(&mut session, Button::Speak);
    let Some(MenuView::Commands(menu)) = view(&listed).menu.clone() else {
        panic!("TECH opens the technique list");
    };
    assert_eq!(menu.page, MenuPage::Techniques);
    let enabled = menu.rows[0].enabled;
    // Choose the first row and any target it asks for, then let Chaz and
    // Hahn defend.
    let mut defenders = 0;
    for _ in 0..40 {
        let current = tick(&mut session, Pad::NEUTRAL);
        let Some(MenuView::Commands(menu)) = view(&current).menu.clone() else {
            break;
        };
        match (menu.page, menu.actor) {
            (MenuPage::Techniques, _) => {
                tap(&mut session, Button::Speak);
            }
            (MenuPage::Targets(_), _) => {
                tap(&mut session, Button::Speak);
            }
            (MenuPage::Actions, Some(2 | 3)) => {
                let steps = (4 + 5 - menu.cursor) % 5;
                for _ in 0..steps {
                    tap(&mut session, Button::Right);
                }
                tap(&mut session, Button::Speak);
                defenders += 1;
            }
            _ => break,
        }
    }
    assert_eq!(defenders, 2, "Chaz and Hahn defended");
    let mut lines = Vec::new();
    for _ in 0..3_000 {
        let frame = tick(&mut session, Pad::NEUTRAL);
        let current = view(&frame);
        if !current.message.is_empty() && lines.last() != Some(&current.message) {
            lines.push(current.message.clone());
        }
        if current.ready {
            let alys = current.party.iter().find(|m| m.fighter == 1).unwrap();
            return Some((lines, alys.tp, enabled));
        }
    }
    panic!("the round never ended: {lines:?}");
}

/// A sealed member may choose a technique: the window refuses an entry on TP
/// alone (`ps4.asm:1721`, `2628`), and the cast is paid and then wasted
/// (`CharTech_CheckTPCost` `ps4.asm:14211`, `CharTech_Cast` `ps4.asm:14256`).
/// The control is the same round unsealed: the cast lands and says nothing of
/// a seal.
#[test]
fn a_sealed_member_may_choose_a_technique_and_the_cast_is_wasted() {
    let Some((sealed_lines, sealed_tp, sealed_enabled)) =
        alys_casts_her_first_technique("top,status=1/16")
    else {
        return;
    };
    assert!(sealed_enabled, "the window does not refuse a sealed member");
    assert!(
        sealed_lines.iter().any(|line| line == "Tech sealed!"),
        "the cast is wasted: {sealed_lines:?}"
    );
    assert_eq!(sealed_tp, 40 - 6, "SANER's 6 TP are paid anyway");

    let (free_lines, free_tp, free_enabled) = alys_casts_her_first_technique("top").unwrap();
    assert!(free_enabled);
    assert!(
        !free_lines.iter().any(|line| line == "Tech sealed!"),
        "negative control: an unsealed cast is not wasted: {free_lines:?}"
    );
    assert_eq!(free_tp, 40 - 6);
}
