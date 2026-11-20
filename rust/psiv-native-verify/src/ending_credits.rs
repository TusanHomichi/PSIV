//! Ending credits observation and comparison contracts (`#641`, `#642`, `#635`).
//! The launcher only runs the oracle, Godot and `psiv-compare`; this module
//! owns oracle identities, observation ordering/state and acceptance.

use std::path::Path;

use serde_json::{Value, json};

use crate::{Result, array, number, require, sha256};

/// Full retail 320×224 screen (both Plane A and Plane B + backdrop).
const REGION: [u64; 4] = [0, 0, 320, 224];
const TAPE: &str = "oracle/tapes/48_ending_credits.tape";
const WHITE: &str = "779f77ad9bb3403f7714ccd9996486fa80fa41c4165bce2d6596afc57844f94d";

struct Pair {
    label: &'static str,
    frame: u64,
    digest: &'static str,
    stage: u64,
    text_counter: u64,
    fg_y: u64,
    bg_y: u64,
    column_scroll: bool,
    word0: u64,
    word18: u64,
    word30: u64,
    word47: u64,
    word50: u64,
}

const PAIRS: [Pair; 8] = [
    Pair {
        label: "stage1_entry",
        frame: 43275,
        digest: "16aa7e80efed7643f68ec804fbd75d09e40b288a3a66cd607bd00690aae30842",
        stage: 1,
        text_counter: 0x0019,
        fg_y: 0x0000,
        bg_y: 0xFF39,
        column_scroll: false,
        word0: 0x0000,
        word18: 0x0000,
        word30: 0x0000,
        word47: 0x0EEE,
        word50: 0x08EE,
    },
    Pair {
        label: "stage1_scroll",
        frame: 46000,
        digest: "64fb0478588cd8933419ab1f830717bb7da07047734a38339af177255e81a59f",
        stage: 1,
        text_counter: 0x0156,
        fg_y: 0x0000,
        bg_y: 0x0076,
        column_scroll: false,
        word0: 0x0000,
        word18: 0x0000,
        word30: 0x0000,
        word47: 0x0EEE,
        word50: 0x08EE,
    },
    Pair {
        label: "stage2_column_scroll",
        frame: 51421,
        digest: "16ee8f1aad71b8f9af226c1bc8189159212c32eab26922bb7ca23396ca3e5257",
        stage: 2,
        text_counter: 0x03BA,
        fg_y: 0x003A,
        bg_y: 0x02BD,
        column_scroll: true,
        word0: 0x0000,
        word18: 0x0000,
        word30: 0x0000,
        word47: 0x0EEE,
        word50: 0x08EE,
    },
    Pair {
        label: "stage3_presented",
        frame: 53108,
        digest: "fbec21470844ebf9e39b8f12b08995bdb512dd7c2fdd4636c149b722067f3e34",
        stage: 3,
        text_counter: 0x0480,
        fg_y: 0x0100,
        bg_y: 0x0320,
        column_scroll: true,
        word0: 0x0000,
        word18: 0x0000,
        word30: 0x0000,
        word47: 0x0000,
        word50: 0x08EE,
    },
    Pair {
        label: "ramp_mid",
        frame: 53157,
        digest: "8cb343288115f997e5255b4392f8f7ed3c32c9e5880a4be50496df6b1973fb18",
        stage: 3,
        text_counter: 0x0480,
        fg_y: 0x0100,
        bg_y: 0x0320,
        column_scroll: true,
        word0: 0x0000,
        word18: 0x0ECC,
        word30: 0x0000,
        word47: 0x0000,
        word50: 0x08EE,
    },
    Pair {
        label: "tone_baseline",
        frame: 53216,
        digest: "6ca8d437689efa30d2eeb772c3987fde9f83d2d5992e0d846cfa71f1af6c595c",
        stage: 3,
        text_counter: 0x0480,
        fg_y: 0x0100,
        bg_y: 0x0320,
        column_scroll: true,
        word0: 0x0000,
        word18: 0x0EEE,
        word30: 0x0ECC,
        word47: 0x0000,
        word50: 0x08EE,
    },
    Pair {
        label: "tone_rise",
        frame: 53230,
        digest: "2c649024605eccd9515d9808446e0627b27eb6f260995dd4ba1efe148f86d585",
        stage: 3,
        text_counter: 0x0480,
        fg_y: 0x0100,
        bg_y: 0x0320,
        column_scroll: true,
        word0: 0x0888,
        word18: 0x0EEE,
        word30: 0x0EEE,
        word47: 0x0888,
        word50: 0x0EEE,
    },
    Pair {
        label: "tone_white",
        frame: 53242,
        digest: WHITE,
        stage: 3,
        text_counter: 0x0480,
        fg_y: 0x0100,
        bg_y: 0x0320,
        column_scroll: true,
        word0: 0x0EEE,
        word18: 0x0EEE,
        word30: 0x0EEE,
        word47: 0x0EEE,
        word50: 0x0EEE,
    },
];

