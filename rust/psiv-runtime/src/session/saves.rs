//! The session's save surface: one store, every slot operation.
//!
//! The directory is the shell's decision (`rust/psiv-godot/src/save_dir.rs`
//! resolves `PSIV_SAVE_DIR` and refuses a scripted run that names none) and it
//! reaches the runtime only through [`SaveStore`], which a session keeps for
//! its whole life — including across the START and CONTINUE that build a fresh
//! runtime. Everything that touches a slot goes through it:
//!
//! ```text
//! the title's slot list      store.slots(...)     presence only
//! the title's CONTINUE       Session::continue_save
//! the title's ERASE DATA     Session::erase_slot
//! the camp's STATE > SAVE    Session::finish_camp_save
//! ```
//!
//! A session without a store refuses all four rather than falling back to a
//! repository default: `SessionSaveError::NoDirectory` is the refusal, and the
//! shell's own resolver error has already been logged where the store would
//! have come from.

use std::path::PathBuf;

use psiv_core::StepFrames;

use crate::Runtime;
use crate::save::{SaveStore, SessionSaveError};

use super::{CampSaveFailure, Mode, Session};

impl Session {
    /// The run's save store, when the shell resolved a directory for it.
    #[must_use]
    pub fn save_store(&self) -> Option<&SaveStore> {
        self.save_store.as_ref()
    }

    /// Hands the session the store over the run's directory.
    ///
    /// The shell resolves the path once, at construction; a driver that owns
    /// its own run directory sets the store before its first frame.
    pub fn set_save_store(&mut self, store: SaveStore) {
        self.save_store = Some(store);
    }

    /// The three visible slots, presence only: the validation CONTINUE runs,
    /// without loading anything a run did not ask for.
    ///
    /// A session without a store shows three empty slots, which is what the
    /// shell presented after its resolver refused.
    #[must_use]
    pub fn save_slots(&self) -> [bool; 3] {
        self.save_store
            .as_ref()
            .map_or([false; 3], |store| store.slots(self.runtime.data()))
    }

    /// Writes the current state to `slot`.
    ///
    /// # Errors
    ///
    /// [`SessionSaveError::NoDirectory`] when the run named no directory, or
    /// the store's own I/O or encoding failure.
    pub fn save_slot(&self, slot: usize) -> Result<PathBuf, SessionSaveError> {
        Ok(self.store()?.write(&self.runtime, slot)?)
    }

    /// Performs the title's destructive ERASE DATA for `slot`: the file's
    /// common header survives and its physical payload is zeroed.
    ///
    /// # Errors
    ///
    /// [`SessionSaveError::NoDirectory`] when the run named no directory, or
    /// the store's own I/O or format failure.
    pub fn erase_slot(&self, slot: usize) -> Result<PathBuf, SessionSaveError> {
        Ok(self.store()?.erase(slot)?)
    }

    /// The store, or the refusal that keeps a run off a directory it never
    /// named.
    fn store(&self) -> Result<&SaveStore, SessionSaveError> {
        self.save_store
            .as_ref()
            .ok_or(SessionSaveError::NoDirectory)
    }

    /// CONTINUE: loads one visible slot and installs its runtime.
    ///
    /// The load is the same validated path the boot's `PSIV_LOAD_SLOT` uses, so
    /// a slot that loads there loads here and a corrupt one is refused with the
    /// validation's own message.
    pub(crate) fn continue_save(&mut self, slot: usize) -> Result<(), String> {
        let data = self.runtime.data().clone();
        let runtime = self
            .store()
            .map_err(|error| error.to_string())?
            .load(data, slot, StepFrames::default())
            .map_err(|error| error.to_string())?;
        self.install_runtime(runtime)
    }

