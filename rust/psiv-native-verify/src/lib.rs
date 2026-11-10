//! Native observation contracts. Image decoding and pixel comparisons stay with
//! the image-library caller; schema, recorded state and file identity live here.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, String>;

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut out, b| {
            write!(out, "{b:02x}").expect("write String");
            out
        })
}

fn array(v: &Value) -> Result<&Vec<Value>> {
    v.as_array().ok_or_else(|| "expected an array".into())
}

// Godot's JSON state serializes integer-valued Variant numbers as floats.
// Request/result frame addresses deliberately use strict JSON integers instead.
fn number(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| {
        let n = v.as_f64()?;
        (n >= 0.0 && n < u64::MAX as f64 && n.fract() == 0.0).then_some(n as u64)
    })
}

fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}

/// Validate the manifest before paths or frame addresses reach Godot.
pub fn requests(input: &Value, tape_sha256: Option<&str>) -> Result<Value> {
    let entries = if let Some(obj) = input.as_object() {
        let hash = obj.get("tape_sha256").and_then(Value::as_str);
        require(
            obj.len() == 2
                && obj.contains_key("requests")
                && hash.is_some_and(|s| {
                    s.len() == 64
                        && s.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
                && hash == tape_sha256,
            "observation manifest does not name this tape SHA-256",
        )?;
        &input["requests"]
    } else {
        input
    };
    let rows = array(entries)?;
    require(
        !rows.is_empty(),
        "observations must be a non-empty JSON array",
    )?;
    let mut previous = 0;
    let mut labels = BTreeSet::new();
    for row in rows {
        let obj = row.as_object().ok_or("observation needs an object")?;
        require(
            obj.keys()
                .all(|k| matches!(k.as_str(), "frame" | "label" | "capture")),
            "unknown observation field",
        )?;
        let frame = row["frame"]
            .as_u64()
            .ok_or("observation frames must be positive ascending integers")?;
        require(
            frame > 0 && frame >= previous,
            "observation frames must be positive ascending integers",
        )?;
        let label = row["label"].as_str().ok_or("observation needs a label")?;
        require(
            !label.is_empty()
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                && labels.insert(label),
            "observation labels must be unique safe artifact names",
        )?;
        require(
            obj.get("capture").is_none_or(Value::is_boolean),
            "observation capture must be a boolean",
        )?;
        previous = frame;
    }
    Ok(entries.clone())
}

/// A successful process cannot hide an omitted, misaddressed or changed product.
pub fn receipt(wanted: &Value, report: &Value, directory: &Path) -> Result<()> {
    let rows = array(wanted)?;
    if rows.is_empty() {
        return Ok(());
    }
    requests(wanted, None)?;
    let actual = array(&report["observations"])?;
    require(rows.len() == actual.len(), "missing observation results")?;
    for (want, got) in rows.iter().zip(actual) {
        require(
            got["label"] == want["label"]
                && got["frame"].as_u64().is_some()
                && got["frame"] == want["frame"],
            "observation result label/frame mismatch",
        )?;
        let state = got["state"]
            .as_object()
            .ok_or("missing observation state")?;
        require(
            state.contains_key("map") && state.contains_key("cell"),
            "missing observation map/cell",
        )?;
        if want["capture"].as_bool() == Some(true) {
            let path = directory.join(format!("{}.png", want["label"].as_str().unwrap()));
            require(
                got["capture"].as_str() == path.to_str(),
                "capture path mismatch",
            )?;
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            require(
                !bytes.is_empty()
                    && got["capture_file_sha256"].as_str() == Some(sha256(&bytes).as_str()),
                "capture file SHA-256 mismatch",
            )?;
        }
    }
    Ok(())
}

fn observations(native: &Value) -> Result<BTreeMap<&str, &Value>> {
    let mut rows = BTreeMap::new();
    for row in array(&native["observations"])? {
        let label = row["label"].as_str().ok_or("missing observation label")?;
        require(
            rows.insert(label, row).is_none(),
            "duplicate observation label",
        )?;
    }
    Ok(rows)
}

fn indices(rows: &Value) -> Result<BTreeSet<u64>> {
    let rows = array(rows)?;
    let set: BTreeSet<_> = rows
        .iter()
        .map(|v| number(v).ok_or("invalid resident index".into()))
        .collect::<Result<_>>()?;
    require(
        rows.len() == 7 && set.len() == 7,
        "expected seven distinct active resident indices",
    )?;
    Ok(set)
}

/// US MapDataMan_ChkZemaNormal ($051E56): all restored active residents must
/// receive their extracted replacement sheets, alongside the live palette copy.
pub fn zema(native: &Value, map: &Value) -> Result<Value> {
    let rows = observations(native)?;
    let row = |label: &str| {
        rows.get(label)
            .copied()
            .ok_or_else(|| format!("missing {label}"))
    };
    let mut pages = BTreeSet::new();
    let mut frames = Vec::new();
    let mut captures = Vec::new();
    for n in 1..=50 {
        let label = format!("zema-page-{n:02}");
        let r = row(&label)?;
        let page = &r["state"]["dialogue_page"];
        require(
            number(&r["state"]["map"]) == Some(0x24)
                && page["ready"] == true
                && page.get("tree").is_some()
                && page.get("lines").is_some(),
            "Zema page is not a ready page in Zema",
        )?;
        pages.insert(
            serde_json::to_string(&(page["tree"].clone(), page["lines"].clone()))
                .map_err(|e| e.to_string())?,
        );
        frames.push(r["frame"].as_u64().ok_or("invalid Zema page frame")?);
        captures.push(json!({"label":label,"path":r["capture"]}));
    }
    require(
        pages.len() == 50 && frames.windows(2).all(|p| p[0] < p[1]),
        "Zema must have 50 distinct pages at increasing tape frames",
    )?;
    for (label, y) in [("igglanova-halfway", 176), ("igglanova-arrived", 192)] {
        let r = row(label)?;
        let state = &r["state"];
        require(
            number(&state["map"]) == Some(0x24)
                && array(&state["temporary_objects"])?.iter().any(|o| {
                    number(&o["id"]) == Some(0x188)
                        && o["position"].as_array().is_some_and(|p| {
                            p.len() == 2 && number(&p[0]) == Some(480) && number(&p[1]) == Some(y)
                        })
                }),
            "missing Igglanova at recorded entrance position",
        )?;
        captures.push(json!({"label":label,"path":r["capture"]}));
    }
    let r = row("zema-restored")?;
    let restored = &r["state"];
    require(
        number(&restored["map"]) == Some(0x24)
            && array(&restored["flags"])?
                .iter()
                .any(|v| number(v) == Some(0x37)),
        "Zema aftermath not restored",
    )?;
    let active = indices(&restored["active_npcs"])?;
    let mut copies = Vec::new();
    for entry in array(&map["map_effects"])? {
        for path in array(&entry["paths"])? {
            if let Some(effects) = path.get("deferred_effects") {
                copies.extend(array(effects)?.iter().filter(|e| e["at"] == "0x051E56"));
            }
        }
    }
    require(
        copies.len() == 1,
        "pack must contain one US Zema palette copy at $051E56",
    )?;
    let copy = copies[0];
    let words: Vec<_> = array(&copy["words_hex"])?
        .iter()
        .map(|v| {
            v.as_str()
                .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .ok_or("invalid palette word".into())
        })
        .collect::<Result<_>>()?;
    let cram = array(&restored["cram"]["words"])?;
    require(
        words.len() == 16
            && cram.len() == 64
            && restored["cram"]["live"] == true
            && cram[48..]
                .iter()
                .map(number)
                .eq(words.iter().copied().map(Some)),
        "restored Zema CRAM line 3 differs from extracted palette copy",
    )?;
    let mut targets = BTreeMap::new();
    for target in array(&copy["affects"]["npc_sheets"])? {
        let index = number(&target["npc_index"]).ok_or("invalid replacement resident index")?;
        let sheet = target["to"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("invalid replacement sheet")?;
        require(
            targets.insert(index, sheet).is_none(),
            "duplicate replacement resident index",
        )?;
    }
    require(
        targets.keys().copied().collect::<BTreeSet<_>>() == active,
        "replacement mapping must cover all seven active residents",
    )?;
    let mut sheets = BTreeMap::new();
    for sheet in array(&restored["npc_sheets"])? {
        let index = number(&sheet["index"]).ok_or("invalid captured resident index")?;
        require(
            sheets.insert(index, sheet["sheet"].as_str()).is_none(),
            "duplicate captured resident index",
        )?;
    }
    require(
        sheets.keys().copied().collect::<BTreeSet<_>>() == active,
        "captured sheets must cover exactly the seven active residents",
    )?;
    for (index, target) in &targets {
        require(
            sheets.get(index) == Some(&Some(*target)),
            "resident has not taken its restored palette sheet",
        )?;
    }
    captures.push(json!({"label":"zema-restored","path":r["capture"]}));
    Ok(
        json!({"pass":true,"pages":50,"resident_sheets":7,"palette_copy":"0x051E56","cram_line":3,"frames":frames,"captures":captures}),
    )
}

pub fn exchange(request: &Value) -> Result<Value> {
    match request["operation"].as_str() {
        Some("requests") => requests(&request["manifest"], request["tape_sha256"].as_str()),
        Some("receipt") => {
            receipt(
                &request["requests"],
                &request["report"],
                Path::new(request["directory"].as_str().ok_or("missing directory")?),
            )?;
            Ok(json!({"pass":true}))
        }
        Some("zema") => zema(&request["native"], &request["map"]),
        _ => Err("unknown native verification operation".into()),
    }
}

#[cfg(test)]
mod tests;
