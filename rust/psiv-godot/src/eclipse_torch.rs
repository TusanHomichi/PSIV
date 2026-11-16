//! Draw `$338` from runtime snapshots; no presentation age drives this path.

use crate::Field;
use godot::classes::Sprite2D;
use godot::prelude::*;
use psiv_core::{EclipseTorchObject, EclipseTorchSequence};
use std::collections::HashSet;

fn sequence_name(sequence: EclipseTorchSequence) -> &'static str {
    match sequence {
        EclipseTorchSequence::Fast => "walk_down",
        EclipseTorchSequence::Medium => "walk_up",
        EclipseTorchSequence::Slow => "walk_right",
    }
}

fn draw_pose(object: &EclipseTorchObject) -> Option<(&'static str, u8, (i32, i32))> {
    let spawn = object.spawn();
    Some((
        sequence_name(spawn.sequence),
        object.mapping()?,
        (spawn.x, spawn.y),
    ))
}

impl Field {
    pub(crate) fn sync_eclipse_torch(&mut self, objects: &[EclipseTorchObject]) -> HashSet<usize> {
        let mut slots = HashSet::new();
        for object in objects {
            let Some((sequence, mapping, position)) = draw_pose(object) else {
                continue;
            };
            let spawn = object.spawn();
            let asset = self
                .cutscene_layer
                .as_ref()
                .and_then(|layer| layer.bind().temporary_asset(0x338, 0x3AF));
            let Some(asset) = asset else {
                continue;
            };
            if !asset.art_loaded(&self.presentation) {
                continue;
            }
            let Some(frame) = asset.mapping_frame(sequence, mapping) else {
                continue;
            };
            if !self.temporary_nodes.contains_key(&spawn.slot) {
                let mut node = Sprite2D::new_alloc();
                node.set_centered(false);
                node.set_z_index(5);
                self.base_mut().add_child(&node);
                self.temporary_nodes.insert(spawn.slot, node);
            }
            if let Some(node) = self.temporary_nodes.get_mut(&spawn.slot) {
                asset.apply(node, frame, &self.cram.borrow());
                node.set_position(Vector2::new(
                    (position.0 - asset.origin_x) as f32,
                    (position.1 - asset.origin_y + asset.frame_height) as f32,
                ));
                slots.insert(spawn.slot);
            }
        }
        slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eclipse_torch_draws_only_the_runtime_mapping_at_literal_pixels() {
        let mut object = EclipseTorchObject::new(psiv_core::EclipseTorchSpawn {
            slot: 3,
            x: 0xBC0,
            y: 0xA0,
            sequence: EclipseTorchSequence::Medium,
        });
        assert_eq!(draw_pose(&object), None);
        for pass in 1..=48 {
            object.object_pass();
            let expected = if pass < 48 {
                Some(("walk_up", pass / 12, (0xBC0, 0xA0)))
            } else {
                None
            };
            for _ in 0..5 {
                assert_eq!(draw_pose(&object), expected);
            }
        }
    }

    #[test]
    fn eclipse_torch_selects_the_packed_retail_sequences() {
        assert_eq!(sequence_name(EclipseTorchSequence::Fast), "walk_down");
        assert_eq!(sequence_name(EclipseTorchSequence::Medium), "walk_up");
        assert_eq!(sequence_name(EclipseTorchSequence::Slow), "walk_right");
    }
}
