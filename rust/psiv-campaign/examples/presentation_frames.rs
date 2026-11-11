//! Discover #93's named captures: TAPE REPORT PACK OUT (a fresh manifest).
//! Reads ordinary Session frames; saves go only to the fresh OUT.saves directory.
//! Zema's US entrance steps slot 7 down 64 half-pixels (docs/scenes/SCENE_DIALOGUE.md),
//! and its rescue has 50 distinct ready pages before event flag $37 restores it.
//! BioPlant uses eight Pal_VariableFadeToRed/FromRed stages, four frames apart
//! (US $05B2C0/$05B30C). These are capture expectations, not gameplay rules.

use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use psiv_campaign::split::chapters_of;
use psiv_campaign::start::{StartPoint, open_session};
use psiv_campaign::tape::{Tape, TapeStart};
use psiv_core::palette::{RED_FADE_STAGES, dbf_frames};
use psiv_core::{FlagBank, PresentationOp, SceneOp};
use psiv_runtime::{Pad, RuntimeEvent, SaveStore};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Serialize)]
struct Request {
    frame: u64,
    label: String,
    capture: bool,
}

fn add(requests: &mut Vec<Request>, frame: u64, label: impl Into<String>) {
    requests.push(Request {
        frame,
        label: label.into(),
        capture: true,
    });
}

fn run() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [tape_path, report_path, pack, out] = args.as_slice() else {
        return Err("usage: presentation_frames TAPE REPORT PACK OUT".into());
    };
    let bytes = fs::read(tape_path)?;
    let tape = Tape::parse(std::str::from_utf8(&bytes)?)?;
    if tape.start != TapeStart::NewGame {
        return Err("presentation discovery requires a New Game tape".into());
    }
    let report: Value = serde_json::from_slice(&fs::read(report_path)?)?;
    let chapters = chapters_of(&report)?;
    let mut total = 0_u64;
    let mut rescue = None;
    for chapter in &chapters {
        let start = total;
        total = total
            .checked_add(chapter.frames)
            .ok_or("report frame overflow")?;
        if chapter.id == "zema-rescue" && rescue.replace((start, total)).is_some() {
            return Err("report repeats zema-rescue".into());
        }
    }
    if total != u64::try_from(tape.pads.len())? || report["frames"].as_u64() != Some(total) {
        return Err("completed report frame count differs from the tape".into());
    }
    let (rescue_start, rescue_end) = rescue.ok_or("report lacks zema-rescue")?;
    let mut save_name = out.as_os_str().to_owned();
    save_name.push(".saves");
    let saves = Path::new(&save_name);
    if out.exists() || saves.exists() {
        return Err("OUT and OUT.saves must both be fresh paths".into());
    }
    let (mut session, _) = open_session(pack, &StartPoint::NewGame)?;
    fs::create_dir(saves)?;
    session.set_save_store(SaveStore::new(saves));
    let mut requests = Vec::new();
    let mut pages = HashSet::new();
    let mut entrance = None;
    let mut restored = false;
    let mut red = None;
    let mut normal = None;
    for (index, pad) in tape.pads.iter().enumerate() {
        let frame = u64::try_from(index)? + 1;
        let produced = session.frame(Pad::from_bits(*pad));
        let runtime = session.runtime();
        if runtime.game_over() || produced.camp_save_error.is_some() {
            return Err(format!("replay failed at frame {frame}").into());
        }
        let in_rescue = runtime.map_id().0 == 0x24 && frame > rescue_start && frame <= rescue_end;
        if in_rescue {
            if let Some(page) = runtime.dialogue_view().filter(|view| view.dismissable)
                && pages.insert(serde_json::to_string(&(page.tree, &page.lines))?)
            {
                add(
                    &mut requests,
                    frame,
                    format!("zema-page-{:02}", pages.len()),
                );
            }
            if !restored
                && !runtime.scene_active()
                && runtime
                    .game()
                    .set_flags()
                    .any(|flag| flag.bank == FlagBank::Event && flag.id == 0x37)
            {
                add(&mut requests, frame, "zema-restored");
                restored = true;
            }
        }
        for event in produced.events.iter().chain(&produced.menu_events) {
            let RuntimeEvent::ScenePresentation { op, .. } = event else {
                continue;
            };
            match op {
                SceneOp::StepFieldObject {
                    slot: 7,
                    step_x: 0,
                    step_y: 0x8000,
                    frames: 64,
                } if in_rescue && entrance.is_none() => {
                    entrance = Some(frame);
                    // The shell advances existing objects before consuming this op:
                    // its first half-pixel is drawn on the following frame.
                    add(&mut requests, frame + 32, "igglanova-halfway");
                    add(&mut requests, frame + 64, "igglanova-arrived");
                }
                SceneOp::Presentation {
                    op: PresentationOp::FadeToRed { lines },
                } if runtime.map_id().0 == 0xa3 && red.is_none() => {
                    if dbf_frames(*lines) != 4 {
                        return Err("alarm cadence changed".into());
                    }
                    red = Some(frame);
                }
                SceneOp::Presentation {
                    op: PresentationOp::FadeFromRed { lines },
                } if runtime.map_id().0 == 0xa3 && red.is_some() && normal.is_none() => {
                    if dbf_frames(*lines) != 4 {
                        return Err("alarm cadence changed".into());
                    }
                    normal = Some(frame);
                }
                _ => {}
            }
        }
    }
    if pages.len() != 50 || entrance.is_none() || !restored {
        return Err(
            "incomplete Zema observations (50 pages, entrance and restoration required)".into(),
        );
    }
    let red = red.ok_or("missing BioPlant FadeToRed")?;
    let normal = normal.ok_or("missing BioPlant FadeFromRed")?;
    if normal != red + u64::from(RED_FADE_STAGES) * 4 {
        return Err("BioPlant fade stages are not contiguous at four-frame cadence".into());
    }
    for (direction, start) in [("out", red), ("in", normal)] {
        for stage in 0..RED_FADE_STAGES {
            add(
                &mut requests,
                start + u64::from(stage) * 4,
                format!("alarm-{direction}-{}", stage + 1),
            );
        }
    }
    requests.sort_by_key(|request| request.frame);
    if requests.len() != 69 || requests.iter().any(|request| request.frame > total) {
        return Err("capture requests extend beyond the complete tape".into());
    }
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(&bytes) {
        write!(digest, "{byte:02x}")?;
    }
    let manifest = serde_json::json!({"tape_sha256": digest, "requests": requests});
    let mut output = OpenOptions::new().write(true).create_new(true).open(out)?;
    serde_json::to_writer_pretty(&mut output, &manifest)?;
    writeln!(output)?;
    println!("69 captures discovered from {total} ordinary Session frames");
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("presentation_frames: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