pub fn plan() -> Value {
    json!({
        "tape": TAPE,
        "patches": ["7200:FFFFECA8:8021", "7200:FFFFEC20:000C"],
        "pairs": PAIRS.iter().map(|p| json!({"label":p.label,"frame":p.frame})).collect::<Vec<_>>(),
        "region": REGION,
        "negative_control": {"native_label":"tone_baseline","oracle_frame":53230}
    })
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn pinned(path: &Path, expected: &str) -> Result<()> {
    require(
        sha256(&read(path)?) == expected,
        &format!("oracle identity changed: {}", path.display()),
    )
}

/// Check regenerated oracle bytes before allowing a native run.
pub fn oracle_identity(directory: &Path) -> Result<()> {
    for pair in &PAIRS {
        pinned(
            &directory.join(format!("frame_{}.png", pair.frame)),
            pair.digest,
        )?;
    }
    Ok(())
}

fn validate_observations(captures: &Value) -> Result<()> {
    let rows = array(captures)?;
    require(
        rows.len() == PAIRS.len(),
        "expected all eight credits observations",
    )?;
    let mut previous = 0;
    for (row, pair) in rows.iter().zip(&PAIRS) {
        require(
            row["label"].as_str() == Some(pair.label),
            "missing or reordered credits observations",
        )?;
        let tick = number(&row["tick"]).ok_or("invalid native observation tick")?;
        require(
            tick > previous,
            "credits ticks must be positive and strictly increasing",
        )?;
        require(
            number(&row["stage"]) == Some(pair.stage)
                && number(&row["text_counter"]) == Some(pair.text_counter)
                && number(&row["fg_y"]) == Some(pair.fg_y)
                && number(&row["bg_y"]) == Some(pair.bg_y)
                && row["column_scroll"].as_bool() == Some(pair.column_scroll),
            &format!("credits {} state does not match its phase", pair.label),
        )?;
        require(
            number(&row["word0"]) == Some(pair.word0)
                && number(&row["word18"]) == Some(pair.word18)
                && number(&row["word30"]) == Some(pair.word30)
                && number(&row["word47"]) == Some(pair.word47)
                && number(&row["word50"]) == Some(pair.word50),
            &format!("credits {} has the wrong CRAM words", pair.label),
        )?;
        let viewport = array(&row["viewport"])?;
        require(
            viewport.len() == 2
                && number(&viewport[0]) == Some(1280)
                && number(&viewport[1]) == Some(800),
            "credits viewport must be 1280x800",
        )?;
        previous = tick;
    }
    Ok(())
}

pub fn observations(directory: &Path) -> Result<Value> {
    let captures: Value = serde_json::from_slice(&read(&directory.join("captures.json"))?)
        .map_err(|error| error.to_string())?;
    validate_observations(&captures)?;
    Ok(captures)
}

fn validate_comparisons(comparisons: &Value, negative: &Value, leaks: &Value) -> Result<()> {
    require(array(leaks)?.is_empty(), "Godot reported exit leaks")?;
    let rows = array(comparisons)?;
    require(
        rows.len() == PAIRS.len(),
        "expected all eight credits comparisons",
    )?;
    for (row, pair) in rows.iter().zip(&PAIRS) {
        require(
            row["label"].as_str() == Some(pair.label),
            "missing or reordered credits comparisons",
        )?;
        require(
            row["rmse"].as_str() == Some("0.000000"),
            &format!(
                "credits {} is not an exact match: {}",
                pair.label, row["rmse"]
            ),
        )?;
        require(
            row["comparison"].as_str().is_some_and(|s| !s.is_empty()),
            "missing comparison report",
        )?;
    }
    let rejected = negative["rmse"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .is_some_and(|rmse| rmse.is_finite() && rmse > 0.0);
    require(
        rejected,
        "untoned-baseline negative control was not rejected",
    )?;
    require(
        negative["comparison"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "missing negative-control report",
    )
}

/// All acceptance policy and file identities are decided here. Errors exit
/// nonzero through the JSON bridge; Python never interprets a verdict.
pub fn verify(request: &Value) -> Result<Value> {
    let directory = Path::new(request["directory"].as_str().ok_or("missing directory")?);
    let root = Path::new(request["root"].as_str().ok_or("missing source root")?);
    oracle_identity(&directory.join("oracle"))?;
    let captures = observations(directory)?;
    validate_comparisons(
        &request["comparisons"],
        &request["negative_control"],
        &request["exit_leaks"],
    )?;
    let mut results = Vec::new();
    for ((native, pair), comparison) in array(&captures)?
        .iter()
        .zip(&PAIRS)
        .zip(array(&request["comparisons"])?)
    {
        let mut row = native
            .as_object()
            .ok_or("credits observation must be an object")?
            .clone();
        row.insert("oracle_frame".into(), json!(pair.frame));
        row.insert("oracle_sha256".into(), json!(pair.digest));
        row.insert(
            "capture_sha256".into(),
            json!(sha256(&read(
                &directory.join(format!("{}.png", pair.label))
            )?)),
        );
        row.insert("rmse".into(), comparison["rmse"].clone());
        row.insert("comparison".into(), comparison["comparison"].clone());
        results.push(Value::Object(row));
    }
    let mut inputs = serde_json::Map::new();
    for name in [
        "Phantasy Star IV (USA).md",
        TAPE,
        "runtime-pack/presentation/credits.json",
        "runtime-pack/dialogue/font.png",
        "oracle/core/genesis_plus_gx_libretro.so",
    ] {
        inputs.insert(name.into(), json!(sha256(&read(&root.join(name))?)));
    }
    Ok(json!({
        "pass": true,
        "git": request["metadata"]["git"],
        "inputs": inputs,
        "run_root": request["metadata"]["run_root"],
        "region": REGION,
        "results": results,
        "untoned_baseline_control": request["negative_control"],
        "elapsed_s": request["metadata"]["elapsed_s"]
    }))
}

#[cfg(test)]
mod tests;
