//! Map and camera geometry shared by the runtime shell and save seam.

use psiv_core::{
    CameraBounds, CameraEdges, Driver, FieldMap, FieldState, Npc, ONE_PIXEL, PixelPos, Topology,
    Wanderer,
};

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
