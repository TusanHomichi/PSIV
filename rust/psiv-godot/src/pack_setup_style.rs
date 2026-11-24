//! Phantasy Star IV–inspired procedural styling for the first-run ROM setup
//! screen (`pack_setup.rs`).
//!
//! Draws entirely from procedural primitives and Godot `StyleBoxFlat` themes —
//! no ROM art, tiles, or copyrighted assets are referenced before the user
//! supplies their own cartridge image.

use godot::builtin::Side;
use godot::classes::canvas_item::TextureFilter;
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::image::Format;
use godot::classes::text_server::AutowrapMode;
use godot::classes::{
    Button, ColorRect, Control, IControl, Image, ImageTexture, Label, PanelContainer, ProgressBar,
    StyleBoxFlat,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

/// Logical pixel resolution of the procedural 16-bit space backdrop (exact 4×
/// integer scale at the default 1280×800 window size).
const BACKDROP_W: i32 = 320;
const BACKDROP_H: i32 = 200;

/// Classic Phantasy Star IV window palette (cobalt interior + silver bevels).
pub(super) const COBALT_BG: Color = Color::from_rgb(0.0, 0.11, 0.48);
pub(super) const COBALT_DEEP: Color = Color::from_rgb(0.0, 0.06, 0.28);
pub(super) const COBALT_BRIGHT: Color = Color::from_rgb(0.06, 0.24, 0.72);
pub(super) const BORDER_SHADOW: Color = Color::from_rgb(0.03, 0.05, 0.14);
pub(super) const BORDER_SILVER: Color = Color::from_rgb(0.86, 0.91, 0.98);
pub(super) const BORDER_STEEL: Color = Color::from_rgb(0.40, 0.50, 0.70);
pub(super) const BORDER_LIP: Color = Color::from_rgb(0.0, 0.04, 0.22);
pub(super) const DIVIDER_COLOR: Color = Color::from_rgb(0.32, 0.55, 0.88);

pub(super) const TEXT_GOLD: Color = Color::from_rgb(0.98, 0.80, 0.28);
pub(super) const TEXT_CYAN: Color = Color::from_rgb(0.46, 0.94, 0.90);
pub(super) const TEXT_WHITE: Color = Color::from_rgb(0.96, 0.98, 1.0);
pub(super) const TEXT_BODY: Color = Color::from_rgb(0.88, 0.93, 1.0);
pub(super) const TEXT_MUTED: Color = Color::from_rgb(0.64, 0.76, 0.95);
pub(super) const TEXT_SHADOW: Color = Color::from_rgb(0.0, 0.02, 0.12);
pub(super) const TEXT_ERROR: Color = Color::from_rgb(1.0, 0.56, 0.54);
pub(super) const TEXT_GREEN: Color = Color::from_rgb(0.52, 0.95, 0.64);

/// Procedural 16-bit space starfield backdrop baked at 320×200 with a
/// checkerboard-dithered cosmic nebula band, a distant twin-star glow, a
/// stepped-pixel planet horizon in the lower-right corner, and twinkling stars.
#[derive(GodotClass)]
#[class(base=Control)]
pub(crate) struct SetupBackdrop {
    base: Base<Control>,
    texture: Option<Gd<ImageTexture>>,
    elapsed: f64,
    last_redraw: f64,
}

impl SetupBackdrop {
    fn build_backdrop_texture() -> Option<Gd<ImageTexture>> {
        let mut pixels = PackedByteArray::new();
        pixels.resize((BACKDROP_W * BACKDROP_H * 4) as usize);
        let data = pixels.as_mut_slice();

        // Lower-right planet geometry (Algo-inspired sci-fi horizon).
        let planet_cx = BACKDROP_W as f32 * 0.90;
        let planet_cy = BACKDROP_H as f32 * 1.24;
        let planet_r = BACKDROP_H as f32 * 0.62;

        // Upper-left distant moon and upper-right distant golden star glow.
        let moon_cx = BACKDROP_W as f32 * 0.13;
        let moon_cy = BACKDROP_H as f32 * 0.21;
        let moon_r = 13.0_f32;
        let star_cx = BACKDROP_W as f32 * 0.86;
        let star_cy = BACKDROP_H as f32 * 0.20;

        for y in 0..BACKDROP_H {
            let fy = y as f32;
            let ny = fy / BACKDROP_H as f32;
            for x in 0..BACKDROP_W {
                let fx = x as f32;
                let nx = fx / BACKDROP_W as f32;
                let checker = (x + y) & 1 == 0;
                let sparse = (x & 1 == 0) && (y & 1 == 0);

                // 1. Deep space + diagonal 16-bit dithered cobalt/indigo nebula.
                let band_center = 0.24 + 0.44 * nx + 0.06 * (nx * std::f32::consts::TAU).sin();
                let dist = ((ny - band_center) * 2.1).abs();
                let mut rgb: [u8; 3] = if dist < 0.14 {
                    if checker { [18, 42, 108] } else { [13, 33, 92] }
                } else if dist < 0.26 {
                    if checker { [13, 32, 88] } else { [9, 23, 68] }
                } else if dist < 0.40 {
                    if checker { [8, 20, 60] } else { [5, 13, 42] }
                } else if dist < 0.56 {
                    if sparse { [6, 14, 44] } else { [3, 6, 20] }
                } else {
                    [3, 5, 16]
                };

                // 1b. Distant golden star flare in the upper-right sky.
                let sdx = (fx - star_cx).abs();
                let sdy = (fy - star_cy).abs();
                let sd = (sdx * sdx + sdy * sdy).sqrt();
                if sd <= 2.2 {
                    rgb = [252, 238, 178];
                } else if (sdx <= 0.6 && sdy <= 9.0) || (sdy <= 0.6 && sdx <= 9.0) {
                    if checker || sd <= 5.0 {
                        rgb = [228, 184, 92];
                    }
                } else if sd <= 8.5 {
                    if checker {
                        rgb = [88, 66, 48];
                    }
                } else if sd <= 15.0 && sparse {
                    rgb = [34, 28, 46];
                }

                // 2. Subtle distant indigo/amber moon in the upper-left corner.
                let mdx = fx - moon_cx;
                let mdy = fy - moon_cy;
                let md = (mdx * mdx + mdy * mdy).sqrt();
                if md <= moon_r + 1.5 {
                    let shade = (mdx * 0.7 - mdy * 0.7) / moon_r;
                    if md > moon_r {
                        if checker {
                            rgb = [40, 88, 128];
                        }
                    } else if shade > 0.35 {
                        rgb = if checker {
                            [132, 190, 212]
                        } else {
                            [98, 152, 188]
                        };
                    } else if shade > -0.15 {
                        rgb = if checker {
                            [66, 114, 162]
                        } else {
                            [46, 84, 134]
                        };
                    } else {
                        rgb = if checker { [24, 46, 88] } else { [14, 28, 60] };
                    }
                }

                // 3. Stepped-pixel aqua/teal planet rim in the lower-right.
                let pdx = fx - planet_cx;
                let pdy = fy - planet_cy;
                let pd = (pdx * pdx + pdy * pdy).sqrt();
                if pd <= planet_r + 3.0 {
                    let band = ((fx * 0.06 + fy * 0.11).sin() * 0.5 + 0.5) > 0.55;
                    if pd > planet_r {
                        if checker {
                            rgb = [46, 164, 174];
                        }
                    } else if pd > planet_r - 3.0 {
                        rgb = [98, 224, 216];
                    } else if pd > planet_r - 8.5 {
                        rgb = if checker {
                            [42, 152, 168]
                        } else {
                            [26, 108, 142]
                        };
                    } else if pd > planet_r - 22.0 {
                        rgb = if band && checker {
                            [30, 112, 140]
                        } else if checker {
                            [18, 78, 116]
                        } else {
                            [12, 54, 94]
                        };
                    } else {
                        rgb = if band && checker {
                            [16, 62, 98]
                        } else if checker {
                            [10, 38, 74]
                        } else {
                            [7, 26, 56]
                        };
                    }
                }

                let idx = ((y * BACKDROP_W + x) * 4) as usize;
                data[idx] = rgb[0];
                data[idx + 1] = rgb[1];
                data[idx + 2] = rgb[2];
                data[idx + 3] = 255;
            }
        }

        let image = Image::create_from_data(BACKDROP_W, BACKDROP_H, false, Format::RGBA8, &pixels)?;
        ImageTexture::create_from_image(&image)
    }
}

#[godot_api]
impl IControl for SetupBackdrop {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            texture: None,
            elapsed: 0.0,
            last_redraw: -1.0,
        }
    }

    fn ready(&mut self) {
        self.base_mut()
            .set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().set_texture_filter(TextureFilter::NEAREST);
        self.texture = Self::build_backdrop_texture();
        self.base_mut().queue_redraw();
    }

    fn process(&mut self, delta: f64) {
        self.elapsed += delta;
        // Twinkle at ~12 Hz so software rendering stays light.
        if self.elapsed - self.last_redraw >= 0.08 {
            self.last_redraw = self.elapsed;
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let w = size.x.max(1.0);
        let h = size.y.max(1.0);

        if let Some(tex) = self.texture.clone() {
            self.base_mut().draw_texture_rect(
                &tex,
                Rect2::new(Vector2::ZERO, Vector2::new(w, h)),
                false,
            );
        } else {
            self.base_mut().draw_rect(
                Rect2::new(Vector2::ZERO, Vector2::new(w, h)),
                Color::from_rgb(0.012, 0.02, 0.063),
            );
        }

        // Deterministic pixel stars snapped to the 320×200 logical grid.
        let sx_scale = w / BACKDROP_W as f32;
        let sy_scale = h / BACKDROP_H as f32;
        let planet_cx = BACKDROP_W as f32 * 0.90;
        let planet_cy = BACKDROP_H as f32 * 1.24;
        let planet_r = BACKDROP_H as f32 * 0.62 + 3.0;
        let moon_cx = BACKDROP_W as f32 * 0.13;
        let moon_cy = BACKDROP_H as f32 * 0.21;
        let moon_r = 16.0_f32;

        let mut seed: u32 = 0x5053_4956;
        let step_rng = |s: &mut u32| -> u32 {
            *s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *s
        };
        for i in 0..135 {
            let gx = (step_rng(&mut seed) % BACKDROP_W as u32) as f32;
            let gy = (step_rng(&mut seed) % BACKDROP_H as u32) as f32;
            let kind = step_rng(&mut seed) % 12;
            let phase = (step_rng(&mut seed) % 628) as f64 / 100.0;

            // Keep stars behind the planet and moon silhouettes.
            let pdx = gx - planet_cx;
            let pdy = gy - planet_cy;
            let mdx = gx - moon_cx;
            let mdy = gy - moon_cy;
            if pdx * pdx + pdy * pdy <= planet_r * planet_r
                || mdx * mdx + mdy * mdy <= moon_r * moon_r
            {
                continue;
            }

            let pulse = (self.elapsed * 2.4 + phase).sin() as f32 * 0.24 + 0.76;
            let base_color = match kind {
                0 => Color::from_rgb(0.48, 0.95, 0.92),
                1 | 2 => Color::from_rgb(0.48, 0.72, 1.0),
                3 => Color::from_rgb(0.98, 0.85, 0.48),
                _ => Color::from_rgb(0.88, 0.93, 1.0),
            };
            let alpha = if i % 4 == 0 {
                0.96 * pulse
            } else {
                0.58 * pulse
            };
            let color = Color::from_rgba(base_color.r, base_color.g, base_color.b, alpha);
            let px_x = gx * sx_scale;
            let px_y = gy * sy_scale;
            let dot_w = sx_scale.max(2.0);
            let dot_h = sy_scale.max(2.0);
            self.base_mut().draw_rect(
                Rect2::new(Vector2::new(px_x, px_y), Vector2::new(dot_w, dot_h)),
                color,
            );
            // Subtle 4-point cross glint on a few bright stars.
            if i % 19 == 0 {
                let arm = Color::from_rgba(base_color.r, base_color.g, base_color.b, alpha * 0.48);
                self.base_mut().draw_rect(
                    Rect2::new(
                        Vector2::new(px_x - dot_w, px_y),
                        Vector2::new(dot_w * 3.0, dot_h),
                    ),
                    arm,
                );
                self.base_mut().draw_rect(
                    Rect2::new(
                        Vector2::new(px_x, px_y - dot_h),
                        Vector2::new(dot_w, dot_h * 3.0),
                    ),
                    arm,
                );
            }
        }
    }
}

