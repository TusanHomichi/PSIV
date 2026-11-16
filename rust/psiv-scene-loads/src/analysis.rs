//! Trace ownership and validation. The Python capture driver supplies the
//! host's CSV records as JSON strings; all interpretation happens here.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

// US NTSC, non-interlaced: the pinned core's system_frame_gen rebases the
// instruction counter by this many master cycles per retro_run.
pub const FRAME_CYCLES: u64 = 3420 * 262;
// Endpoints are BEFORE RTS, except load_art's caller continuation.
pub const ROUTINES: [(&str, u32, u32); 4] = [
    ("panel", 0x06AF9C, 0x06B008),
    ("nemesis", 0x041740, 0x041788),
    ("enigma", 0x041A90, 0x041B54),
    ("load_art", 0x0702DE, 0x0702E4),
];
pub const POST_ART_WATCHES: [u32; 3] = [0x070430, 0x070436, 0x07043C];
pub type Row = BTreeMap<String, String>;

fn number(row: &Row, key: &str, radix: u32) -> Result<u64, String> {
    u64::from_str_radix(row.get(key).ok_or_else(|| format!("missing {key}"))?, radix)
        .map_err(|e| format!("invalid {key}: {e}"))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub frame: u64,
    pub pc: u32,
    pub cycles: u64,
}

impl Point {
    fn read(row: &Row) -> Result<Self, String> {
        let value = Self {
            frame: number(row, "frame", 10)?,
            pc: u32::try_from(number(row, "pc", 16)?).map_err(|e| e.to_string())?,
            cycles: number(row, "cycles", 10)?,
        };
        value.absolute()?;
        Ok(value)
    }

