//! Sparse cell-level patching and reverse diffing for 2D pack grids (#789).
//!
//! Retail maps store collision grids (`collision.rows`,
//! `layout_variants.*.collision.rows`) and raw chunk grids
//! (`vehicle_battle.rows`, `layout_variants.*.vehicle_battle.rows`) as dense 2D
//! row arrays, and `battle/formations.json` stores overworld encounter grids
//! (`position_grids.*.cells`) the same way.
//!
//! Instead of embedding an entire retail grid when a mod changes a few tiles,
//! sparse cell patches use:
//!
//! ```json
//! { "cells": [[row, col, value], ...] }
//! ```
//!
//! Hand-authored patches may also spell each entry as an explicit object
//! `{"row": r, "col": c, "value": v}`. Both forms validate bounds, integer
//! types, value domains, duplicate `(row, col)` targets, and unknown fields
//! fail-closed before mutating the grid.

use super::tables::{FilePattern, MAPS_JSON};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// Allowed value domain for one 2D grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellValueKind {
    /// 4-bit retail collision nibble (`0..=15`); base rows may be numeric
    /// arrays or hex-nibble strings (`Collision` serialization format).
    CollisionNibble,
    /// Unsigned 8-bit cell code (`0..=255`).
    U8,
    /// Unsigned 16-bit chunk/tile code (`0..=65535`).
    U16,
}

impl CellValueKind {
    fn max_value(self) -> u64 {
        match self {
            Self::CollisionNibble => 0xF,
            Self::U8 => u64::from(u8::MAX),
            Self::U16 => u64::from(u16::MAX),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::CollisionNibble => "4-bit collision code (0..=15)",
            Self::U8 => "u8 cell code (0..=255)",
            Self::U16 => "u16 chunk id (0..=65535)",
        }
    }

    fn allows_hex_rows(self) -> bool {
        matches!(self, Self::CollisionNibble)
    }
}

/// Declarative specification of one 2D grid that supports sparse cell patches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellGridSpec {
    pub file: FilePattern,
    pub path: &'static [&'static str],
    pub kind: CellValueKind,
}

/// Every 2D tile/cell grid in the runtime pack that supports sparse cell-level
/// patching and diffing.
pub(crate) const CELL_GRIDS: &[CellGridSpec] = &[
    CellGridSpec {
        file: MAPS_JSON,
        path: &["collision", "rows"],
        kind: CellValueKind::CollisionNibble,
    },
    CellGridSpec {
        file: MAPS_JSON,
        path: &["vehicle_battle", "rows"],
        kind: CellValueKind::U16,
    },
    CellGridSpec {
        file: MAPS_JSON,
        path: &["layout_variants", "*", "collision", "rows"],
        kind: CellValueKind::CollisionNibble,
    },
    CellGridSpec {
        file: MAPS_JSON,
        path: &["layout_variants", "*", "vehicle_battle", "rows"],
        kind: CellValueKind::U16,
    },
    CellGridSpec {
        file: FilePattern::Exact("battle/formations.json"),
        path: &["position_grids", "*", "cells"],
        kind: CellValueKind::U8,
    },
];

/// Returns the grid spec when `path` inside `rel_file` addresses a known 2D
/// cell/tile grid.
pub(crate) fn cell_grid_spec(rel_file: &str, path: &[&str]) -> Option<&'static CellGridSpec> {
    CELL_GRIDS
        .iter()
        .find(|spec| spec.file.matches(rel_file) && spec.path == path)
}

