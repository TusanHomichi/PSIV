//! The runner against the real pack: the same pads give the same game, a tape
//! replays to the digest it recorded, a chapter save resumes the next chapter,
//! and a broken route halts with a report that names what broke.
//!
//! Every case plays the opening academy chapter or a piece of it, which takes
//! seconds in a debug build. The whole route from New Game is `#[ignore]`d:
//!
//! ```text
//! cargo test --release -p psiv-campaign --test runner -- --ignored --test-threads=1
//! ```
//!
//! Cases that need the pack skip with a message when `runtime-pack` is absent,
//! like the other crates' tests.

mod common;

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use psiv_campaign::driver::Driver;
use psiv_campaign::halt::HaltKind;
use psiv_campaign::replay::replay;
use psiv_campaign::route::{Chapter, Route};
use psiv_campaign::runner::{RunConfig, RunResult, run};
use psiv_campaign::start::{StartPoint, open_session};
use psiv_campaign::tape::Tape;
use psiv_core::battle::{FighterId, Side, item_targets, technique_targets};
use psiv_core::{CharId, Flag, GameState, RetailLocation, RetailSave};
use psiv_runtime::{PartyStatus, Session, TechniqueEntry};

use common::{MAIN_ROUTE, pack, pack_dir};

fn main_text() -> String {
    std::fs::read_to_string(MAIN_ROUTE).expect("main.json reads")
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("runner")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn config(text: &str, save: &str, until: Option<&str>, from: Option<&str>) -> RunConfig {
    config_in(text, scratch(save), until, from)
}

/// A run configuration over an existing save directory.
fn config_in(text: &str, save_dir: PathBuf, until: Option<&str>, from: Option<&str>) -> RunConfig {
    RunConfig {
        route: Route::parse(text).expect("the route parses"),
        pack: pack_dir(),
        from_chapter: from.map(str::to_owned),
        until_chapter: until.map(str::to_owned),
        save_dir,
        verbose: false,
    }
}

fn mutate(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "the route has no {from:?} to mutate");
    text.replacen(from, to, 1)
}

/// Chapter one, played once for the whole binary: its tape, digest and saves
/// are the reference several cases compare against.
fn academy() -> Option<&'static (RunConfig, RunResult)> {
    static ACADEMY: OnceLock<Option<(RunConfig, RunResult)>> = OnceLock::new();
    ACADEMY
        .get_or_init(|| {
            pack()?;
            let config = config(&main_text(), "academy-a", Some("academy"), None);
            let result = run(&config).expect("chapter one sets up");
            Some((config, result))
        })
        .as_ref()
}

#[test]
fn chapter_one_plays_through_to_motavia() {
    let Some((_, result)) = academy() else {
        return;
    };
    assert!(result.completed, "report: {:#?}", result.report);
    assert_eq!(result.chapters.len(), 1);
    assert_eq!(result.chapters[0].id, "academy");
    assert!(result.chapters[0].save.is_file(), "the chapter save exists");
    assert!(result.tape.pads.len() > 10_000, "a real chapter of frames");
    assert!(
        result.battles.iter().any(|b| b.kind == "event"),
        "Igglanova's scripted battle was fought"
    );
}

/// The determinism contract: same route, same pack, same pads, same game.
#[test]
fn chapter_one_twice_gives_identical_tapes_digests_and_saves() {
    let Some((_, first)) = academy() else {
        return;
    };
    let second = run(&config(&main_text(), "academy-b", Some("academy"), None)).unwrap();
    assert!(second.completed);
    assert_eq!(first.tape, second.tape, "identical tapes");
    assert_eq!(first.digest, second.digest, "identical final-state digests");
    let a = std::fs::read(&first.chapters[0].save).unwrap();
    let b = std::fs::read(&second.chapters[0].save).unwrap();
    assert_eq!(a, b, "identical chapter saves");
}

