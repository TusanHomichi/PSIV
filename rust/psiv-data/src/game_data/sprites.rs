//! The sprite index files, and the sheets a map record may point at.
//!
//! `sprites/party.json`, `sprites/npcs.json` and the optional
//! `sprites/vehicles.json` are one shape: a declared count, a list of sheets and the
//! vehicle palette variants. A sheet whose geometry or frame references cannot be
//! drawn stops the load, and so does a chest, NPC or palette replacement naming a
//! sheet or a sequence that is not there.

use super::GameData;
use crate::error::DataError;

pub(super) fn validate_sheet(sheet: &crate::sprites::Sheet) -> Result<(), DataError> {
    let fail = |message: String| DataError::Sprite {
        who: sheet.id.clone(),
        message,
    };
    if sheet.frame_width == 0 || sheet.frame_height == 0 || sheet.frame_count == 0 {
        return Err(fail(format!(
            "degenerate geometry {}x{} x{} frames",
            sheet.frame_width, sheet.frame_height, sheet.frame_count
        )));
    }
    if sheet.sequences.is_empty() {
        return Err(fail("no sequences".into()));
    }
    for (name, sequence) in &sheet.sequences {
        if sequence.frames.is_empty() {
            return Err(fail(format!("sequence {name} has no frames")));
        }
        for frame in &sequence.frames {
            if frame.index >= sheet.frame_count {
                return Err(fail(format!(
                    "sequence {name} names frame {} of a {}-frame strip",
                    frame.index, sheet.frame_count
                )));
            }
            if frame.duration_ticks == 0 {
                return Err(fail(format!("sequence {name} holds a frame for 0 ticks")));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_sprite_refs(data: &GameData) -> Result<(), DataError> {
    for (id, record) in &data.maps {
        for palette in record
            .map_effects
            .iter()
            .flat_map(|effect| &effect.paths)
            .flat_map(|path| &path.deferred_effects)
        {
            let reject = |message| DataError::Sprite {
                who: format!("map {id} palette copy"),
                message,
            };
            if palette.resolved_as != "palette_write" {
                return Err(reject(format!(
                    "unknown resolved effect {}",
                    palette.resolved_as
                )));
            }
            for replacement in &palette.affects.npc_sheets {
                let sprite = record
                    .npcs
                    .get(replacement.npc_index)
                    .and_then(|npc| npc.sprite.as_ref())
                    .ok_or_else(|| {
                        reject(format!("missing sprite for NPC {}", replacement.npc_index))
                    })?;
                if sprite.sheet != replacement.from {
                    return Err(reject(format!(
                        "NPC {} palette source disagrees with its base sheet",
                        replacement.npc_index
                    )));
                }
                let sheet = data
                    .sheets
                    .get(&replacement.to)
                    .ok_or_else(|| reject(format!("missing palette sheet {}", replacement.to)))?;
                for sequence in [&sprite.idle_sequence, &sprite.walk_sequence] {
                    if sheet.sequence(sequence).is_none() {
                        return Err(reject(format!(
                            "palette sheet {} lacks {sequence}",
                            replacement.to
                        )));
                    }
                }
            }
        }
        for chest in &record.treasure_chests {
            // Older packs did not extract chest art. Present references must
            // provide both lids; silently drawing frame zero hides open state.
            if let Some(sprite) = &chest.sprite {
                let who = format!("map {id} chest {}", chest.index);
                let sheet = data
                    .sheets
                    .get(&sprite.sheet)
                    .ok_or_else(|| DataError::Sprite {
                        who: who.clone(),
                        message: format!("missing chest sheet {}", sprite.sheet),
                    })?;
                for sequence in ["idle_down", "idle_up"] {
                    if sheet.sequence(sequence).is_none() {
                        return Err(DataError::Sprite {
                            who,
                            message: format!("missing chest lid {sequence}"),
                        });
                    }
                }
            }
        }
        for npc in &record.npcs {
            let who = format!("map {id} npc {}", npc.index);
            match (&npc.sprite, &npc.sprite_reason) {
                (Some(sprite), None) => {
                    let Some(sheet) = data.sheets.get(&sprite.sheet) else {
                        return Err(DataError::Sprite {
                            who,
                            message: format!("references missing sheet {}", sprite.sheet),
                        });
                    };
                    for sequence in [&sprite.idle_sequence, &sprite.walk_sequence] {
                        if sheet.sequence(sequence).is_none() {
                            return Err(DataError::Sprite {
                                who,
                                message: format!(
                                    "sheet {} has no sequence {sequence}",
                                    sprite.sheet
                                ),
                            });
                        }
                    }
                }
                (None, Some(_)) => {}
                (Some(_), Some(_)) | (None, None) => {
                    return Err(DataError::Sprite {
                        who,
                        message: "exactly one of sprite / sprite_reason must be set".into(),
                    });
                }
            }
        }
    }
    Ok(())
}
