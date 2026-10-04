//! Sprite construction for one battle: background, enemies, party.
//!
//! Lifted out of the single `battle/ui.rs` file with the S3 node. Every value
//! here comes from the pack (art tables, formation positions, party sheets);
//! nothing is decided.

use godot::classes::{Image, ImageTexture, Sprite2D};
use godot::prelude::*;

use psiv_core::battle::FighterId;

use super::super::attack::{EnemySprite, enemy_sprite_origin};
use super::super::enemy_overlay::EnemyAnimation;
use super::super::vehicle::texture as vehicle_texture;
use super::{BATTLE_BACKGROUND_HEIGHT, BATTLE_BACKGROUND_WIDTH, BATTLE_CELL_PIXELS, BattleScreen};
use crate::battle::{BattleSetup, EnemyPlacement};

impl BattleScreen {
    pub(super) fn clear_sprites(&mut self) {
        if let Some(mut background) = self.background.take() {
            background.queue_free();
        }
        for mut enemy in self.enemy_nodes.drain(..) {
            enemy.node.queue_free();
        }
        for mut party in self.party_nodes.drain(..) {
            party.node.queue_free();
        }
    }

    pub(super) fn build_background(&mut self, setup: &BattleSetup) {
        let Some(art) = self.art.as_ref() else {
            return;
        };
        let selected = art
            .background_path(
                setup.event_battle,
                setup.map_id,
                setup.motavia_terrain,
                setup.dark_force_2,
                setup.vehicle_index.is_some(),
            )
            .or_else(|| {
                godot_error!(
                    "battle background selection has no asset for map {:#05x}; using background 0 provisionally",
                    setup.map_id
                );
                art.background_path_by_index(0)
            });
        let mut node = Sprite2D::new_alloc();
        node.set_centered(false);
        node.set_z_index(-20);
        // The §1 background is 512x192 at world origin; the 320x224 camera
        // frame shows its left 320 pixels and leaves the bottom UI strip clear.
        node.set_position(Vector2::ZERO);
        if let Some(path) = selected {
            let full = format!("{}/{}", self.pack_dir, path);
            match Image::load_from_file(&GString::from(full.as_str())) {
                Some(image) => {
                    if image.get_width() != BATTLE_BACKGROUND_WIDTH
                        || image.get_height() != BATTLE_BACKGROUND_HEIGHT
                    {
                        godot_error!(
                            "battle background {full} is {}x{}, expected {}x{}",
                            image.get_width(),
                            image.get_height(),
                            BATTLE_BACKGROUND_WIDTH,
                            BATTLE_BACKGROUND_HEIGHT
                        );
                    }
                    match ImageTexture::create_from_image(&image) {
                        Some(texture) => node.set_texture(&texture),
                        None => godot_error!("battle background texture creation failed: {full}"),
                    }
                }
                None => godot_error!("battle background image failed to load: {full}"),
            }
        }
        self.base_mut().add_child(&node);
        self.background = Some(node);
    }

    pub(super) fn build_enemies(
        &mut self,
        enemies: &[EnemyPlacement],
        animation_phase_ticks: usize,
    ) {
        if self.art.is_none() {
            return;
        }
        for enemy in enemies {
            let Some(fighter) = FighterId::new(enemy.fighter_id) else {
                godot_error!(
                    "battle formation has invalid enemy fighter id {}",
                    enemy.fighter_id
                );
                continue;
            };
            let (width, height) = self
                .art
                .as_ref()
                .and_then(|art| art.enemy_size(enemy.enemy_id))
                .unwrap_or((0, 0));
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(-10);
            let (x, y) = enemy_sprite_origin(enemy.position, width, height);
            node.set_position(Vector2::new(x as f32, y as f32));
            let mut animation = self.art.as_ref().and_then(|art| {
                art.enemy_animation(
                    &self.pack_dir,
                    enemy.enemy_id,
                    enemy.position,
                    animation_phase_ticks,
                )
            });
            let line = super::super::art::enemy_cram_line(enemy.position);
            let key = (enemy.enemy_id, line);
            let texture = if let Some(current) = animation.as_ref().map(EnemyAnimation::texture) {
                Some(current)
            } else {
                self.enemy_textures.get(&key).cloned().or_else(|| {
                    let texture = self.art.as_ref().and_then(|art| {
                        art.enemy_texture(&self.pack_dir, enemy.enemy_id, enemy.position)
                    });
                    if let Some(texture) = texture.as_ref() {
                        self.enemy_textures.insert(key, texture.clone());
                    } else {
                        godot_error!(
                            "battle enemy {} body failed to load for fighter {}",
                            enemy.enemy_id,
                            enemy.fighter_id
                        );
                    }
                    texture
                })
            };
            if let Some(texture) = texture {
                node.set_texture(&texture);
            }
            self.base_mut().add_child(&node);
            self.enemy_positions
                .insert(enemy.fighter_id, enemy.position);
            self.enemy_nodes.push(EnemySprite {
                fighter,
                enemy_id: enemy.enemy_id,
                node,
                animation: animation.take(),
                attack: None,
            });
        }
    }