#[test]
fn replaying_a_recorded_tape_reproduces_its_digest() {
    let Some((_, result)) = academy() else {
        return;
    };
    let text = result.tape.render();
    let tape = Tape::parse(&text).expect("a rendered tape parses");
    assert_eq!(tape, result.tape);
    let replayed = replay(pack_dir().as_path(), &tape, None).expect("replay sets up");
    assert_eq!(replayed.frames, result.tape.pads.len() as u64);
    assert_eq!(replayed.digest, result.digest);
    assert!(replayed.faults.is_empty(), "{:?}", replayed.faults);
}

/// A chapter starts from the save the chapter before it ended on, and its tape
/// replays from that save to the same digest.
#[test]
fn a_chapter_save_resumes_the_next_chapter_and_its_tape_replays() {
    let Some((first_config, _)) = academy() else {
        return;
    };
    let resume = config_in(
        &main_text(),
        first_config.save_dir.clone(),
        Some("holt"),
        Some("holt"),
    );
    let result = run(&resume).expect("holt sets up from the academy save");
    assert!(result.completed, "report: {:#?}", result.report);
    assert_eq!(result.chapters[0].id, "holt");
    let save = psiv_campaign::runner::chapter_save_file(
        &first_config.save_dir,
        0,
        &resume.route.chapters[0],
    );
    let replayed =
        replay(pack_dir().as_path(), &result.tape, Some(&save)).expect("replay from the save");
    assert_eq!(replayed.digest, result.digest);
    // The tape names its save: the wrong file is refused.
    let other = result.chapters[0].save.clone();
    assert!(replay(pack_dir().as_path(), &result.tape, Some(&other)).is_err());
    assert!(replay(pack_dir().as_path(), &result.tape, None).is_err());
}

#[test]
fn a_chapter_without_its_predecessors_save_is_a_setup_error() {
    if pack().is_none() {
        return;
    }
    let missing = config(&main_text(), "no-saves", Some("holt"), Some("holt"));
    let error = run(&missing).unwrap_err();
    assert!(error.0.contains("does not exist"), "{error}");
    let unknown = config(&main_text(), "unknown", Some("nowhere"), None);
    let error = run(&unknown).unwrap_err();
    assert!(error.0.contains("nowhere"), "{error}");
}

/// Negative control: an object index the map does not have halts the run at
/// that objective, with a report that names it.
#[test]
fn a_wrong_object_index_halts_naming_the_objective() {
    if pack().is_none() {
        return;
    }
    let text = mutate(
        &main_text(),
        "{\"do\": \"talk\", \"npc\": 0, \"note\": \"principal: face Up, Action\"}",
        "{\"do\": \"talk\", \"npc\": 77, \"note\": \"principal: face Up, Action\"}",
    );
    let result = run(&config(&text, "bad-npc", Some("academy"), None)).unwrap();
    assert!(!result.completed);
    let report = result.report.expect("a halt writes a report");
    assert_eq!(report["chapter"], "academy");
    assert_eq!(report["objective_index"], 4);
    assert_eq!(report["objective_kind"], "talk");
    assert_eq!(report["halt"]["kind"], "wrong_object");
    assert!(
        report["halt"]["detail"]
            .as_str()
            .unwrap()
            .contains("object 77 does not exist"),
        "{report}"
    );
    for key in [
        "frame", "map", "cell", "mode", "party", "money", "events", "view",
    ] {
        assert!(report.get(key).is_some(), "the report holds {key}");
    }
    assert_eq!(report["party"][0]["name"], "Alys");
    assert!(report["events"].as_array().unwrap().len() <= 50);
    // A halted run still replays to the state it halted in.
    let replayed = replay(pack_dir().as_path(), &result.tape, None).unwrap();
    assert_eq!(replayed.digest, result.digest);
}