    pub fn absolute(&self) -> Result<u64, String> {
        self.frame
            .checked_sub(1)
            .and_then(|f| f.checked_mul(FRAME_CYCLES))
            .and_then(|f| f.checked_add(self.cycles))
            .ok_or_else(|| "invalid/overflowing trace timestamp".into())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Call {
    pub routine: String,
    pub start: Point,
    pub end: Point,
    pub components: Vec<Call>,
    pub master_cycles: u64,
    pub held_frames: Vec<u64>,
    pub lag_frames: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    pub following_held_frames: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub following_count_resumed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outside_decompressors_master_cycles: Option<u64>,
}

impl Call {
    fn new(routine: &str, start: Point) -> Self {
        Self {
            routine: routine.into(),
            end: start.clone(),
            start,
            components: vec![],
            master_cycles: 0,
            held_frames: vec![],
            lag_frames: 0,
            query: None,
            input: None,
            following_held_frames: vec![],
            following_count_resumed: None,
            outside_decompressors_master_cycles: None,
        }
    }
}

pub fn spans(trace: &[Point]) -> Result<Vec<Call>, String> {
    let mut stack: Vec<Call> = vec![];
    let mut loads = vec![];
    let mut previous = 0;
    for point in trace {
        let stamp = point.absolute()?;
        if stamp < previous {
            return Err("instruction trace is not in execution order".into());
        }
        previous = stamp;
        if let Some(&(name, _, _)) = ROUTINES.iter().find(|(_, start, _)| *start == point.pc) {
            if matches!(name, "panel" | "load_art") || !stack.is_empty() {
                stack.push(Call::new(name, point.clone()));
            }
        } else if let Some(&(name, _, _)) = ROUTINES.iter().find(|(_, _, end)| *end == point.pc) {
            let Some(mut call) = stack.pop() else {
                if matches!(name, "panel" | "load_art") {
                    return Err(format!("{name} returned without an entry"));
                }
                continue;
            };
            if call.routine != name {
                return Err(format!("unmatched {name} return inside {}", call.routine));
            }
            call.master_cycles = stamp - call.start.absolute()?;
            call.end = point.clone();
            if let Some(parent) = stack.last_mut() {
                parent.components.push(call);
            } else {
                loads.push(call);
            }
        }
    }
    if !stack.is_empty() {
        return Err("trace ends inside a measured load".into());
    }
    Ok(loads)
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub count: u16,
    pub seed: u32,
    pub panel: u16,
}
pub type Samples = BTreeMap<u64, Sample>;

fn sample(frames: &Samples, frame: u64) -> Result<&Sample, String> {
    frames
        .get(&frame)
        .ok_or_else(|| format!("missing RAM sample {frame}"))
}

fn held(before: &Sample, after: &Sample, frame: u64) -> Result<bool, String> {
    let same = before.count == after.count;
    if same && before.seed != after.seed {
        return Err(format!("seed changed on held frame {frame}"));
    }
    Ok(same)
}

fn annotate_one(
    call: &mut Call,
    frames: &Samples,
    returns: &BTreeMap<u64, usize>,
) -> Result<(), String> {
    let start = call.start.frame;
    let end = call.end.frame;
    call.held_frames.clear();
    for f in start + 1..=end {
        if held(sample(frames, f - 1)?, sample(frames, f)?, f)? {
            call.held_frames.push(f);
        }
    }
    call.lag_frames = call.held_frames.len();
    if call.routine == "panel" {
        if returns.get(&end) != Some(&1) {
            return Err("RAM id is ambiguous: multiple panels return in one frame".into());
        }
        let panel = sample(frames, end)?.panel;
        for f in start + 1..=end {
            if sample(frames, f)?.panel != panel {
                return Err("panel id changed during measured call".into());
            }
        }
        call.query = Some(format!("panel:{panel:x}"));
    } else if call.routine == "load_art" {
        call.query = Some("art:1d712c".into());
    }
    for child in &mut call.components {
        annotate_one(child, frames, returns)?;
    }
    Ok(())
}

pub fn annotate(loads: &mut [Call], frames: &Samples) -> Result<(), String> {
    let mut returns = BTreeMap::new();
    for c in loads.iter().filter(|c| c.routine == "panel") {
        *returns.entry(c.end.frame).or_default() += 1;
    }
    for c in loads {
        annotate_one(c, frames, &returns)?;
        c.following_held_frames.clear();
        let end = sample(frames, c.end.frame)?;
        let mut f = c.end.frame + 1;
        while let Some(after) = frames.get(&f) {
            if !held(end, after, f)? {
                break;
            }
            c.following_held_frames.push(f);
            f += 1;
        }
        c.following_count_resumed = Some(frames.contains_key(&f));
    }
    Ok(())
}

pub fn coverage(scene: u16, calls: &[Call]) -> Result<(), String> {
    let required: &[&str] = match scene {
        46 => &["panel:10d", "art:1d712c"],
        43 => &["panel:18b", "panel:18f", "panel:190"],
        38 => &["panel:143", "panel:144"],
        _ => return Err(format!("unsupported fixture {scene}")),
    };
    for query in required {
        if !calls.iter().any(|c| c.query.as_deref() == Some(query)) {
            return Err(format!("tape {scene} missed required load {query}"));
        }
    }
    Ok(())
}

pub fn attach_work(call: &mut Call, work: Value) -> Result<(), String> {
    let mut inputs = vec![];
    let mut names = vec![];
    let art = if call.routine == "panel" {
        work.get("art").ok_or("missing art")?
    } else {
        &work
    };
    if !art.is_null() {
        inputs.push(art.clone());
        names.push("nemesis");
    }
    if call.routine == "panel" {
        for mapping in work
            .get("mappings")
            .and_then(Value::as_array)
            .ok_or("missing mappings")?
        {
            inputs.push(mapping.clone());
            names.push("enigma");
        }
    }
    if names
        != call
            .components
            .iter()
            .map(|c| c.routine.as_str())
            .collect::<Vec<_>>()
    {
        return Err("trace and decoded record select different calls".into());
    }
    for (child, source) in call.components.iter_mut().zip(inputs) {
        child.input = Some(source);
    }
    call.outside_decompressors_master_cycles = Some(
        call.master_cycles
            .checked_sub(call.components.iter().map(|c| c.master_cycles).sum())
            .ok_or("components exceed parent duration")?,
    );
    call.input = Some(work);
    Ok(())
}

#[derive(Deserialize)]
pub struct Capture {
    pub scene: u16,
    pub trace: Vec<Row>,
    pub frames: Vec<Row>,
}

pub fn analyze(capture: Capture) -> Result<Vec<Call>, String> {
    let trace = capture
        .trace
        .iter()
        .map(Point::read)
        .collect::<Result<Vec<_>, _>>()?;
    let mut frames = BTreeMap::new();
    for row in &capture.frames {
        let frame = number(row, "frame", 10)?;
        let sample = Sample {
            count: u16::try_from(number(row, "main_frame_count", 10)?)
                .map_err(|e| e.to_string())?,
            seed: u32::try_from(number(row, "rng_seed", 16)?).map_err(|e| e.to_string())?,
            panel: u16::try_from(number(row, "panel_index", 16)?).map_err(|e| e.to_string())?,
        };
        if frames.insert(frame, sample).is_some() {
            return Err(format!("duplicate RAM sample {frame}"));
        }
    }
    let mut calls = spans(&trace)?;
    annotate(&mut calls, &frames)?;
    coverage(capture.scene, &calls)?;
    Ok(calls)
}

pub fn config() -> Value {
    let mut watches: Vec<u32> = ROUTINES
        .iter()
        .flat_map(|(_, a, b)| [*a, *b])
        .chain(POST_ART_WATCHES)
        .collect();
    watches.sort_unstable();
    watches.dedup();
    json!({"watches": watches, "frame_master_cycles": FRAME_CYCLES})
}

#[cfg(test)]
mod tests;
