//! The paged overworld's page hooks, checked against the tiles they resolve into.
//!
//! A full-map build composes each flag's final chunks from the raw hooks, so the two
//! have to agree: every resolved tile needs a raw write behind it, two flags may not
//! edit one chunk, and the composed atlas entry has to hold the plane pair the
//! resolution claims. `MapDataManager`'s own writes are `crate::map`'s schema and
//! nothing here re-checks them.

use crate::collision::Plane;
use crate::error::DataError;
use crate::map::MapRecord;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_overworld_patches(record: &MapRecord) -> Result<(), DataError> {
    let id = record.id;
    let fail = |field: String, why: String| DataError::validation(id, field, why);
    if record.layout_patches.is_empty() {
        if matches!(id.0, 0 | 1) {
            return Err(fail(
                "layout_patches".into(),
                "retail overworld page hooks are missing; rebuild the pack".into(),
            ));
        }
        if record.overworld_patches.is_some() {
            return Err(fail(
                "overworld_patches".into(),
                "resolved page hooks exist without raw layout_patches".into(),
            ));
        }
        return Ok(());
    }
    let resolved = record.overworld_patches.as_ref().ok_or_else(|| {
        fail(
            "overworld_patches".into(),
            "raw page hooks need resolved tiles; rebuild this older runtime pack".into(),
        )
    })?;
    let atlas = record.patch_tiles.as_ref().ok_or_else(|| {
        fail(
            "patch_tiles".into(),
            "overworld page hooks need a composed atlas; rebuild the pack".into(),
        )
    })?;
    if record.png_over.is_none() || atlas.png_over.is_none() {
        return Err(fail(
            "patch_tiles.png_over".into(),
            "overworld patch must replace the priority overlay too".into(),
        ));
    }
    if atlas.tile_pixels != 32 || atlas.count as usize != atlas.tiles.len() {
        return Err(fail(
            "patch_tiles".into(),
            "invalid 32px atlas geometry".into(),
        ));
    }

    // The full-map atlas can precompose a flag's final FG/BG pair only when
    // distinct flags do not edit the same chunk. Repeated writes by one flag
    // retain the raw page-hook order; their final value is validated below.
    let mut owner = BTreeMap::<(u32, u32), u16>::new();
    let mut writes = BTreeMap::<(u16, u32, u32), (Option<u16>, Option<u16>)>::new();
    for (index, patch) in record.layout_patches.iter().enumerate() {
        let field = format!("layout_patches[{index}]");
        let flag = patch.event_flag.id;
        if flag >= 512 {
            return Err(fail(
                format!("{field}.event_flag.id"),
                format!("{flag} is outside event flags"),
            ));
        }
        let hex = patch
            .event_flag
            .id_hex
            .strip_prefix("0x")
            .and_then(|s| u16::from_str_radix(s, 16).ok());
        if hex != Some(flag) {
            return Err(fail(
                format!("{field}.event_flag.id_hex"),
                "hex flag id disagrees with numeric id".into(),
            ));
        }
        if patch.triggered_by_plane != "fg" && patch.triggered_by_plane != "bg" || patch.page >= 16
        {
            return Err(fail(
                field.clone(),
                "invalid page hook plane or page".into(),
            ));
        }
        if patch.writes.is_empty() {
            return Err(fail(format!("{field}.writes"), "empty page hook".into()));
        }
        for (write_index, write) in patch.writes.iter().enumerate() {
            let place = format!("{field}.writes[{write_index}]");
            if write.plane != "fg" && write.plane != "bg" {
                return Err(fail(format!("{place}.plane"), "expected fg or bg".into()));
            }
            if write.cell_x != write.chunk_x.saturating_mul(2)
                || write.cell_y != write.chunk_y.saturating_mul(2)
                || write.chunk_ids.is_empty()
            {
                return Err(fail(
                    place.clone(),
                    "cell/chunk geometry or chunk list is invalid".into(),
                ));
            }
            for (step, raw) in write.chunk_ids.iter().enumerate() {
                let x = write
                    .chunk_x
                    .checked_add(step as u32)
                    .ok_or_else(|| fail(place.clone(), "chunk coordinate overflow".into()))?;
                let y = write.chunk_y;
                if x >= record.dimensions.width_chunks || y >= record.dimensions.height_chunks {
                    return Err(fail(
                        place.clone(),
                        format!("chunk ({x},{y}) is outside the map"),
                    ));
                }
                let chunk = raw
                    .strip_prefix("0x")
                    .and_then(|s| u8::from_str_radix(s, 16).ok())
                    .ok_or_else(|| {
                        fail(
                            format!("{place}.chunk_ids[{step}]"),
                            format!("{raw:?} is not a raw chunk byte"),
                        )
                    })?;
                if let Some(previous) = owner.insert((x, y), flag)
                    && previous != flag
                {
                    return Err(fail(
                        place.clone(),
                        format!("chunk ({x},{y}) has overlapping event flags {previous}/{flag}"),
                    ));
                }
                let pair = writes.entry((flag, x, y)).or_default();
                if write.plane == "fg" {
                    pair.0 = Some(u16::from(chunk));
                } else {
                    pair.1 = Some(u16::from(chunk));
                }
            }
        }
    }

    let mut seen_flags = BTreeSet::new();
    let mut seen_tiles = BTreeSet::new();
    for (index, patch) in resolved.iter().enumerate() {
        let field = format!("overworld_patches[{index}]");
        if !seen_flags.insert(patch.event_flag) || patch.tiles.is_empty() {
            return Err(fail(
                field.clone(),
                "duplicate flag or empty tile group".into(),
            ));
        }
        for (tile_index, tile) in patch.tiles.iter().enumerate() {
            let place = format!("{field}.tiles[{tile_index}]");
            let key = (patch.event_flag, tile.chunk_x, tile.chunk_y);
            let Some((written_fg, written_bg)) = writes.get(&key) else {
                return Err(fail(place, "no matching raw page-hook write".into()));
            };
            if !seen_tiles.insert(key)
                || written_fg.is_some_and(|chunk| chunk != tile.fg_chunk_id)
                || written_bg.is_some_and(|chunk| chunk != tile.bg_chunk_id)
            {
                return Err(fail(
                    place.clone(),
                    "duplicate tile or final plane disagrees with raw writes".into(),
                ));
            }
            let authoritative = match record.collision.plane {
                Plane::Fg => tile.fg_chunk_id,
                Plane::Bg => tile.bg_chunk_id,
            };
            if tile.collision_chunk_id != authoritative || tile.collision.iter().any(|v| *v > 15) {
                return Err(fail(
                    place.clone(),
                    "collision plane/chunk/cells disagree".into(),
                ));
            }
            let unchanged_authority = match record.collision.plane {
                Plane::Fg => written_fg.is_none(),
                Plane::Bg => written_bg.is_none(),
            };
            if unchanged_authority {
                let base = record
                    .vehicle_battle
                    .as_ref()
                    .and_then(|grid| grid.rows.get(tile.chunk_y as usize))
                    .and_then(|row| row.get(tile.chunk_x as usize));
                if base != Some(&authoritative) {
                    return Err(fail(
                        place.clone(),
                        "unchanged collision plane disagrees with base chunk grid".into(),
                    ));
                }
            }
            let atlas_tile = atlas.tiles.get(tile.patch_tile as usize).ok_or_else(|| {
                fail(
                    format!("{place}.patch_tile"),
                    "atlas index is outside its tiles".into(),
                )
            })?;
            if atlas_tile.index != tile.patch_tile
                || atlas_tile.x != tile.patch_tile.saturating_mul(32)
                || atlas_tile.chunk_id.is_some()
                || atlas_tile.fg_chunk_id != Some(tile.fg_chunk_id)
                || atlas_tile.bg_chunk_id != Some(tile.bg_chunk_id)
                || atlas_tile.collision != Some(tile.collision)
            {
                return Err(fail(
                    place,
                    "composed atlas tile disagrees with patch".into(),
                ));
            }
        }
    }
    if seen_tiles.len() != writes.len() {
        return Err(fail(
            "overworld_patches".into(),
            "resolved tile set does not cover every raw page-hook coordinate".into(),
        ));
    }
    Ok(())
}