    /// START: the retail initializer's new game, with the opening's event
    /// already fired (`loc_64EAC` is the initializer `Title_StartOption` calls,
    /// `ps4.asm:87728`; Event_GameStart is the pack's own opening).
    ///
    /// Returns whether the opening scene started, the edge the shell uses for
    /// its letterbox and its scene transition.
    pub(crate) fn start_new_game(&mut self) -> Result<bool, String> {
        let data = self.runtime.data().clone();
        let mut runtime =
            Runtime::new_game(data, StepFrames::default()).map_err(|error| error.to_string())?;
        let event = runtime
            .data()
            .new_game()
            .expect("validated title initializer")
            .event_index;
        let started = runtime.start_event(event);
        self.install_runtime(runtime)?;
        Ok(started)
    }

    /// Installs a freshly built runtime as this session's game.
    ///
    /// The session's own configuration — the save store, the battle pack and
    /// the debug switches — carries over, and the open windows and modes reset
    /// to what a fresh session had at construction.
    ///
    /// The pad does not reset: the button that chose START or CONTINUE is
    /// still held on the field's first frames, and it must not read as a press
    /// there (#537). The cartridge's field reads its confirm from
    /// `Joypad_Pressed` (`FieldControls_GetInput`, `ps4.asm:114890`), so a
    /// button held over from the title never reaches it. Here `prev_pad`
    /// keeps the choice frame's pad (`Session::frame` stored it before the
    /// title ran), so the menus see no new edge, and the confirm is blocked
    /// until it is released, because the field's own confirm is level-driven.
    fn install_runtime(&mut self, mut runtime: Runtime) -> Result<(), String> {
        if let Some(files) = self.battle_files.clone() {
            runtime
                .enable_battles(&files)
                .map_err(|error| error.to_string())?;
        }
        self.game_cleared_written = runtime.game_cleared();
        self.runtime = runtime;
        self.accept_blocked = true;
        self.shop = None;
        self.camp = None;
        self.menu_scene = None;
        self.destination = None;
        self.job_board = None;
        self.notice_open = false;
        self.mode = Mode::Field;
        Ok(())
    }

    /// Completes the camp's SAVE page: writes `slot` and shows the result line
    /// the page holds.
    ///
    /// `FILE SAVED`, or `SAVE ERROR: <why>` — the same two lines the shell's
    /// round trip produced. A failure also comes back for the shell's log, on
    /// the frame it happened.
    pub(crate) fn finish_camp_save(&mut self, slot: usize) -> Option<CampSaveFailure> {
        let result = self
            .save_slot(slot)
            .map(|_| ())
            .map_err(|error| error.to_string());
        let failure = result.as_ref().err().map(|error| CampSaveFailure {
            slot,
            error: error.clone(),
        });
        if let Some(camp) = self.camp.as_mut() {
            camp.finish_save(result);
        }
        failure
    }

