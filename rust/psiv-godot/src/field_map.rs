//! The field's map picture, its priority overlay, and the NPC sprite nodes.
//!
//! Split from the Godot bridge so the bridge stays below the repository's
//! one-thousand-line maintenance limit. The party sprite and the per-frame
//! animation half of the same drawing path stay in `field_visuals.rs`.

use godot::classes::{Image, ImageTexture, Sprite2D};
use godot::prelude::*;

use super::Field;
use super::view::{
    self, NpcNode, SheetView, camp_receipt_frame, camp_receipt_sheet, npc_pixel_position,
};

impl Field {
    /// Loads the current map's PNG and rebuilds NPC sprites.
    pub(crate) fn load_map_visuals(&mut self) {
        self.refresh_party_sheets();
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let id = runtime.map_id().0;
        // Surface every gap the effect layer knows about — silence here
        // would read as "fully patched" when it is not.
        let fx = runtime.map_effects();
        if fx.unresolved_layout_writes > 0 {
            godot_warn!(
                "map {id:#05x}: {} layout write(s) active but unresolved - collision/visual patch pending pack support",
                fx.unresolved_layout_writes
            );
        }
        if fx.undecoded_entries > 0 {
            godot_print!(
                "map {id:#05x}: {} undecoded effect entr(ies) - possibly incompletely patched",
                fx.undecoded_entries
            );
        }
        if fx.variant.is_some() {
            godot_print!("map {id:#05x}: layout variant active");
        }

        // Active layout_writes blit their 32px patch tiles over the baked
        // PNG — the doors MapDataManager opens have to LOOK open, not just
        // walk open. Collected before the image loads to keep borrows flat.
        let blits: Vec<(u32, u32, u32)> = fx.patch_blits.clone();
        let atlas = runtime
            .map_record()
            .and_then(|r| r.patch_tiles.clone())
            .filter(|_| !blits.is_empty());

        match runtime.map_png().map(str::to_owned) {
            Some(name) => {
                let path = format!("{}/{name}", self.pack_dir);
                match Image::load_from_file(&GString::from(path.as_str())) {
                    Some(mut image) => {
                        if let Some(tiles) = &atlas {
                            let atlas_path = format!("{}/{}", self.pack_dir, tiles.png);
                            match Image::load_from_file(&GString::from(atlas_path.as_str())) {
                                Some(atlas_img) => {
                                    let edge = tiles.tile_pixels as i32;
                                    for &(cx, cy, index) in &blits {
                                        let Some(tile) =
                                            tiles.tiles.iter().find(|t| t.index == index)
                                        else {
                                            godot_error!("patch tile {index} missing from atlas");
                                            continue;
                                        };
                                        image.blit_rect(
                                            &atlas_img,
                                            Rect2i::new(
                                                Vector2i::new(tile.x as i32, 0),
                                                Vector2i::new(edge, edge),
                                            ),
                                            Vector2i::new(cx as i32 * edge, cy as i32 * edge),
                                        );
                                    }
                                    godot_print!(
                                        "map {id:#05x}: {} patch tile(s) applied",
                                        blits.len()
                                    );
                                }
                                None => godot_error!("could not load patch atlas {atlas_path}"),
                            }
                        }
                        if let Some(texture) = ImageTexture::create_from_image(&image)
                            && let Some(sprite) = self.map_sprite.as_mut()
                        {
                            sprite.set_texture(&texture);
                            // A scene's InitVramAndCram may have blanked the
                            // field; a map redraw is what restores it — the
                            // actor sprites ride the same reload.
                            sprite.set_visible(true);
                            self.presentation.set_vram_blanked(false);
                        }
                    }
                    None => godot_error!("could not load map image {path}"),
                }
            }
            None => godot_error!("map {id:#05x} has no png declared in the pack"),
        }

        // The priority overlay: absent on the 22 maps with no priority tiles.
        let over = runtime.map_png_over().map(str::to_owned);
        if let Some(sprite) = self.overlay_sprite.as_mut() {
            match over {
                Some(name) => {
                    let path = format!("{}/{name}", self.pack_dir);
                    match Image::load_from_file(&GString::from(path.as_str())) {
                        Some(mut image) => {
                            // The overlay atlas mirrors the base one: same
                            // indices, above-sprites pixels only.
                            if let Some(tiles) = &atlas
                                && let Some(over_png) = &tiles.png_over
                            {
                                let atlas_path = format!("{}/{over_png}", self.pack_dir);
                                if let Some(atlas_img) =
                                    Image::load_from_file(&GString::from(atlas_path.as_str()))
                                {
                                    let edge = tiles.tile_pixels as i32;
                                    for &(cx, cy, index) in &blits {
                                        if let Some(tile) =
                                            tiles.tiles.iter().find(|t| t.index == index)
                                        {
                                            image.blit_rect(
                                                &atlas_img,
                                                Rect2i::new(
                                                    Vector2i::new(tile.x as i32, 0),
                                                    Vector2i::new(edge, edge),
                                                ),
                                                Vector2i::new(cx as i32 * edge, cy as i32 * edge),
                                            );
                                        }
                                    }
                                } else {
                                    godot_error!("could not load overlay atlas {atlas_path}");
                                }
                            }
                            match ImageTexture::create_from_image(&image) {
                                Some(texture) => {
                                    sprite.set_texture(&texture);
                                    sprite.set_visible(true);
                                }
                                None => godot_error!("could not texture overlay {path}"),
                            }
                        }
                        None => godot_error!("could not load overlay {path}"),
                    }
                }
                None => sprite.set_visible(false),
            }
        }

        for NpcNode { node, .. } in &mut self.npc_nodes {
            node.queue_free();
        }
        self.npc_nodes.clear();

        // Gather NPC draw info first; borrowing data and adding children at
        // the same time fights the base borrow.
        struct NpcDraw {
            index: usize,
            sheet: String,
            idle: String,
            // Object pixel coordinates, not cells: 85 retail objects sit on
            // half-cells (8px-scaled words), so x/y_pixels are authoritative.
            x: i32,
            y: i32,
        }
        let mut draws: Vec<NpcDraw> = Vec::new();
        if let Some(record) = runtime.map_record() {
            for (index, npc) in record.npcs.iter().enumerate() {
                // Invisible triggers (sprite_reason set) still block in the
                // engine, exactly like the cartridge's invisible objects, but
                // draw nothing. Despawned objects (engine `active` false —
                // the single source of truth) draw nothing either, which is
                // what keeps Alys's old self from resurrecting on rebuild.
                if !runtime.map().npcs().get(index).is_none_or(|n| n.active) {
                    continue;
                }
                let Some(sprite) = &npc.sprite else { continue };
                let selected = runtime
                    .map_effects()
                    .sprite_overrides
                    .get(&index)
                    .unwrap_or(&sprite.sheet);
                let sheet = camp_receipt_sheet(index, selected)
                    .filter(|receipt| runtime.data().sheet(receipt).is_some())
                    .map_or_else(|| selected.clone(), str::to_owned);
                draws.push(NpcDraw {
                    index,
                    sheet,
                    idle: sprite.idle_sequence.clone(),
                    x: npc.x_pixels as i32,
                    y: npc.y_pixels as i32,
                });
            }
            for (chest_index, chest) in record.treasure_chests.iter().enumerate() {
                let index = runtime.map().chest_slot_base() + chest_index;
                if !runtime
                    .map()
                    .npcs()
                    .get(index)
                    .is_some_and(|npc| npc.active)
                {
                    continue;
                }
                let Some(sprite) = &chest.sprite else {
                    continue;
                };
                draws.push(NpcDraw {
                    index,
                    sheet: sprite.sheet.clone(),
                    idle: sprite.idle_sequence.clone(),
                    x: chest.x_pixels as i32,
                    y: chest.y_pixels as i32,
                });
            }
            let missing: Vec<String> = draws
                .iter()
                .filter(|d| !self.sheet_views.contains_key(&d.sheet))
                .map(|d| d.sheet.clone())
                .collect();
            for sheet_id in missing {
                if let Some(sheet) = runtime.data().sheet(&sheet_id) {
                    if let Some(view) = SheetView::build(&self.pack_dir, sheet) {
                        self.sheet_views.insert(sheet_id, view);
                    } else {
                        godot_error!("sheet {sheet_id} png failed to load");
                    }
                }
            }
        }

        for draw in draws {
            let Some(view) = self.sheet_views.get(&draw.sheet) else {
                continue;
            };
            let live = self
                .runtime
                .as_ref()
                .and_then(|rt| rt.map().npcs().get(draw.index))
                .map(|npc| (npc.cell, (i32::from(npc.offset.x), i32::from(npc.offset.y))));
            let (base, spawn) = live.map_or(((draw.x, draw.y), (0, 0)), |(cell, offset)| {
                (
                    npc_pixel_position(cell, offset),
                    (i32::from(cell.x), i32::from(cell.y)),
                )
            });
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(5);
            let frame = camp_receipt_frame(draw.index, &draw.sheet)
                .unwrap_or_else(|| view.frame_at(&draw.idle, 0));
            view.apply(&mut node, frame);
            // "Draw a frame at (object_x - origin_x, object_y - origin_y) and
            // it lands exactly where the VDP would put it."
            node.set_position(Vector2::new(
                (base.0 - view.origin_x) as f32,
                (base.1 - view.origin_y
                    + view.frame_height
                    + view::camp_receipt_y_offset(draw.index, &draw.sheet)) as f32,
            ));
            self.base_mut().add_child(&node);
            self.npc_nodes.push(NpcNode {
                node,
                sheet: draw.sheet,
                idle: draw.idle,
                index: draw.index,
                base,
                spawn,
            });
        }
    }
}
