//! Where the retail 320x224 surface sits in a layer's coordinates.
//!
//! The field camera centres the surface, at 3x, in whatever viewport the
//! renderer has, and its centre is a whole world pixel (`field_visuals.rs`).
//! The layers that draw in screen space (scene panels, the opening's text, the
//! dialogue window) put themselves at the surface's top-left, which
//! [`surface_origin`] reads back through the inverse canvas transform. A
//! viewport whose width is not a multiple of three (1280 at 3x is 426.67 world
//! pixels) leaves f32 error in that read-back, so the origin is rounded:
//! flooring it lost a pixel at some camera positions and not at others. At
//! camera (0,0) it lost one each way, which the MeetingRika pair once measured
//! as a one-pixel "plane residue" and the shell added back, so every other
//! camera position drew the scene one pixel right and down.
//!
//! Its tests are `tests/retail_surface.rs`, which includes this file.

use godot::prelude::*;

/// The retail frame, in world pixels.
pub(crate) const SURFACE: Vector2 = Vector2::new(320.0, 224.0);

/// The top-left of the retail surface centred in `viewport`, for a layer whose
/// inverse canvas transform is `canvas_inverse`: a whole world pixel.
pub(crate) fn surface_origin(canvas_inverse: Transform2D, viewport: Rect2) -> Vector2 {
    let top_left = canvas_inverse * viewport.position;
    let bottom_right = canvas_inverse * (viewport.position + viewport.size);
    let visible = bottom_right - top_left;
    (top_left + (visible - SURFACE) / 2.0).round()
}
