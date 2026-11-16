//! Ending staff observation and comparison contracts. The launcher only runs
//! the oracle, Godot and psiv-compare; this module owns their acceptance.

use std::path::Path;

use serde_json::{Value, json};

use crate::{Result, array, number, require, sha256};

const REGION: [u64; 4] = [24, 64, 256, 88];
const TAPE: &str = "oracle/tapes/47_ending_staff.tape";
const WHITE: &str = "779f77ad9bb3403f7714ccd9996486fa80fa41c4165bce2d6596afc57844f94d";

struct Pair {
    label: &'static str,
    frame: u64,
    digest: &'static str,
    word: u64,
}

const PAIRS: [Pair; 6] = [
    Pair {
        label: "before",
        frame: 42503,
        digest: WHITE,
        word: 0xEEE,
    },
    Pair {
        label: "fall",
        frame: 42525,
        digest: "805c7aaa712e730d6eee66a4d31f6cad1927ea3df32bff087cf09062c64b945f",
        word: 0x888,
    },
    Pair {
        label: "dark",
        frame: 42555,
        digest: "0baa1fbd6c9ce8a7db30ce57b22d69bb07eee13e7fd8811e3226e49a79d193a6",
        word: 0x222,
    },
    Pair {
        label: "rise",
        frame: 42945,
        digest: "75a610dbc87ff7760c47fe2254fd33c6a551fbf9d989945bcaed2657b54aa6dc",
        word: 0xAAA,
    },
    Pair {
        label: "after",
        frame: 42963,
        digest: WHITE,
        word: 0xEEE,
    },
    Pair {
        label: "cleared",
        frame: 42966,
        digest: WHITE,
        word: 0xEEE,
    },
];

pub fn plan() -> Value {
    json!({
        "tape": TAPE,
        "patches": ["7200:FFFFECA8:8021", "7200:FFFFEC20:000C"],
        "pairs": PAIRS.iter().map(|p| json!({"label":p.label,"frame":p.frame})).collect::<Vec<_>>(),
        "region": REGION,
        "negative_control": {"native_label":"cleared","oracle_frame":42525}
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
        "expected all six staff observations",
    )?;
    let mut previous = 0;
    for (index, (row, pair)) in rows.iter().zip(&PAIRS).enumerate() {
        require(
            row["label"].as_str() == Some(pair.label),
            "missing or reordered staff observations",
        )?;
        let tick = number(&row["tick"]).ok_or("invalid native observation tick")?;
        require(
            tick > previous,
            "staff ticks must be positive and strictly increasing",
        )?;
        require(
            number(&row["word47"]) == Some(pair.word),
            "staff observation has the wrong CRAM word",
        )?;
        require(
            row["visible"].as_bool() == Some(index < 5),
            "staff visibility does not match its phase",
        )?;
        let viewport = array(&row["viewport"])?;
        require(
            viewport.len() == 2
                && number(&viewport[0]) == Some(1280)
                && number(&viewport[1]) == Some(800),
            "staff viewport must be 1280x800",
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
        "expected all six staff comparisons",
    )?;
    for (row, pair) in rows.iter().zip(&PAIRS) {
        require(
            row["label"].as_str() == Some(pair.label),
            "missing or reordered staff comparisons",
        )?;
        // psiv-compare emits six decimal places. Missing/malformed metrics
        // and any nonzero difference are failures, not absent observations.
        require(
            row["rmse"].as_str() == Some("0.000000"),
            &format!(
                "staff {} is not an exact match: {}",
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
        "missing-transfer negative control was not rejected",
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
            .ok_or("staff observation must be an object")?
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
        "runtime-pack/dialogue/font.png",
        "runtime-pack/dialogue/trees.json",
        "oracle/core/genesis_plus_gx_libretro.so",
    ] {
        inputs.insert(name.into(), json!(sha256(&read(&root.join(name))?)));
    }
    Ok(
        json!({"pass":true,"git":request["metadata"]["git"],"inputs":inputs,
        "run_root":request["metadata"]["run_root"],"region":REGION,"results":results,
        "missing_transfer_control":request["negative_control"],"elapsed_s":request["metadata"]["elapsed_s"]}),
    )
}

#[cfg(test)]
mod tests;
