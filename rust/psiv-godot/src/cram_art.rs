//! Sprite sheets, scene temporary objects, scene panels and the native
//! windows drawn from the runtime's live CRAM.
//!
//! The pack's sprite and panel PNGs are indexed. [`IndexedArt`] keeps each
//! pixel's palette index ([`psiv_data::color::index_png`]) and recolours its
//! texture from the shared [`CramState`] on the frame the art is drawn, when
//! the words changed since it was last drawn
//! ([`psiv_data::color::recolor`]). So a tone, a flash, a held line or a
//! cycle reaches every sprite and panel on the line it touches, and nothing
//! else. The field map draws its index textures through its own shader
//! (`field_map.rs`); both read `Runtime::cram`, which `Field::sync_map_palette`
//! copies into [`CramState`] at the top of every drawn frame.
//!
//! While no live palette is known (a pack whose map records carry no 64-word
//! shadow), [`IndexedArt::texture`] answers `None` and the caller draws the
//! colours baked into the file.

use std::cell::RefCell;
use std::rc::Rc;

use godot::classes::image::Format;
use godot::classes::{FileAccess, Image, ImageTexture};
use godot::prelude::*;

use psiv_data::color::{IndexLayout, index_png, recolor};

/// The palette the field draws with this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CramState {
    /// `Palette_Table_Buffer`, as `Runtime::cram` holds it.
    pub(crate) words: [u16; 64],
    /// `CramView::revision`: changes whenever any word does.
    pub(crate) revision: u64,
    /// Whether the words are the cartridge's: false until a map with a
    /// 64-word shadow loads.
    pub(crate) live: bool,
}

impl Default for CramState {
    fn default() -> Self {
        Self {
            words: [0; 64],
            revision: 0,
            live: false,
        }
    }
}

/// One [`CramState`] the field and the scene layer read.
pub(crate) type SharedCram = Rc<RefCell<CramState>>;

/// `path`, an indexed PNG, decoded with each pixel's palette index in red
/// and its transparency in alpha ([`index_png`]). `None` (reported) for a
/// file that is missing, not indexed or does not decode.
pub(crate) fn load_index_image(path: &str) -> Option<Gd<Image>> {
    let bytes = FileAccess::get_file_as_bytes(path);
    if bytes.is_empty() {
        godot_error!("indexed art {path} could not be read");
        return None;
    }
    let indexed = match index_png(bytes.as_slice()) {
        Ok(indexed) => indexed,
        Err(error) => {
            godot_error!("indexed art {path}: {error}");
            return None;
        }
    };
    let mut image = Image::new_gd();
    if image.load_png_from_buffer(&PackedByteArray::from(indexed.as_slice()))
        != godot::global::Error::OK
    {
        godot_error!("indexed art {path} does not decode");
        return None;
    }
    Some(image)
}

/// An indexed image's palette indices, transparency and layout, and the
/// CRAM revision it was last recoloured for. Pure data, so the recolour rule
/// is testable without Godot; [`IndexedArt`] holds one with its texture.
#[derive(Debug, Clone)]
pub(crate) struct IndexedPixels {
    layout: IndexLayout,
    indices: Vec<u8>,
    alpha: Vec<u8>,
    /// The revision the last [`IndexedPixels::recolor`] drew, if any.
    drawn: Option<u64>,
}

impl IndexedPixels {
    /// One index and one alpha byte per pixel, read through `layout`.
    pub(crate) fn new(layout: IndexLayout, indices: Vec<u8>, alpha: Vec<u8>) -> Self {
        Self {
            layout,
            indices,
            alpha,
            drawn: None,
        }
    }

    /// The pixels in `cram`'s colours when `cram` is live and its revision
    /// is not the one last drawn; `None` otherwise (not live: draw the baked
    /// file; unchanged: the texture already holds these colours).
    pub(crate) fn recolor(&mut self, cram: &CramState) -> Option<Vec<u8>> {
        if !cram.live || self.drawn == Some(cram.revision) {
            return None;
        }
        let layout = self.layout;
        let rgba = recolor(
            &self.indices,
            &self.alpha,
            |index| layout.slot(index),
            &cram.words,
        );
        self.drawn = Some(cram.revision);
        Some(rgba)
    }
}