    /// Redraws one enemy slot as another record: Fusion seats a MetaSlug in
    /// slot 1 at the position byte `loc_1A2F4` carries (`ps4.asm:35847`). The
    /// body, its idle overlay clock and its damage column are rebuilt exactly
    /// as the battle's own load builds them, because the cartridge rebuilds
    /// the enemy side through that same load (`loc_14D46`, `ps4.asm:29735`).
    pub(super) fn reseat_enemy(&mut self, fighter: u8, enemy_id: u16, position: u8) {
        let Some(art) = self.art.as_ref() else {
            return;
        };
        let (width, height) = art.enemy_size(enemy_id).unwrap_or((0, 0));
        let (x, y) = enemy_sprite_origin(position, width, height);
        let texture = art.enemy_texture(&self.pack_dir, enemy_id, position);
        let animation = art.enemy_animation(&self.pack_dir, enemy_id, position, 0);
        let shown = animation.as_ref().map(EnemyAnimation::texture).or(texture);
        let Some(enemy) = self
            .enemy_nodes
            .iter_mut()
            .find(|enemy| enemy.fighter.get() == fighter)
        else {
            return;
        };
        let Some(shown) = shown else {
            godot_error!("battle enemy {enemy_id} body failed to load for fighter {fighter}");
            return;
        };
        enemy.clear_attack();
        enemy.enemy_id = enemy_id;
        enemy.animation = animation;
        enemy.node.set_texture(&shown);
        enemy.node.set_position(Vector2::new(x as f32, y as f32));
        self.enemy_positions.insert(fighter, position);
    }

    pub(super) fn build_party(&mut self, setup: &BattleSetup) {
        let party = &setup.party;
        let vehicle_surface = setup.vehicle_index.is_some();
        if self.art.is_none() && !vehicle_surface {
            return;
        }
        for member in party {
            let Some(fighter) = FighterId::new(member.fighter_id) else {
                godot_error!("battle party has invalid fighter id {}", member.fighter_id);
                continue;
            };
            let idle = setup
                .vehicle_png
                .as_deref()
                .zip(setup.vehicle_frame)
                .and_then(|(path, (width, height))| {
                    vehicle_texture(&self.pack_dir, path, width, height)
                })
                .or_else(|| {
                    self.art
                        .as_ref()
                        .and_then(|art| art.character_texture(&self.pack_dir, member.character, 0))
                });
            let Some(idle) = idle else {
                godot_error!(
                    "battle character {} idle pose failed to load for fighter {}",
                    member.character,
                    member.fighter_id
                );
                continue;
            };
            let attack = if vehicle_surface {
                None
            } else {
                self.art
                    .as_ref()
                    .and_then(|art| art.character_texture(&self.pack_dir, member.character, 1))
            };
            let mut node = Sprite2D::new_alloc();
            node.set_centered(false);
            node.set_z_index(-10);
            let column = member
                .fighter_id
                .checked_sub(1)
                .and_then(|index| super::PARTY_COLUMNS.get(index as usize).copied())
                .unwrap_or(0);
            node.set_position(Vector2::new(
                column as f32 * BATTLE_CELL_PIXELS as f32,
                super::PARTY_ROW_Y,
            ));
            node.set_texture(&idle);
            self.base_mut().add_child(&node);
            self.party_nodes.push(super::PartySprite {
                fighter,
                node,
                idle,
                attack,
            });
        }
    }
}
