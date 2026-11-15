//! Draw the loader's two planes from their uploaded rows and live palette.
use super::BattleScreen;
use godot::classes::{Image, ImageTexture, Sprite2D};
use godot::prelude::*;
use psiv_data::color::{RAMP_B, RAMP_G, RAMP_R, cram_rgb};
use psiv_runtime::BattleLoadView;

struct Plane {
    width: usize,
    height: usize,
    pixels: Vec<Option<u16>>,
    x: i32,
    y: i32,
}

impl Plane {
    fn from_sprite(sprite: &Gd<Sprite2D>) -> Option<Self> {
        let image = sprite.get_texture()?.get_image()?;
        let mut pixels = Vec::new();
        for y in 0..image.get_height() {
            for x in 0..image.get_width() {
                let color = image.get_pixel(x, y);
                let word = if color.a == 0.0 {
                    None
                } else {
                    let component = |value: f32, ramp: &[u8; 8]| {
                        ramp.iter()
                            .position(|&v| v == (value * 255.0).round() as u8)
                    };
                    Some(
                        (component(color.r, &RAMP_R)? as u16) << 1
                            | (component(color.g, &RAMP_G)? as u16) << 5
                            | (component(color.b, &RAMP_B)? as u16) << 9,
                    )
                };
                pixels.push(word);
            }
        }
        let position = sprite.get_position();
        Some(Self {
            width: image.get_width() as usize,
            height: image.get_height() as usize,
            pixels,
            x: position.x as i32,
            y: position.y as i32,
        })
    }

    fn pixel(&self, x: i32, y: i32) -> Option<u16> {
        let (x, y) = (x - self.x, y - self.y);
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.pixels[y as usize * self.width + x as usize]
    }
}

pub(super) struct LoadingStage {
    node: Gd<Sprite2D>,
    background: Option<Plane>,
    enemies: Vec<Plane>,
}

impl BattleScreen {
    pub(crate) fn loading(&self) -> bool {
        self.load.is_some()
    }

    pub(crate) fn finish_loading(&mut self) {
        if let Some(mut load) = self.load.take() {
            load.node.queue_free();
        }
    }

    pub(crate) fn draw_load(&mut self, view: BattleLoadView) {
        if self.load.is_none() {
            for enemy in &mut self.enemy_nodes {
                enemy.node.set_visible(view.enemy_visible(enemy.fighter));
            }
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(20);
            self.base_mut().add_child(&node);
            self.load = Some(LoadingStage {
                node,
                background: self.background.as_ref().and_then(Plane::from_sprite),
                enemies: self
                    .enemy_nodes
                    .iter()
                    .filter(|enemy| view.enemy_visible(enemy.fighter))
                    .filter_map(|enemy| Plane::from_sprite(&enemy.node))
                    .collect(),
            });
        }
        let load = self.load.as_mut().unwrap();
        let mut rgba = Vec::with_capacity(320 * 224 * 4);
        for y in 0..224 {
            let by = (y + i32::from(view.background_y)) & 255;
            let ay = (y + i32::from(view.foreground_y)) & 255;
            for x in 0..320 {
                let mut word = 0;
                if by >= i32::from(view.background_first_row) * 16 {
                    word = load
                        .background
                        .as_ref()
                        .and_then(|plane| plane.pixel(x, by))
                        .unwrap_or(0);
                }
                if ay < i32::from(view.foreground_rows) * 16 {
                    for enemy in &load.enemies {
                        if let Some(pixel) = enemy.pixel(x, ay) {
                            word = pixel;
                        }
                    }
                }
                let rgb = cram_rgb(view.palette_word(word));
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        if let Some(image) = Image::create_from_data(
            320,
            224,
            false,
            godot::classes::image::Format::RGBA8,
            &PackedByteArray::from(rgba.as_slice()),
        ) && let Some(texture) = ImageTexture::create_from_image(&image)
        {
            load.node.set_texture(&texture);
        }
    }
}