/// Negative control: an `expect` that cannot hold halts at that objective.
#[test]
fn an_expectation_that_cannot_hold_halts_at_that_objective() {
    if pack().is_none() {
        return;
    }
    let text = mutate(
        &main_text(),
        "{\"do\": \"expect\", \"flags_set\": [\"event:0x8\"], \"map\": 19,",
        "{\"do\": \"expect\", \"flags_set\": [\"event:0x8\", \"event:0x1ff\"], \"map\": 19,",
    );
    let result = run(&config(&text, "bad-expect", Some("academy"), None)).unwrap();
    let report = result.report.expect("a halt writes a report");
    assert_eq!(report["objective_index"], 1);
    assert_eq!(report["objective_kind"], "expect");
    assert_eq!(report["halt"]["kind"], "expect_failed");
    let detail = report["halt"]["detail"].as_str().unwrap();
    assert!(detail.contains("event:0x1ff"), "{detail}");
    assert!(
        !detail.contains("event:0x8 "),
        "the clause that held is not blamed: {detail}"
    );
}

/// Negative control: a target the planner cannot reach halts as unreachable.
#[test]
fn an_unreachable_target_halts_as_unreachable() {
    if pack().is_none() {
        return;
    }
    let text = mutate(
        &main_text(),
        "{\"do\": \"go_to\", \"map\": 19, \"cell\": [38, 17],",
        "{\"do\": \"go_to\", \"map\": 19, \"cell\": [0, 0],",
    );
    let result = run(&config(&text, "bad-cell", Some("academy"), None)).unwrap();
    let report = result.report.expect("a halt writes a report");
    assert_eq!(report["objective_index"], 0);
    assert_eq!(report["halt"]["kind"], "unreachable");
}

/// Negative control: the frame budget is a halt, not a hang.
#[test]
fn an_exhausted_frame_budget_halts() {
    if pack().is_none() {
        return;
    }
    let (session, _) = open_session(pack_dir().as_path(), &StartPoint::NewGame).unwrap();
    let mut driver = Driver::new(session, None);
    driver.set_budget(10);
    let halt = driver.neutral(50).unwrap_err();
    assert_eq!(halt.kind, HaltKind::BudgetExhausted);
    assert_eq!(
        driver.frames(),
        10,
        "the budget stops the tape at 10 frames"
    );
}

/// A one-chapter shopping trip after the academy: the route file's controllers
/// the main route never needs (`sell`, `use_item`, an explicit `use_technique`)
/// walk the same menus.
fn shop_trip(objectives: &str) -> Route {
    let mut route = Route::parse(&main_text()).unwrap();
    route.chapters.truncate(1);
    let chapter = format!(
        r#"{{"id": "item-shop", "title": "Piata item shop", "source": "tests/runner.rs",
            "random_battle_policy": "default",
            "objectives": [
              {{"do": "go_to_map", "map": 27}},
              {{"do": "go_to", "map": 27, "cell": [33, 32]}},
              {objectives}
            ]}}"#
    );
    route
        .chapters
        .push(serde_json::from_str::<Chapter>(&chapter).expect("the chapter parses"));
    route
}

fn shop_trip_result(objectives: &str) -> Option<RunResult> {
    let (first_config, _) = academy()?;
    let config = RunConfig {
        route: shop_trip(objectives),
        pack: pack_dir(),
        from_chapter: Some("item-shop".to_owned()),
        until_chapter: Some("item-shop".to_owned()),
        save_dir: first_config.save_dir.clone(),
        verbose: false,
    };
    Some(run(&config).expect("the trip sets up from the academy save"))
}

#[test]
fn the_item_shop_buys_sells_and_the_camp_uses_what_was_bought() {
    let Some(result) = shop_trip_result(
        r#"{"do": "buy", "item": "MONOMATE", "count": 2, "face": "up"},
           {"do": "sell", "item": "MONOMATE", "face": "up"},
           {"do": "use_technique", "caster": "Hahn", "technique": "RES", "target": "Alys"},
           {"do": "use_item", "item": "MONOMATE", "target": "Alys"}"#,
    ) else {
        return;
    };
    assert!(result.completed, "report: {:#?}", result.report);
}

