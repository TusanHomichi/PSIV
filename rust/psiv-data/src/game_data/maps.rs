//! One map record's own consistency: its numbers against each other, and
//! everything it places inside its own bounds.
//!
//! Nothing here compares a record with another record; that is `pack`'s job, and
//! the page hooks are `overworld`'s.
//!
//! ---------------------------------------------------------------------------
//! Validation
//!
//! Always on, no lenient mode. A runtime that tolerates a broken pack stops
//! being a test oracle for the cartridge (docs/RUNTIME_DESIGN.md, "Goal").
//! Every failure names the map and the field, down to the array index.
//! ---------------------------------------------------------------------------

use super::overworld::validate_overworld_patches;
use crate::collision::Collision;
use crate::error::DataError;
use crate::ids::MapId;
use crate::map::{
    COLLISION_CELL_PIXELS, CellPos, ContentsType, DIALOGUE_TREE_COUNT, Dimensions, MapRecord,
    Treasure, Warp,
};

pub(super) fn validate_map(record: &MapRecord) -> Result<(), DataError> {
    let id = record.id;
    let dims = &record.dimensions;

    validate_dimensions(id, dims)?;
    validate_collision(id, dims, &record.collision)?;
    validate_overworld_patches(record)?;

    // The trees are numbered from one, so zero is as wrong as forty-four.
    if record.dialogue_tree == 0 || record.dialogue_tree > DIALOGUE_TREE_COUNT {
        return Err(DataError::validation(
            id,
            "dialogue_tree",
            format!(
                "tree {} does not exist; the ROM holds {DIALOGUE_TREE_COUNT} Kosinski dialogue \
                 trees, numbered 1..={DIALOGUE_TREE_COUNT}",
                record.dialogue_tree
            ),
        ));
    }

    for (index, npc) in record.npcs.iter().enumerate() {
        check_in_bounds(id, dims, npc.pos(), &format!("npcs[{index}]"))?;
    }

    for (index, chest) in record.treasure_chests.iter().enumerate() {
        check_in_bounds(id, dims, chest.pos(), &format!("treasure_chests[{index}]"))?;
        validate_treasure_contents(id, index, chest)?;
    }

    for (index, warp) in record.warps.iter().enumerate() {
        validate_warp_geometry(id, dims, index, warp)?;
    }

    for (index, area) in record.interaction_areas.iter().enumerate() {
        validate_interaction_geometry(id, dims, index, area)?;
    }

    Ok(())
}

