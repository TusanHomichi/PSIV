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

use common::{MAIN_ROUTE, PACK, pack};

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
        pack: PathBuf::from(PACK),
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
    let replayed = replay(PACK.as_ref(), &tape, None).expect("replay sets up");
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
    let replayed = replay(PACK.as_ref(), &result.tape, Some(&save)).expect("replay from the save");
    assert_eq!(replayed.digest, result.digest);
    // The tape names its save: the wrong file is refused.
    let other = result.chapters[0].save.clone();
    assert!(replay(PACK.as_ref(), &result.tape, Some(&other)).is_err());
    assert!(replay(PACK.as_ref(), &result.tape, None).is_err());
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
    let replayed = replay(PACK.as_ref(), &result.tape, None).unwrap();
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
    let (session, _) = open_session(PACK.as_ref(), &StartPoint::NewGame).unwrap();
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
        pack: PathBuf::from(PACK),
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

/// The whole route from New Game, pads only. Run it in release.
///
/// The route runs on the engine as it stands, and two port defects stop it:
/// `Cutscene_AlysWounded` faults in `zio-fort-demi` (RUNNER_LOG.md H17) and
/// `Event_ZioFortBarrier` is not transcribed (H18, `zio-fort-barrier`). The
/// test pins what is true today and stays honest when they are fixed: every
/// chapter before the first defect passes, the run either completes (last
/// chapter `zio-fort-barrier`) or halts in one of the two chapters the log
/// names, and its tape replays to the digest the run printed.
#[test]
#[ignore = "plays the whole route: cargo test --release -p psiv-campaign --test runner -- --ignored"]
fn the_whole_route_plays_to_its_documented_defect_and_replays() {
    if pack().is_none() {
        return;
    }
    let config = config(&main_text(), "whole-route", None, None);
    let result = run(&config).expect("the route sets up");
    let done: Vec<&str> = result.chapters.iter().map(|c| c.id.as_str()).collect();
    assert!(
        done.contains(&"zio-fort-juza"),
        "Juza's battle is won: {done:?}"
    );
    if result.completed {
        assert_eq!(done.last(), Some(&"zio-fort-barrier"));
        assert_eq!(result.chapters.len(), config.route.chapters.len());
    } else {
        let report = result.report.as_ref().expect("a halted run has a report");
        let chapter = report["chapter"].as_str().unwrap_or_default();
        assert!(
            ["zio-fort-demi", "zio-fort-barrier"].contains(&chapter),
            "an undocumented halt: {report:#}"
        );
    }
    let replayed = replay(PACK.as_ref(), &result.tape, None).unwrap();
    assert_eq!(replayed.digest, result.digest);
}
