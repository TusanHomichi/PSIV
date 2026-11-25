//! Vehicle-specific battle art loading.

use godot::classes::ImageTexture;
use godot::prelude::*;

use crate::pack_assets::PackAssets;

/// Loads the first field-sheet frame as the one-fighter vehicle battle body.
/// The pack records the retail frame dimensions, so the battle surface does
/// not need to know the sheet's total frame count.
pub(super) fn texture(
    assets: &PackAssets,
    path: &str,
    width: u32,
    height: u32,
) -> Option<Gd<ImageTexture>> {
    let image = assets.load_image(path)?;
    let frame = image.get_region(Rect2i::new(
        Vector2i::ZERO,
        Vector2i::new(width as i32, height as i32),
    ))?;
    ImageTexture::create_from_image(&frame)
}