    /// Enables the battle pack on this session's runtime and remembers it for
    /// every runtime the front door builds afterwards.
    ///
    /// The pack is loaded by the shell (`psiv-data` owns the schema) and handed
    /// here, because only the session may put it on the runtime.
    ///
    /// # Errors
    ///
    /// Whatever [`Runtime::enable_battles`] rejects: battle data that does not
    /// convert.
    pub fn enable_battles(
        &mut self,
        files: &psiv_data::BattleFiles,
    ) -> Result<(), crate::BridgeError> {
        self.runtime.enable_battles(files)?;
        self.battle_files = Some(files.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::{Button, Pad};
    use crate::session::menu_scene::MenuScene;
    use crate::{RuntimeEvent, TitleEntry, TitlePhase};
    use std::path::Path;

    #[test]
    fn a_continue_of_a_cleared_save_reads_the_cleared_flag() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "psiv-cleared-continue-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).expect("new isolated save directory");

        // A save as the game writes it, then the ending's own header write.
        let mut played = Session::start(data.clone())
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        played.runtime.game.set_money(100);
        played.save_slot(0).expect("write the slot");
        let path = SaveStore::new(&directory).directory().join("slot_1.sram");
        let bytes = std::fs::read(&path).expect("read the slot");
        let cleared = psiv_core::RetailSlot::from_bytes(&bytes, 0)
            .expect("valid slot")
            .mark_game_cleared();
        std::fs::write(&path, cleared.as_bytes()).expect("write the cleared header");

        // A fresh session's CONTINUE reads it back.
        let mut resumed = Session::start(data)
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        resumed.continue_save(0).expect("CONTINUE loads the slot");
        assert!(resumed.runtime().game_cleared());
        assert_eq!(resumed.runtime().game().money(), 100);
    }

    #[test]
    fn the_cleared_byte_is_written_by_the_store_and_kept_by_a_later_save() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("psiv-cleared-store-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).expect("new isolated save directory");
        let path = directory.join("slot_1.sram");
        let byte = |path: &Path| std::fs::read(path).expect("read slot")[0x35] & 1;

        let played = Session::start(data.clone())
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        played.save_slot(0).expect("write the slot");
        assert_eq!(byte(&path), 0, "a fresh slot is uncleared");

        // The ending's own write, without a SAVE.
        SaveStore::new(&directory)
            .mark_game_cleared()
            .expect("mark the header");
        assert_eq!(byte(&path), 1);

        // A later save from a game whose latch is false keeps the byte.
        let later = Session::start(data)
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        later.save_slot(0).expect("save over the cleared slot");
        assert_eq!(byte(&path), 1, "the header byte is sticky");

        // A directory with no slot file has no header to write: a no-op.
        let empty = directory.join("empty");
        std::fs::create_dir(&empty).expect("new empty directory");
        SaveStore::new(&empty)
            .mark_game_cleared()
            .expect("no slot, no header, no error");
    }

    #[test]
    fn a_continue_keeps_the_saved_party_bridge() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("psiv-saved-party-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).expect("new isolated save directory");
        let bridge = [
            Some(psiv_core::CharId(0)),
            Some(psiv_core::CharId(1)),
            Some(psiv_core::CharId(2)),
            None,
            None,
        ];

        let mut played = Session::start(data.clone())
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        played.runtime.saved_party_slots = Some(bridge);
        played.save_slot(0).expect("write the slot");

        let mut resumed = Session::start(data)
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        resumed.continue_save(0).expect("CONTINUE loads the slot");
        assert_eq!(resumed.runtime().saved_party_slots, Some(bridge));
    }

    #[test]
    fn start_and_continue_discard_an_old_inn_scene_handoff() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        let stale_bill = MenuScene::AiedoInn {
            event: 0x54,
            counter: 0,
            cost: 25,
        };
        let assert_not_replayed = |session: &mut Session| {
            assert_eq!(session.menu_scene, None);
            let money = session.runtime().game().money();
            session.resume_menu_scene(&[RuntimeEvent::SceneEnded]);
            assert_eq!(session.runtime().game().money(), money, "no stale bill");
            assert!(session.shop.is_none(), "no stale inn window");
        };

        let mut start = Session::start(data.clone()).field().expect("field boot");
        start.menu_scene = Some(stale_bill);
        start.start_new_game().expect("START installs a runtime");
        assert_eq!(start.runtime().saved_sound_index(), 0);
        assert_not_replayed(&mut start);

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "psiv-f2-menu-scene-reset-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).expect("new isolated save directory");
        let mut resumed = Session::start(data)
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot");
        resumed.runtime.game.set_money(100);
        resumed.save_slot(0).expect("write isolated source slot");
        resumed.menu_scene = Some(stale_bill);
        resumed
            .continue_save(0)
            .expect("CONTINUE installs a runtime");
        assert_eq!(
            resumed.runtime().saved_sound_index(),
            resumed.runtime().map_record().unwrap().music.id
        );
        assert_not_replayed(&mut resumed);
        std::fs::remove_dir_all(&directory).expect("remove only our test directory");
    }

