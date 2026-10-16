//! The run loop: chapters, objectives, saves, and the halt report.
//!
//! [`run`] opens a session (power-on, or the save a chapter ended on), plays
//! each objective through its controller, and writes a chapter save when a
//! chapter's closing assertions hold. On the first [`Halt`] it stops and
//! returns a report; it never skips an objective and never edits state.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::digest::Digest;
use crate::driver::{BattleRecord, Driver, MemberAtEnd};
use crate::exec::{Memory, budget_for, execute};
use crate::halt::Halt;
use crate::policy;
use crate::route::{Chapter, Route};
use crate::start::{SetupError, StartPoint, open_session};
use crate::tape::Tape;

/// Everything a run needs.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// The route to play.
    pub route: Route,
    /// The runtime pack directory.
    pub pack: PathBuf,
    /// Start at this chapter, from the save the chapter before it ended on.
    pub from_chapter: Option<String>,
    /// Stop after this chapter completes.
    pub until_chapter: Option<String>,
    /// Where chapter saves and the route's own `save` objectives are written.
    pub save_dir: PathBuf,
    /// Print one progress line per objective to stderr.
    pub verbose: bool,
}

/// One finished chapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterSummary {
    /// The chapter id.
    pub id: String,
    /// Frames the chapter took.
    pub frames: u64,
    /// Battles fought in it.
    pub battles: usize,
    /// The chapter-boundary save.
    pub save: PathBuf,
    /// The party as the chapter ended: `Name Lv hp/max`, leader first.
    pub party: String,
    /// The scripted (event) battles fought in the chapter, in order, each with
    /// the party as it ended.
    pub event_battles: Vec<BattleRecord>,
}

/// What a run produced.
#[derive(Debug)]
pub struct RunResult {
    /// Whether every requested chapter completed.
    pub completed: bool,
    /// The pads of every frame played.
    pub tape: Tape,
    /// The final state's digest.
    pub digest: Digest,
    /// The halt report, when the run halted.
    pub report: Option<Value>,
    /// The chapters that completed.
    pub chapters: Vec<ChapterSummary>,
    /// Every battle fought.
    pub battles: Vec<BattleRecord>,
}

/// The directory a chapter's boundary save lives in: `NN-id` under the run's
/// save directory, the chapter's position in the route first so a listing
/// sorts in story order.
#[must_use]
pub fn chapter_save_dir(save_dir: &Path, index: usize, chapter: &Chapter) -> PathBuf {
    save_dir.join(format!("{index:02}-{}", chapter.id))
}

/// The slot file inside a chapter-save directory.
#[must_use]
pub fn chapter_save_file(save_dir: &Path, index: usize, chapter: &Chapter) -> PathBuf {
    chapter_save_dir(save_dir, index, chapter).join("slot_1.sram")
}

/// Plays `config.route`.
///
/// # Errors
///
/// [`SetupError`] for an unknown chapter id, an unknown policy name, a missing
/// predecessor save, or a pack that will not load. A halt is not an error: it
/// is a [`RunResult`] with `completed == false` and a report.
pub fn run(config: &RunConfig) -> Result<RunResult, SetupError> {
    let route = &config.route;
    let index_of = |id: &str| {
        route
            .chapters
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| SetupError(format!("the route has no chapter {id:?}")))
    };
    let first = config.from_chapter.as_deref().map(index_of).transpose()?;
    let last = config.until_chapter.as_deref().map(index_of).transpose()?;
    let first = first.unwrap_or(0);
    let last = last.unwrap_or(route.chapters.len().saturating_sub(1));
    if first > last {
        return Err(SetupError(
            "--from-chapter comes after --until-chapter".into(),
        ));
    }
    for chapter in &route.chapters {
        if !policy::is_known(&chapter.random_battle_policy) {
            return Err(SetupError(format!(
                "chapter {:?} names the battle policy {:?}, which the runner does not define",
                chapter.id, chapter.random_battle_policy
            )));
        }
    }
    if route.start.is_some() {
        return Err(SetupError(
            "the route has a `start` position; the runner plays from power-on or a chapter save only"
                .into(),
        ));
    }
    let start = if first == 0 {
        StartPoint::NewGame
    } else {
        let save = chapter_save_file(&config.save_dir, first - 1, &route.chapters[first - 1]);
        if !save.is_file() {
            return Err(SetupError(format!(
                "chapter {:?} starts from the save {} and it does not exist; run the earlier \
                 chapters first",
                route.chapters[first].id,
                save.display()
            )));
        }
        StartPoint::Save(save)
    };
    let (session, tape_start) = open_session(&config.pack, &start)?;
    let mut driver = Driver::new(session, Some(config.save_dir.clone()));
    let mut memory = Memory::default();
    let mut chapters = Vec::new();
    let mut report = None;
    for index in first..=last {
        let chapter = &route.chapters[index];
        match play_chapter(&mut driver, &mut memory, config, index, chapter) {
            Ok(summary) => chapters.push(summary),
            Err(failure) => {
                report = Some(report_of(&driver, chapter, index, &failure));
                break;
            }
        }
    }
    let digest = Digest::of(driver.runtime(), driver.frames());
    Ok(RunResult {
        completed: report.is_none(),
        tape: Tape {
            start: tape_start,
            pads: driver.pads().to_vec(),
        },
        digest,
        report,
        chapters,
        battles: driver.battles().to_vec(),
    })
}

