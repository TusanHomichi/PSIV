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

#[cfg(test)]
mod tests {
    use super::*;

    /// The inverse canvas transform a layer reads under a 3x `Camera2D`
    /// centred on `center` in a `viewport`-sized view. Godot builds the
    /// camera's transform from `1 / zoom` (the view's top-left and a
    /// one-third basis) and sets the canvas transform to its inverse
    /// (`Camera2D::get_camera_transform`); the layer inverts that again.
    fn canvas_inverse(center: Vector2, viewport: Vector2) -> Transform2D {
        let zoom_scale = 1.0 / 3.0;
        let view = Transform2D::from_cols(
            Vector2::new(zoom_scale, 0.0),
            Vector2::new(0.0, zoom_scale),
            center - viewport * 0.5 * zoom_scale,
        );
        view.affine_inverse().affine_inverse()
    }

    fn floored(canvas_inverse: Transform2D, viewport: Rect2) -> Vector2 {
        let top_left = canvas_inverse * viewport.position;
        let bottom_right = canvas_inverse * (viewport.position + viewport.size);
        let visible = bottom_right - top_left;
        (top_left + (visible - SURFACE) / 2.0).floor()
    }

    #[test]
    fn the_surface_origin_is_the_camera_s_top_left_at_every_position() {
        // The field camera centres (x + 160, y + 112) for camera (x, y).
        for viewport in [Vector2::new(1280.0, 800.0), Vector2::new(960.0, 672.0)] {
            let rect = Rect2::new(Vector2::ZERO, viewport);
            for y in (0..2048).step_by(7) {
                for x in (0..4096).step_by(5) {
                    let camera = Vector2::new(x as f32, y as f32);
                    let origin =
                        surface_origin(canvas_inverse(camera + SURFACE / 2.0, viewport), rect);
                    assert_eq!(origin, camera, "camera {camera} in {viewport}");
                }
            }
        }
    }

    #[test]
    fn flooring_the_origin_loses_a_pixel_at_some_cameras() {
        // The control: the read-back is not exact in a 1280-wide viewport,
        // so a floor would land one pixel short somewhere, which is the
        // error the rounding removes.
        let viewport = Vector2::new(1280.0, 800.0);
        let rect = Rect2::new(Vector2::ZERO, viewport);
        let short = (0..4096)
            .map(|x| Vector2::new(x as f32, 0.0))
            .filter(|&camera| {
                floored(canvas_inverse(camera + SURFACE / 2.0, viewport), rect) != camera
            })
            .count();
        assert!(short > 0, "a floored origin was exact at every camera");
    }
}