/// Negative control for the trip above: two bought, one sold, one used leaves
/// none, and asking the pack for a second use halts naming what the pack holds.
#[test]
fn using_an_item_the_pack_no_longer_holds_halts() {
    let Some(result) = shop_trip_result(
        r#"{"do": "buy", "item": "MONOMATE", "count": 2, "face": "up"},
           {"do": "sell", "item": "MONOMATE", "face": "up"},
           {"do": "use_item", "item": "MONOMATE", "target": "Alys"},
           {"do": "use_item", "item": "MONOMATE", "target": "Alys"}"#,
    ) else {
        return;
    };
    let report = result.report.expect("the second use halts");
    assert_eq!(report["objective_kind"], "use_item");
    assert_eq!(report["objective_index"], 5);
    assert_eq!(report["halt"]["kind"], "menu_entry_missing");
    assert!(
        report["halt"]["detail"]
            .as_str()
            .unwrap()
            .contains("MONOMATE"),
        "{report}"
    );
}

fn menu_window(actor: u8) -> psiv_runtime::CommandMenuView {
    psiv_runtime::CommandMenuView {
        strip: None,
        list: None,
        title: String::new(),
        page: psiv_runtime::MenuPage::Actions,
        rows: Vec::new(),
        cursor: 0,
        actor: Some(actor),
        character: Some(actor - 1),
        party: Vec::new(),
        enemies: vec![6],
        techniques: Vec::new(),
        skills: Vec::new(),
        targets: Vec::new(),
    }
}

/// RES and MONOMATE offer humans, not Demi. A desperate android in the party
/// strip must never turn a healing policy's intent into a missing menu target.
#[test]
fn mixed_party_cures_use_the_menus_eligible_targets() {
    use psiv_campaign::policy::{DefaultPolicy, Intent, Policy};
    use psiv_campaign::policy_boss::BossPolicy;

    let Some(set) = pack() else {
        return;
    };
    let initial = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .field()
        .expect("the pack boots");
    let item_id = initial
        .runtime()
        .battle_items()
        .find(|item| item.name == "MONOMATE")
        .expect("MONOMATE is in the battle pack")
        .id;
    let mut game = GameState::from_snapshot(&initial.runtime().game().snapshot());
    game.set_party([
        Some(CharId(0)),
        Some(CharId(6)),
        Some(CharId(5)),
        None,
        None,
    ]);
    game.roster_mut().get_mut(CharId(0)).unwrap().curr_tp = 20;
    game.roster_mut().get_mut(CharId(6)).unwrap().curr_hp = 1;
    let rika = game.roster_mut().get_mut(CharId(5)).unwrap();
    rika.curr_hp = (rika.max_hp / 3).max(2);
    rika.curr_tp = 50;
    rika.techniques[0] = 28;
    game.inventory_mut().add(item_id).unwrap();
    let mut session = Session::start(set.data.clone())
        .with_battles(set.battle.clone())
        .from_save(RetailSave {
            snapshot: game.snapshot(),
            location: RetailLocation {
                world_index: 0,
                map_index_2: 0,
                map_index: 0x15,
                char_x: 20 * 16,
                char_y: 10 * 16,
            },
        })
        .expect("the mixed party loads");
    assert!(session.debug_battle(0x8a).fault.is_none());
    let runtime = session.runtime();
    let roster = runtime.battle_roster().expect("the battle has a roster");
    let actor = FighterId::new(1).unwrap();
    let demi = FighterId::new(2).unwrap();
    let rika = FighterId::new(3).unwrap();
    let res = runtime
        .battle_techniques()
        .find(|tech| tech.id == 24)
        .unwrap();
    let monomate = runtime
        .battle_items()
        .find(|item| item.id == item_id)
        .unwrap();
    for targets in [
        technique_targets(roster, actor, res),
        item_targets(roster, actor, monomate),
    ] {
        assert!(targets.contains(&rika), "Rika must be selectable");
        assert!(
            !targets.contains(&demi),
            "Demi must not appear in the cure menu"
        );
    }

    let mut menu = menu_window(1);
    menu.party = roster
        .side(Side::Party)
        .map(|fighter| PartyStatus {
            fighter: fighter.id.get(),
            name: fighter.name.clone(),
            hp: fighter.stats.curr_hp,
            max_hp: fighter.stats.max_hp,
            tp: fighter.stats.curr_tp,
            status: fighter.stats.status,
        })
        .collect();
    assert_eq!(menu.party[1].fighter, demi.get());
    assert!(menu.party[1].hp < menu.party[2].hp);
    menu.techniques.push(TechniqueEntry {
        id: res.id,
        name: res.name.clone(),
        cost: res.cost,
        available: true,
    });
    let cure = Intent::Technique {
        id: res.id,
        target: Some(rika.get()),
    };
    assert_eq!(DefaultPolicy::default().choose(&menu, runtime), cure);
    assert_eq!(BossPolicy::default().choose(&menu, runtime), cure);

    menu.techniques.clear();
    let item_cure = Intent::Item {
        name: monomate.name.clone(),
        target: Some(rika.get()),
    };
    assert_eq!(DefaultPolicy::default().choose(&menu, runtime), item_cure);
    assert_eq!(BossPolicy::default().choose(&menu, runtime), item_cure);

    // Only the android needs HP now: neither policy may order an unavailable
    // human cure or spend the item on the wrong character.
    menu.party[2].hp = menu.party[2].max_hp;
    assert_eq!(
        DefaultPolicy::default().choose(&menu, runtime),
        Intent::Attack
    );
    assert_eq!(BossPolicy::default().choose(&menu, runtime), Intent::Attack);

    // After group damage, two humans at 60% warrant the learned GISAR now;
    // waiting for both to fall below half costs a lethal extra enemy round.
    let gisar = runtime
        .battle_techniques()
        .find(|tech| tech.id == 28)
        .unwrap();
    assert!(
        roster
            .get(rika)
            .unwrap()
            .stats
            .techniques
            .contains(&gisar.id)
    );
    menu.actor = Some(rika.get());
    menu.character = Some(5);
    menu.party[0].hp = menu.party[0].max_hp * 3 / 5;
    menu.party[2].hp = menu.party[2].max_hp * 3 / 5;
    menu.techniques.push(TechniqueEntry {
        id: gisar.id,
        name: gisar.name.clone(),
        cost: gisar.cost,
        available: true,
    });
    assert_eq!(
        BossPolicy::default().choose(&menu, runtime),
        Intent::Technique {
            id: gisar.id,
            target: None
        }
    );
}

