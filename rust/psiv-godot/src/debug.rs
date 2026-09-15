use super::*;

impl Field {
    /// Read-only observation for input-driven native regression routes.
    pub(super) fn play_probe(&self) -> GString {
        if std::env::var("PSIV_DEBUG_ROUTE").as_deref() != Ok("1") {
            return GString::new();
        }
        let Some(rt) = self.runtime.as_ref() else {
            return GString::new();
        };
        let dialogue = self.dialogue.as_ref().is_some_and(|w| w.bind().is_open());
        let state = serde_json::json!({
            "tick": self.anim_tick, "map": rt.map_id().0,
            "previous_map": rt.previous_map_id(), "world": rt.world_index(),
            "dungeon_exit": rt.dungeon_exit_index(),
            "cell": [rt.state().cell().x, rt.state().cell().y],
            "stepping": rt.state().is_stepping(), "scene": rt.scene_active(),
            "dialogue": dialogue, "title": self.title.is_some(),
            "title_menu": self.title.as_ref().map(|title| title.debug_menu()),
            "dialogue_choice": self.dialogue.as_ref().and_then(|window| window.bind().debug_choice()),
            "dialogue_page": self.dialogue.as_ref().and_then(|window| window.bind().debug_page()),
            "game_over": rt.game_over(),
            "poison_flash": self.poison_flash_visible(),
            "field_notice": rt.field_notice().map(|notice| format!("{notice:?}")),
            "party_resources": rt.game().party_members().into_iter().filter_map(|who| rt.game().roster().get(who).map(|stats| serde_json::json!({"id":who.0,"level":stats.level,"tp":stats.curr_tp,"max_tp":stats.max_tp,"techniques":stats.techniques,"skills":stats.skills,"skill_uses":stats.curr_skill_uses,"max_skill_uses":stats.max_skill_uses}))).collect::<Vec<_>>(),
            "party_status": rt.game().party_members().into_iter().filter_map(|who| rt.game().roster().get(who).map(|stats| serde_json::json!({"id":who.0,"hp":stats.curr_hp,"max_hp":stats.max_hp,"status":stats.status}))).collect::<Vec<_>>(),
            "transition": self.transition.is_some(), "money": rt.game().money(),
            "camp": self.camp_menu.as_ref().filter(|menu| menu.bind().is_open()).map(|menu| menu.bind().debug_menu()),
            "shop": self.shop.as_ref().filter(|menu| menu.bind().is_open()).map(|menu| menu.bind().debug_menu()),
            "temporary_objects": self.presentation.temporary_draws().iter().filter(|(_, o)| o.frames_left > 0).map(|(slot, o)| serde_json::json!({"slot": slot, "id": o.object_id, "elapsed": o.elapsed, "position":o.destination})).collect::<Vec<_>>(),
            "map_patches": rt.map_effects().patch_blits.len(),
            "chunk_patches": rt.map_effects().chunk_patches,
            "red_palette": self.red_palette_probe(),
            "leader": self.leader_char,
            "active_npcs": rt.map().npcs().iter().enumerate().filter_map(|(i, npc)| npc.active.then_some(i)).collect::<Vec<_>>(),
            "npc_sheets": self.npc_nodes.iter().map(|npc| serde_json::json!({"index":npc.index,"sheet":npc.sheet})).collect::<Vec<_>>(),
            "followers": self.follower_nodes.iter().filter(|(node, _, _)| node.is_visible()).count(),
            "inventory": rt.game().inventory().slots().as_slice(),
            "loot": rt.loot_state().map(|loot| format!("{:?}", loot.outcome)),
            "chests": rt.map().chests().iter().map(|chest| serde_json::json!({"flag":chest.flag,"claimed":rt.game().chest_is_open(chest),"cell":[chest.cell.x,chest.cell.y],"open":rt.map().chest_is_open_at_slot(chest.object_slot(rt.map().chest_slot_base()))})).collect::<Vec<_>>(),
            "flags": (0..0x200).filter(|id| rt.game().is_set(psiv_core::Flag::event(*id))).collect::<Vec<_>>(),
            "temp_flags": (0..0x100).filter(|id| rt.game().is_set(psiv_core::Flag::temp(*id))).collect::<Vec<_>>(),
            "battle": self.battle_screen.as_ref().filter(|_| self.battle_presentation_active()).map(|screen| screen.bind().debug_menu()),
        });
        GString::from(state.to_string().as_str())
    }

