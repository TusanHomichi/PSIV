//! The pure retail dialogue text loop.

use psiv_data::{
    ActionKind, CHARS_PER_LINE, Ctrl, DialogueEntry, LINES_PER_WINDOW, PORTRAIT_HIDE, PageEnd,
    Segment,
};

/// What opening an entry produced.
pub enum Opening {
    /// A `$FA` preamble was taken: reopen at this entry instead.
    Jump(u16),
    /// A window to show, driven by this flow.
    Window(Box<TextFlow>),
    /// The preamble was `$F6`: the entry fires an event and shows nothing.
    Event(u16),
    /// The entry has nothing to say -- an empty entry, which the cartridge's
    /// dense ids are full of.
    Silent,
}

/// One retail `$F2` action, released when the typewriter reaches its byte
/// position. Payload-bearing variants keep the decoder's word/byte widths;
/// action dispatch itself belongs to the Godot field shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueAction {
    LoadPanel(u16),
    DestroyLastPanel,
    DestroyAllPanels,
    LoadSound(u8),
    LoadSound2(u8),
    UpdatePalette,
    ZioEyesRed,
    PauseMusic,
    ResumeMusic,
    SabotageAlarmRedPalette,
    SetEventFlag(u8),
    ElsydeonBroken,
}

/// `RunText_CharacterLoop`, one page at a time.
///
/// The rules are the routine's, not a reimplementation of them: the column
/// counter wraps at 32 and breaks the line by itself, the line counter is one
/// bit wide so a full second line waits for input, `$FC` is swallowed when it
/// lands exactly on a wrap, and the interrupt looks at the byte it stopped on
/// before it shows an arrow. `psiv_tools.dialogue_pack.paginate` is the same
/// walk done offline; the tests hold this against it entry by entry.
pub struct TextFlow {
    segments: Vec<Segment>,
    /// Next segment to run.
    index: usize,
    /// Where in the current text run to resume, when a page filled mid-run.
    char_offset: usize,
    lines: Vec<String>,
    line: usize,
    column: usize,
    portrait: Option<u8>,
    /// Frames left on a `$F9`.
    hold: u16,
    /// Set while a finished page is on screen.
    stop: Option<PageEnd>,
    /// No segments left to run.
    done: bool,
    /// The window is gone.
    closed: bool,
    /// Diagnostic messages for controls that are intentionally not visual
    /// actions, such as a not-taken flag check.
    log: Vec<String>,
    /// The event-flag bank at open time, for mid-message `$FA`.
    flags: Vec<bool>,
    /// A taken mid-message `$FA` jump target; the window rebuilds the flow.
    jump: Option<u16>,
    /// A mid-message `$F6`; the window forwards it to the runtime.
    event: Option<u16>,
    /// The next action at the retail byte position. Pumping stops here so the
    /// window can wait for the preceding glyphs before the shell dispatches it.
    pending_action: Option<DialogueAction>,
    /// This flow's entry id — `$FA` branch targets are relative to it.
    entry_id: u16,
    /// Relative entry counts after `$F5`; zero continues at the current byte.
    choice: Option<[u16; 2]>,
}

impl TextFlow {
    /// Walks the preamble the interaction code eats and starts the message.
    ///
    /// `entry := $FA* ( $F6 event | $F3? text... )`. The `$FA` run is followed
    /// on its not-set branch -- event flags do not exist yet -- and every skip
    /// is logged.
    #[must_use]
    #[allow(dead_code)] // The flag-free form, kept for tests and future callers.
    pub fn open(entry: &DialogueEntry) -> Opening {
        TextFlow::open_with_flags(entry, &[])
    }