pub(super) fn validate_dimensions(id: MapId, dims: &Dimensions) -> Result<(), DataError> {
    if dims.width_cells == 0 || dims.height_cells == 0 {
        return Err(DataError::validation(
            id,
            "dimensions",
            format!(
                "a map cannot be {}x{} cells",
                dims.width_cells, dims.height_cells
            ),
        ));
    }
    if dims.cell_pixels != COLLISION_CELL_PIXELS {
        return Err(DataError::validation(
            id,
            "dimensions.cell_pixels",
            format!(
                "collision cells are {COLLISION_CELL_PIXELS} pixels, not {}; the whole fidelity \
                 spine is built on the 16-pixel cell grid",
                dims.cell_pixels
            ),
        ));
    }
    // A chunk is 4x4 tiles and a cell is 2x2, so the three ways the pack states
    // a map's size are one fact. Any disagreement means the packer's layout and
    // its collision decode came apart.
    for (axis, cells, chunks, pixels) in [
        (
            "width",
            dims.width_cells,
            dims.width_chunks,
            dims.width_pixels,
        ),
        (
            "height",
            dims.height_cells,
            dims.height_chunks,
            dims.height_pixels,
        ),
    ] {
        let from_chunks = chunks.saturating_mul(2);
        if cells != from_chunks {
            return Err(DataError::validation(
                id,
                format!("dimensions.{axis}_cells"),
                format!("{chunks} chunks is {from_chunks} cells, but the pack says {cells}"),
            ));
        }
        let expected = cells.saturating_mul(dims.cell_pixels);
        if pixels != expected {
            return Err(DataError::validation(
                id,
                format!("dimensions.{axis}_pixels"),
                format!(
                    "{cells} cells at {} pixels is {expected} pixels, but the pack says {pixels}",
                    dims.cell_pixels
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_collision(
    id: MapId,
    dims: &Dimensions,
    section: &Collision,
) -> Result<(), DataError> {
    let grid = &section.grid;
    if grid.width() != dims.width_cells || grid.height() != dims.height_cells {
        return Err(DataError::validation(
            id,
            "collision",
            format!(
                "the grid is {}x{} cells but the map declares {}x{}",
                grid.width(),
                grid.height(),
                dims.width_cells,
                dims.height_cells
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_treasure_contents(
    id: MapId,
    index: usize,
    chest: &Treasure,
) -> Result<(), DataError> {
    // Byte 1 of the chest record selects how byte 3 is read. If the pack says
    // "item" and carries a meseta amount, one of the two is a lie and we cannot
    // tell which.
    let (present, absent, expected) = match chest.contents_type {
        ContentsType::Item => (chest.item_id.is_some(), chest.meseta.is_some(), "item_id"),
        ContentsType::Meseta => (chest.meseta.is_some(), chest.item_id.is_some(), "meseta"),
    };
    if !present {
        return Err(DataError::validation(
            id,
            format!("treasure_chests[{index}].{expected}"),
            format!(
                "contents_type is {:?} but {expected} is null",
                chest.contents_type
            ),
        ));
    }
    if absent {
        return Err(DataError::validation(
            id,
            format!("treasure_chests[{index}]"),
            format!(
                "contents_type is {:?} but both item_id and meseta are set",
                chest.contents_type
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_warp_geometry(
    id: MapId,
    dims: &Dimensions,
    index: usize,
    warp: &Warp,
) -> Result<(), DataError> {
    // `rect` is the trigger area and is the thing that has to be on the map;
    // the packer clips it. `source` is the raw record coordinate, which the
    // standing-cell shift can push one row past the bottom edge, so it is not
    // bounds-checked -- see the module note on `WarpSource`.
    let Some(rect) = warp.rect else {
        return Ok(());
    };

    if rect.width == 0 || rect.height == 0 {
        return Err(DataError::validation(
            id,
            format!("warps[{index}].rect"),
            format!(
                "a {}x{} trigger rectangle can never fire; the packer emits no rectangle at all \
                 for an empty area",
                rect.width, rect.height
            ),
        ));
    }
    let end = rect.end();
    if end.x > dims.width_cells || end.y > dims.height_cells {
        return Err(DataError::validation(
            id,
            format!("warps[{index}].rect"),
            format!(
                "cells {}..{} x {}..{} run outside the {}x{} map; the packer must clip \
                 open-ended XYRange entries such as {} to the map",
                rect.x, end.x, rect.y, end.y, dims.width_cells, dims.height_cells, warp.range.name
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_interaction_geometry(
    id: MapId,
    dims: &Dimensions,
    index: usize,
    area: &crate::map::InteractionArea,
) -> Result<(), DataError> {
    let Some(rect) = area.rect else {
        return Ok(());
    };
    if rect.width == 0 || rect.height == 0 {
        return Err(DataError::validation(
            id,
            format!("interaction_areas[{index}].rect"),
            "an interaction area cannot have zero width or height",
        ));
    }
    let end = rect.end();
    if end.x > dims.width_cells || end.y > dims.height_cells {
        return Err(DataError::validation(
            id,
            format!("interaction_areas[{index}].rect"),
            format!(
                "cells {}..{} x {}..{} run outside the {}x{} map",
                rect.x, end.x, rect.y, end.y, dims.width_cells, dims.height_cells
            ),
        ));
    }
    Ok(())
}

pub(super) fn check_in_bounds(
    id: MapId,
    dims: &Dimensions,
    pos: CellPos,
    field: &str,
) -> Result<(), DataError> {
    if pos.x >= dims.width_cells || pos.y >= dims.height_cells {
        return Err(DataError::validation(
            id,
            field.to_string(),
            format!(
                "cell ({}, {}) is outside the {}x{} map",
                pos.x, pos.y, dims.width_cells, dims.height_cells
            ),
        ));
    }
    Ok(())
}