/// Applies either a sparse cell patch (`{"cells": [...]}`) or a full 2D row
/// replacement array onto `base_rows` in place.
pub(crate) fn apply_cell_grid_patch(
    base_rows: &mut Vec<Value>,
    patch: &Value,
    display_path: &[&str],
    kind: CellValueKind,
) -> Result<(), String> {
    let label = display_path.join(".");
    match patch {
        Value::Object(obj) => {
            for key in obj.keys() {
                if key != "cells" {
                    return Err(format!(
                        "unknown field `{label}.{key}` (sparse cell grid patch accepts only `cells`)"
                    ));
                }
            }
            let Some(cells_val) = obj.get("cells") else {
                return Err(format!(
                    "{label}: sparse cell grid patch object must contain `cells`"
                ));
            };
            let Some(entries) = cells_val.as_array() else {
                return Err(format!(
                    "{label}.cells: expected an array of `[row, col, value]` tuples or `{{\"row\", \"col\", \"value\"}}` objects"
                ));
            };

            let row_widths = measure_base_rows(base_rows, &label, kind)?;
            let height = row_widths.len();
            let mut seen = BTreeSet::new();
            let mut updates = Vec::with_capacity(entries.len());

            for (idx, entry) in entries.iter().enumerate() {
                let entry_path = format!("{label}.cells[{idx}]");
                let (row, col, value) = parse_cell_entry(entry, &entry_path, kind)?;
                if row >= height {
                    let width = row_widths.first().copied().unwrap_or(0);
                    return Err(format!(
                        "{entry_path}: cell (row={row}, col={col}) is out of bounds for {height}x{width} grid"
                    ));
                }
                let row_width = row_widths[row];
                if col >= row_width {
                    return Err(format!(
                        "{entry_path}: cell (row={row}, col={col}) is out of bounds for row {row} (width {row_width})"
                    ));
                }
                if !seen.insert((row, col)) {
                    return Err(format!(
                        "{entry_path}: duplicate cell target (row={row}, col={col})"
                    ));
                }
                updates.push((row, col, value));
            }

            // All entries validated; now apply atomically.
            for (row, col, value) in updates {
                write_grid_cell(&mut base_rows[row], col, value, kind)?;
            }
            Ok(())
        }
        Value::Array(replacement) => {
            // Full 2D row replacement (used by private `--allow-derived` exports):
            // still validate every cell type and domain fail-closed.
            decode_grid_rows(replacement, &label, kind)?;
            *base_rows = replacement.clone();
            Ok(())
        }
        _ => Err(format!(
            "{label}: expected sparse cell patch `{{\"cells\": [...]}}` or a 2D row array"
        )),
    }
}

/// Computes a compact sparse cell patch `{"cells": [[row, col, value], ...]}`
/// between two 2D grids of matching shape.
///
/// If grid dimensions changed, refuses in strict shareable mode
/// (`!allow_derived`), or emits the replacement array and records `label` in
/// `copied_base_arrays` when `allow_derived` is `true`.
pub(crate) fn diff_cell_grid(
    base_rows: &[Value],
    edited_rows: &[Value],
    display_path: &[&str],
    kind: CellValueKind,
    allow_derived: bool,
    copied_base_arrays: &mut Vec<String>,
) -> Result<Option<Value>, String> {
    let label = display_path.join(".");
    let base_grid = decode_grid_rows(base_rows, &label, kind)?;
    let edited_grid = decode_grid_rows(edited_rows, &label, kind)?;

    let same_shape = base_grid.len() == edited_grid.len()
        && base_grid
            .iter()
            .zip(edited_grid.iter())
            .all(|(b, e)| b.len() == e.len());

    if same_shape {
        let mut cells = Vec::new();
        for (row_idx, (base_row, edited_row)) in
            base_grid.iter().zip(edited_grid.iter()).enumerate()
        {
            for (col_idx, (&base_val, &edited_val)) in
                base_row.iter().zip(edited_row.iter()).enumerate()
            {
                if base_val != edited_val {
                    cells.push(json!([row_idx, col_idx, edited_val]));
                }
            }
        }
        if cells.is_empty() {
            return Ok(None);
        }
        return Ok(Some(json!({ "cells": cells })));
    }

    if !base_rows.is_empty() {
        if !allow_derived {
            let base_h = base_grid.len();
            let base_w = base_grid.first().map_or(0, Vec::len);
            let edited_h = edited_grid.len();
            let edited_w = edited_grid.first().map_or(0, Vec::len);
            return Err(format!(
                "refusing to export whole grid replacement `{label}` (dimensions changed from {base_h}x{base_w} to {edited_h}x{edited_w}): copying a base pack grid embeds cartridge-derived data into a shareable mod (re-run with `--allow-derived` for private local edits)"
            ));
        }
        copied_base_arrays.push(label);
    }
    Ok(Some(Value::Array(edited_rows.to_vec())))
}

fn measure_base_rows(
    base_rows: &[Value],
    label: &str,
    kind: CellValueKind,
) -> Result<Vec<usize>, String> {
    let mut widths = Vec::with_capacity(base_rows.len());
    for (row_idx, row) in base_rows.iter().enumerate() {
        match row {
            Value::Array(cols) => widths.push(cols.len()),
            Value::String(hex) if kind.allows_hex_rows() => {
                if !hex.is_ascii() {
                    return Err(format!("{label}[{row_idx}]: base hex row must be ASCII"));
                }
                widths.push(hex.len());
            }
            _ => {
                return Err(format!(
                    "{label}[{row_idx}]: base row is not a supported 2D grid row"
                ));
            }
        }
    }
    Ok(widths)
}