/// An indexed pack PNG and the texture it is drawn through.
pub(crate) struct IndexedArt {
    pixels: IndexedPixels,
    width: i32,
    height: i32,
    texture: Gd<ImageTexture>,
}

impl IndexedArt {
    /// Reads `path`, an indexed PNG, as palette indices laid out by
    /// `layout`. `None` (reported) for a file that is missing or not indexed.
    pub(crate) fn load(path: &str, layout: IndexLayout) -> Option<Self> {
        let mut image = load_index_image(path)?;
        image.convert(Format::RGBA8);
        let data = image.get_data();
        let pixels = data.as_slice();
        let (rgba, _) = pixels.as_chunks::<4>();
        let indices = rgba.iter().map(|pixel| pixel[0]).collect();
        let alpha = rgba.iter().map(|pixel| pixel[3]).collect();
        let texture = ImageTexture::create_from_image(&image)?;
        Some(Self {
            pixels: IndexedPixels::new(layout, indices, alpha),
            width: image.get_width(),
            height: image.get_height(),
            texture,
        })
    }

    /// The texture in `cram`'s colours, recoloured first when the words
    /// changed since it was last drawn; `None` while `cram` is not live.
    pub(crate) fn texture(&mut self, cram: &CramState) -> Option<Gd<ImageTexture>> {
        if !cram.live {
            return None;
        }
        if let Some(rgba) = self.pixels.recolor(cram) {
            let image = Image::create_from_data(
                self.width,
                self.height,
                false,
                Format::RGBA8,
                &PackedByteArray::from(rgba.as_slice()),
            )?;
            self.texture.update(&image);
        }
        Some(self.texture.clone())
    }
}

/// The native windows (`window_palette::WindowFamily`) draw tiles, glyphs
/// and portraits cut from art baked in line 2's `Pal_Init_Line_3` colours,
/// each pixel one of its sixteen. The shader finds a pixel's colour among
/// `baked` (indices 1-15; 0 is the transparent colour) and draws the live
/// word at that index instead, so the window follows line 2 of the CRAM
/// (`WindowLine::colour` is the same rule as data). Line 2's baked colours
/// are distinct apart from index 0's black, which index 1 shares; no window
/// pixel is an opaque index 0.
const WINDOW_LINE_SHADER: &str = r#"
shader_type canvas_item;
render_mode unshaded;
uniform vec3 baked[16];
uniform vec3 live[16];
varying vec4 tint;
void vertex() {
    tint = COLOR;
}
void fragment() {
    vec4 pixel = texture(TEXTURE, UV);
    ivec3 found = ivec3(round(pixel.rgb * 255.0));
    vec3 rgb = pixel.rgb;
    for (int i = 1; i < 16; i++) {
        if (ivec3(round(baked[i] * 255.0)) == found) {
            rgb = live[i];
            break;
        }
    }
    COLOR = vec4(rgb, pixel.a) * tint;
}
"#;

/// A material for the nodes whose art is baked in line 2's colours. The
/// shell makes one and attaches it to every `window_palette::WindowNode`.
pub(crate) fn window_line_material() -> Gd<godot::classes::ShaderMaterial> {
    let mut shader = godot::classes::Shader::new_gd();
    shader.set_code(WINDOW_LINE_SHADER);
    let mut material = godot::classes::ShaderMaterial::new_gd();
    material.set_shader(&shader);
    material
}

/// Points `material` at `line`: the art's baked words and the live ones.
pub(crate) fn set_window_line(
    material: &mut Gd<godot::classes::ShaderMaterial>,
    line: &crate::window_palette::WindowLine,
) {
    let colours = |words: &[u16; 16]| {
        let mut array = PackedVector3Array::new();
        for &word in words {
            let [r, g, b] = psiv_data::color::cram_rgb(word);
            array.push(Vector3::new(
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
            ));
        }
        array
    };
    material.set_shader_parameter("baked", &colours(&line.baked).to_variant());
    material.set_shader_parameter("live", &colours(&line.live).to_variant());
}
