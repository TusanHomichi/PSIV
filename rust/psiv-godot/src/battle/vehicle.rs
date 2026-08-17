//! Vehicle-specific battle art loading.

use godot::classes::{Image, ImageTexture};
use godot::prelude::*;

/// Loads the first field-sheet frame as the one-fighter vehicle battle body.
/// The pack records the retail frame dimensions, so the battle surface does
/// not need to know the sheet's total frame count.
pub(super) fn texture(
    pack_dir: &str,
    path: &str,
    width: u32,
    height: u32,
) -> Option<Gd<ImageTexture>> {
    let full = format!("{pack_dir}/{path}");
    let image = Image::load_from_file(&GString::from(full.as_str()))?;
    let frame = image.get_region(Rect2i::new(
        Vector2i::ZERO,
        Vector2i::new(width as i32, height as i32),
    ))?;
    ImageTexture::create_from_image(&frame)
}
