//! `_coordinate_sites` and `_movement_evidence` from
//! `psiv_tools/battle_animations.py`: a small opcode decoder for the direct
//! writes to a battle object's `$2C`/`$2E` position and `$30`/`$34`
//! fixed-point fields, and the movement verdict for the selected object.

use std::collections::BTreeSet;

use super::{hex6, int, long, string, word};
use crate::json::Json;

const FIGHTER_X: u16 = 0x2C;
const FIGHTER_Y: u16 = 0x2E;
const FIXED_X: u16 = 0x30;
const FIXED_Y: u16 = 0x34;
const BASE_OBJECT_Y: i128 = 0x00D8;

const KNOWN_KINDS: [&str; 8] = [
    "delta",
    "absolute",
    "compare",
    "fixed_point_absolute",
    "fixed_point_delta",
    "fixed_point_load",
    "fixed_point_add",
    "fixed_point_commit",
];

/// The `fields` object every movement record carries.
pub fn fields() -> Json {
    Json::object()
        .with("x", "$2C(a4)")
        .with("y", "$2E(a4)")
        .with("fixed_point_x", "$30(a4)")
        .with("fixed_point_y", "$34(a4)")
        .with("shared_init", "loc_12618")
}

fn axis(field: u16) -> &'static str {
    match field {
        FIGHTER_X | FIXED_X => "x",
        FIGHTER_Y | FIXED_Y => "y",
        _ => "fixed",
    }
}

fn site(object_id: i128, at: usize, field: u16, kind: &str) -> Json {
    Json::object()
        .with("object_id", Json::Int(object_id))
        .with("rom_offset", hex6(at))
        .with("field", format!("${field:02X}(a4)"))
        .with("axis", axis(field))
        .with("kind", kind)
}

fn s16(value: u16) -> i64 {
    i64::from(value as i16)
}

fn s32(value: u32) -> i64 {
    i64::from(value as i32)
}

/// `_coordinate_sites`: one object's direct coordinate writes, in order.
pub fn coordinate_sites(rom: &[u8], start: usize, end: usize, object_id: i128) -> Vec<Json> {
    let w = |at: usize| word(rom, at).expect("a word inside the ROM");
    let l = |at: usize| long(rom, at).expect("a long inside the ROM");
    let accumulators = [0x20, 0x24, FIXED_X, FIXED_Y];
    let mut sites = Vec::new();
    for at in (start..start.max(end.saturating_sub(7))).step_by(2) {
        let opcode = w(at);
        if opcode == 0x06AC {
            let field = w(at + 6);
            if accumulators.contains(&field) {
                sites.push(
                    site(object_id, at, field, "fixed_point_delta")
                        .with("delta_fixed", s32(l(at + 2)))
                        .with("operation", "addi.l")
                        .with("accumulator_field", format!("${field:02X}(a4)")),
                );
            }
            continue;
        }
        if opcode & 0xF1FF == 0x202C {
            let field = w(at + 2);
            if accumulators.contains(&field) {
                sites.push(
                    site(object_id, at, field, "fixed_point_load")
                        .with("source_field", format!("${field:02X}(a4)"))
                        .with("register", format!("d{}", (opcode >> 9) & 0x07))
                        .with("operation", "move.l d16(a4),Dn"),
                );
            }
            continue;
        }
        if opcode & 0xF1FF == 0xD0AC && opcode & 0x00C0 == 0x0080 {
            let field = w(at + 2);
            if field == FIXED_X || field == FIXED_Y {
                sites.push(
                    site(object_id, at, field, "fixed_point_add")
                        .with("register", format!("d{}", (opcode >> 9) & 0x07))
                        .with("operation", "add.l Dn,d16(a4)"),
                );
            }
            continue;
        }
        if opcode == 0x297C {
            let field = w(at + 6);
            if accumulators.contains(&field) {
                sites.push(
                    site(object_id, at, field, "fixed_point_absolute")
                        .with("value_fixed", s32(l(at + 2)))
                        .with("operation", "move.l #imm"),
                );
            }
            continue;
        }
        if opcode == 0x066C || opcode == 0x046C {
            let field = w(at + 4);
            if field == FIGHTER_X || field == FIGHTER_Y {
                let immediate = s16(w(at + 2));
                let add = opcode == 0x066C;
                sites.push(
                    site(object_id, at, field, "delta")
                        .with("delta", if add { immediate } else { -immediate })
                        .with("operation", if add { "addi.w" } else { "subi.w" }),
                );
            }
            continue;
        }
        // addq/subq.w #n,d16(a4), read as the Python decoder reads it.
        if (0x5000..=0x5FFF).contains(&opcode) && opcode & 0x00FF == 0x6C {
            let field = w(at + 2);
            if field == FIGHTER_X || field == FIGHTER_Y {
                let quick = match (opcode >> 1) & 0x07 {
                    0 => 8,
                    n => i64::from(n),
                };
                let add = opcode & 0x0001 == 0;
                sites.push(
                    site(object_id, at, field, "delta")
                        .with("delta", if add { quick } else { -quick })
                        .with("operation", if add { "addq.w" } else { "subq.w" }),
                );
            }
            continue;
        }
        if opcode == 0x397C || opcode == 0x0C6C {
            let field = w(at + 4);
            if field == FIGHTER_X || field == FIGHTER_Y {
                let (kind, operation) = if opcode == 0x397C {
                    ("absolute", "move.w #imm")
                } else {
                    ("compare", "cmpi.w #imm")
                };
                sites.push(
                    site(object_id, at, field, kind)
                        .with("value", w(at + 2))
                        .with("operation", operation),
                );
            }
            continue;
        }
        if opcode == 0x396C {
            let field = w(at + 4);
            if field == FIGHTER_X || field == FIGHTER_Y {
                sites.push(
                    site(object_id, at, field, "fixed_point_commit")
                        .with("source_field", format!("${:02X}(a4)", w(at + 2)))
                        .with("operation", "move.w d16(a4),d16(a4)"),
                );
            }
        }
    }
    sites
}

