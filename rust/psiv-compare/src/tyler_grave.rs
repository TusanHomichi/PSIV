//! Tyler's flagged grave replay, without dialogue dwell. The fixture's RAM
//! writes select map/party/event; the unmodified cartridge executes the scene.
//! Retail basis and the runtime comparison limit: docs/scenes/92_TylerGraveOpening.md.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

type Row<'a> = BTreeMap<&'a str, &'a str>;

fn records(csv: &str) -> Result<BTreeMap<u32, Row<'_>>, String> {
    // The oracle host emits unquoted numeric cells and identifier headers,
    // not general text CSV. Reject unsupported syntax instead of misreading it.
    let mut lines = csv
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty());
    let header: Vec<_> = lines
        .next()
        .ok_or("missing CSV header")?
        .split(',')
        .collect();
    if header.iter().copied().collect::<BTreeSet<_>>().len() != header.len() {
        return Err("duplicate CSV header".into());
    }
    let mut frames = BTreeMap::new();
    for line in lines {
        let values: Vec<_> = line.split(',').collect();
        if values.len() != header.len() || line.contains('"') {
            return Err("invalid oracle numeric CSV row".into());
        }
        let row: Row<'_> = header.iter().copied().zip(values).collect();
        let frame = row
            .get("frame")
            .ok_or("missing frame column")?
            .parse::<u32>()
            .map_err(|error| format!("invalid frame: {error}"))?;
        if frames.insert(frame, row).is_some() {
            return Err(format!("duplicate frame {frame}"));
        }
    }
    Ok(frames)
}

fn expect(
    frames: &BTreeMap<u32, Row<'_>>,
    frame: u32,
    fields: &[(&str, String)],
) -> Result<(), String> {
    let row = frames
        .get(&frame)
        .ok_or_else(|| format!("missing frame {frame}"))?;
    for (key, expected) in fields {
        let actual = row
            .get(key)
            .ok_or_else(|| format!("missing column {key}"))?;
        if *actual != expected {
            return Err(format!("frame {frame} {key}: {actual} != {expected}"));
        }
    }
    Ok(())
}

/// Reject changed camera, animation, drift or event-return boundaries, and
/// return the measured envelope. Every frame in 7199..=7330 is checked.
pub fn measure(csv: &str) -> Result<Value, String> {
    let frames = records(csv)?;
    for frame in 7199..=7330 {
        expect(
            &frames,
            frame,
            &[
                ("map_index", "0120".into()),
                ("c1_x_px", "304".into()),
                ("c1_y_px", "432".into()),
                ("cam_y_fg_px", "344".into()),
                (
                    "cam_x_fg_px",
                    if matches!(frame, 7200 | 7268) {
                        "156"
                    } else {
                        "152"
                    }
                    .into(),
                ),
                (
                    "game_mode_routine",
                    if matches!(frame, 7199 | 7330) {
                        "0000"
                    } else {
                        "000C"
                    }
                    .into(),
                ),
            ],
        )?;
    }
    for frame in 7200..7204 {
        expect(
            &frames,
            frame,
            &[
                ("o02_x_px", "288".into()),
                ("o03_x_px", "352".into()),
                ("o02_facing", "0".into()),
                ("o03_facing", "4".into()),
            ],
        )?;
    }
    for update in 1_i32..=64 {
        let index = match update {
            1..=4 => 0,
            5..=9 => 1,
            10..=14 => 2,
            _ => 3,
        };
        expect(
            &frames,
            7203 + update as u32,
            &[
                ("o02_x_px", (288 + (-update).div_euclid(4)).to_string()),
                ("o03_x_px", (352 + update / 2).to_string()),
                ("o02_facing", "16".into()),
                ("o03_facing", "20".into()),
                ("o02_map_idx", index.to_string()),
                ("o03_map_idx", index.to_string()),
            ],
        )?;
    }
    for frame in 7268..=7330 {
        expect(
            &frames,
            frame,
            &[
                ("o02_x_px", "272".into()),
                ("o03_x_px", "384".into()),
                ("o02_facing", "8".into()),
                ("o03_facing", "12".into()),
                ("o02_map_idx", "0".into()),
                ("o03_map_idx", "0".into()),
            ],
        )?;
    }
    Ok(
        json!({"entry_frame":7200,"first_drift_frame":7204,"last_drift_frame":7267,
        "return_frame":7330,"frames_through_return":131,"drift_frames":64,
        "animation_index_edges":[7204,7208,7213,7218],"camera_lead_frames":[7200,7268]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "frame,map_index,c1_x_px,c1_y_px,cam_x_fg_px,cam_y_fg_px,game_mode_routine,o02_x_px,o03_x_px,o02_facing,o03_facing,o02_map_idx,o03_map_idx";

    fn trace(change: Option<(u32, usize, &str)>) -> String {
        let mut csv = format!("# constructed boundary control\n{HEADER}\n");
        for frame in 7199_u32..=7330 {
            let update = (frame as i32 - 7203).clamp(0, 64);
            let sliding = (7204..=7267).contains(&frame);
            let restored = frame >= 7268;
            let index = match update {
                0..=4 => 0,
                5..=9 => 1,
                10..=14 => 2,
                _ => 3,
            };
            let mut fields = vec![
                frame.to_string(),
                "0120".into(),
                "304".into(),
                "432".into(),
                if matches!(frame, 7200 | 7268) {
                    "156"
                } else {
                    "152"
                }
                .into(),
                "344".into(),
                if matches!(frame, 7199 | 7330) {
                    "0000"
                } else {
                    "000C"
                }
                .into(),
                (288 + (-update).div_euclid(4)).to_string(),
                (352 + update / 2).to_string(),
                if restored {
                    "8"
                } else if sliding {
                    "16"
                } else {
                    "0"
                }
                .into(),
                if restored {
                    "12"
                } else if sliding {
                    "20"
                } else {
                    "4"
                }
                .into(),
                if sliding { index } else { 0 }.to_string(),
                if sliding { index } else { 0 }.to_string(),
            ];
            if let Some((at, column, value)) = change
                && at == frame
            {
                fields[column] = value.into();
            }
            csv.push_str(&fields.join(","));
            csv.push('\n');
        }
        csv
    }

    #[test]
    fn accepts_recorded_boundaries() {
        assert_eq!(measure(&trace(None)).unwrap()["frames_through_return"], 131);
    }

    #[test]
    fn rejects_early_animation_missing_pan_and_short_wait() {
        for mutation in [(7207, 11, "1"), (7200, 4, "152"), (7329, 6, "0000")] {
            assert!(
                measure(&trace(Some(mutation))).is_err(),
                "mutation {mutation:?}"
            );
        }
    }

    #[test]
    fn rejects_missing_or_duplicate_frames_and_missing_columns() {
        let csv = trace(None);
        let missing = csv
            .lines()
            .filter(|line| !line.starts_with("7200,"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            measure(&missing)
                .unwrap_err()
                .contains("missing frame 7200")
        );
        let duplicate = format!(
            "{csv}{}\n",
            csv.lines().find(|line| line.starts_with("7200,")).unwrap()
        );
        assert!(
            measure(&duplicate)
                .unwrap_err()
                .contains("duplicate frame 7200")
        );
        assert!(
            measure(&csv.replace("o02_map_idx", "unknown"))
                .unwrap_err()
                .contains("missing column o02_map_idx")
        );
    }
}