    /// Current patched collision and doorway areas; never moves the party.
    pub(super) fn walk_probe(&self) -> GString {
        if std::env::var("PSIV_DEBUG_ROUTE").as_deref() != Ok("1") {
            return GString::new();
        }
        let Some(rt) = self.runtime.as_ref() else {
            return GString::new();
        };
        let map = rt.map();
        let data = serde_json::json!({
            "width": map.width(), "height": map.height(),
            "walkable": (0..map.height()).map(|y| (0..map.width()).map(|x| map.is_walkable(psiv_core::Cell::new(x, y))).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "warps": map.warps().iter().map(|w| [w.source.x, w.source.y, w.source.width, w.source.height]).collect::<Vec<_>>(),
        });
        GString::from(data.to_string().as_str())
    }
}

impl Field {
    /// Debug-only automation for the fix loop (no effect without the debug
    /// selectors): `PSIV_DEBUG_BATTLE=<formation hex>` or
    /// `--psiv-debug-battle=<formation hex>` starts that battle a few frames
    /// after boot with no play needed; `PSIV_DEBUG_SHOT=<path.png>` (with
    /// optional `PSIV_DEBUG_SHOT_FRAME=<n>`, default 180) saves a viewport
    /// screenshot so an agent can see what a player would. `PSIV_DEBUG_EVENT`
    /// starts a transcribed scene at tick 30; combine it with
    /// `PSIV_DEBUG_AUTOCLOSE_SCENE=1` for deterministic headless scene runs,
    /// or add `PSIV_DEBUG_RETAIL_PACE=1` to keep the retail 3f/character
    /// typewriter and four-frame dismiss hold while using the debug selector.
    /// `PSIV_DEBUG_CAMP=1` opens the field camp at tick 30. A screenshot of
    /// an active battle is deferred until after that tick's battle drive so a
    /// settled receipt observes the same update boundary as the simulation.
    pub(super) fn debug_hooks_tick(&mut self) {
        self.capture_debug_state();
        let vehicle_battle = std::env::var("PSIV_DEBUG_VEHICLE_BATTLE").ok();
        let formation = std::env::var("PSIV_DEBUG_BATTLE")
            .ok()
            .or_else(|| vehicle_battle.clone())
            .or_else(|| {
                std::env::args().find_map(|argument| {
                    argument
                        .strip_prefix("--psiv-debug-battle=")
                        .map(str::to_owned)
                })
            });
        if self.anim_tick == 30
            && let Some(formation) = formation
        {
            let trimmed = formation.trim_start_matches("0x");
            match u16::from_str_radix(trimmed, 16) {
                Ok(id) => {
                    godot_print!("debug: starting battle {id:#05x}");
                    if id == 0x88 && vehicle_battle.is_none() {
                        godot_print!("debug: battle theme dispatch: 0x95");
                        self.play_sound(0x95);
                        self.start_oracle_debug_battle();
                    } else if id == 0x89 && vehicle_battle.is_none() {
                        godot_print!("debug: battle theme dispatch: 0x95");
                        self.play_sound(0x95);
                        self.start_newly_exact_debug_battle();
                    } else {
                        let music = self
                            .runtime
                            .as_ref()
                            .filter(|runtime| runtime.vehicle_active())
                            .map_or(0x8f, |_| 0x96);
                        godot_print!("debug: battle theme dispatch: {music:#04x}");
                        self.play_sound(music);
                        self.start_random_battle(id);
                    }
                }
                Err(_) => godot_error!("debug battle selector {formation} is not hex"),
            }
        }
        if self.anim_tick == 30 && std::env::var("PSIV_DEBUG_CAMP").is_ok_and(|value| value == "1")
        {
            godot_print!("debug: opening camp menu");
            self.open_camp_menu();
            // The tape-22 frame-7675 receipt: retail's camera settled at
            // ($258,$E8), 16px below the fixture's player-centred default
            // (the receipt reflects the walk history the fixture does not
            // replay). Field-only correlation confirmed dy=16 exactly.
            if let Some(rt) = self.runtime.as_mut() {
                rt.set_camera(0x258, 0xE8);
            }
        }
        if self.anim_tick == 30
            && let Ok(value) = std::env::var("PSIV_DEBUG_SHOP")
            && let Ok(index) = value.parse::<usize>()
        {
            let opened = match (self.shop.as_mut(), self.runtime.as_ref()) {
                (Some(shop), Some(runtime)) => shop.bind_mut().open_index(index, runtime),
                _ => false,
            };
            if opened {
                godot_print!("debug: opening shop counter {index}");
                self.place_shop_window();
            }
        }
        if self.anim_tick == 30
            && let Ok(value) = std::env::var("PSIV_DEBUG_EVENT")
        {
            let trimmed = value.trim().trim_start_matches("0x");
            match u16::from_str_radix(trimmed, 16) {
                Ok(event) => {
                    let started = self
                        .runtime
                        .as_mut()
                        .is_some_and(|runtime| runtime.start_event(event));
                    if started {
                        self.presentation.reset_scene();
                        self.set_letterbox(true);
                        if event & 0x8000 != 0 {
                            self.scene_transition_active = true;
                            self.start_transition(crate::transitions::TransitionKind::SceneStart);
                        }
                        godot_print!("debug: starting scene event {event:#06x}");
                    } else {
                        godot_error!("debug event {value} has no transcribed scene");
                    }
                }
                Err(_) => godot_error!("debug event {value} is not a hexadecimal event id"),
            }
        }
        // Field/camp receipts retain the historical pre-drive capture point.
        // Battle receipts are captured after `drive_battle_if_active`, below,
        // because the battle clock advances in that drive.
        if !self.battle_presentation_active() {
            self.capture_debug_shot();
        }
    }

