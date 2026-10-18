//! What owns the session between frames, for observers.
//!
//! A read-only answer to "what is on screen right now", in the order
//! [`Session::frame`] dispatches a frame: the title, the defeat fade and the
//! battle loop own the whole frame; then a scene's destination menu, a menu's
//! scene, the shop and the camp windows; then the field's own scene and message
//! box. The shell's memory check (`tools/native_rss.py`, #92) compares Godot's
//! memory only between samples taken at [`Activity::Field`], so a battle's or a
//! scene's art is never read as growth.

use super::{Mode, Session};

/// What owns the session, as [`Session::activity`] reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// The title, including its slot lists.
    Title,
    /// The defeat fade before the title returns.
    GameOver,
    /// The battle loop, from the frame it began to the frame it closed.
    Battle,
    /// A scene's ship destination menu.
    Destination,
    /// The shop or inn window.
    Shop,
    /// The camp menu or a chest window.
    Camp,
    /// A scene runs: a field scene, a menu's scene or a flight.
    Scene,
    /// The message box is open over the field, a scene's or a talk's or a
    /// field-status window.
    Dialogue,
    /// The field with nothing over it: the party walks, stands or rides.
    Field,
}

impl Activity {
    /// The name the shell's samples record (`native_tape.gd`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Activity::Title => "title",
            Activity::GameOver => "game_over",
            Activity::Battle => "battle",
            Activity::Destination => "destination",
            Activity::Shop => "shop",
            Activity::Camp => "camp",
            Activity::Scene => "scene",
            Activity::Dialogue => "dialogue",
            Activity::Field => "field",
        }
    }
}

impl Session {
    /// What owns the session now: what the next [`Session::frame`] serves, read
    /// without running it.
    #[must_use]
    pub fn activity(&self) -> Activity {
        match self.mode {
            Mode::Title(_) => return Activity::Title,
            Mode::GameOver(_) => return Activity::GameOver,
            Mode::Battle(_) => return Activity::Battle,
            Mode::Field => {}
        }
        if self.runtime.game_over() {
            Activity::GameOver
        } else if self.destination.is_some() {
            Activity::Destination
        } else if self.menu_scene.is_some() || self.flight.is_some() {
            Activity::Scene
        } else if !self.runtime.dialogue_open() && self.shop.is_some() {
            Activity::Shop
        } else if !self.runtime.dialogue_open() && self.camp.is_some() {
            Activity::Camp
        } else if self.runtime.scene_active() {
            Activity::Scene
        } else if self.runtime.dialogue_open() {
            Activity::Dialogue
        } else {
            Activity::Field
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use psiv_core::StepFrames;
    use psiv_data::{BattleFiles, GameData};

    use super::Activity;
    use crate::pad::{Button, Pad};

    const PACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack");

    fn camp_session() -> Option<crate::Session> {
        if !Path::new(PACK).join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return None;
        }
        let data = GameData::load(Path::new(PACK)).expect("pack loads");
        let mut session =
            crate::camp_fixture(data, StepFrames::default()).expect("the fixture builds");
        session
            .enable_battles(&BattleFiles::load(Path::new(PACK)).expect("battle pack loads"))
            .expect("battles enable");
        Some(session)
    }

    #[test]
    fn the_field_the_camp_menu_and_a_battle_read_as_their_own_activity() {
        let Some(mut session) = camp_session() else {
            return;
        };
        for _ in 0..3 {
            session.frame(Pad::NEUTRAL);
        }
        assert_eq!(session.activity(), Activity::Field);
        assert!(session.open_camp_menu());
        assert_eq!(session.activity(), Activity::Camp);
        let _ = session.debug_battle(0x8a);
        assert_eq!(session.activity(), Activity::Battle);
        assert_eq!(Activity::Battle.name(), "battle");
    }

    #[test]
    fn reading_the_activity_changes_nothing() {
        let Some(mut session) = camp_session() else {
            return;
        };
        let mut twin = camp_session().expect("pack present");
        let pads = [Pad::NEUTRAL, Pad::new(Button::Down), Pad::NEUTRAL];
        for pad in pads.iter().cycle().take(60) {
            let _ = session.activity();
            let a = session.frame(*pad);
            let b = twin.frame(*pad);
            assert_eq!(a.mode, b.mode);
        }
        assert_eq!(
            session.runtime().state().cell(),
            twin.runtime().state().cell()
        );
    }
}