fn parse_cell_entry(
    entry: &Value,
    entry_path: &str,
    kind: CellValueKind,
) -> Result<(usize, usize, u16), String> {
    match entry {
        Value::Array(items) => {
            if items.len() != 3 {
                return Err(format!(
                    "{entry_path}: expected 3-element `[row, col, value]` tuple, got {} element(s)",
                    items.len()
                ));
            }
            let row = parse_index(&items[0], &format!("{entry_path}[0] (row)"))?;
            let col = parse_index(&items[1], &format!("{entry_path}[1] (col)"))?;
            let val = parse_cell_value(&items[2], &format!("{entry_path}[2] (value)"), kind)?;
            Ok((row, col, val))
        }
        Value::Object(map) => {
            for key in map.keys() {
                if !matches!(key.as_str(), "row" | "col" | "value") {
                    return Err(format!(
                        "unknown field `{entry_path}.{key}` (cell entry accepts only `row`, `col`, `value`)"
                    ));
                }
            }
            let row_val = map
                .get("row")
                .ok_or_else(|| format!("{entry_path}: missing required field `row`"))?;
            let col_val = map
                .get("col")
                .ok_or_else(|| format!("{entry_path}: missing required field `col`"))?;
            let val_val = map
                .get("value")
                .ok_or_else(|| format!("{entry_path}: missing required field `value`"))?;
            let row = parse_index(row_val, &format!("{entry_path}.row"))?;
            let col = parse_index(col_val, &format!("{entry_path}.col"))?;
            let val = parse_cell_value(val_val, &format!("{entry_path}.value"), kind)?;
            Ok((row, col, val))
        }
        _ => Err(format!(
            "{entry_path}: expected `[row, col, value]` tuple or `{{\"row\", \"col\", \"value\"}}` object"
        )),
    }
}

fn parse_index(value: &Value, field_path: &str) -> Result<usize, String> {
    let Some(n) = value.as_u64() else {
        return Err(format!(
            "{field_path}: expected non-negative integer index, got {value}"
        ));
    };
    usize::try_from(n).map_err(|_| format!("{field_path}: index {n} overflows usize"))
}

fn parse_cell_value(value: &Value, field_path: &str, kind: CellValueKind) -> Result<u16, String> {
    let Some(n) = value.as_u64() else {
        return Err(format!(
            "{field_path}: expected {}, got {value}",
            kind.label()
        ));
    };
    if n > kind.max_value() {
        return Err(format!(
            "{field_path}: value {n} out of range for {}",
            kind.label()
        ));
    }
    Ok(n as u16)
}

fn write_grid_cell(
    row: &mut Value,
    col: usize,
    value: u16,
    kind: CellValueKind,
) -> Result<(), String> {
    match row {
        Value::Array(cols) => {
            cols[col] = Value::from(value);
            Ok(())
        }
        Value::String(hex) if kind.allows_hex_rows() => {
            let nibble = char::from_digit(u32::from(value), 16)
                .ok_or_else(|| format!("value {value} is not a hex nibble"))?;
            let mut bytes = hex.as_bytes().to_vec();
            bytes[col] = nibble as u8;
            *hex = String::from_utf8(bytes).map_err(|e| format!("hex row UTF-8 error: {e}"))?;
            Ok(())
        }
        _ => Err("base row is not a supported 2D grid row".to_owned()),
    }
}

fn decode_grid_rows(
    rows: &[Value],
    label: &str,
    kind: CellValueKind,
) -> Result<Vec<Vec<u16>>, String> {
    let mut out = Vec::with_capacity(rows.len());
    for (row_idx, row) in rows.iter().enumerate() {
        match row {
            Value::Array(cols) => {
                let mut decoded = Vec::with_capacity(cols.len());
                for (col_idx, cell) in cols.iter().enumerate() {
                    let v =
                        parse_cell_value(cell, &format!("{label}[{row_idx}][{col_idx}]"), kind)?;
                    decoded.push(v);
                }
                out.push(decoded);
            }
            Value::String(hex) if kind.allows_hex_rows() => {
                let mut decoded = Vec::with_capacity(hex.len());
                for (col_idx, ch) in hex.chars().enumerate() {
                    let digit = ch.to_digit(16).ok_or_else(|| {
                        format!("{label}[{row_idx}][{col_idx}]: {ch:?} is not a hex digit")
                    })?;
                    decoded.push(digit as u16);
                }
                out.push(decoded);
            }
            _ => {
                return Err(format!(
                    "{label}[{row_idx}]: expected a row of {}",
                    kind.label()
                ));
            }
        }
    }
    Ok(out)
}