    /// Machine-readable evidence from the same runtime the Godot window is
    /// driving. Uses the screenshot tick, but does not require a renderer.
    fn capture_debug_state(&self) {
        let Ok(path) = std::env::var("PSIV_DEBUG_STATE") else {
            return;
        };
        let at = std::env::var("PSIV_DEBUG_SHOT_FRAME")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(180);
        if self.anim_tick != at {
            return;
        }
        let Some(runtime) = self.runtime.as_ref() else {
            return;
        };
        let state = runtime.game().snapshot();
        let report = serde_json::json!({
            "tick": self.anim_tick,
            "map": runtime.map_id().0,
            "cell": [runtime.state().cell().x, runtime.state().cell().y],
            "scene_active": runtime.scene_active(),
            "leader_sheet": self.leader_char,
            "visible_followers": self.follower_nodes.iter().filter(|(node, _, _)| node.is_visible()).count(),
            "characters": state.characters.iter().enumerate().filter_map(|(id, stats)| stats.as_ref().map(|stats| serde_json::json!({
                "id": id, "hp": stats.curr_hp, "tp": stats.curr_tp, "status": stats.status,
                "skill_uses": stats.curr_skill_uses,
            }))).collect::<Vec<_>>(),
            "battle": runtime.battle_roster().map(|roster| roster.iter().map(|fighter| serde_json::json!({
                "id": fighter.id.get(), "name": fighter.name,
                "active": fighter.active,
                "hp": fighter.stats.curr_hp, "tp": fighter.stats.curr_tp,
                "status": fighter.stats.status,
                "attack": fighter.stats.attack.battle, "agility": fighter.stats.agility.battle,
                "dexterity": fighter.stats.dexterity.battle,
                "skills": fighter.stats.skills, "skill_uses": fighter.stats.curr_skill_uses,
                "max_skill_uses": fighter.stats.max_skill_uses,
            })).collect::<Vec<_>>()),
            "money": state.money,
            "inventory": runtime.game().inventory().slots().to_vec(),
            "party": state.party,
            "event_flags": state.event_flags.as_slice(),
            "town_flags": state.town_flags,
        });
        match std::fs::write(&path, format!("{report:#}\n")) {
            Ok(()) => godot_print!("debug: runtime state -> {path}"),
            Err(error) => godot_error!("debug: runtime state failed: {error}"),
        }
    }

    pub(super) fn capture_debug_shot(&mut self) {
        let Ok(path) = std::env::var("PSIV_DEBUG_SHOT") else {
            return;
        };
        let at: u64 = std::env::var("PSIV_DEBUG_SHOT_FRAME")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(180);
        if self.anim_tick != at {
            return;
        }
        if let Some(viewport) = self.base().get_viewport()
            && let Some(texture) = viewport.get_texture()
            && let Some(image) = texture.get_image()
        {
            let err = image.save_png(&GString::from(path.as_str()));
            godot_print!("debug: screenshot -> {path} ({err:?})");
        }
    }
}
