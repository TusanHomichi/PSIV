//! The field's map picture, its priority overlay, and the NPC sprite nodes.
//!
//! Split from the Godot bridge so the bridge stays below the repository's
//! one-thousand-line maintenance limit. The party sprite and the per-frame
//! animation half of the same drawing path stay in `field_visuals.rs`.

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, Material, Shader, ShaderMaterial, Sprite2D};
use godot::prelude::*;

use super::pack_assets::PackAssets;
use super::view::{NpcNode, SheetView, camp_receipt_frame, camp_receipt_sheet, npc_pixel_position};
use super::{Field, Session};

impl Field {
    /// Loads the current map's PNG and rebuilds NPC sprites.
    pub(crate) fn load_map_visuals(&mut self) {
        self.refresh_party_sheets();
        let Some(runtime) = self.session.as_ref().map(Session::runtime) else {
            return;
        };
        self.map_art_frames.clear();
        self.overlay_art_frames.clear();
        self.map_art_frame = 0;
        for node in &mut self.belt_tile_nodes {
            node.queue_free();
        }
        self.belt_tile_nodes.clear();
        self.belt_tile_texture = None;
        self.belt_tile_material = None;
        self.belt_tile_x.clear();
        self.belt_tile_edge = 0;
        self.belt_revision = 0;
        let id = runtime.map_id().0;
        let active_art_frame = usize::from(runtime.tile_art_frame());
        let palette_images = runtime.map_record().map(|r| &r.map_update_images);
        let art_cycles = runtime
            .map_record()
            .map(|r| r.art_cycle_images.clone())
            .unwrap_or_default();
        let has_conveyor_belts = runtime
            .map_record()
            .and_then(|r| r.map_updates.as_ref())
            .is_some_and(|updates| {
                updates
                    .iter()
                    .any(|e| matches!(e.program, psiv_data::UpdateProgram::ConveyorBelts { .. }))
            });
        let belt_atlas = runtime
            .map_record()
            .and_then(|r| r.patch_tiles.clone())
            .filter(|_| has_conveyor_belts);
        // Map pictures draw from the live CRAM only once it holds the map's
        // palette; a record without the 64-word shadow keeps its baked
        // colours on every picture (base, overlay, variant, patch atlas).
        let cram = runtime.cram();
        let palette = cram
            .live
            .then(|| map_palette_colors(&cram.words, cram.revision));
        // Surface every gap the effect layer knows about — silence here
        // would read as "fully patched" when it is not.
        let fx = runtime.map_effects();
        for skipped in &runtime.map_updates().unsupported {
            godot_warn!(
                "map {id:#05x}: update {:#04x} requires {} (#41)",
                skipped.index,
                skipped.missing
            );
        }
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
                let source = map_image_source(palette.is_some(), palette_images, &name);
                let (loaded, indexed) = load_map_image(&self.pack_assets, source, &name);
                match loaded {
                    Some(mut image) => {
                        if let Some(tiles) = &atlas {
                            // Match the map's colour representation: when a mod
                            // supplies a baked RGB/RGBA map PNG on a live-palette
                            // map, load the baked patch atlas so open doors blit
                            // cleanly instead of colliding with `_indices.png`.
                            let source = map_image_source(indexed, palette_images, &tiles.png);
                            match load_map_image(&self.pack_assets, source, &tiles.png) {
                                (Some(_), atlas_indexed) if atlas_indexed != indexed => {
                                    godot_error!(
                                        "patch atlas {} does not match the map's colours",
                                        tiles.png
                                    );
                                }
                                (Some(atlas_img), _) => {
                                    blit_patch_tiles(&mut image, &atlas_img, tiles, &blits, true);
                                    godot_print!(
                                        "map {id:#05x}: {} patch tile(s) applied",
                                        blits.len()
                                    );
                                }
                                (None, _) => {
                                    godot_error!("could not load patch atlas {}", tiles.png);
                                }
                            }
                        }
                        if let Some(texture) = ImageTexture::create_from_image(&image)
                            && let Some(sprite) = self.map_sprite.as_mut()
                        {
                            sprite.set_texture(&texture);
                            install_palette_material(sprite, palette.as_ref().filter(|_| indexed));
                            // A scene's InitVramAndCram may have blanked the
                            // field; a map redraw is what restores it — the
                            // actor sprites ride the same reload.
                            sprite.set_visible(true);
                            self.presentation.set_vram_blanked(false);
                            if let Some(frames) = art_cycles.get(&name).filter(|f| f.len() > 1) {
                                self.map_art_frames = build_art_cycle_textures(
                                    &self.pack_assets,
                                    palette_images,
                                    &name,
                                    &texture,
                                    indexed,
                                    frames,
                                    atlas.as_ref(),
                                    &blits,
                                    false,
                                );
                            }
                        }
                    }
                    None => {
                        godot_error!("could not load map image {name}");
                    }
                }
            }
            None => {
                godot_error!("map {id:#05x} has no png declared in the pack");
            }
        }

        // The priority overlay: absent on the 22 maps with no priority tiles.
        let over = runtime.map_png_over().map(str::to_owned);
        if let Some(sprite) = self.overlay_sprite.as_mut() {
            match over {
                Some(name) => {
                    let source = map_image_source(palette.is_some(), palette_images, &name);
                    let (loaded, indexed) = load_map_image(&self.pack_assets, source, &name);
                    match loaded {
                        Some(mut image) => {
                            // The overlay atlas mirrors the base one: same
                            // indices, above-sprites pixels only.
                            if let Some(tiles) = &atlas
                                && let Some(over_png) = &tiles.png_over
                            {
                                let source = map_image_source(indexed, palette_images, over_png);
                                let (atlas_img, atlas_indexed) =
                                    load_map_image(&self.pack_assets, source, over_png);
                                if atlas_img.is_some() && atlas_indexed != indexed {
                                    godot_error!(
                                        "overlay atlas {over_png} does not match the map's colours"
                                    );
                                } else if let Some(atlas_img) = atlas_img {
                                    blit_patch_tiles(&mut image, &atlas_img, tiles, &blits, false);
                                } else {
                                    godot_error!("could not load overlay atlas {over_png}");
                                }
                            }
                            match ImageTexture::create_from_image(&image) {
                                Some(texture) => {
                                    sprite.set_texture(&texture);
                                    install_palette_material(
                                        sprite,
                                        palette.as_ref().filter(|_| indexed),
                                    );
                                    sprite.set_visible(true);
                                    if let Some(frames) =
                                        art_cycles.get(&name).filter(|f| f.len() > 1)
                                    {
                                        self.overlay_art_frames = build_art_cycle_textures(
                                            &self.pack_assets,
                                            palette_images,
                                            &name,
                                            &texture,
                                            indexed,
                                            frames,
                                            atlas.as_ref(),
                                            &blits,
                                            true,
                                        );
                                    }
                                }
                                None => {
                                    godot_error!("could not texture overlay {name}");
                                }
                            }
                        }
                        None => {
                            godot_error!("could not load overlay {name}");
                        }
                    }
                }
                None => sprite.set_visible(false),
            }
        }

        if let Some(tiles) = &belt_atlas {
            let source = map_image_source(palette.is_some(), palette_images, &tiles.png);
            let (loaded, indexed) = load_map_image(&self.pack_assets, source, &tiles.png);
            if let Some(image) = loaded
                && let Some(texture) = ImageTexture::create_from_image(&image)
            {
                self.belt_tile_texture = Some(texture);
                self.belt_tile_material = palette
                    .as_ref()
                    .filter(|_| indexed)
                    .map(create_palette_material);
                self.belt_tile_edge = tiles.tile_pixels as i32;
                self.belt_tile_x = tiles
                    .tiles
                    .iter()
                    .map(|tile| (tile.index, tile.x as i32))
                    .collect();
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
                    if let Some(view) = SheetView::build(&self.pack_assets, sheet, Some(&self.cram))
                    {
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
                .runtime()
                .and_then(|rt| rt.map().npcs().get(draw.index))
                .map(|npc| (npc.cell, (i32::from(npc.offset.x), i32::from(npc.offset.y))));
            let base = live.map_or((draw.x, draw.y), |(cell, offset)| {
                npc_pixel_position(cell, offset)
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
                (base.1 - view.origin_y + view.frame_height) as f32,
            ));
            self.base_mut().add_child(&node);
            self.npc_nodes.push(NpcNode {
                node,
                sheet: draw.sheet,
                idle: draw.idle,
                index: draw.index,
                base,
            });
        }
        self.apply_map_art_frame(active_art_frame);
        self.sync_belt_tiles();
    }

    /// Updates conveyor-belt overlay sprites whenever the runtime's even-frame
    /// belt blit list advances (`MapUpdateView::belt_revision`, `#99`).
    pub(crate) fn sync_belt_tiles(&mut self) {
        let Some(texture) = self.belt_tile_texture.clone() else {
            return;
        };
        let edge = self.belt_tile_edge;
        if edge <= 0 {
            return;
        }
        let Some((revision, blits)) = self.session.as_ref().map(|session| {
            let updates = session.runtime().map_updates();
            (updates.belt_revision, updates.belt_blits.clone())
        }) else {
            return;
        };
        if revision == self.belt_revision {
            return;
        }
        self.belt_revision = revision;
        while self.belt_tile_nodes.len() > blits.len() {
            if let Some(mut node) = self.belt_tile_nodes.pop() {
                node.queue_free();
            }
        }
        while self.belt_tile_nodes.len() < blits.len() {
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(1);
            node.set_region_enabled(true);
            node.set_texture(&texture);
            if let Some(material) = self.belt_tile_material.as_ref() {
                node.set_material(material);
            }
            self.base_mut().add_child(&node);
            self.belt_tile_nodes.push(node);
        }
        let edge_f = edge as f32;
        for (node, &(cx, cy, tile_index)) in self.belt_tile_nodes.iter_mut().zip(&blits) {
            let Some(&tile_x) = self.belt_tile_x.get(&tile_index) else {
                node.set_visible(false);
                continue;
            };
            node.set_position(Vector2::new(
                (cx as i32 * edge) as f32,
                (cy as i32 * edge) as f32,
            ));
            node.set_region_rect(Rect2::new(
                Vector2::new(tile_x as f32, 0.0),
                Vector2::new(edge_f, edge_f),
            ));
            node.set_visible(true);
        }
    }

    /// Swaps the map and priority-overlay textures to match the runtime's
    /// live tile-DMA art-cycle frame (`Runtime::tile_art_frame`), leaving the
    /// live CRAM palette material intact.
    pub(crate) fn sync_map_art_frame(&mut self) {
        let Some(frame) = self
            .session
            .as_ref()
            .map(|session| usize::from(session.runtime().tile_art_frame()))
        else {
            return;
        };
        if frame != self.map_art_frame {
            self.apply_map_art_frame(frame);
        }
    }

    fn apply_map_art_frame(&mut self, frame: usize) {
        self.map_art_frame = frame;
        if let (Some(sprite), Some(texture)) =
            (self.map_sprite.as_mut(), self.map_art_frames.get(frame))
        {
            sprite.set_texture(texture);
        }
        if let (Some(sprite), Some(texture)) = (
            self.overlay_sprite.as_mut(),
            self.overlay_art_frames.get(frame),
        ) {
            sprite.set_texture(texture);
        }
    }
}