fn kind(site: &Json) -> &str {
    string(site, "kind").unwrap_or_default()
}

fn pixels(x: i128, y: i128) -> Vec<Json> {
    vec![Json::Int(x), Json::Int(y)]
}

fn static_offset(x: i128, y: i128) -> Json {
    Json::object()
        .with("kind", "static_offset")
        .with("initial_offset_pixels", pixels(x, y))
        .with("step_pixels", pixels(0, 0))
}

/// `_movement_evidence`. `by_object` is each reachable object's sites, in
/// visit order; `primary` the selected sequence's object id, if any.
pub fn evidence(by_object: &[(i128, Vec<Json>)], frame_sequence: Option<&Json>) -> Json {
    let sites: Vec<Json> = by_object.iter().flat_map(|(_, s)| s.clone()).collect();
    let primary_id = frame_sequence.and_then(|s| int(s, "object_id"));
    let lookup = match primary_id {
        Some(0) | None => -1,
        Some(id) => id,
    };
    let primary: Vec<Json> = by_object
        .iter()
        .find(|(id, _)| *id == lookup)
        .map(|(_, s)| s.clone())
        .unwrap_or_default();
    let tracks: Vec<Json> = by_object
        .iter()
        .filter(|(id, s)| Some(*id) != primary_id && !s.is_empty())
        .map(|(id, s)| {
            Json::object()
                .with("object_id", Json::Int(*id))
                .with("status", "exact")
                .with("kind", "object_track")
                .with("writes", s.clone())
                .with(
                    "reason",
                    "reachable effect/projectile object kept separate from the selected mapping \
                     object",
                )
        })
        .collect();
    let mut runtime = Json::Null;
    let mut status = "deferred";
    let mut reason = "no positive-duration mapping record selects a movement clock";
    if frame_sequence.is_some() {
        let delta = |s: &Json| int(s, "delta").unwrap_or(0);
        let value = |s: &Json| int(s, "value").unwrap_or(0);
        let axis_of = |s: &Json| string(s, "axis").unwrap_or_default().to_owned();
        if !primary.is_empty() && primary.iter().all(|s| kind(s) == "delta") {
            let (mut x, mut y) = (0, 0);
            for s in &primary {
                if axis_of(s) == "x" {
                    x += delta(s);
                } else {
                    y += delta(s);
                }
            }
            runtime = static_offset(x, y);
            status = "exact";
            reason = "selected object uses only direct relative coordinate deltas";
        } else if primary.is_empty() {
            runtime = static_offset(0, 0);
            status = "exact";
            reason = "selected mapping object stays on the shared fighter pivot; reachable \
                      effect tracks are recorded separately";
        } else {
            let of = |k: &str| primary.iter().filter(|s| kind(s) == k).collect::<Vec<_>>();
            let (absolutes, deltas, compares) = (of("absolute"), of("delta"), of("compare"));
            let axes: BTreeSet<String> = primary
                .iter()
                .filter(|s| kind(s) != "compare")
                .map(axis_of)
                .collect();
            let single = axes.iter().next().cloned().unwrap_or_default();
            if absolutes.len() == 1
                && deltas.len() == 1
                && compares.len() == 1
                && axes.len() == 1
                && primary.iter().all(|s| axis_of(s) == single)
            {
                let start = value(absolutes[0]);
                let step = delta(deltas[0]);
                let on = |axis: &str, v: i128| if single == axis { v } else { 0 };
                runtime = Json::object()
                    .with("kind", "linear")
                    .with("axis", single.as_str())
                    .with("start_hardware", Json::Int(start))
                    .with("step_hardware", Json::Int(step))
                    .with("limit_hardware", Json::Int(value(compares[0])))
                    .with(
                        "initial_offset_pixels",
                        pixels(
                            on("x", start - BASE_OBJECT_Y),
                            on("y", start - BASE_OBJECT_Y),
                        ),
                    )
                    .with("step_pixels", pixels(on("x", step), on("y", step)));
                status = "exact";
                reason = "absolute start, per-tick delta, and terminal compare are all \
                          retail-proven";
            } else if primary.iter().all(|s| KNOWN_KINDS.contains(&kind(s))) {
                let (mut x, mut y) = (0, 0);
                if let Some(first) = absolutes.first() {
                    let offset = value(first) - BASE_OBJECT_Y;
                    if axis_of(first) == "x" {
                        x = offset;
                    } else {
                        y = offset;
                    }
                }
                let fixed = primary.iter().any(|s| kind(s).starts_with("fixed_point"));
                runtime = Json::object()
                    .with(
                        "kind",
                        if fixed {
                            "fixed_point_branch"
                        } else {
                            "branch_table"
                        },
                    )
                    .with("initial_offset_pixels", pixels(x, y))
                    .with("step_pixels", pixels(0, 0))
                    .with("branches", primary.clone());
                status = "exact";
                reason = "every selected-object $2C/$2E/$30/$34 write is decoded; branch and \
                          fixed-point variants are retained as an exact structural track";
            } else {
                status = "partial";
                reason = "coordinate writes include an opcode outside the decoded \
                          $2C/$2E/$30/$34 track";
            }
        }
    }
    let complete = sites.iter().all(|s| KNOWN_KINDS.contains(&kind(s)));
    Json::object()
        .with("status", status)
        .with("reason", reason)
        .with("fields", fields())
        .with("writes", sites)
        .with("tracks", tracks)
        .with(
            "track_completion",
            Json::object()
                .with("status", if complete { "exact" } else { "partial" })
                .with("fields", vec!["$2C(a4)", "$2E(a4)", "$30(a4)", "$34(a4)"])
                .with(
                    "reason",
                    "all statically recognized coordinate writes are retained per object; \
                     unresolved opcodes remain explicitly partial",
                ),
        )
        .with("runtime", runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(rom: &mut [u8], at: usize, words: &[u16]) {
        for (i, w) in words.iter().enumerate() {
            rom[at + i * 2..at + i * 2 + 2].copy_from_slice(&w.to_be_bytes());
        }
    }

    #[test]
    fn a_lunge_is_a_start_a_step_and_a_limit_on_one_axis() {
        let mut rom = vec![0u8; 0x40];
        // move.w #$E0,$2C(a4); subi.w #2,$2C(a4); cmpi.w #$C0,$2C(a4).
        put(
            &mut rom,
            0,
            &[
                0x397C, 0x00E0, 0x002C, 0x046C, 0x0002, 0x002C, 0x0C6C, 0x00C0, 0x002C,
            ],
        );
        let sites = coordinate_sites(&rom, 0, 0x30, 0x370);
        assert_eq!(sites.len(), 3);
        let sequence = Json::object().with("object_id", 0x370u16);
        let movement = serde_json::Value::from(&evidence(&[(0x370, sites)], Some(&sequence)));
        assert_eq!(movement["status"], "exact");
        assert_eq!(movement["runtime"]["kind"], "linear");
        assert_eq!(
            movement["runtime"]["initial_offset_pixels"],
            serde_json::json!([8, 0])
        );
        assert_eq!(
            movement["runtime"]["step_pixels"],
            serde_json::json!([-2, 0])
        );
    }

    #[test]
    fn no_sequence_is_deferred_and_other_objects_are_tracks() {
        let mut rom = vec![0u8; 0x20];
        put(&mut rom, 0, &[0x066C, 0x0004, 0x002E]);
        let sites = coordinate_sites(&rom, 0, 0x10, 0x44);
        let movement = serde_json::Value::from(&evidence(&[(0x44, sites)], None));
        assert_eq!(movement["status"], "deferred");
        assert!(movement["runtime"].is_null());
        assert_eq!(movement["tracks"][0]["writes"][0]["delta"], 4);
        assert_eq!(movement["tracks"][0]["writes"][0]["axis"], "y");
    }

    /// Pinned to the Python decoder, which reads the quick value from the
    /// effective-address bits: every `addq`/`subq.w #n,d16(a4)` decodes as
    /// `addq.w #6` (#410). Fixing it changes both builders together.
    #[test]
    fn addq_and_subq_decode_as_the_python_decoder_reads_them() {
        let mut rom = vec![0u8; 0x20];
        put(&mut rom, 0, &[0x5F6C, 0x002C, 0x536C, 0x002E]);
        let sites = coordinate_sites(&rom, 0, 0x10, 0x40);
        let sites: Vec<serde_json::Value> = sites.iter().map(serde_json::Value::from).collect();
        assert_eq!(sites.len(), 2);
        assert!(
            sites
                .iter()
                .all(|s| s["delta"] == 6 && s["operation"] == "addq.w")
        );
    }
}
