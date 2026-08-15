//! MapDataManager consumption: evaluating a map's flag-gated patches at
//! build time, exactly when the cartridge does (`docs/MAP_EFFECTS.md` —
//! load-time only, never re-evaluated on a flag change).
//!
//! [`evaluate`] is pure: record + flag state in, an [`EffectOutcome`] out.
//! The runtime applies the outcome while constructing the [`FieldMap`], so
//! a patched map is patched from its first tick and unpatched state never
//! exists to leak.

use std::collections::BTreeMap;

use psiv_core::{Flag, GameState};
use psiv_data::{EffectGate, MapRecord};

/// What a map's effect list does to this build, given the current flags.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectOutcome {
    /// Record object indices to despawn (build with `active = false`).
    pub despawns: Vec<usize>,
    /// Record object indices whose object id changes, with the new id.
    /// The engine applies the id; the renderer's sprite for the slot is a
    /// known gap until sprites are looked up by live id (logged, filed).
    pub rewrites: Vec<(usize, u16)>,
    /// Dialogue-id overrides by record object index. The renderer must
    /// consult these before the record's own `dialogue_id`.
    pub dialogue_overrides: BTreeMap<usize, u16>,
    /// Which layout variant replaces the base layout, as an index into
    /// `record.layout_variants` — from an active `layout_replace` whose
    /// source matches the variant's changed plane.
    pub variant: Option<usize>,
    /// Per-cell collision patches from active, collision-authoritative
    /// `layout_write`s, applied to the (possibly variant) grid:
    /// `(x, y, collision_type)`.
    pub cell_patches: Vec<(u32, u32, u8)>,
    /// Patch tiles the renderer blits over the baked map PNG for active
    /// `layout_write`s: `(chunk_x, chunk_y, atlas_index)`. Emitted for
    /// picture-only writes too — the picture is the point there.
    pub patch_blits: Vec<(u32, u32, u32)>,
    /// Active `layout_write`s the pack has not yet resolved into cells —
    /// gameplay-affecting and *unapplied*. Must be surfaced, never silent.
    pub unresolved_layout_writes: usize,
    /// Entries the decoder could not decode. The map may be incompletely
    /// patched; surfaced for the log.
    pub undecoded_entries: usize,
    /// Writes skipped because their object index is past this map's object
    /// count — the census's defensive-code cases, skipped by design.
    pub out_of_range_objects: usize,
    /// Gates on banks this consumer does not recognise. A non-empty list is
    /// a schema drift and the map build should fail.
    pub unknown_banks: Vec<String>,
    /// Flags cleared by active `flag_clear` writes during the walk — the
    /// destination-load clears tape 18 measured (the Xanafalgue respawn's
    /// mechanism). Applied in walk order so later gates see them.
    pub flags_cleared: Vec<Flag>,
}

fn gate_holds(gate: &EffectGate, game: &GameState, unknown: &mut Vec<String>) -> bool {
    let flag = match gate.bank.as_str() {
        "event_flags" => Flag::event(gate.flag),
        // The $F140 door: the corrected flag model identifies it as the
        // TEMP bank (SOURCE_NOTES, FLAG MODEL FINAL). The pack's historical
        // label was "chest_flags" (the clone's fiction); the renamed
        // emission says "temp_flags". Both accepted so either side can
        // deploy first; mapping either to Flag::chest would read $F120 —
        // the silent-wrong-bank class apply_map_load was caught with.
        "chest_flags" | "temp_flags" => Flag::temp(gate.flag),
        other => {
            unknown.push(other.to_owned());
            return false;
        }
    };
    match gate.required.as_str() {
        "set" => game.is_set(flag),
        "clear" => game.is_clear(flag),
        other => {
            unknown.push(format!("required={other}"));
            false
        }
    }
}