/// Blits active `patch_tiles` onto `image`, normalizing both images to `RGBA8`
/// first when their formats differ (for example an RGB8 mod map PNG paired with
/// an RGBA8 base patch atlas).
fn blit_patch_tiles(
    image: &mut Gd<Image>,
    atlas_img: &Gd<Image>,
    tiles: &psiv_data::PatchTiles,
    blits: &[(u32, u32, u32)],
    log_missing: bool,
) {
    let mut converted_atlas: Option<Gd<Image>> = None;
    let src: &Gd<Image> = if image.get_format() != atlas_img.get_format() {
        image.convert(Format::RGBA8);
        let mut copy = atlas_img.duplicate_resource();
        copy.convert(Format::RGBA8);
        converted_atlas.insert(copy)
    } else {
        atlas_img
    };
    let edge = tiles.tile_pixels as i32;
    for &(cx, cy, index) in blits {
        let Some(tile) = tiles.tiles.iter().find(|t| t.index == index) else {
            if log_missing {
                godot_error!("patch tile {index} missing from atlas");
            }
            continue;
        };
        image.blit_rect(
            src,
            Rect2i::new(Vector2i::new(tile.x as i32, 0), Vector2i::new(edge, edge)),
            Vector2i::new(cx as i32 * edge, cy as i32 * edge),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn build_art_cycle_textures(
    assets: &PackAssets,
    palette_images: Option<&std::collections::BTreeMap<String, String>>,
    base_name: &str,
    base_texture: &Gd<ImageTexture>,
    base_indexed: bool,
    frame_names: &[String],
    atlas: Option<&psiv_data::PatchTiles>,
    blits: &[(u32, u32, u32)],
    overlay: bool,
) -> Vec<Gd<ImageTexture>> {
    let atlas_img = atlas.and_then(|tiles| {
        let atlas_name = if overlay {
            tiles.png_over.as_deref()?
        } else {
            tiles.png.as_str()
        };
        let source = map_image_source(base_indexed, palette_images, atlas_name);
        let (img, indexed) = load_map_image(assets, source, atlas_name);
        (indexed == base_indexed).then_some(img).flatten()
    });
    frame_names
        .iter()
        .map(|frame_name| {
            if frame_name == base_name {
                return base_texture.clone();
            }
            let source = map_image_source(base_indexed, palette_images, frame_name);
            let (loaded, frame_indexed) = load_map_image(assets, source, frame_name);
            if frame_indexed != base_indexed {
                godot_error!("art-cycle frame {frame_name} does not match the map's colours");
                return base_texture.clone();
            }
            let Some(mut image) = loaded else {
                godot_error!("could not load art-cycle frame {frame_name}");
                return base_texture.clone();
            };
            if let (Some(tiles), Some(atlas_img)) = (atlas, atlas_img.as_ref()) {
                blit_patch_tiles(&mut image, atlas_img, tiles, blits, false);
            }
            ImageTexture::create_from_image(&image).unwrap_or_else(|| {
                godot_error!("could not texture art-cycle frame {frame_name}");
                base_texture.clone()
            })
        })
        .collect()
}

// Indexed textures are an additive pack input. Timing, gates and color writes
// have already been decided by the runtime; this shader only looks up a slot.
const MAP_PALETTE_SHADER: &str = r#"
shader_type canvas_item;
render_mode unshaded;
uniform vec4 palette[32];
uniform int revision = -1;
void fragment() {
    vec4 pixel = texture(TEXTURE, UV);
    int slot = clamp(int(round(pixel.r * 255.0)), 0, 31);
    COLOR = vec4(palette[slot].rgb, pixel.a);
}
"#;

/// Where a map picture's pixels come from.
#[derive(Debug, PartialEq, Eq)]
enum MapImageSource<'a> {
    /// The pack's own index image for it, by file name.
    PackIndex(&'a str),
    /// The picture itself, rewritten to indices at load
    /// ([`crate::cram_art::load_index_image`]).
    Rewrite,
    /// The picture's baked colours: the palette is not live.
    Baked,
}

/// How a map picture `name` (base, overlay, layout variant or patch atlas)
/// is drawn: from the live CRAM, through the map shader, only while `live`
/// (the record carries the 64-word shadow the shader's palette comes from).
fn map_image_source<'a>(
    live: bool,
    images: Option<&'a std::collections::BTreeMap<String, String>>,
    name: &str,
) -> MapImageSource<'a> {
    if !live {
        return MapImageSource::Baked;
    }
    match images.and_then(|images| images.get(name)) {
        Some(indexed) => MapImageSource::PackIndex(indexed),
        None => MapImageSource::Rewrite,
    }
}

/// The picture `name` from `source`, with whether its pixels are palette
/// indices for the map shader. A picture that is not an indexed PNG falls
/// back to its baked colours.
fn load_map_image(
    assets: &PackAssets,
    source: MapImageSource<'_>,
    name: &str,
) -> (Option<Gd<Image>>, bool) {
    match source {
        MapImageSource::PackIndex(indexed) => (assets.load_image(indexed), true),
        MapImageSource::Rewrite => match crate::cram_art::load_index_image(assets, name) {
            Some(image) => (Some(image), true),
            None => (assets.load_image(name), false),
        },
        MapImageSource::Baked => (assets.load_image(name), false),
    }
}

/// The map shader's palette, lines 0 and 1 of `words`, with the revision it
/// was made from.
pub(crate) struct MapPalette {
    colors: PackedColorArray,
    revision: i64,
}

pub(crate) fn map_palette_colors(words: &[u16; 64], revision: u64) -> MapPalette {
    let mut colors = PackedColorArray::new();
    for &word in &words[..32] {
        let [r, g, b] = psiv_data::color::cram_rgb(word);
        colors.push(Color::from_rgba8(r, g, b, 255));
    }
    MapPalette {
        colors,
        revision: revision as i64,
    }
}

/// Points a palette shader material at `palette`, unless it already holds that
/// revision.
pub(crate) fn upload_map_palette_material(material: &mut Gd<ShaderMaterial>, palette: &MapPalette) {
    if material
        .get_shader_parameter("revision")
        .try_to::<i64>()
        .ok()
        == Some(palette.revision)
    {
        return;
    }
    material.set_shader_parameter("palette", &palette.colors.to_variant());
    material.set_shader_parameter("revision", &palette.revision.to_variant());
}

/// Points a map sprite's shader at `palette`, unless it already holds that
/// revision. A sprite without the shader is left alone.
pub(crate) fn upload_map_palette(sprite: &mut Gd<Sprite2D>, palette: &MapPalette) {
    let Some(mut material) = sprite
        .get_material()
        .and_then(|m| m.try_cast::<ShaderMaterial>().ok())
    else {
        return;
    };
    upload_map_palette_material(&mut material, palette);
}

fn create_palette_material(palette: &MapPalette) -> Gd<ShaderMaterial> {
    let mut shader = Shader::new_gd();
    shader.set_code(MAP_PALETTE_SHADER);
    let mut material = ShaderMaterial::new_gd();
    material.set_shader(&shader);
    upload_map_palette_material(&mut material, palette);
    material
}

/// Draws an indexed map picture through the palette shader, its palette set
/// before the first frame draws it; `None` draws the baked colours.
fn install_palette_material(sprite: &mut Gd<Sprite2D>, palette: Option<&MapPalette>) {
    if let Some(palette) = palette {
        let material = create_palette_material(palette);
        sprite.set_material(&material);
    } else {
        sprite.set_material(Gd::<Material>::null_arg());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{MapImageSource, map_image_source};

    /// Indexed pictures only with a live palette, for every picture of the
    /// map alike; the pack's index image wins over the load-time rewrite.
    #[test]
    fn map_pictures_are_indexed_only_with_a_live_palette() {
        let images: BTreeMap<String, String> =
            [("map.png".to_owned(), "map_indices.png".to_owned())].into();
        for name in ["map.png", "map_over.png", "atlas.png"] {
            assert_eq!(
                map_image_source(false, Some(&images), name),
                MapImageSource::Baked
            );
            assert_eq!(map_image_source(false, None, name), MapImageSource::Baked);
            assert_eq!(map_image_source(true, None, name), MapImageSource::Rewrite);
        }
        assert_eq!(
            map_image_source(true, Some(&images), "map.png"),
            MapImageSource::PackIndex("map_indices.png")
        );
        assert_eq!(
            map_image_source(true, Some(&images), "atlas.png"),
            MapImageSource::Rewrite
        );
    }

    /// Replacing `assets/maps/010_Piata.png` without `_indices.png` causes
    /// `PackOverlay` to drop the `map_update_images` redirect for that PNG so
    /// the renderer routes through `Rewrite` (live CRAM if indexed, baked if
    /// RGB/RGBA) instead of loading the base `_indices.png`; patch atlases then
    /// follow the map's actual `indexed` state (`false` -> `Baked`).
    #[test]
    fn reconciled_mod_map_png_routes_to_rewrite_and_baked_atlas_follows() {
        let mut images: BTreeMap<String, String> = [
            (
                "maps/010_Piata.png".to_owned(),
                "maps/010_Piata_indices.png".to_owned(),
            ),
            (
                "maps/010_Piata_over.png".to_owned(),
                "maps/010_Piata_over_indices.png".to_owned(),
            ),
            (
                "maps/010_Piata_patch.png".to_owned(),
                "maps/010_Piata_patch_indices.png".to_owned(),
            ),
        ]
        .into();

        // Unmodded (or mod that explicitly replaced `_indices.png`): PackIndex wins.
        assert_eq!(
            map_image_source(true, Some(&images), "maps/010_Piata.png"),
            MapImageSource::PackIndex("maps/010_Piata_indices.png")
        );

        // After `PackOverlay` drops the stale redirect for a replaced `.png`:
        images.remove("maps/010_Piata.png");
        assert_eq!(
            map_image_source(true, Some(&images), "maps/010_Piata.png"),
            MapImageSource::Rewrite
        );
        // Untouched overlay keeps its live-palette index image.
        assert_eq!(
            map_image_source(true, Some(&images), "maps/010_Piata_over.png"),
            MapImageSource::PackIndex("maps/010_Piata_over_indices.png")
        );
        // If the mod's `.png` was baked RGB/RGBA (`indexed == false`), the
        // patch atlas resolves to `Baked` so `blit_rect` matches colour spaces.
        assert_eq!(
            map_image_source(false, Some(&images), "maps/010_Piata_patch.png"),
            MapImageSource::Baked
        );
    }
}