    /// [`TextFlow::open`], consulting the live event-flag bank so `$FA`
    /// chains take their set branches (this is how the town reacts to the
    /// story, and how the principal's entry 0 routes to his briefing).
    #[must_use]
    pub fn open_with_flags(entry: &DialogueEntry, flags: &[bool]) -> Opening {
        let mut log = Vec::new();
        let mut index = 0;
        while let Some(Segment::Control(ctrl @ Ctrl::FlagCheck { .. })) = entry.segments.get(index)
        {
            if let Ctrl::FlagCheck {
                flag, then_entry, ..
            } = ctrl
            {
                if flags.get(*flag as usize).copied().unwrap_or(false) {
                    // Branch targets are RELATIVE: GetOffsetByID counts
                    // forward from the current entry. (Absolute worked for
                    // the principal only because his chain starts at 0.)
                    return Opening::Jump(entry.id + *then_entry);
                }
                log.push(format!(
                    "flag_check: flag {flag} -> entry {then_entry}, not taken"
                ));
            }
            index += 1;
        }
        if let Some(Segment::Control(Ctrl::Event { id, .. })) = entry.segments.get(index) {
            return Opening::Event(*id);
        }
        if let Some(Segment::Control(Ctrl::KeepNpcFacing { .. })) = entry.segments.get(index) {
            log.push("keep_npc_facing: the NPC keeps its facing (not modelled yet)".to_owned());
            index += 1;
        }

        let mut flow = TextFlow {
            segments: entry.segments[index..].to_vec(),
            index: 0,
            char_offset: 0,
            lines: vec![String::new()],
            line: 0,
            column: 0,
            portrait: None,
            hold: 0,
            stop: None,
            done: false,
            closed: false,
            log,
            flags: flags.to_vec(),
            jump: None,
            event: None,
            pending_action: None,
            entry_id: entry.id,
            choice: None,
        };
        flow.pump();
        if flow.done && flow.stop.is_none() {
            return Opening::Silent;
        }
        Opening::Window(Box::new(flow))
    }

    /// The one or two lines currently on screen.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The portrait id currently shown, if any.
    #[must_use]
    pub fn portrait(&self) -> Option<u8> {
        self.portrait
    }

    /// How the page on screen ended, if a page is finished.
    #[must_use]
    pub fn page_end(&self) -> Option<PageEnd> {
        self.stop
    }