/// Evaluates a record's effect list, walking entries in list order exactly
/// as `MapDataManager` does — which means `flag_clear` writes MUTATE the
/// state mid-walk, so a later entry's gates see the cleared flag. This is
/// why the parameter is `&mut`: the cartridge's dispatcher is stateful and a
/// pure model would evaluate later gates against pre-clear state.
#[must_use]
pub fn evaluate(record: &MapRecord, game: &mut GameState) -> EffectOutcome {
    let mut out = EffectOutcome::default();
    let object_count = record.npcs.len();

    'entries: for entry in &record.map_effects {
        if !entry.decoded {
            out.undecoded_entries += 1;
            continue;
        }
        for path in &entry.paths {
            let holds = path.unconditional
                || path
                    .gates
                    .iter()
                    .all(|g| gate_holds(g, game, &mut out.unknown_banks));
            if !holds {
                continue;
            }
            for write in &path.writes {
                match write.kind.as_str() {
                    "object_despawn" => match write.object_index {
                        Some(i) if (i as usize) < object_count => {
                            out.despawns.push(i as usize);
                        }
                        _ => out.out_of_range_objects += 1,
                    },
                    "object_rewrite" => {
                        if let (Some(i), Some(id)) = (write.object_index, write.object_id) {
                            if (i as usize) < object_count {
                                out.rewrites.push((i as usize, id));
                            } else {
                                out.out_of_range_objects += 1;
                            }
                        }
                    }
                    "object_dialogue" => {
                        if let (Some(i), Some(id)) = (write.object_index, write.dialogue_id) {
                            if (i as usize) < object_count {
                                out.dialogue_overrides.insert(i as usize, id);
                            } else {
                                out.out_of_range_objects += 1;
                            }
                        }
                    }
                    "layout_write" => {
                        if write.cells.is_empty() {
                            out.unresolved_layout_writes += 1;
                        } else {
                            // Cells reach the grid only when this write's
                            // plane is the one collision reads on this map;
                            // a picture-only write still blits its tile.
                            if write.collision_authoritative != Some(false) {
                                for cell in &write.cells {
                                    out.cell_patches.push((cell.x, cell.y, cell.collision));
                                }
                            }
                            if let (Some(tile), Some(cx), Some(cy)) = (
                                write.patch_tile,
                                write.cell_x.map(|x| x / 2),
                                write.cell_y.map(|y| y / 2),
                            ) {
                                out.patch_blits.push((cx, cy, tile));
                            }
                        }
                    }
                    "flag_clear" => match (write.bank.as_deref(), write.flag) {
                        (Some(bank @ ("chest_flags" | "temp_flags")), Some(id)) => {
                            let _ = bank;
                            let flag = Flag::temp(id);
                            if game.is_set(flag) {
                                let _ = game.clear(flag);
                                out.flags_cleared.push(flag);
                            }
                        }
                        (Some("event_flags"), Some(id)) => {
                            let flag = Flag::event(id);
                            if game.is_set(flag) {
                                let _ = game.clear(flag);
                                out.flags_cleared.push(flag);
                            }
                        }
                        other => out.unknown_banks.push(format!("flag_clear {other:?}")),
                    },
                    "layout_replace" => {
                        // Match the write's source to the variant whose
                        // changed plane came from it.
                        let found = record.layout_variants.iter().position(|v| {
                            v.planes
                                .iter()
                                .any(|p| !p.identical_to_base && p.source == write.source)
                        });
                        match found {
                            Some(index) => out.variant = Some(index),
                            None => out.unknown_banks.push(format!(
                                "layout_replace source {:?} has no variant",
                                write.source
                            )),
                        }
                    }
                    other => out.unknown_banks.push(format!("write kind {other}")),
                }
            }
            if path.aborts_remaining_entries {
                // Dormant in retail, carried per the extraction contract.
                break 'entries;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use psiv_data::{EffectPath, EffectWrite, MapEffect};

    fn write(kind: &str, index: u32) -> EffectWrite {
        EffectWrite {
            kind: kind.to_owned(),
            at: None,
            object_index: Some(index),
            object_id: Some(56),
            dialogue_id: Some(89),
            cell_x: None,
            cell_y: None,
            chunk_id: None,
            plane: None,
            source: None,
            cells: Vec::new(),
            collision_authoritative: None,
            patch_tile: None,
            bank: None,
            flag: None,
        }
    }

    fn gated_effect(flag: u16, required: &str, writes: Vec<EffectWrite>) -> MapEffect {
        MapEffect {
            entry: 1,
            decoded: true,
            kinds: Vec::new(),
            paths: vec![EffectPath {
                gates: vec![EffectGate {
                    bank: "event_flags".into(),
                    flag,
                    required: required.into(),
                    symbol: None,
                }],
                unconditional: false,
                aborts_remaining_entries: false,
                deferred: Vec::new(),
                writes,
            }],
            reason: None,
        }
    }

    fn record_with(effects: Vec<MapEffect>, npcs: usize) -> MapRecord {
        let data = psiv_data::GameData::load(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../runtime-pack"
        )))
        .expect("pack loads");
        let mut record = data
            .map(psiv_data::MapId(0x013))
            .expect("fixture map")
            .clone();
        record.npcs.truncate(npcs);
        record.map_effects = effects;
        record.layout_variants = Vec::new();
        record
    }

    #[test]
    fn a_set_gate_applies_only_when_the_flag_is_set() {
        if !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../runtime-pack/manifest.json"
        ))
        .is_file()
        {
            eprintln!("pack absent; skipping");
            return;
        }
        let record = record_with(
            vec![gated_effect(0x33, "set", vec![write("object_despawn", 2)])],
            5,
        );
        let mut game = GameState::new();
        assert_eq!(evaluate(&record, &mut game).despawns, Vec::<usize>::new());
        game.set(Flag::event(0x33)).unwrap();
        assert_eq!(evaluate(&record, &mut game).despawns, vec![2]);
    }

    #[test]
    fn out_of_range_objects_are_counted_not_applied() {
        if !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../runtime-pack/manifest.json"
        ))
        .is_file()
        {
            eprintln!("pack absent; skipping");
            return;
        }
        let record = record_with(
            vec![gated_effect(
                0x33,
                "clear",
                vec![write("object_despawn", 40)],
            )],
            5,
        );
        let out = evaluate(&record, &mut GameState::new());
        assert!(out.despawns.is_empty());
        assert_eq!(out.out_of_range_objects, 1);
    }

    #[test]
    fn an_unknown_bank_is_loud() {
        if !std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../runtime-pack/manifest.json"
        ))
        .is_file()
        {
            eprintln!("pack absent; skipping");
            return;
        }
        let mut effect = gated_effect(1, "set", vec![write("object_despawn", 0)]);
        effect.paths[0].gates[0].bank = "town_flags".into();
        let record = record_with(vec![effect], 5);
        let out = evaluate(&record, &mut GameState::new());
        assert_eq!(out.unknown_banks, vec!["town_flags".to_string()]);
    }
}