/// Where a chapter stopped.
struct Failure {
    /// Objective index, or `objectives.len() + n` for closing assertion `n`.
    index: usize,
    kind: String,
    objective: Value,
    halt: Halt,
}

fn play_chapter(
    driver: &mut Driver,
    memory: &mut Memory,
    config: &RunConfig,
    index: usize,
    chapter: &Chapter,
) -> Result<ChapterSummary, Failure> {
    let started = driver.frames();
    let battles_before = driver.battles().len();
    let policy = policy::by_name(&chapter.random_battle_policy)
        .expect("policy names were checked before the run began");
    driver.set_policy(policy);
    for (at, step) in chapter.objectives.iter().enumerate() {
        let objective = serde_json::to_value(step).unwrap_or(Value::Null);
        let kind = objective["do"].as_str().unwrap_or("?").to_owned();
        if config.verbose {
            eprintln!(
                "[{}] #{at} {kind} (frame {}, map {:#x} cell {:?})",
                chapter.id,
                driver.frames(),
                driver.map(),
                (driver.cell().x, driver.cell().y)
            );
        }
        driver.set_budget(budget_for(&step.objective));
        driver.clear_routed();
        driver.note(format!("objective #{at} {kind} begins"));
        execute(driver, memory, &step.objective).map_err(|halt| Failure {
            index: at,
            kind: kind.clone(),
            objective: objective.clone(),
            halt,
        })?;
    }
    for (n, closing) in chapter.closing.iter().enumerate() {
        driver.set_budget(60_000);
        driver.expect(closing).map_err(|halt| Failure {
            index: chapter.objectives.len() + n,
            kind: "closing".to_owned(),
            objective: serde_json::to_value(closing).unwrap_or(Value::Null),
            halt,
        })?;
    }
    let save = save_chapter(driver, config, index, chapter).map_err(|halt| Failure {
        index: chapter.objectives.len() + chapter.closing.len(),
        kind: "chapter_save".to_owned(),
        objective: Value::Null,
        halt,
    })?;
    Ok(ChapterSummary {
        id: chapter.id.clone(),
        frames: driver.frames() - started,
        battles: driver.battles().len() - battles_before,
        save,
        party: party_line(driver),
        event_battles: driver.battles()[battles_before..]
            .iter()
            .filter(|b| b.kind == "event")
            .cloned()
            .collect(),
    })
}

fn party_line(driver: &Driver) -> String {
    driver
        .runtime()
        .camp_state()
        .party
        .iter()
        .map(|m| format!("{} L{} {}/{}", m.name, m.level, m.current_hp, m.max_hp))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Writes the boundary save the next chapter can start from.
fn save_chapter(
    driver: &mut Driver,
    config: &RunConfig,
    index: usize,
    chapter: &Chapter,
) -> Result<PathBuf, Halt> {
    driver.set_budget(60_000);
    driver.settle(false)?;
    let dir = chapter_save_dir(&config.save_dir, index, chapter);
    driver
        .runtime()
        .save_slot(&dir, 0)
        .map_err(|e| Halt::new(crate::halt::HaltKind::SaveFailed, e.to_string()))
}

fn report_of(driver: &Driver, chapter: &Chapter, index: usize, failure: &Failure) -> Value {
    let mut report = serde_json::Map::new();
    report.insert("result".into(), json!("halted"));
    report.insert("chapter".into(), json!(chapter.id));
    report.insert("chapter_index".into(), json!(index));
    report.insert("objective_index".into(), json!(failure.index));
    report.insert("objective_kind".into(), json!(failure.kind));
    report.insert("objective".into(), failure.objective.clone());
    report.insert(
        "halt".into(),
        json!({"kind": failure.halt.kind, "detail": failure.halt.detail}),
    );
    if let Value::Object(state) = driver.snapshot() {
        report.extend(state);
    }
    report.insert(
        "battles".into(),
        json!(
            driver
                .battles()
                .iter()
                .map(|b| {
                    json!({
                        "kind": b.kind, "id": b.id, "start_frame": b.start_frame,
                        "end_frame": b.end_frame, "meseta": b.meseta, "experience": b.experience,
                        "party_at_end": b.party_at_end.iter().map(MemberAtEnd::to_json).collect::<Vec<_>>(),
                    })
                })
                .collect::<Vec<_>>()
        ),
    );
    Value::Object(report)
}