/// The opening-item policy (`psycho_wand_then_win`'s type) over a pack that
/// holds an item, bought at the Piata shop as a player does: the first actor
/// of the first command round takes ITEM and the rest fight; the next round
/// and the next battle start from the boss policy. Negative controls: an
/// enemy-only ambush round, a random encounter and a pack without the item.
#[test]
fn the_first_actor_of_a_scripted_battle_opens_with_the_item() {
    use psiv_campaign::policy::{Intent, Policy};
    use psiv_campaign::policy_opening::OpeningItemPolicy;

    let Some(result) =
        shop_trip_result(r#"{"do": "buy", "item": "MONOMATE", "count": 1, "face": "up"}"#)
    else {
        return;
    };
    assert!(result.completed, "report: {:#?}", result.report);
    let save = result.chapters.last().expect("the trip saved").save.clone();
    let (session, _) = open_session(pack_dir().as_path(), &StartPoint::Save(save)).unwrap();
    let runtime = session.runtime();
    let item = runtime
        .battle_items()
        .find(|item| item.name == "MONOMATE")
        .expect("MONOMATE has battle data");
    assert!(runtime.game().inventory().contains(item.id));
    let opening = Intent::Item {
        name: item.name.clone(),
        target: None,
    };

    let mut policy = OpeningItemPolicy::new("test_opening", item.id);
    policy.battle_begins(true);
    policy.end_round();
    assert_eq!(
        policy.choose(&menu_window(1), runtime),
        opening,
        "an ambush round with no command window cannot spend the opening"
    );
    assert_eq!(policy.choose(&menu_window(1), runtime), opening);
    assert_eq!(
        policy.choose(&menu_window(1), runtime),
        opening,
        "the same actor's window reopens on the same choice"
    );
    assert_eq!(
        policy.choose(&menu_window(2), runtime),
        Intent::Attack,
        "the opening is spent: everyone else fights"
    );
    policy.end_round();
    assert_eq!(policy.choose(&menu_window(1), runtime), Intent::Attack);
    policy.battle_begins(true);
    assert_eq!(
        policy.choose(&menu_window(1), runtime),
        opening,
        "the next scripted battle opens again"
    );

    let mut random = OpeningItemPolicy::new("test_opening", item.id);
    random.battle_begins(false);
    assert_eq!(random.choose(&menu_window(1), runtime), Intent::Attack);
    let mut missing = OpeningItemPolicy::new("test_opening", 0xFE);
    missing.battle_begins(true);
    assert_eq!(missing.choose(&menu_window(1), runtime), Intent::Attack);
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_psiv-campaign"))
}

/// The exit statuses: 2 halted with a report, 1 a setup error, 0 completed.
#[test]
fn run_exits_two_when_halted_one_on_setup_errors_and_zero_when_done() {
    if pack().is_none() {
        return;
    }
    let broken = mutate(
        &main_text(),
        "{\"do\": \"talk\", \"npc\": 0, \"note\": \"principal: face Up, Action\"}",
        "{\"do\": \"talk\", \"npc\": 77, \"note\": \"principal: face Up, Action\"}",
    );
    let dir = scratch("cli");
    std::fs::create_dir_all(&dir).unwrap();
    let route = dir.join("broken.json");
    std::fs::write(&route, broken).unwrap();
    let report = dir.join("report.json");
    let tape = dir.join("broken.tape");
    let halted = bin()
        .arg("run")
        .arg(&route)
        .args(["--until-chapter", "academy", "--save-dir"])
        .arg(dir.join("saves"))
        .arg("--report")
        .arg(&report)
        .arg("--tape")
        .arg(&tape)
        .output()
        .unwrap();
    assert_eq!(halted.status.code(), Some(2));
    let text = std::fs::read_to_string(&report).expect("the report is written");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["objective_index"], 4);
    assert_eq!(json["halt"]["kind"], "wrong_object");
    let said = String::from_utf8_lossy(&halted.stdout);
    assert!(
        said.contains("HALTED in chapter academy at objective 4"),
        "{said}"
    );
    // The tape it wrote replays through the binary to the digest it printed.
    let digest = said
        .lines()
        .find_map(|l| l.split("digest ").nth(1))
        .expect("the run prints a digest")
        .trim()
        .to_owned();
    let replayed = bin().arg("replay").arg(&tape).output().unwrap();
    assert_eq!(replayed.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&replayed.stdout).contains(&digest),
        "{}",
        String::from_utf8_lossy(&replayed.stdout)
    );

    let usage = bin().arg("run").output().unwrap();
    assert_eq!(usage.status.code(), Some(1));
    let missing = bin().args(["run", "no-such-route.json"]).output().unwrap();
    assert_eq!(missing.status.code(), Some(1));
    let chapter = bin()
        .arg("run")
        .arg(MAIN_ROUTE)
        .args(["--until-chapter", "no-such-chapter"])
        .output()
        .unwrap();
    assert_eq!(chapter.status.code(), Some(1));
    assert_eq!(bin().arg("replay").output().unwrap().status.code(), Some(1));
    let policy = mutate(
        &main_text(),
        "\"random_battle_policy\": \"attack_all\"",
        "\"random_battle_policy\": \"berserk\"",
    );
    let policy_route = dir.join("policy.json");
    std::fs::write(&policy_route, policy).unwrap();
    let unknown = bin().arg("run").arg(&policy_route).output().unwrap();
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("berserk"));
}