fn flat_box(
    bg: Color,
    border: Color,
    border_width: i32,
    corner_radius: i32,
    pad_x: f32,
    pad_y: f32,
) -> Gd<StyleBoxFlat> {
    let mut style = StyleBoxFlat::new_gd();
    style.set_bg_color(bg);
    style.set_border_color(border);
    style.set_border_width_all(border_width);
    style.set_corner_radius_all(corner_radius);
    style.set_corner_detail(1);
    style.set_anti_aliased(false);
    style.set_content_margin(Side::LEFT, pad_x);
    style.set_content_margin(Side::RIGHT, pad_x);
    style.set_content_margin(Side::TOP, pad_y);
    style.set_content_margin(Side::BOTTOM, pad_y);
    style
}

/// Wraps `content` in a classic Phantasy Star IV cobalt-blue window with a
/// chamfered multi-step metallic bevel (`shadow -> silver -> steel -> lip`).
pub(super) fn psiv_window<T: Inherits<godot::classes::Node>>(
    content: &Gd<T>,
    pad_x: f32,
    pad_y: f32,
) -> Gd<PanelContainer> {
    let mut outer = PanelContainer::new_alloc();
    outer.add_theme_stylebox_override(
        "panel",
        &flat_box(BORDER_SILVER, BORDER_SHADOW, 3, 8, 3.0, 3.0),
    );

    let mut mid = PanelContainer::new_alloc();
    mid.add_theme_stylebox_override("panel", &flat_box(BORDER_LIP, BORDER_STEEL, 3, 5, 3.0, 3.0));

    let mut inner = PanelContainer::new_alloc();
    inner.add_theme_stylebox_override("panel", &flat_box(COBALT_BG, COBALT_BG, 0, 2, pad_x, pad_y));
    inner.add_child(content);
    mid.add_child(&inner);
    outer.add_child(&mid);
    outer
}

