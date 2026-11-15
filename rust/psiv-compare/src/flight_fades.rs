//! Flight Pal_FadeIn instruction boundaries and raw CRAM shadow progression.
//! Python adapts the host CSV to JSON; all lasting validation lives here.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

pub type Row = BTreeMap<String, String>;

#[derive(Deserialize)]
pub struct Capture {
    pub route: String,
    pub rows: Vec<Row>,
    pub calls: Vec<Row>,
}

fn sites(route: &str) -> Result<[u32; 3], String> {
    // Instruction locations verified by fixed_fades_image, not palette data.
    match route {
        "mota-zelan" => Ok([0x645D6, 0x64C8E, 0x647B8]),
        "zelan-mota" => Ok([0x648F8, 0x64C8E, 0x64A2E]),
        _ => Err(format!("unknown flight route {route}")),
    }
}

/// The host watches both call instructions and the following instructions.
pub fn watches() -> Vec<u32> {
    ["mota-zelan", "zelan-mota"]
        .into_iter()
        .flat_map(|route| sites(route).unwrap())
        .flat_map(|site| [site, site + 6])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn number(row: &Row, field: &str, radix: u32) -> Result<u32, String> {
    let value = row
        .get(field)
        .ok_or_else(|| format!("missing field {field}"))?;
    u32::from_str_radix(value, radix).map_err(|_| format!("invalid {field}: {value}"))
}

fn word(row: &Row, field: &str) -> Result<u16, String> {
    u16::try_from(number(row, field, 16)?).map_err(|_| format!("{field} exceeds a word"))
}

/// Check each selected fade, returning only timing and target hashes.
pub fn analyze(capture: Capture) -> Result<Value, String> {
    let expected = sites(&capture.route)?;
    measure(&capture.rows, &capture.calls, &expected).map(Value::Array)
}

fn measure(rows: &[Row], calls: &[Row], sites: &[u32]) -> Result<Vec<Value>, String> {
    let mut frames = BTreeMap::new();
    for row in rows {
        let frame = number(row, "frame", 10)?;
        if frames.insert(frame, row).is_some() {
            return Err(format!("duplicate frame {frame}"));
        }
    }
    let pcs: Vec<u32> = calls
        .iter()
        .map(|row| number(row, "pc", 16))
        .collect::<Result<_, _>>()?;
    let expected: Vec<u32> = sites.iter().flat_map(|&site| [site, site + 6]).collect();
    if pcs != expected {
        return Err("omitted, duplicated or reordered flight fade".into());
    }
    let mut result = Vec::new();
    for pair in calls.as_chunks::<2>().0 {
        let start = number(&pair[0], "frame", 10)?;
        let end = number(&pair[1], "frame", 10)?;
        if end.checked_sub(start) != Some(16) {
            return Err("flight fade did not spend 16 frames".into());
        }
        let first = frames
            .get(&start)
            .ok_or_else(|| format!("missing frame {start}"))?;
        let mut target = [0u16; 64];
        for (index, value) in target.iter_mut().enumerate() {
            *value = word(first, &format!("saved_{index}"))?;
        }
        for elapsed in 0..16u16 {
            let frame = start
                .checked_add(u32::from(elapsed))
                .ok_or("frame overflow")?;
            let row = frames
                .get(&frame)
                .ok_or_else(|| format!("missing frame {frame}"))?;
            for (index, &target) in target.iter().enumerate() {
                let expected = [0, 4, 8, 12].into_iter().fold(0u16, |value, shift| {
                    value | (((target >> shift) & 15).min(elapsed) << shift)
                });
                if word(row, &format!("live_{index}"))? != expected {
                    return Err(format!("wrong palette pass at frame {frame}, word {index}"));
                }
            }
            if (word(row, "vdp_reg1")? & 0x40 != 0) != (elapsed != 0) {
                return Err(format!("wrong display-enable frame {frame}"));
            }
        }
        let bytes: Vec<u8> = target.into_iter().flat_map(u16::to_be_bytes).collect();
        result.push(json!({
            "site": format!("{:06X}", number(&pair[0], "pc", 16)?),
            "start": start, "return": end, "offset": i64::from(start) - 7401,
            "target_sha256": psiv_pack::png::hex(&Sha256::digest(bytes)),
        }));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<Row>, Vec<Row>) {
        let rows = (0..16u16)
            .map(|elapsed| {
                let mut row = Row::from([
                    ("frame".into(), (100 + elapsed).to_string()),
                    (
                        "vdp_reg1".into(),
                        if elapsed == 0 { "8124" } else { "8164" }.into(),
                    ),
                ]);
                for index in 0..64 {
                    row.insert(format!("saved_{index}"), "0EEE".into());
                    row.insert(
                        format!("live_{index}"),
                        format!("{:04X}", elapsed.min(14) * 0x111),
                    );
                }
                row
            })
            .collect();
        let calls = [("001000", "100"), ("001006", "116")]
            .map(|(pc, frame)| {
                Row::from([("pc".into(), pc.into()), ("frame".into(), frame.into())])
            })
            .to_vec();
        (rows, calls)
    }

    #[test]
    fn all_words_display_wait_and_terminal_pass_are_checked() {
        let (rows, calls) = fixture();
        assert_eq!(measure(&rows, &calls, &[0x1000]).unwrap()[0]["return"], 116);
        for (field, value, expected) in [
            ("live_1", "0EEE", "wrong palette"),
            ("live_63", "0000", "wrong palette"),
            ("vdp_reg1", "8124", "wrong display"),
        ] {
            let mut altered = rows.clone();
            altered[1].insert(field.into(), value.into());
            assert!(
                measure(&altered, &calls, &[0x1000])
                    .unwrap_err()
                    .contains(expected)
            );
        }
        let mut altered = rows.clone();
        altered[0].insert("vdp_reg1".into(), "8164".into());
        assert!(
            measure(&altered, &calls, &[0x1000])
                .unwrap_err()
                .contains("wrong display")
        );
        let mut altered = rows.clone();
        altered[15].insert("live_0".into(), "0DDD".into());
        assert!(
            measure(&altered, &calls, &[0x1000])
                .unwrap_err()
                .contains("wrong palette")
        );
    }

    #[test]
    fn omitted_doubled_reordered_calls_and_bad_duration_fail() {
        let (rows, calls) = fixture();
        for altered in [
            calls[..1].to_vec(),
            [calls.clone(), calls.clone()].concat(),
            calls.iter().rev().cloned().collect(),
        ] {
            assert!(
                measure(&rows, &altered, &[0x1000])
                    .unwrap_err()
                    .contains("omitted, duplicated or reordered")
            );
        }
        let mut altered = calls.clone();
        altered[1].insert("frame".into(), "115".into());
        assert!(
            measure(&rows, &altered, &[0x1000])
                .unwrap_err()
                .contains("16 frames")
        );
    }

    #[test]
    fn missing_or_duplicate_samples_and_malformed_words_fail() {
        let (rows, calls) = fixture();
        assert!(
            measure(&rows[1..], &calls, &[0x1000])
                .unwrap_err()
                .contains("missing frame")
        );
        let mut altered = rows.clone();
        altered.push(rows[0].clone());
        assert!(
            measure(&altered, &calls, &[0x1000])
                .unwrap_err()
                .contains("duplicate frame")
        );
        for value in ["garbage", "10000"] {
            let mut altered = rows.clone();
            altered[1].insert("live_0".into(), value.into());
            assert!(measure(&altered, &calls, &[0x1000]).is_err());
        }
    }

    #[test]
    fn the_planet_watch_is_shared_and_unknown_routes_fail() {
        assert_eq!(watches().len(), 10, "five unique calls and their returns");
        assert_eq!(watches().iter().filter(|&&pc| pc == 0x64C8E).count(), 1);
        assert!(
            analyze(Capture {
                route: "unknown".into(),
                rows: vec![],
                calls: vec![]
            })
            .is_err()
        );
    }
}
