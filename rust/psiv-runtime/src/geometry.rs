//! Map and camera geometry shared by the runtime shell and save seam.

use psiv_core::{
    Camera, CameraBounds, CameraEdges, CameraGates, Driver, FieldMap, FieldState, Npc, ONE_PIXEL,
    PixelPos, Topology, Wanderer,
};
use psiv_data::MapRecord;

/// The camera bounds a map imposes: its pixel extent, and whether the edges
/// clamp or wrap.
pub(super) fn bounds_of(map: &FieldMap) -> CameraBounds {
    CameraBounds::from_cells(
        map.width(),
        map.height(),
        if map.topology() == Topology::Torus {
            CameraEdges::Wrapping
        } else {
            CameraEdges::Clamped
        },
    )
}

/// Builds the camera's live map-load bytes from `loc_51AB2`'s packed section.
pub(super) fn camera_for_record(
    driver: Driver,
    map: &FieldMap,
    record: &MapRecord,
) -> Result<Camera, String> {
    let scroll = &record.scroll;
    let counters = |value: Option<&psiv_data::ScrollCounters>| -> Result<(i32, i32), String> {
        let Some(value) = value else {
            return Ok((0, 0));
        };
        Ok((parse_fixed(&value.x)?, parse_fixed(&value.y)?))
    };
    let step_fg = counters(scroll.fg_step_counters.as_ref())?;
    let step_bg = counters(scroll.bg_step_counters.as_ref())?;
    Ok(Camera::placed_on_planes_with_steps(
        driver,
        bounds_of(map),
        bounds_of(map),
        CameraGates {
            ec24: scroll.mode,
            ec25: scroll.fg_scroll_mode,
            ec26: scroll.bg_scroll_mode,
        },
        step_fg,
        step_bg,
    ))
}

fn parse_fixed(value: &str) -> Result<i32, String> {
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .ok_or_else(|| format!("camera step {value:?} is not a hexadecimal longword"))?;
    if digits.len() != 8 {
        return Err(format!(
            "camera step {value:?} is not an eight-digit hexadecimal longword"
        ));
    }
    u32::from_str_radix(digits, 16)
        .map(|bits| bits as i32)
        .map_err(|_| format!("camera step {value:?} is not a hexadecimal longword"))
}

/// The leader's 16.16 position, as the camera reads it.
///
/// The camera derives the velocity it latches on from successive positions, so
/// there is deliberately no velocity here to get wrong.
pub(super) fn driver_of(leader: &FieldState) -> Driver {
    let (ox, oy) = leader.render_offset_16ths();
    let at = PixelPos::from_cell(leader.cell());
    Driver {
        x: (at.x + ox) * ONE_PIXEL,
        y: (at.y + oy) * ONE_PIXEL,
    }
}

/// An object's 16.16 map position, including the part-cell travel of a step in
/// progress.
///
/// A stepping object's cell is already its destination — the engine commits it
/// at step start — so the pixel position interpolates from the origin the step
/// remembers, not from the cell.
pub(super) fn object_position(npc: &Npc, wanderer: Option<&Wanderer>) -> (i32, i32) {
    let base = wanderer.and_then(Wanderer::step_origin).unwrap_or(npc.cell);
    let at = PixelPos::from_cell(base);
    let (tx, ty) = wanderer.map_or((0, 0), Wanderer::travelled_px);
    (
        (at.x + i32::from(npc.offset.x) + tx) * ONE_PIXEL,
        (at.y + i32::from(npc.offset.y) + ty) * ONE_PIXEL,
    )
}