/// Builds a crisp shadowed label in the setup screen's retro palette.
/// Pass `wrap = false` for single-line headers so containers size to the text.
pub(super) fn styled_label(text: &str, size: i32, color: Color, wrap: bool) -> Gd<Label> {
    styled_label_aligned(text, size, color, wrap, HorizontalAlignment::CENTER)
}

/// Builds a shadowed retro label with an explicit horizontal alignment.
pub(super) fn styled_label_aligned(
    text: &str,
    size: i32,
    color: Color,
    wrap: bool,
    align: HorizontalAlignment,
) -> Gd<Label> {
    let mut label = Label::new_alloc();
    label.set_text(text);
    label.set_autowrap_mode(if wrap {
        AutowrapMode::WORD_SMART
    } else {
        AutowrapMode::OFF
    });
    label.set_horizontal_alignment(align);
    label.add_theme_font_size_override("font_size", size);
    label.add_theme_color_override("font_color", color);
    label.add_theme_color_override("font_shadow_color", TEXT_SHADOW);
    label.add_theme_constant_override("shadow_offset_x", 2);
    label.add_theme_constant_override("shadow_offset_y", 2);
    label
}

/// Compact cobalt-blue PSIV badge box for HUD strips and row cards.
pub(super) fn psiv_compact_panel<T: Inherits<godot::classes::Node>>(
    content: &Gd<T>,
    pad_x: f32,
    pad_y: f32,
    border: Color,
) -> Gd<PanelContainer> {
    let mut panel = PanelContainer::new_alloc();
    panel.add_theme_stylebox_override("panel", &flat_box(COBALT_DEEP, border, 2, 4, pad_x, pad_y));
    panel.add_child(content);
    panel
}