    /// A session over a saved slot 1, powered on at the title and settled on
    /// its slot list, the row on that slot: the frame before the player's
    /// confirm. Returns the session and the run directory to remove.
    fn at_the_slot_list(name: &str) -> Option<(Session, std::path::PathBuf)> {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime-pack");
        if !pack.join("manifest.json").is_file() {
            eprintln!("runtime pack not present; skipping");
            return None;
        }
        let data = psiv_data::GameData::load(&pack).expect("pack loads");
        // Keyed by the caller's name and the process: one directory per test.
        let directory =
            std::env::temp_dir().join(format!("psiv-title-handoff-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).expect("new isolated save directory");
        Session::start(data.clone())
            .with_saves(SaveStore::new(&directory))
            .field()
            .expect("field boot")
            .save_slot(0)
            .expect("write the slot");

        let mut session = Session::start(data)
            .with_saves(SaveStore::new(&directory))
            .power_on()
            .expect("power on");
        for _ in 0..2000 {
            if session
                .title()
                .is_some_and(|view| view.phase == TitlePhase::Menu)
            {
                break;
            }
            session.frame(Pad::NEUTRAL);
        }
        let view = session.title().expect("the title is up");
        assert_eq!(view.phase, TitlePhase::Menu, "the option window opened");
        assert_eq!(view.cursor, 0, "CONTINUE is the first row");
        session.frame(Pad::new(Button::Speak));
        session.frame(Pad::NEUTRAL);
        let view = session.title().expect("the slot list is up");
        assert_eq!(view.phase, TitlePhase::Slots);
        assert_eq!(view.cursor, 0, "the row is on the saved slot");
        Some((session, directory))
    }

    /// Holds `button` across the slot's confirm and `held` field frames, then
    /// releases it for a few frames.
    fn continue_holding(session: &mut Session, button: Button, held: usize) {
        let frame = session.frame(Pad::new(button));
        assert_eq!(
            frame.title.and_then(|title| title.entered),
            Some(TitleEntry::Continued { slot: 0 }),
            "the press confirmed the slot"
        );
        for frame in 0..held {
            session.frame(Pad::new(button));
            assert!(
                !session.runtime().dialogue_open(),
                "field frame {frame}: the held confirm opened a window (#537)"
            );
            assert!(
                session.camp_view().is_none(),
                "field frame {frame}: the held confirm opened the camp menu (#537)"
            );
        }
        for _ in 0..4 {
            session.frame(Pad::NEUTRAL);
        }
        assert!(!session.runtime().dialogue_open());
        assert!(session.camp_view().is_none());
    }

    /// #537: the Speak that confirms a CONTINUE slot is still held on the
    /// field's first frames, and it is not a search there: the cartridge's
    /// field reads `Joypad_Pressed`. A fresh press after the release is.
    #[test]
    fn a_continue_confirm_held_into_the_field_is_not_a_search() {
        let Some((mut session, directory)) = at_the_slot_list("speak") else {
            return;
        };
        // Nine frames is the 150 ms hold the issue reproduced with.
        continue_holding(&mut session, Button::Speak, 9);
        // Control: the field does read a fresh Speak press after the release.
        let mut opened = false;
        for _ in 0..8 {
            session.frame(Pad::new(Button::Speak));
            if session.runtime().dialogue_open() {
                opened = true;
                break;
            }
        }
        assert!(opened, "a fresh Speak press on the field opens its window");
        std::fs::remove_dir_all(&directory).expect("remove only our test directory");
    }

    /// The same hand-off for the menus' edges: a Camp (A) that confirms the
    /// slot does not open the camp menu on the field, and a fresh one does.
    #[test]
    fn a_continue_confirm_held_on_camp_does_not_open_the_camp_menu() {
        let Some((mut session, directory)) = at_the_slot_list("camp") else {
            return;
        };
        continue_holding(&mut session, Button::Camp, 9);
        session.frame(Pad::new(Button::Camp));
        assert!(
            session.camp_view().is_some(),
            "a fresh Camp press on the field opens the camp menu"
        );
        std::fs::remove_dir_all(&directory).expect("remove only our test directory");
    }
}
