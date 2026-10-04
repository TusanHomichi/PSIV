//! A menu action that starts a scene: the cartridge's hand-off, and what the
//! session does when the scene ends.
//!
//! Two menus in the cartridge do not answer a command inside their own window.
//! They destroy their windows, hand the field to an event, and — for one of
//! them — come back to close a transaction afterwards:
//!
//! ```text
//! ITEM > USE on a vehicle item   DestroyAllWindows + Event_Index + bit 1   (:123419)
//! the Aiedo inn's rest           window destroy, Event_GirlsSneakingOut,   (:136391)
//!                                the window rebuilt, the bill charged
//! ```
//!
//! `Field_MenuExit` is what makes the first one work: the menu loop ends when
//! its last window is destroyed, and the routine reads `Routine_Exit_Flags`
//! (`ps4.asm:117150-117178`) — bit 1 is "run the event this menu wrote into
//! `Event_Index`", which the field's own event routine then runs on the
//! following frames. The port models that as one field, [`MenuScene`]: the
//! frame the menu command answered on closes the menu and starts the scene, the
//! field owns the frames until [`RuntimeEvent::SceneEnded`], and then the menu's
//! own resume — nothing at all for ITEM, the bill and the result line for the
//! inn — runs.
//!
//! [`RuntimeEvent::SceneEnded`]: crate::RuntimeEvent::SceneEnded

use crate::RuntimeEvent;

use super::Session;
use super::shop::ShopPage;

/// A menu command that handed the field to a scene, and what finishes when
/// that scene ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuScene {
    /// The ITEM menu's accepted action: the menu closed for good
    /// (`ps4.asm:123419-123431`).
    ItemAction {
        /// The event the action wrote into `Event_Index`.
        event: u16,
    },
    /// The Aiedo inn's rest: the window is rebuilt and the bill charged when
    /// the scene returns (`ps4.asm:136391-136414`).
    AiedoInn {
        /// `Event_GirlsSneakingOut`.
        event: u16,
        /// The counter row, so the window can be rebuilt.
        counter: usize,
        /// The bill `RecoverStats` was already priced at.
        cost: u32,
    },
}

impl MenuScene {
    /// The scene this hand-off starts.
    #[must_use]
    pub(crate) fn event(self) -> u16 {
        match self {
            MenuScene::ItemAction { event } | MenuScene::AiedoInn { event, .. } => event,
        }
    }
}

impl Session {
    /// Starts `event` for the menu command in `what`, closing the menu as the
    /// cartridge's `DestroyAllWindows` does. `false` when the pack has no such
    /// scene (or one already runs), leaving the caller to answer the command
    /// some other way.
    pub(crate) fn start_menu_scene(&mut self, what: MenuScene) -> bool {
        if self.runtime.scene_active() || !self.runtime.start_event(what.event()) {
            return false;
        }
        self.menu_scene = Some(what);
        true
    }

    /// The frame's own end of a menu scene: when this frame's events ended the
    /// scene a menu started, the menu's resume runs on the same frame.
    pub(crate) fn resume_menu_scene(&mut self, events: &[RuntimeEvent]) {
        if self.menu_scene.is_none()
            || !events
                .iter()
                .any(|event| matches!(event, RuntimeEvent::SceneEnded))
        {
            return;
        }
        match self.menu_scene.take() {
            // The ITEM menu is gone for good: its action destroyed the windows
            // and set `Routine_Exit_Flags` bit 1 with no window to rebuild
            // (`ps4.asm:123419-123431`). The press the player is still holding
            // must not read as a talk on the field it comes back to.
            Some(MenuScene::ItemAction { .. }) | None => self.block_accept(),
            Some(MenuScene::AiedoInn {
                event: _,
                counter,
                cost,
            }) => self.finish_aiedo_stay(counter, cost),
        }
    }

    /// Closes the Aiedo rest's transaction after its scene: the bill is
    /// charged, then the window is rebuilt on the "rest well" line
    /// (`ps4.asm:136411-136463`).
    pub(crate) fn finish_aiedo_stay(&mut self, counter: usize, cost: u32) {
        self.runtime.inn_charge(cost);
        let shops = self.runtime.data().shops();
        let Some(counter) = shops.and_then(|shops| shops.counter_index(counter)) else {
            // A counter the pack does not have cannot have opened the window;
            // the bill is charged above and the party keeps the field.
            self.block_accept();
            return;
        };
        let Some(shops) = shops else {
            self.block_accept();
            return;
        };
        let mut view = super::shop::ShopView::open(counter, shops, &self.runtime);
        // The rebuilt window shows the night's result, not the greeting: the
        // cartridge draws the purse and the "rest well" line it drew for an
        // ordinary night (`loc_2AC166`).
        view.page = ShopPage::Message;
        view.message = "Thank you very much.\nPlease come again.".to_owned();
        self.shop = Some(view);
        self.runtime.set_field_suspended(true);
    }
}