/// The boundary the runner is built against: it plays through
/// `Session::frame` and reads views. It never takes the session's mutable
/// runtime, and the only runtime mutators it names are the two the title would
/// call to build a game (in `start.rs`, before the runtime becomes a session).
#[test]
fn the_runner_reaches_no_runtime_mutator() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        // The needles are assembled so this file does not match itself.
        let runtime_mut = ["runtime", "_mut"].concat();
        assert!(
            !text.contains(&runtime_mut),
            "{name} reaches the session's mutable runtime"
        );
        if name != "start.rs" {
            for needle in ["enable_battles(", "start_event(", "from_save(", "new_game("] {
                assert!(
                    !text.contains(needle),
                    "{name} calls {needle}: construction belongs to start.rs"
                );
            }
        }
    }
}

/// Chapter one's save, read back by the `inspect` subcommand: position, party
/// with equipment and flags come out of the runtime's own views.
#[test]
fn inspect_reads_a_chapter_save_and_refuses_a_missing_one() {
    let Some((_, result)) = academy() else {
        return;
    };
    let bin = env!("CARGO_BIN_EXE_psiv-campaign");
    let out = Command::new(bin)
        .arg("inspect")
        .arg(&result.chapters[0].save)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("map 0x0 "), "{text}");
    for line in ["party 0 Alys L7", "wears", "inventory", "event flags"] {
        assert!(text.contains(line), "{line}: {text}");
    }
    let missing = Command::new(bin)
        .args(["inspect", "no-such-file.sram"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
}

/// A scene that asks a question on arrival (Chaz's house offers a rest) ends the
/// walk with the prompt open, and the next objective answers it. Negative
/// control: with the `answer` replaced the next objective halts on the open
/// prompt instead of walking through it.
#[test]
#[ignore = "plays the route to Aiedo: cargo test --release -p psiv-campaign --test runner -- --ignored"]
fn an_arrival_prompt_is_the_next_objectives_to_answer() {
    if pack().is_none() {
        return;
    }
    let text = main_text();
    let ok = run(&config(
        &text,
        "chaz-prompt",
        Some("aiedo-chaz-house"),
        None,
    ))
    .unwrap();
    assert!(ok.completed, "report: {:#?}", ok.report);
    let broken = mutate(
        &text,
        "{\"do\": \"answer\", \"yes\": true, \"note\": \"yes: RecoverStats",
        "{\"do\": \"expect\", \"map\": 94, \"note\": \"yes: RecoverStats",
    );
    let halted = run(&config(
        &broken,
        "chaz-no-answer",
        Some("aiedo-chaz-house"),
        None,
    ))
    .unwrap();
    assert!(!halted.completed);
    let report = halted.report.expect("a halt writes a report");
    assert_eq!(report["chapter"], "aiedo-chaz-house");
    assert_eq!(report["halt"]["kind"], "unexpected_state");
    assert!(
        report["halt"]["detail"]
            .as_str()
            .unwrap()
            .contains("yes/no prompt"),
        "{report}"
    );
}

/// The whole route from New Game to Zio's defeat, the Mota Spaceport, Zelan,
/// the sabotage and the crash landing to Raja Temple on Dezolis, pads only. The
/// Zio chapter makes an ordinary SAVE; a new Session reads it, and replaying the
/// tape in another Session reaches the same digest.
#[test]
#[ignore = "plays the whole route: cargo test --release -p psiv-campaign --test runner -- --ignored"]
fn the_whole_route_defeats_zio_saves_and_replays() {
    if pack().is_none() {
        return;
    }
    let config = config(&main_text(), "whole-route", None, None);
    let result = run(&config).expect("the route sets up");
    let done: Vec<&str> = result.chapters.iter().map(|c| c.id.as_str()).collect();
    assert!(result.completed, "route halted: {:#?}", result.report);
    assert_eq!(done.last(), Some(&"dezolis-tyler-grave"));
    assert_eq!(done[done.len() - 8], "nurvus-zio");
    assert_eq!(result.chapters.len(), config.route.chapters.len());
    let save_of = |id: &str| {
        let chapter = result
            .chapters
            .iter()
            .find(|c| c.id == id)
            .expect("the chapter ran");
        assert!(
            chapter.save.is_file(),
            "the runner wrote its read-only snapshot of {id}"
        );
        chapter.save.clone()
    };
    // The spaceport chapter ends in Zelan: the ship's destination menu opened on
    // the Mota Spaceport's boarding row, `World_Index` 3 was picked with the
    // pad, and the flight landed on loc_64B5A's Zelan row (RUNNER_LOG M23).
    let (at_zelan, _) = open_session(
        pack_dir().as_path(),
        &StartPoint::Save(save_of("mota-spaceport")),
    )
    .unwrap();
    assert_eq!(at_zelan.runtime().map_id().0, 0x18D);
    assert_eq!(at_zelan.runtime().world_index(), 3);
    let cell = psiv_campaign::driver::standing_cell(at_zelan.runtime());
    assert_eq!((cell.x, cell.y), (31, 46));
    // The route ends at first control on Dezolis: Raja Temple's `$14C`, the
    // exit's foot, with the five-member party, the crash landing's flags and
    // `World_Index` 1 (`ps4.asm:155847`). Beyond it the exit's trigger event
    // `$43` plays in the next chapter (RUNNER_LOG_DEZOLIS H28).
    let (on_dezolis, _) = open_session(
        pack_dir().as_path(),
        &StartPoint::Save(save_of("dezolis-first-control")),
    )
    .unwrap();
    assert_eq!(on_dezolis.runtime().map_id().0, 0x14C);
    assert_eq!(on_dezolis.runtime().world_index(), 1);
    let cell = psiv_campaign::driver::standing_cell(on_dezolis.runtime());
    assert_eq!((cell.x, cell.y), (95, 37));
    let dezolis_game = on_dezolis.runtime().game();
    assert_eq!(dezolis_game.party_members().len(), 5);
    for flag in [0x70, 0x71, 0x72, 0x85, 0x88] {
        assert!(
            dezolis_game.is_set(Flag::event(flag)),
            "event flag {flag:#x}"
        );
    }
    assert!(
        !dezolis_game.is_set(Flag::event(0x80)),
        "Snowstorm is still clear: the exit's event has not run"
    );
    // The next chapter walks out through the exit: `Event_OutsideRajaTemple`
    // (`$43`) played and set Snowstorm, and the party stands where it arrived.
    let (outside, _) = open_session(
        pack_dir().as_path(),
        &StartPoint::Save(save_of("dezolis-outside-raja-temple")),
    )
    .unwrap();
    assert_eq!(outside.runtime().map_id().0, 0x001);
    assert!(outside.runtime().game().is_set(Flag::event(0x80)));
    // Gyuna's space-ship answer set `$81`; the grave's flag is still clear.
    let (pub_, _) = open_session(
        pack_dir().as_path(),
        &StartPoint::Save(save_of("dezolis-gyuna")),
    )
    .unwrap();
    assert_eq!(pub_.runtime().map_id().0, 0x14A);
    assert!(pub_.runtime().game().is_set(Flag::event(0x81)));
    assert!(!pub_.runtime().game().is_set(Flag::event(0x84)));
    // Tyler's grave opened (`$84`) and the stairs led to the Hangar (`$15F`).
    let (hangar, _) = open_session(
        pack_dir().as_path(),
        &StartPoint::Save(save_of("dezolis-tyler-grave")),
    )
    .unwrap();
    assert_eq!(hangar.runtime().map_id().0, 0x15F);
    for flag in [0x81, 0x84] {
        assert!(
            hangar.runtime().game().is_set(Flag::event(flag)),
            "{flag:#x}"
        );
    }
    let pad_save = config.save_dir.join("route/slot_1.sram");
    assert!(
        pad_save.is_file(),
        "the last chapter's pad SAVE wrote the separate route slot"
    );
    let (loaded, _) = open_session(pack_dir().as_path(), &StartPoint::Save(pad_save)).unwrap();
    let game = loaded.runtime().game();
    assert_eq!(loaded.runtime().map_id().0, 0);
    assert_eq!(game.party_members(), [CharId(0), CharId(5), CharId(3)]);
    assert!(game.roster().get(CharId(0)).unwrap().curr_hp > 0);
    for flag in [0x65, 0x68, 0x66, 0x61] {
        assert!(game.is_set(Flag::event(flag)), "event flag {flag:#x}");
    }
    let replayed = replay(pack_dir().as_path(), &result.tape, None).unwrap();
    assert_eq!(replayed.digest, result.digest);
    assert!(
        replayed.faults.is_empty(),
        "replay faults: {:#?}",
        replayed.faults
    );
}
