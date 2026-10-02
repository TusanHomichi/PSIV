//! The field-status windows a defeat queues: the fallen and perished notices.
//!
//! `UpdateFieldStatus` walks the party every frame and, when somebody's HP
//! reaches zero, queues the retail message windows: `DisplayDeadCharacterMessage`
//! (`ps4.asm:117032`) prints `WinTiles_DeadString` (`ps4.asm:340603`) with the
//! character's name, and `DisplayPerishedMessage` (`ps4.asm:117087`) prints
//! `WinTiles_PerishString` (`ps4.asm:340611`) and then leaves through
//! `PalFadeOut_ClrSpriteTbl` and `MainGameProgram_Continue` — the defeat
//! boundary [`crate::Runtime::game_over`] models.
//!
//! Both windows are modal: the party stands still until the player
//! acknowledges one, and the next one opens after it. The runtime owns the
//! queue and parks the field while a notice is pending; this service owns the
//! window: it opens the box for the notice at the head of the queue, and
//! acknowledges it once the player has dismissed it. It runs at the *top* of
//! [`Session::frame`](crate::Session::frame) because the shell's old
//! dispatcher serviced its notices before everything else, and the box that
//! opens here takes its first input half on the same frame.
//!
//! A window dismisses on `ButtonCancel|ButtonSpeak|ButtonCamp`
//! (`ps4.asm:117104-117105`) — the one place a Cancel press advances a message
//! box, which `DialogueRunner::input` carries as its `notice_up` exception.
//!
//! The perished line's name is the retail string's own `CharName_Chaz`
//! constant rather than the current leader's name: this port prints
//! `Chaz and his Companions` for every defeat, where the cartridge expands the
//! constant to the leader. Kept as it was, because the native field-status
//! fixture's receipt was recorded with it.

use crate::{FieldNotice, Runtime};

use super::Session;

/// The field-status window the session opened this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldNoticeOpened {
    /// Which window it is.
    pub notice: FieldNotice,
    /// The lines it shows, for the shell's log.
    pub lines: Vec<String>,
}

impl Session {
    /// Services the runtime's field-status windows.
    ///
    /// The window that has been read is acknowledged, and the next one opens;
    /// `Some` carries the lines of a window that opened this frame, for the
    /// shell's log. The field stays parked while a notice is pending — the
    /// runtime's own tick refuses to run — so this service is the only thing
    /// that moves a defeat along.
    pub(crate) fn serve_field_notice(&mut self) -> Option<FieldNoticeOpened> {
        if self.notice_open {
            if self.runtime.dialogue_open() {
                return None;
            }
            self.notice_open = false;
            self.runtime.acknowledge_field_notice();
        }
        let notice = self.runtime.field_notice()?;
        let lines = notice_lines(&self.runtime, notice);
        self.notice_open = self.runtime.open_status_dialogue(&lines);
        Some(FieldNoticeOpened { notice, lines })
    }
}

/// The retail lines for a notice, with the character's saved name.
fn notice_lines(runtime: &Runtime, notice: FieldNotice) -> Vec<String> {
    match notice {
        FieldNotice::Fallen(who) => {
            let name = runtime
                .game()
                .roster()
                .get(who)
                .map(|stats| stats.display_name())
                .unwrap_or_default();
            vec![format!("{name} is on the verge"), "of Death.".into()]
        }
        FieldNotice::Perished => {
            vec!["Chaz and his Companions".into(), "have perished.".into()]
        }
    }
}
