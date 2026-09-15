//! Input adapter for the pure per-character command menu and vehicle menu.
use super::*;

impl BattleScreen {
    pub(crate) fn debug_menu(&self) -> serde_json::Value {
        serde_json::json!({
            "ready": self.command_open && self.current.is_none() && self.events.is_empty(),
            "finishing": self.finish_outcome.is_some() || self.finish_request.is_some(),
            "message": self.message,
            "cursor": self.cursor,
            "menu": self.orders_menu.as_ref().map(|menu| menu.debug_menu()),
        })
    }

    /// Reads the retail command menu. The field calls Runtime after this
    /// returns; this node never resolves a round itself.
    pub(crate) fn sync_commands(&mut self, runtime: &psiv_runtime::Runtime) {
        self.command_roster = runtime.battle_roster().cloned().unwrap_or_default();
        self.techniques = runtime
            .battle_techniques()
            .map(|t| (t.id, t.clone()))
            .collect();
        self.skills = runtime.battle_skills().map(|s| (s.id, s.clone())).collect();
        self.armed_fighters = runtime.battle_armed_fighters();
        self.items = runtime
            .battle_items()
            .map(|item| (item.id, item.clone()))
            .collect();
        self.inventory = runtime.game().inventory().clone();
    }

    pub(crate) fn take_command(&mut self) -> Option<RoundOrders> {
        if !self.command_open
            || self.current.is_some()
            || !self.events.is_empty()
            || self.finish_outcome.is_some()
            || self.finish_request.is_some()
        {
            return None;
        }
        let input = Input::singleton();
        if let Some(menu) = self.orders_menu.as_mut() {
            let orders = if input.is_action_just_pressed("ui_cancel") {
                menu.cancel();
                None
            } else {
                if input.is_action_just_pressed("ui_up") || input.is_action_just_pressed("ui_left")
                {
                    menu.move_cursor(-1);
                }
                if input.is_action_just_pressed("ui_down")
                    || input.is_action_just_pressed("ui_right")
                {
                    menu.move_cursor(1);
                }
                if input.is_action_just_pressed("ui_accept") {
                    menu.accept()
                } else {
                    None
                }
            };
            if !menu.open {
                self.orders_menu = None;
            }
            if orders.is_some() {
                self.command_open = false;
            }
            self.base_mut().queue_redraw();
            return orders;
        }
        if self.skill_open {
            if input.is_action_just_pressed("ui_cancel") {
                self.skill_open = false;
                self.skill_cursor = 0;
                self.base_mut().queue_redraw();
                return None;
            }
            if input.is_action_just_pressed("ui_up") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), -2);
                self.base_mut().queue_redraw();
            }
            if input.is_action_just_pressed("ui_down") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), 2);
                self.base_mut().queue_redraw();
            }
            if input.is_action_just_pressed("ui_left") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), -1);
                self.base_mut().queue_redraw();
            }
            if input.is_action_just_pressed("ui_right") {
                self.skill_cursor =
                    vehicle_ui::move_cursor(self.skill_cursor, self.vehicle_skills.len(), 1);
                self.base_mut().queue_redraw();
            }
            if !input.is_action_just_pressed("ui_accept") {
                return None;
            }
            let skill = vehicle_ui::selected(&self.vehicle_skills, self.skill_cursor)?;
            self.vehicle_skills[self.skill_cursor].current -= 1;
            self.skill_open = false;
            self.command_open = false;
            self.base_mut().queue_redraw();
            return Some(RoundOrders::Commands(vec![
                psiv_core::battle::Command::VehicleSkill(skill),
            ]));
        }
        if input.is_action_just_pressed("ui_up") || input.is_action_just_pressed("ui_left") {
            self.cursor = self.cursor.saturating_sub(1);
            self.base_mut().queue_redraw();
        }
        if input.is_action_just_pressed("ui_down") || input.is_action_just_pressed("ui_right") {
            self.cursor = (self.cursor + 1).min(2);
            self.base_mut().queue_redraw();
        }
        if !input.is_action_just_pressed("ui_accept") {
            return None;
        }
        if self.cursor == 1 {
            if self.vehicle_index.is_some() {
                self.skill_open = true;
                self.skill_cursor = 0;
            }
            // MACR is visible for ordinary party parity; macro execution is
            // Tier 3. A vehicle's middle entry opens the retail OPTIN list.
            self.base_mut().queue_redraw();
            return None;
        }
        if self.cursor == 0 && self.vehicle_index.is_none() {
            self.orders_menu = Some(CommandsMenu::new(
                self.command_roster.clone(),
                self.techniques.clone(),
                self.skills.clone(),
                self.armed_fighters.clone(),
                self.items.clone(),
                self.inventory.clone(),
            ));
            self.base_mut().queue_redraw();
            return None;
        }
        self.command_open = false;
        self.base_mut().queue_redraw();
        Some(if self.cursor == 0 {
            RoundOrders::attack_all()
        } else {
            RoundOrders::Run
        })
    }
}
