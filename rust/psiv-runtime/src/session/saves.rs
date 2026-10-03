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
use crate::pad::Pad;
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
    /// the debug switches — carries over, and the per-frame state resets to
    /// what a fresh session had at construction: the shell's START and CONTINUE
    /// built a whole new session there, and a held button must not read as a
    /// press of the new game's first frame.
    fn install_runtime(&mut self, mut runtime: Runtime) -> Result<(), String> {
        if let Some(files) = self.battle_files.clone() {
            runtime
                .enable_battles(&files)
                .map_err(|error| error.to_string())?;
        }
        self.runtime = runtime;
        self.accept_blocked = false;
        self.prev_pad = Pad::NEUTRAL;
        self.shop = None;
        self.camp = None;
        self.menu_scene = None;
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

    /// Presses Start on the ending's final gate (`finish_cutscene_presentation`
    /// is the shell's; the gate itself is the runtime's).
    pub fn ending_continue(&mut self) {
        self.runtime.ending_continue();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeEvent;
    use crate::session::menu_scene::MenuScene;
    use std::path::Path;

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
}