/// Styles a compact secondary or row-action button in the PSIV palette.
pub(super) fn style_compact_button(button: &mut Gd<Button>) {
    button.add_theme_font_size_override("font_size", 15);
    button.set_h_size_flags(SizeFlags::SHRINK_CENTER);

    let normal = flat_box(COBALT_DEEP, BORDER_SILVER, 2, 4, 14.0, 6.0);
    let active = flat_box(COBALT_BRIGHT, TEXT_GOLD, 2, 4, 14.0, 6.0);
    let pressed = flat_box(BORDER_LIP, TEXT_CYAN, 2, 4, 14.0, 6.0);
    let disabled = flat_box(
        Color::from_rgb(0.06, 0.10, 0.24),
        BORDER_STEEL,
        2,
        4,
        14.0,
        6.0,
    );

    button.add_theme_stylebox_override("normal", &normal);
    button.add_theme_stylebox_override("hover", &active);
    button.add_theme_stylebox_override("focus", &active);
    button.add_theme_stylebox_override("pressed", &pressed);
    button.add_theme_stylebox_override("disabled", &disabled);

    button.add_theme_color_override("font_color", TEXT_WHITE);
    button.add_theme_color_override("font_hover_color", TEXT_GOLD);
    button.add_theme_color_override("font_focus_color", TEXT_GOLD);
    button.add_theme_color_override("font_pressed_color", TEXT_CYAN);
    button.add_theme_color_override("font_disabled_color", TEXT_MUTED);
}

