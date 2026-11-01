//! Warp trigger rectangles, `XYRangeJmpTbl`: `psiv_tools/warps.py`, which owns
//! the retail citations and why the rectangles are in collision-grid cells.

use crate::PackError;
use crate::json::Json;

const COLLISION_CELL_PIXELS: i32 = 16;
/// `GetChunkAndCollision`: `addi.w #$10,d6` before the shift down to a cell.
pub const STANDING_CELL_Y_OFFSET: i32 = 1;
/// `XYRange_XYLowerWithPlayerY` needs the player's Y strictly past $2A0.
const PLAYER_Y_FLOOR_CELL: i32 = 0x2A0 / COLLISION_CELL_PIXELS + 1 + STANDING_CELL_Y_OFFSET;

/// A half-open rectangle of collision cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn cells(self) -> impl Iterator<Item = (i32, i32)> {
        (self.y..self.y + self.height)
            .flat_map(move |y| (self.x..self.x + self.width).map(move |x| (x, y)))
    }

    pub fn to_json(self) -> Json {
        Json::object()
            .with("x", self.x)
            .with("y", self.y)
            .with("width", self.width)
            .with("height", self.height)
    }
}

fn out_of_table(range_id: u16) -> PackError {
    PackError(format!(
        "XYRange index {range_id} is outside the 15-entry jump table"
    ))
}

/// The eight box routines: their size in pixels, as `addi.w` spells it.
fn box_range(range_id: u16) -> Option<(&'static str, i32, i32)> {
    Some(match range_id {
        0x1 => ("XYPlus40", 0x40, 0x40),
        0x2 => ("XYPlus20", 0x20, 0x20),
        0x9 => ("XPlus20_YPlus10", 0x20, 0x10),
        0xA => ("XPlus10_YPlus60", 0x10, 0x60),
        0xB => ("XPlus40_YPlus20", 0x40, 0x20),
        0xC => ("XPlus10_YPlus20", 0x10, 0x20),
        0xD => ("XPlus60_YPlus10", 0x60, 0x10),
        0xE => ("XPlus40_YPlus10", 0x40, 0x10),
        _ => return None,
    })
}

/// `xy_range_name`.
pub fn xy_range_name(range_id: u16) -> Result<&'static str, PackError> {
    if let Some((name, _, _)) = box_range(range_id) {
        return Ok(name);
    }
    Ok(match range_id {
        0x0 => "Null",
        0x3 => "XYExact",
        0x4 => "XLower",
        0x5 => "XHigher",
        0x6 => "YLower",
        0x7 => "YHigher",
        0x8 => "XYLowerWithPlayerY",
        _ => return Err(out_of_table(range_id)),
    })
}

/// The cells a transition record covers, clipped to its map; `None` when it
/// can never fire. `x_byte` and `y_byte` are the record's bytes as stored.
pub fn warp_rect(
    range_id: u16,
    x_byte: i32,
    y_byte: i32,
    width_cells: i32,
    height_cells: i32,
) -> Result<Option<Rect>, PackError> {
    if width_cells <= 0 || height_cells <= 0 {
        return Err(PackError(format!(
            "Map is {width_cells}x{height_cells} cells"
        )));
    }
    let x = x_byte;
    let y = y_byte + STANDING_CELL_Y_OFFSET;
    let (x0, y0, x1, y1) = if let Some((_, pixels_x, pixels_y)) = box_range(range_id) {
        (
            x,
            y,
            x + pixels_x / COLLISION_CELL_PIXELS,
            y + pixels_y / COLLISION_CELL_PIXELS,
        )
    } else {
        match range_id {
            0x0 => return Ok(None),
            0x3 => (x, y, x + 1, y + 1),
            0x4 => (0, 0, x + 1, height_cells),
            0x5 => (x, 0, width_cells, height_cells),
            0x6 => (0, 0, width_cells, y + 1),
            0x7 => (0, y, width_cells, height_cells),
            0x8 => (0, PLAYER_Y_FLOOR_CELL, x + 1, y + 1),
            _ => return Err(out_of_table(range_id)),
        }
    };
    let (x0, y0) = (x0.max(0), y0.max(0));
    let (x1, y1) = (x1.min(width_cells), y1.min(height_cells));
    if x1 <= x0 || y1 <= y0 {
        return Ok(None);
    }
    Ok(Some(Rect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }))
}

/// An interaction area's 8-pixel tile words, resolved into map cells.
pub fn interaction_rect(
    range_id: u16,
    x_tile: i32,
    y_tile: i32,
    width_cells: i32,
    height_cells: i32,
) -> Result<Option<Rect>, PackError> {
    if x_tile % 2 != 0 || y_tile % 2 != 0 {
        return Err(PackError(format!(
            "interaction area coordinates must land on the 16-pixel collision grid: \
             ({x_tile}, {y_tile})"
        )));
    }
    warp_rect(range_id, x_tile / 2, y_tile / 2, width_cells, height_cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_is_clipped_to_the_map_and_shifted_one_row_down() {
        // XYPlus40 at (2, 2): four cells square from row 3, clipped at 5 wide.
        assert_eq!(
            warp_rect(1, 2, 2, 5, 20).unwrap(),
            Some(Rect {
                x: 2,
                y: 3,
                width: 3,
                height: 4
            })
        );
    }

    #[test]
    fn null_and_off_map_rectangles_never_fire() {
        assert_eq!(warp_rect(0, 1, 1, 5, 5).unwrap(), None);
        assert_eq!(warp_rect(3, 9, 9, 5, 5).unwrap(), None);
    }

    #[test]
    fn the_player_y_floor_starts_two_cells_past_2a0() {
        let rect = warp_rect(8, 3, 60, 10, 80).unwrap().unwrap();
        assert_eq!((rect.y, rect.height), (44, 18));
    }

    #[test]
    fn indexes_past_the_table_and_empty_maps_are_refused() {
        assert!(
            warp_rect(15, 0, 0, 5, 5)
                .unwrap_err()
                .0
                .contains("15-entry")
        );
        assert!(xy_range_name(15).is_err());
        assert_eq!(warp_rect(1, 0, 0, 0, 5).unwrap_err().0, "Map is 0x5 cells");
    }

    #[test]
    fn interaction_coordinates_must_be_on_the_collision_grid() {
        assert!(interaction_rect(3, 1, 0, 5, 5).is_err());
        assert_eq!(
            interaction_rect(3, 2, 2, 5, 5).unwrap(),
            warp_rect(3, 1, 1, 5, 5).unwrap()
        );
    }

    #[test]
    fn a_rect_walks_its_cells_row_by_row() {
        let rect = Rect {
            x: 1,
            y: 2,
            width: 2,
            height: 2,
        };
        assert_eq!(
            rect.cells().collect::<Vec<_>>(),
            vec![(1, 2), (2, 2), (1, 3), (2, 3)]
        );
    }
}
