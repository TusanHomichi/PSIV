//! Rewritten map NPCs whose scene routine loads a temporary-object sheet
//! (for example, `StripClub` object 0 rewritten to `$34C` after `LoadArt` `$372`).

use std::collections::HashSet;

use godot::classes::Sprite2D;
use godot::prelude::*;

use super::Field;
use super::view::sequence_name;

impl Field {
    pub(crate) fn sync_rewritten_npcs(
        &mut self,
        scene_sprites_visible: bool,
        custom_slots: &mut HashSet<usize>,
    ) {
        let rewritten_npcs: Vec<(usize, u16, psiv_core::Direction, (i32, i32))> = self
            .runtime()
            .and_then(|rt| {
                let record = rt.map_record()?;
                Some(
                    rt.map()
                        .npcs()
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, npc)| {
                            let rec = record.npcs.get(slot)?;
                            if !npc.active
                                || npc.id.0 == rec.object_id
                                || custom_slots.contains(&slot)
                            {
                                return None;
                            }
                            let pos = (
                                i32::from(npc.cell.x) * 16 + i32::from(npc.offset.x),
                                (i32::from(npc.cell.y) - 1) * 16 + i32::from(npc.offset.y),
                            );
                            Some((slot, npc.id.0, npc.facing, pos))
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        for (slot, object_id, facing, pos) in rewritten_npcs {
            let Some(asset) = self
                .cutscene_layer
                .as_ref()
                .and_then(|layer| layer.bind().temporary_asset_by_object(object_id))
            else {
                continue;
            };
            if !asset.art_loaded(&self.presentation) {
                continue;
            }
            if !self.temporary_nodes.contains_key(&slot) {
                let mut node = Sprite2D::new_alloc();
                node.set_centered(false);
                node.set_z_index(5);
                self.base_mut().add_child(&node);
                self.temporary_nodes.insert(slot, node);
            }
            if let Some(node) = self.temporary_nodes.get_mut(&slot) {
                let seq = sequence_name("idle", facing);
                let frame = asset.frame_at(&seq, self.anim_tick);
                asset.apply(node, frame, &self.cram.borrow());
                node.set_position(Vector2::new(
                    (pos.0 - asset.origin_x) as f32,
                    (pos.1 - asset.origin_y + asset.frame_height) as f32,
                ));
                node.set_visible(scene_sprites_visible);
                custom_slots.insert(slot);
            }
        }
    }
}