    /// Whether the arrow is up: the page is full and more text follows.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        matches!(
            self.stop,
            Some(PageEnd::Full | PageEnd::Wait | PageEnd::Close)
        )
    }

    /// Whether an accept press would advance or dismiss the page. Unlike
    /// [`TextFlow::is_waiting`] this includes the entry's final `End` page —
    /// the state a scene dialogue rests in until dismissed. `Choice` is
    /// deliberately excluded: advancing one selects an answer.
    #[must_use]
    pub fn is_dismissable(&self) -> bool {
        matches!(
            self.stop,
            Some(PageEnd::Full | PageEnd::Wait | PageEnd::Close | PageEnd::End)
        )
    }

    /// Whether the window is still showing anything.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.closed
    }

    /// Whether a `$F9` delay is running.
    #[must_use]
    #[allow(dead_code)] // Field will want this when scenes gate on hold state.
    pub fn is_holding(&self) -> bool {
        self.hold > 0
    }

    /// One engine tick: runs down a `$F9` delay. Returns whether anything
    /// changed.
    pub fn tick(&mut self) -> bool {
        if self.hold == 0 {
            return false;
        }
        self.hold -= 1;
        if self.hold > 0 {
            return false;
        }
        self.pump();
        true
    }

    /// The accept press. Clears a finished page and runs on, or closes the
    /// window when the message is over.
    pub fn advance(&mut self) {
        match self.stop {
            None => {}
            Some(PageEnd::Choice) => {}
            Some(PageEnd::End) => {
                self.stop = None;
                self.closed = true;
            }
            Some(_) => {
                self.stop = None;
                self.lines = vec![String::new()];
                self.line = 0;
                self.column = 0;
                if self.done {
                    self.closed = true;
                } else {
                    self.pump();
                }
            }
        }
    }

    pub fn has_choice(&self) -> bool {
        self.choice.is_some()
    }

    /// Retail counts `$FF` delimiters from the byte AFTER both operands.
    /// Zero stays inside this entry; positive values skip to later entries.
    pub fn answer_choice(&mut self, yes: bool) -> bool {
        let Some(offsets) = self.choice.take() else {
            return false;
        };
        let offset = offsets[usize::from(!yes)];
        self.lines = vec![String::new()];
        self.line = 0;
        self.column = 0;
        self.stop = None;
        if offset == 0 {
            self.pump();
        } else {
            self.jump = Some(self.entry_id + offset);
        }
        true
    }

    /// Continue a taken in-stream branch without reopening the window or
    /// losing the portrait. Unlike interaction preambles, this executes the
    /// destination through the retail character loop.
    pub fn continue_at(&mut self, entry: &DialogueEntry) {
        self.entry_id = entry.id;
        self.segments.clone_from(&entry.segments);
        self.index = 0;
        self.char_offset = 0;
        self.stop = None;
        self.done = false;
        self.closed = false;
        self.pump();
    }

    /// `$F7` and the entry terminator return from `Event_RunDialogue`.
    /// Preserve the byte cursor while the scene moves its actors. Ordinary
    /// page waits continue inside the same call and must not release it.
    pub fn pause_for_scene(&mut self) -> bool {
        if !matches!(self.stop, Some(PageEnd::Close | PageEnd::End)) {
            return false;
        }
        self.closed = true;
        true
    }

    /// The scene's next `RunDialogueResume` reopens at `Saved_Dialogue_Addr`.
    /// At `$F7` this is inside the same entry; at `$FF` it is the next entry.
    pub fn resume_scene(mut self, flags: &[bool]) -> Opening {
        if self.index >= self.segments.len() {
            return Opening::Jump(self.entry_id + 1);
        }
        self.flags = flags.to_vec();
        self.lines = vec![String::new()];
        self.line = 0;
        self.column = 0;
        self.portrait = None;
        self.stop = None;
        self.closed = false;
        self.done = false;
        self.pump();
        Opening::Window(Box::new(self))
    }

    /// A taken mid-message jump, once.
    pub fn take_jump(&mut self) -> Option<u16> {
        self.jump.take()
    }

    /// A mid-message `$F6` event, once.
    pub fn take_event(&mut self) -> Option<u16> {
        self.event.take()
    }

    /// Whether the next `$F2` has reached the end of the currently revealed
    /// text. An action before the first glyph is ready immediately.
    #[must_use]
    pub fn action_ready(&self, revealed: usize) -> bool {
        self.pending_action.is_some() && revealed >= self.visible_chars()
    }

    /// Takes the action without advancing the byte stream. The caller must
    /// dispatch it, then call [`TextFlow::resume_after_action`].
    pub fn take_pending_action(&mut self) -> Option<DialogueAction> {
        self.pending_action.take()
    }

    /// Resumes the retail text loop after the shell has applied one action.
    pub fn resume_after_action(&mut self) {
        self.pump();
    }

    /// Updates the copied event bank immediately, so a following `$FA` in the
    /// same entry sees a flag written by an embedded action.
    pub fn set_event_flag(&mut self, flag: u8) {
        let index = usize::from(flag);
        if self.flags.len() <= index {
            self.flags.resize(index + 1, false);
        }
        self.flags[index] = true;
    }

    /// Everything the flow could not act on since the last call.
    pub fn drain_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// The code byte `offset` segments ahead, or `None` when that is text or
    /// past the end -- which is the same lookahead the routine does, since the
    /// entry's own `$FF` terminator is not in the list.
    fn peek(&self, offset: usize) -> Option<u8> {
        match self.segments.get(self.index + offset) {
            Some(Segment::Control(ctrl)) => Some(ctrl.code()),
            _ => None,
        }
    }

    fn close(&mut self, end: PageEnd) {
        self.stop = Some(end);
    }

    /// `TextCtrlCode_Interrupt`. Returns whether the message ended here.
    ///
    /// It looks at the byte it stopped on before showing anything: the end of
    /// the entry and `$F7` terminate without an arrow, and a following `$FD`
    /// is swallowed so one page does not wait twice.
    fn interrupt(&mut self, waited: PageEnd) -> bool {
        if self.index >= self.segments.len() {
            self.close(PageEnd::End);
            self.done = true;
            return true;
        }
        match self.peek(0) {
            Some(0xF7) => {
                self.index += 1;
                self.close(PageEnd::Close);
            }
            Some(0xFD) => {
                self.index += 1;
                self.close(waited);
            }
            _ => self.close(waited),
        }
        false
    }

    /// Runs segments until a page is finished, a delay starts, or the entry
    /// ends.
    fn pump(&mut self) {
        while self.index < self.segments.len() {
            let segment = self.segments[self.index].clone();
            self.index += 1;
            let stop = match segment {
                Segment::Text(run) => self.run_text(&run),
                Segment::Control(ctrl) => self.run_control(&ctrl),
            };
            if stop {
                return;
            }
        }
        if self.lines.iter().any(|line| !line.is_empty()) {
            self.close(PageEnd::End);
        }
        self.done = true;
    }

    /// Writes a text run into the window, breaking and paging as it goes.
    /// Returns whether pumping stops here.
    fn run_text(&mut self, run: &str) -> bool {
        let chars: Vec<char> = run.chars().collect();
        let start = std::mem::take(&mut self.char_offset);
        for position in start..chars.len() {
            self.lines[self.line].push(chars[position]);
            self.column += 1;
            if self.column < CHARS_PER_LINE {
                continue;
            }
            self.column = 0;
            // The routine peeks at the next *byte*, which is the next segment
            // only when the run ends on this character.
            let at_run_end = position + 1 == chars.len();
            if at_run_end && self.peek(0) == Some(0xF5) {
                let Segment::Control(ctrl) = self.segments[self.index].clone() else {
                    unreachable!()
                };
                self.index += 1;
                return self.run_control(&ctrl);
            }
            self.line += 1;
            if at_run_end && self.peek(0) == Some(0xFC) {
                self.index += 1;
            }
            if !self.line.is_multiple_of(LINES_PER_WINDOW) {
                self.lines.push(String::new());
            } else if at_run_end {
                self.interrupt(PageEnd::Full);
                return true;
            } else {
                // More glyphs of this run follow, so the interrupt has nothing
                // to swallow: the page just fills. Resume mid-run.
                self.close(PageEnd::Full);
                self.index -= 1;
                self.char_offset = position + 1;
                return true;
            }
        }
        false
    }

    /// Runs one control code. Returns whether pumping stops here.
    fn run_control(&mut self, ctrl: &Ctrl) -> bool {
        match ctrl {
            Ctrl::Newline { .. } => {
                self.column = 0;
                self.line += 1;
                while self.lines.len() <= self.line {
                    // $FC does not check the two-line limit. Retail data never
                    // does this; a pack that did would be a data bug, and the
                    // window would show it rather than hide it.
                    self.lines.push(String::new());
                }
                false
            }
            Ctrl::Wait { .. } => {
                self.interrupt(PageEnd::Wait);
                true
            }
            Ctrl::Close { .. } => {
                self.close(PageEnd::Close);
                true
            }
            Ctrl::YesNo {
                yes_entry,
                no_entry,
                ..
            } => {
                self.choice = Some([*yes_entry, *no_entry]);
                self.close(PageEnd::Choice);
                true
            }
            Ctrl::Portrait { id, position, .. } => {
                self.portrait = (*id != PORTRAIT_HIDE).then_some(*id);
                if let Some(slot) = position {
                    self.log.push(format!(
                        "portrait: talk slot {slot} ignored (talk mode is later)"
                    ));
                }
                false
            }
            Ctrl::Delay { frames, .. } => {
                self.hold = *frames;
                true
            }
            Ctrl::Action {
                action,
                sound,
                panel,
                flag,
                ..
            } => {
                let action = match action {
                    ActionKind::LoadPanel => panel.map(DialogueAction::LoadPanel),
                    ActionKind::DestroyLastPanel => Some(DialogueAction::DestroyLastPanel),
                    ActionKind::DestroyAllPanels => Some(DialogueAction::DestroyAllPanels),
                    ActionKind::LoadSound => Some(DialogueAction::LoadSound(sound.unwrap_or(0))),
                    ActionKind::LoadSound2 => Some(DialogueAction::LoadSound2(sound.unwrap_or(0))),
                    ActionKind::UpdatePalette => Some(DialogueAction::UpdatePalette),
                    ActionKind::ZioEyesRed => Some(DialogueAction::ZioEyesRed),
                    ActionKind::PauseMusic => Some(DialogueAction::PauseMusic),
                    ActionKind::ResumeMusic => Some(DialogueAction::ResumeMusic),
                    ActionKind::SabotageAlarmRedPalette => {
                        Some(DialogueAction::SabotageAlarmRedPalette)
                    }
                    ActionKind::SetEventFlag => flag.map(DialogueAction::SetEventFlag),
                    ActionKind::ElsydeonBroken => Some(DialogueAction::ElsydeonBroken),
                };
                if let Some(action) = action {
                    self.pending_action = Some(action);
                    true
                } else {
                    self.log.push(format!(
                        "action {action:?} has an invalid or missing retail payload"
                    ));
                    false
                }
            }
            Ctrl::FlagCheck {
                flag, then_entry, ..
            } => {
                if self.flags.get(*flag as usize).copied().unwrap_or(false) {
                    self.jump = Some(self.entry_id + *then_entry);
                    self.done = true;
                    self.close(PageEnd::End);
                    return true;
                }
                self.log.push(format!(
                    "flag_check: flag {flag} -> entry {then_entry}, not taken"
                ));
                false
            }
            Ctrl::Event { id, .. } => {
                self.event = Some(*id);
                self.done = true;
                self.close(PageEnd::End);
                true
            }
            Ctrl::KeepNpcFacing { .. } => {
                // Mid-message this reaches TextCtrlCode_Null, which is an rts:
                // the message ends here.
                self.log
                    .push("keep_npc_facing mid-message: ends the message (rts)".to_owned());
                self.close(PageEnd::End);
                self.done = true;
                true
            }
        }
    }

    fn visible_chars(&self) -> usize {
        self.lines.iter().map(|line| line.chars().count()).sum()
    }
}