#[cfg(test)]
mod door_regression {
    use super::*;
    use psiv_core::Flag;

    /// The doorway-into-the-void regression: map $012 carries a decoded
    /// flag_clear entry, and a consumer that treats the kind as schema
    /// drift refuses the whole map — the party warps into nothing. Builds
    /// the real record and asserts the walk both applies the gated clear
    /// and leaves the map buildable.
    #[test]
    fn map_012_with_its_flag_clear_entry_still_builds() {
        let pack = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
        if !pack.join("manifest.json").is_file() {
            eprintln!("pack absent; skipping");
            return;
        }
        let data = psiv_data::GameData::load(pack).expect("pack loads");
        let record = data.map(psiv_data::MapId(0x012)).expect("map $012");

        // Gate: entry $18 clears temp $13 only while event $0B is clear.
        let mut game = GameState::new();
        game.set(Flag::temp(0x13)).unwrap();
        let out = evaluate(record, &mut game);
        assert!(
            out.unknown_banks.is_empty(),
            "flag_clear must be understood"
        );
        assert_eq!(out.flags_cleared, vec![Flag::temp(0x13)]);
        assert!(
            game.is_clear(Flag::temp(0x13)),
            "the Xanafalgue respawn clear"
        );
        crate::field_map_patched(record, Some(&out)).expect("the map builds");

        // With the gate unsatisfied the clear must NOT run.
        let mut game = GameState::new();
        game.set(Flag::event(0x0B)).unwrap();
        game.set(Flag::temp(0x13)).unwrap();
        let out = evaluate(record, &mut game);
        assert!(out.flags_cleared.is_empty());
        assert!(game.is_set(Flag::temp(0x13)));
        crate::field_map_patched(record, Some(&out)).expect("still builds");
    }
}
