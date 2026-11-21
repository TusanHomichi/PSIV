//! Rewritten map NPCs whose scene routine loads a temporary-object sheet
//! (for example, `StripClub` object 0 rewritten to `$34C` after `LoadArt` `$372`).

use std::collections::HashSet;

use godot::classes::Sprite2D;
use godot::prelude::*;

use super::Field;
use super::view::sequence_name;

struct RewrittenNpc {
    slot: usize,
    object_id: u16,
    art_tile: u16,
    facing: psiv_core::Direction,
    pos: (i32, i32),
}

impl Field {
    pub(crate) fn sync_rewritten_npcs(
        &mut self,
        scene_sprites_visible: bool,
        custom_slots: &mut HashSet<usize>,
    ) {
        let rewritten_npcs: Vec<RewrittenNpc> = self
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
                            let art_tile = rewritten_npc_art_tile(
                                self.presentation.npc_art_tile(slot),
                                rec.art_tile,
                            );
                            let pos = (
                                i32::from(npc.cell.x) * 16 + i32::from(npc.offset.x),
                                (i32::from(npc.cell.y) - 1) * 16 + i32::from(npc.offset.y),
                            );
                            Some(RewrittenNpc {
                                slot,
                                object_id: npc.id.0,
                                art_tile,
                                facing: npc.facing,
                                pos,
                            })
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();
        for RewrittenNpc {
            slot,
            object_id,
            art_tile,
            facing,
            pos,
        } in rewritten_npcs
        {
            let Some(asset) = self
                .cutscene_layer
                .as_ref()
                .and_then(|layer| layer.bind().temporary_asset(object_id, art_tile))
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
                let (seq, age) = match asset.playback_sequence.as_deref() {
                    Some(playback) => (
                        playback.to_owned(),
                        self.presentation.rewritten_age(slot, self.anim_tick),
                    ),
                    None => (sequence_name("idle", facing), self.anim_tick),
                };
                let frame = asset.frame_at(&seq, age);
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

fn rewritten_npc_art_tile(override_tile: Option<u16>, record_tile: u32) -> u16 {
    override_tile.unwrap_or(record_tile as u16)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::rewritten_npc_art_tile;

    #[test]
    fn rewritten_npc_lookup_requires_matching_art_tile_as_well_as_routine() {
        let mut assets = BTreeMap::new();
        assets.insert((0x194u16, 0x202u16), "sekreas");
        assets.insert((0x34Cu16, 0x372u16), "strip_club_customer");

        // Event_RanchOwner rewrites object 0 to routine $194 while retaining
        // its original art tile $24D: must NOT match Sekreas ($194, $202).
        let ranch_tile = rewritten_npc_art_tile(None, 0x24D);
        assert_eq!(assets.get(&(0x194, ranch_tile)), None);

        // Event_StrippersAppearing rewrites object 0 to routine $34C on its
        // record tile $372: matches StripClubCustomer ($34C, $372).
        let customer_tile = rewritten_npc_art_tile(None, 0x372);
        assert_eq!(
            assets.get(&(0x34C, customer_tile)),
            Some(&"strip_club_customer")
        );

        // An explicit SetArtTile override wins over the map record's tile.
        let overridden = rewritten_npc_art_tile(Some(0x202), 0x24D);
        assert_eq!(assets.get(&(0x194, overridden)), Some(&"sekreas"));
    }
}
