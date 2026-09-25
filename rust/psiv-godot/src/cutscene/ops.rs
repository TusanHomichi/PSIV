//! The shell's half of the presentation seam: one ordered `SceneOp` at a time.
//!
//! `psiv-runtime` emits typed, ordered ops and owns the field; this module maps
//! each op onto the layer, the audio surface and the shell's own state, in the
//! order the runtime emitted it. It advances no scene and sets no scene flag.

use godot::classes::Input;
use godot::prelude::*;
use psiv_core::{PresentationOp, SceneOp};

use crate::Field;
use crate::transitions::TransitionKind;

impl Field {
    /// Consume one presentation op in the exact event order the runtime
    /// emitted it. State-changing scene ops remain in the runtime; this only
    /// updates the shell's drawable/audio surfaces.
    pub(crate) fn consume_scene_op(&mut self, op: SceneOp) {
        self.presentation.op_count = self.presentation.op_count.saturating_add(1);
        // The tick prefix is what lets a debug capture be paired with an
        // oracle tape frame: the op log becomes a timeline, not just an order.
        if std::env::var_os("PSIV_DEBUG_SCENE_TICKS").is_some() {
            godot_print!("scene op t{}: {op:?}", self.anim_tick);
        }
        match op {
            SceneOp::FadeIn => self.start_transition(TransitionKind::SceneFadeIn),
            SceneOp::FadeOut => self.start_transition(TransitionKind::SceneFadeOut),
            SceneOp::InitVramAndCram => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_all();
                }
                // On hardware this wipes the tile planes AND the sprite art:
                // map and actors are gone until the scene's own
                // LoadMap/RefreshMap reloads them (the oracle shows the
                // opening's first dialogue over pure black — no sprites —
                // before its LoadMap).
                self.set_field_map_visible(false);
                self.presentation.set_vram_blanked(true);
            }
            SceneOp::LoadPalette { rom_addr, words } => {
                self.presentation.loaded_palettes.insert(rom_addr, words);
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind().load_palette(rom_addr, words);
                }
                godot_print!("scene palette: {words} words from {rom_addr:#08x}");
            }
            SceneOp::LoadArt { rom_addr, tile } => {
                self.presentation.load_art(rom_addr, tile);
                godot_print!("scene art: {rom_addr:#08x} -> VRAM tile {tile:#05x}");
            }
            // Camera ops are consumed by the runtime when it translates the
            // effect (the camera is simulation state, not presentation);
            // nothing to do here beyond acknowledging the op.
            SceneOp::SetCameraPos { .. } | SceneOp::MoveCamera { .. } => {}
            SceneOp::LoadTitleImage { .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().begin_opening();
                }
                godot_print!("scene title image loaded");
            }
            SceneOp::SetTextColour { colour } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().set_text_colour(colour);
                }
            }
            SceneOp::DrawTextToPlane { entry, .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer
                        .bind_mut()
                        .draw_text(self.presentation.current_dialogue_tree, entry);
                }
            }
            SceneOp::IntroTextFadeUp => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().fade_text(1);
                }
            }
            SceneOp::IntroTextFadeDown => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().fade_text(-1);
                }
            }
            SceneOp::PlaySound { id } => self.play_scene_sound(id),
            SceneOp::SetSavedMusic { id } => self.save_scene_music(id),
            SceneOp::SetRenderSpritesInCutscene { enabled } => {
                self.presentation.set_render_sprites(enabled);
                self.apply_scene_sprite_visibility();
            }
            SceneOp::WaitFrames { frames } => {
                godot_print!("scene wait-frames: {frames}");
            }
            SceneOp::WaitForStart => {
                self.presentation.ending_waiting_for_start = true;
                godot_print!("scene waiting for ending Start input");
            }
            SceneOp::MarkGameCleared => {
                godot_print!("scene marked game cleared");
            }
            SceneOp::PanelCreate { id } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_create(id);
                }
            }
            SceneOp::PanelDestroy { id } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy(id);
                }
            }
            SceneOp::PanelDestroyLast => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_last();
                }
            }
            SceneOp::PanelDestroyAll => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().panel_destroy_all();
                }
            }
            SceneOp::DmaPlanes => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().dma_planes();
                }
            }
            SceneOp::ObjectAnimation {
                slot,
                object_id,
                art_tile,
                frames,
            } => {
                godot_print!(
                    "scene temporary object: slot {slot}, id {object_id:#06x}, art {art_tile:#05x}, frames {frames}"
                );
                self.presentation
                    .object_animation(slot, object_id, art_tile, frames);
            }
            SceneOp::CreateFieldObject {
                slot,
                object_id,
                art_tile,
                x,
                y,
            } => {
                self.presentation
                    .create_field_object(slot, object_id, art_tile, x, y);
            }
            SceneOp::StepFieldObject {
                slot,
                step_x,
                step_y,
                frames,
            } => {
                self.presentation
                    .step_field_object(slot, step_x, step_y, frames);
            }
            SceneOp::Presentation { op } => self.consume_presentation_op(op),
            SceneOp::SetArtTile { actor, tile } => {
                godot_print!("scene art tile: {actor:?} -> {tile:#05x}");
            }
            SceneOp::SetDialogueTree { rom_addr } => {
                self.presentation.current_dialogue_tree = Some(rom_addr);
            }
            SceneOp::ReloadMapPalette
            | SceneOp::OverlapCharacters
            | SceneOp::SetFollowMode { .. }
            | SceneOp::SetStepOffset { .. }
            | SceneOp::RecoverStats
            | SceneOp::SwapCharSlots { .. }
            | SceneOp::RemoveItem { .. } => {
                godot_print!("scene presentation op consumed: {op:?}");
            }
            _ => {}
        }
    }

    fn consume_presentation_op(&mut self, op: PresentationOp) {
        match op {
            PresentationOp::SetCharacterVisible { who, visible } => {
                if visible {
                    self.presentation.hidden_characters.remove(&who.0);
                } else {
                    self.presentation.hidden_characters.insert(who.0);
                }
            }
            PresentationOp::ReloadMapChunks | PresentationOp::RebuildSprites => {
                self.load_map_visuals();
            }
            PresentationOp::LoadSceneAsset { .. }
            | PresentationOp::SetGameMode { .. }
            | PresentationOp::ClearHeldInput
            | PresentationOp::SavePartySpriteX { .. }
            | PresentationOp::RestorePartySpriteX
            | PresentationOp::SetDialoguePortrait { .. } => {
                godot_print!("scene presentation record consumed: {op:?}");
            }
            PresentationOp::WindowDestroy { .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().window_destroy();
                }
            }
            PresentationOp::WindowCreate { .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().window_create();
                }
            }
            PresentationOp::LoadWindowTiles { asset, .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind().load_window_tiles(asset);
                }
            }
            PresentationOp::LoadPortrait { asset, .. } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().load_portrait(asset);
                }
            }
            PresentationOp::DrawPortrait {
                x,
                y,
                width,
                height,
                ..
            } => {
                if let Some(layer) = self.cutscene_layer.as_mut() {
                    layer.bind_mut().draw_portrait(x, y, width, height);
                }
            }
            PresentationOp::SetPaletteWords {
                offset,
                first,
                second,
            } => {
                godot_print!(
                    "scene palette words: offset {offset:#06x}, {first:#06x}, {second:#06x}"
                );
            }
            PresentationOp::AddMacro { slot } => {
                godot_print!("scene macro added for party slot {slot}");
            }
            PresentationOp::SetObjectDestination { slot, x, y } => {
                self.presentation.object_destination(slot, x, y);
            }
            PresentationOp::FadeToRed { lines } => self.start_red_palette(lines, false),
            PresentationOp::FadeFromRed { lines } => self.start_red_palette(lines, true),
            op @ (PresentationOp::CopyRamWords { .. }
            | PresentationOp::ClearRamWords { .. }
            | PresentationOp::ClearRamLongs { .. }
            | PresentationOp::FillRamWords { .. }
            | PresentationOp::ClearPlanes
            | PresentationOp::EndingFinaleFieldPrep { .. }
            | PresentationOp::DmaPlanesLoop { .. }
            | PresentationOp::PaletteIncreaseTone { .. }
            | PresentationOp::ClearPaletteLine { .. }
            | PresentationOp::VariablePaletteFade { .. }
            | PresentationOp::RykrosPaletteCycle { .. }
            | PresentationOp::CameraToActor { .. }
            | PresentationOp::RajaSickTemporaryObject { .. }
            | PresentationOp::RajaSickResetRaja { .. }
            | PresentationOp::RajaSickArrangeParty { .. }
            | PresentationOp::EndingCreditsAssets { .. }
            | PresentationOp::EndingCreditsStage { .. }
            | PresentationOp::EndingCreditsPaletteRamp { .. }
            | PresentationOp::EndingStaffRollTransition { .. }
            | PresentationOp::EndingFinale { .. }) => {
                godot_print!("scene presentation record consumed: {op:?}");
            }
        }
    }

    pub(crate) fn tick_cutscene_presentation(&mut self) {
        if self.presentation.ending_waiting_for_start
            && Input::singleton().is_action_just_pressed("ui_accept")
        {
            self.presentation.ending_waiting_for_start = false;
            if let Some(runtime) = self.runtime.as_mut() {
                runtime.ending_continue();
            }
        }
        self.presentation.advance_objects();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            layer.bind_mut().tick();
            layer.bind_mut().place();
        }
    }

    pub(crate) fn finish_cutscene_presentation(&mut self) {
        self.clear_red_palette();
        let restored = self.restore_saved_music();
        self.presentation.reset_scene();
        if let Some(layer) = self.cutscene_layer.as_mut() {
            layer.bind_mut().end_opening();
            layer.bind_mut().panel_destroy_all();
        }
        if !restored {
            self.play_map_music();
        }
        self.apply_scene_sprite_visibility();
    }

    pub(super) fn apply_scene_sprite_visibility(&mut self) {
        let event = self.runtime.as_ref().and_then(|rt| rt.scene_event());
        let visible = self.presentation.sprites_visible(event);
        if let Some(party) = self.party.as_mut() {
            party.set_visible(visible);
        }
        for follower in &mut self.follower_nodes {
            follower.0.set_visible(visible);
        }
        for npc in &mut self.npc_nodes {
            npc.node.set_visible(visible);
        }
        let has_temporary = !self.presentation.temporary_objects.is_empty();
        for node in self.temporary_nodes.values_mut() {
            node.set_visible(visible && has_temporary);
        }
    }
}
