//! CRAM component fades. Keep red; step green/blue on the measured RGB565
//! ramps in docs/field/COLOR_PIPELINE.md. The core owns the eight blocking stages.
use crate::Field;
use godot::classes::{ColorRect, Shader, ShaderMaterial};
use godot::prelude::*;

const SHADER: &str = r#"
shader_type canvas_item;
render_mode unshaded, blend_disabled;
uniform sampler2D screen_texture : hint_screen_texture, repeat_disable, filter_nearest;
uniform int stage = 1;
uniform bool restoring = false;
const float GREEN[8] = float[8](0.0,32.0,68.0,101.0,137.0,170.0,206.0,238.0);
const float BLUE[8] = float[8](0.0,32.0,65.0,98.0,139.0,172.0,205.0,238.0);
int level(float value, bool green) {
    int best = 0;
    float distance = 1000.0;
    for (int i = 0; i < 8; i++) {
        float d = abs(value * 255.0 - (green ? GREEN[i] : BLUE[i]));
        if (d < distance) { distance = d; best = i; }
    }
    return best;
}
void fragment() {
    vec4 original = texture(screen_texture, SCREEN_UV);
    int g = level(original.g, true);
    int b = level(original.b, false);
    g = restoring ? min(g, stage) : max(0, g - stage);
    b = restoring ? min(b, stage) : max(0, b - stage);
    COLOR = vec4(original.r, GREEN[g] / 255.0, BLUE[b] / 255.0, original.a);
}
"#;

#[derive(Default)]
pub(super) struct RedPalette {
    node: Option<Gd<ColorRect>>,
    material: Option<Gd<ShaderMaterial>>,
    elapsed: u16,
    stage_frames: u16,
    restoring: bool,
}

impl Field {
    pub(super) fn start_red_palette(&mut self, delay: u8, restoring: bool) {
        if self.red_palette.node.is_none() {
            let mut shader = Shader::new_gd();
            shader.set_code(SHADER);
            let mut material = ShaderMaterial::new_gd();
            material.set_shader(&shader);
            let mut node = ColorRect::new_alloc();
            node.set_z_index(1100);
            node.set_material(&material);
            self.base_mut().add_child(&node);
            self.red_palette.node = Some(node);
            self.red_palette.material = Some(material);
        }
        self.red_palette.elapsed = 0;
        self.red_palette.stage_frames = u16::from(delay) + 1;
        self.red_palette.restoring = restoring;
        self.sync_red_palette();
    }

    pub(super) fn tick_red_palette(&mut self) {
        if self.red_palette.stage_frames == 0 {
            return;
        }
        self.red_palette.elapsed = self.red_palette.elapsed.saturating_add(1);
        if self.red_palette.restoring
            && self.red_palette.elapsed >= 8 * self.red_palette.stage_frames
        {
            self.clear_red_palette();
        } else {
            self.sync_red_palette();
        }
    }

    pub(super) fn clear_red_palette(&mut self) {
        self.red_palette.stage_frames = 0;
        if let Some(node) = self.red_palette.node.as_mut() {
            node.hide();
        }
    }

    fn sync_red_palette(&mut self) {
        let viewport = self.base().get_viewport_rect();
        let inverse = self.base().get_canvas_transform().affine_inverse();
        let start = inverse * viewport.position;
        let end = inverse * (viewport.position + viewport.size);
        if let Some(node) = self.red_palette.node.as_mut() {
            node.set_position(start - Vector2::new(4.0, 4.0));
            node.set_size(end - start + Vector2::new(8.0, 8.0));
            node.show();
        }
        if let Some(material) = self.red_palette.material.as_mut() {
            let stage = (1 + self.red_palette.elapsed / self.red_palette.stage_frames).min(8);
            material.set_shader_parameter("stage", &i32::from(stage).to_variant());
            material.set_shader_parameter("restoring", &self.red_palette.restoring.to_variant());
        }
    }

    pub(super) fn red_palette_probe(&self) -> Option<(bool, u16)> {
        (self.red_palette.stage_frames > 0).then(|| {
            (
                self.red_palette.restoring,
                (1 + self.red_palette.elapsed / self.red_palette.stage_frames).min(8),
            )
        })
    }
}