/// Builds a crisp 2px metallic-blue divider line for inside a PSIV dialog box.
pub(super) fn divider() -> Gd<ColorRect> {
    let mut line = ColorRect::new_alloc();
    line.set_color(DIVIDER_COLOR);
    line.set_custom_minimum_size(Vector2::new(0.0, 2.0));
    line.set_h_size_flags(SizeFlags::EXPAND_FILL);
    line
}

/// Styles the primary action button as a PSIV beveled menu selection box
/// (metallic silver at rest, bright gold-highlighted cobalt on focus/hover).
pub(super) fn style_button(button: &mut Gd<Button>) {
    button.add_theme_font_size_override("font_size", 21);
    button.set_h_size_flags(SizeFlags::SHRINK_CENTER);

    let normal = flat_box(COBALT_DEEP, BORDER_SILVER, 3, 4, 30.0, 12.0);
    let active = flat_box(COBALT_BRIGHT, TEXT_GOLD, 3, 4, 30.0, 12.0);
    let pressed = flat_box(BORDER_LIP, TEXT_CYAN, 3, 4, 30.0, 12.0);
    let disabled = flat_box(
        Color::from_rgb(0.06, 0.10, 0.24),
        BORDER_STEEL,
        2,
        4,
        30.0,
        12.0,
    );

    button.add_theme_stylebox_override("normal", &normal);
    button.add_theme_stylebox_override("hover", &active);
    button.add_theme_stylebox_override("focus", &active);
    button.add_theme_stylebox_override("pressed", &pressed);
    button.add_theme_stylebox_override("disabled", &disabled);

    button.add_theme_color_override("font_color", TEXT_WHITE);
    button.add_theme_color_override("font_hover_color", TEXT_GOLD);
    button.add_theme_color_override("font_focus_color", TEXT_GOLD);
    button.add_theme_color_override("font_pressed_color", TEXT_CYAN);
    button.add_theme_color_override("font_disabled_color", TEXT_MUTED);
}

/// Styles a compact retro progress bar shown while `psiv-pack` builds the pack.
pub(super) fn progress_bar() -> Gd<ProgressBar> {
    let mut bar = ProgressBar::new_alloc();
    bar.set_min(0.0);
    bar.set_max(100.0);
    bar.set_value(0.0);
    bar.set_show_percentage(false);
    bar.set_custom_minimum_size(Vector2::new(420.0, 16.0));
    bar.set_h_size_flags(SizeFlags::SHRINK_CENTER);
    bar.set_visible(false);

    let bg = flat_box(COBALT_DEEP, BORDER_SILVER, 2, 2, 0.0, 0.0);
    let fill = flat_box(TEXT_CYAN, TEXT_CYAN, 0, 0, 0.0, 0.0);
    bar.add_theme_stylebox_override("background", &bg);
    bar.add_theme_stylebox_override("fill", &fill);
    bar
}
