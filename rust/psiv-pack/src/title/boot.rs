//! The title palette cycle, derived from the cartridge: the port of
//! `_TitleBoot`, `run_title_boot` and `derive_palette_cycle` in
//! `psiv_tools/title_pack.py`, whose docs give the reasons. It follows
//! `Palette_Table_Buffer` from power-on to the title menu one game frame at a
//! time, under tape 25's input, reading every count, step and table from the
//! instructions that hold them.

use crate::PackError;
use crate::json::Json;
use crate::m68k::{expect, l, w};

const PALETTE_WORDS: usize = 64;
const START_OPCODE: u16 = 0x0838; // btst #n, (Joypad_Pressed).w
const JOYPAD_PRESSED: u16 = 0xEF03;

/// Tape 25's sampled frames (1-based `retro_run()` frames).
pub const FRAMES: [u32; 16] = [
    25, 50, 75, 100, 125, 150, 175, 200, 300, 400, 401, 450, 500, 550, 600, 650,
];
/// Tape 25 holds START on these frames.
const START_HELD: [(u32, u32); 2] = [(401, 405), (605, 609)];
/// `TITLE_LAG_FRAMES`: measured lag, by routine and game frame within it.
const LAG: [(&str, &[(u32, u32)]); 4] = [
    ("GameMode_Sega", &[(0, 10), (1, 4)]),
    ("TitleRoutine_FadingText", &[(0, 49), (1, 2)]),
    (
        "loc_427DA",
        &[
            (0, 15),
            (3, 1),
            (5, 1),
            (7, 1),
            (9, 1),
            (11, 1),
            (13, 1),
            (14, 1),
            (15, 1),
            (17, 1),
            (19, 1),
            (20, 1),
            (21, 1),
            (23, 1),
        ],
    ),
    (
        "loc_42C0E",
        &[
            (0, 1),
            (1, 2),
            (2, 2),
            (3, 2),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 2),
            (8, 1),
        ],
    ),
];
const NOTE: &str = "Retail CRAM words at Tape 25's frames, derived from the ROM's Sega and title \
routines; values are decimal Genesis CRAM words.";

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn byte(rom: &[u8], at: usize) -> Result<u8, PackError> {
    rom.get(at)
        .copied()
        .ok_or_else(|| PackError(format!("0x{at:06X}: a read runs past the end of the image")))
}

/// `_imm`: the word operand after the opcode at `at`.
fn imm(rom: &[u8], at: usize, opcode: u16, what: &str) -> Result<u16, PackError> {
    expect(rom, at, opcode, what)?;
    Ok(w(rom, at + 2)?)
}

/// `_imm(..., long=True)`.
fn imm_long(rom: &[u8], at: usize, opcode: u16, what: &str) -> Result<u32, PackError> {
    expect(rom, at, opcode, what)?;
    Ok(l(rom, at + 2)?)
}

/// `_moveq`: the immediate of `moveq #n, dR`.
fn moveq(rom: &[u8], at: usize, register: u16, what: &str) -> Result<usize, PackError> {
    let word = w(rom, at)?;
    if word & 0xFF00 != 0x7000 | register << 9 {
        return refuse(format!(
            "0x{at:06X}: expected {what} (moveq to d{register})"
        ));
    }
    Ok(usize::from(word & 0xFF))
}

/// `_entry`: the palette entry an absolute-short `$FBxx.w` operand names.
fn entry(rom: &[u8], at: usize, opcode: u16, what: &str) -> Result<usize, PackError> {
    let operand = i32::from(imm(rom, at, opcode, what)?) - 0xFB00;
    if operand & 1 != 0 || !(0..(PALETTE_WORDS as i32) * 2).contains(&operand) {
        return refuse(format!("0x{at:06X}: {what} is not a palette word"));
    }
    Ok((operand / 2) as usize)
}

/// `_expect_start_test`: the loop tests START in `Joypad_Pressed`.
fn expect_start_test(rom: &[u8], at: usize, what: &str) -> Result<(), PackError> {
    if imm(rom, at, START_OPCODE, what)? != 7 || w(rom, at + 4)? != JOYPAD_PRESSED {
        return refuse(format!(
            "0x{at:06X}: {what} does not test START in Joypad_Pressed"
        ));
    }
    Ok(())
}

fn ror(word: u16, count: u32) -> u16 {
    word.rotate_right(count)
}

fn rol(word: u16, count: u32) -> u16 {
    word.rotate_left(count)
}

/// `_TitleBoot`: the buffer when the game next waits after each VInt.
struct Boot<'a> {
    rom: &'a [u8],
    pal: [u16; PALETTE_WORDS],
    mfc: u32,
    frame: u32,
    states: Vec<[u16; PALETTE_WORDS]>,
    vint_frames: Vec<u32>,
    held: bool,
    pressed: bool,
    routine: &'static str,
    routine_mfc: u32,
}

impl<'a> Boot<'a> {
    fn new(rom: &'a [u8]) -> Self {
        Boot {
            rom,
            pal: [0; PALETTE_WORDS],
            mfc: 0,
            frame: 0,
            states: Vec::new(),
            vint_frames: vec![0],
            held: false,
            pressed: false,
            routine: "",
            routine_mfc: 0,
        }
    }

    fn enter(&mut self, routine: &'static str) {
        self.routine = routine;
        self.routine_mfc = self.mfc;
    }

    /// `VInt_Prepare`: the buffer is uploaded at the next VInt.
    fn wait(&mut self) {
        self.states.push(self.pal);
        let since = self.mfc - self.routine_mfc;
        let lag = LAG
            .iter()
            .find(|(name, _)| *name == self.routine)
            .and_then(|(_, frames)| frames.iter().find(|(k, _)| *k == since))
            .map_or(0, |(_, n)| *n);
        self.frame += 1 + lag;
        self.mfc += 1;
        self.vint_frames.push(self.frame);
        let held = START_HELD
            .iter()
            .any(|&(from, to)| (from..to).contains(&self.frame));
        self.pressed = held && !self.held;
        self.held = held;
    }

    fn palette_at(&self, frame: u32) -> Result<[u16; PALETTE_WORDS], PackError> {
        let k = self.vint_frames.partition_point(|&f| f <= frame) - 1;
        self.states
            .get(k)
            .copied()
            .ok_or_else(|| PackError(format!("frame {frame} is past the modelled title routines")))
    }

    /// `Pal_FadeIn`.
    fn fade_in(&mut self) {
        let target = self.pal;
        self.pal = [0; PALETTE_WORDS];
        self.wait();
        loop {
            let mut changed = false;
            for (index, &goal_word) in target.iter().enumerate() {
                let (mut current, mut goal) = (self.pal[index], goal_word);
                for _ in 0..4 {
                    let nibble = current & 0xF;
                    if nibble != goal & 0xF {
                        changed = true;
                        current = (current & 0xFFF0) | (nibble + 1);
                    }
                    current = ror(current, 4);
                    goal = ror(goal, 4);
                }
                self.pal[index] = current;
            }
            self.wait();
            if !changed {
                return;
            }
        }
    }

    /// `PalFadeOut_ClrSpriteTbl`.
    fn fade_out(&mut self) {
        loop {
            let mut seen = 0;
            for word in self.pal.iter_mut() {
                let mut current = *word;
                for _ in 0..4 {
                    let nibble = (current & 0xF).saturating_sub(1);
                    current &= 0xFFF0;
                    current = rol(current | nibble, 4);
                }
                *word = current;
                seen |= current;
            }
            self.wait();
            if seen == 0 {
                return;
            }
        }
    }

    // -- GameMode_Sega -----------------------------------------------------

    fn sega(&mut self) -> Result<(), PackError> {
        let rom = self.rom;
        self.enter("GameMode_Sega");
        self.fade_out();
        expect(rom, 0x4107A, 0x43F8, "lea (Palette_Table_Buffer).w, a1")?;
        let longs = moveq(rom, 0x4107E, 7, "moveq #$1F, d7")? + 1;
        let fill = imm_long(rom, 0x41080, 0x203C, "move.l #$0E000E00, d0")?;
        for i in 0..longs {
            self.pal[2 * i] = (fill >> 16) as u16;
            self.pal[2 * i + 1] = fill as u16;
        }
        self.fade_in();
        let mut count = i32::from(imm(rom, 0x410A6, 0x323C, "move.w #$B3, d1")?);
        expect_start_test(rom, 0x410C4, "btst #ButtonStart (GameMode_SegaLoop)")?;
        expect(rom, 0x410CA, 0x56C9, "dbne d1, GameMode_SegaLoop")?;
        let (mut timer, mut cycle) = (0u16, 0u16);
        loop {
            (timer, cycle) = self.sega_cycle(timer, cycle)?;
            self.wait();
            if self.pressed {
                break;
            }
            count -= 1;
            if count < 0 {
                break;
            }
        }
        self.sega_exit_palette()?;
        self.wait();
        self.fade_out();
        Ok(())
    }

    /// `DoSegaPaletteCycling`.
    fn sega_cycle(&mut self, timer: u16, mut cycle: u16) -> Result<(u16, u16), PackError> {
        let rom = self.rom;
        let period = imm(rom, 0x41114, 0x0C79, "cmpi.w #5, ($FFFF0002).l")?;
        let mut timer = timer;
        if timer == period {
            let fade_end = imm(rom, 0x41126, 0x0C40, "cmpi.w #8, d0")?;
            let shine_end = imm(rom, 0x41142, 0x0C40, "cmpi.w #$E, d0")?;
            let white_end = imm(rom, 0x41198, 0x0C40, "cmpi.w #$15, d0")?;
            let mut acted = true;
            if cycle < fade_end {
                let table = imm_long(rom, 0x4112C, 0x41F9, "lea (loc_411B8).l, a0")? as usize;
                expect(rom, 0x41138, 0x3341, "move.w d1, 0(a1)")?;
                expect(rom, 0x4113C, 0x3341, "move.w d1, 2(a1)")?;
                let first = usize::from(w(rom, 0x4113A)? / 2);
                let second = usize::from(w(rom, 0x4113E)? / 2);
                let value = u16::from(byte(rom, table + usize::from(cycle))?) << 8;
                self.pal[first] = value;
                self.pal[second] = value;
            } else if cycle < shine_end {
                expect(rom, 0x41148, 0x5140, "subq.w #8, d0")?;
                let step = usize::from(cycle - 8);
                let table = imm_long(rom, 0x4114E, 0x41F9, "lea (loc_411C0).l, a0")? as usize;
                let start = usize::from(imm(rom, 0x41154, 0x45E9, "lea $4(a1), a2")? / 2);
                let count = moveq(rom, 0x41158, 7, "moveq #4, d7")? + 1;
                let mut index = step + 1;
                for offset in 0..count {
                    self.pal[start + offset] = u16::from(byte(rom, table + index)?) << 8;
                    index = index.saturating_sub(1);
                }
                let table = imm_long(rom, 0x41174, 0x41F9, "lea (loc_411C8).l, a0")? as usize;
                let start = usize::from(imm(rom, 0x4117A, 0x45E9, "lea $1A(a1), a2")? / 2);
                let count = moveq(rom, 0x4117E, 7, "moveq #5, d7")? + 1;
                let mut index = step + 1;
                for offset in 0..count {
                    let word = self.pal[start - offset];
                    self.pal[start - offset] =
                        (word & 0xFF00) | u16::from(byte(rom, table + index)?);
                    index = index.saturating_sub(1);
                }
            } else if cycle < white_end {
                let add = imm(rom, 0x4119E, 0x0669, "addi.w #$222, 2(a1)")?;
                let at = usize::from(w(rom, 0x411A2)? / 2);
                self.pal[at] = self.pal[at].wrapping_add(add);
            } else {
                acted = false;
            }
            if acted {
                cycle += 1;
                timer = 0;
            }
        }
        Ok((timer + 1, cycle))
    }

    /// `loc_410E2`: black, white, black, two ramps, two blacks.
    fn sega_exit_palette(&mut self) -> Result<(), PackError> {
        let rom = self.rom;
        expect(rom, 0x410E2, 0x41F8, "lea (Palette_Table_Buffer).w, a0")?;
        let white = imm(rom, 0x410EA, 0x30FC, "move.w #$EEE, (a0)+")?;
        let mut ramp = imm(rom, 0x410F0, 0x323C, "move.w #$200, d1")?;
        let first = moveq(rom, 0x410F4, 7, "moveq #5, d7")? + 1;
        let first_step = imm(rom, 0x410F6, 0x0641, "addi.w #$200, d1")?;
        let second = moveq(rom, 0x41100, 7, "moveq #4, d7")? + 1;
        let second_step = imm(rom, 0x41104, 0x0641, "addi.w #$20, d1")?;
        expect(rom, 0x4110C, 0x20C0, "move.l d0, (a0)+")?;
        let mut words = vec![0, white, 0];
        for _ in 0..first {
            ramp = ramp.wrapping_add(first_step);
            words.push(ramp);
        }
        for _ in 0..second {
            words.push(ramp);
            ramp = ramp.wrapping_add(second_step);
        }
        words.extend([0, 0]);
        self.pal[..words.len()].copy_from_slice(&words);
        Ok(())
    }

    // -- GameMode_Title ----------------------------------------------------

    fn title(&mut self) -> Result<(), PackError> {
        let rom = self.rom;
        self.enter("TitleRoutine_FadingText");
        let source = imm_long(rom, 0x4255C, 0x41F9, "lea (Pal_TitleScreen).l, a0")? as usize;
        let words = usize::from(imm(rom, 0x42566, 0x303C, "move.w #$3F, d0")?) + 1;
        for i in 0..words {
            self.pal[i] = w(rom, source + 2 * i)?;
        }
        self.wait();
        expect(rom, 0x42610, 0x08B8, "bclr #0, (CredText_A).w")?;
        expect(rom, 0x4261A, 0x7000, "moveq #0, d0")?;
        let text_entries = [
            entry(
                rom,
                0x4261C,
                0x31C0,
                "move.w d0, (Palette_Table_Buffer+$3E).w",
            )?,
            entry(
                rom,
                0x42620,
                0x31C0,
                "move.w d0, (Palette_Table_Buffer+$5E).w",
            )?,
        ];
        for e in text_entries {
            self.pal[e] = 0;
        }
        self.wait();
        self.fade_in();
        self.fading_text(text_entries)?;
        self.reveal()?;
        self.press_start()?;
        self.enter_menu()
    }

    /// `TitleRoutine_FadingTextLoop` until START.
    fn fading_text(&mut self, text_entries: [usize; 2]) -> Result<(), PackError> {
        let rom = self.rom;
        expect_start_test(
            rom,
            0x4263C,
            "btst #ButtonStart (TitleRoutine_FadingTextLoop)",
        )?;
        let last = u32::from(imm(rom, 0x42646, 0x0C78, "cmpi.w #$E0, (TextCounter).w")?);
        let mut scroll = u32::from(imm(
            rom,
            0x42534,
            0x31FC,
            "move.w #$FF20, (Camera_Y_Pos_BG).w",
        )?) << 16;
        let speed = imm_long(rom, 0x42654, 0x06B8, "addi.l #$2280, (Camera_Y_Pos_BG).w")?;
        let bit = u32::from(imm(rom, 0x42662, 0x0800, "btst #$10, d0")?);
        let records = imm_long(rom, 0x429E6, 0x41F9, "lea (Title_FadingTextData).l, a0")? as usize;
        let (mut counter, mut control) = (0u32, 0u32);
        while !self.pressed {
            if counter == last {
                return refuse(
                    "TextCounter reached the end of the fading text (loc_426E0), which this \
                     model does not follow"
                        .into(),
                );
            }
            let next = scroll.wrapping_add(speed);
            let moved = scroll ^ next;
            scroll = next;
            if moved >> bit & 1 != 0 {
                counter += 1;
            }
            self.wait();
            control = self.read_fading_text(records, counter, control)?;
            control = self.run_fading_text(text_entries, control)?;
        }
        Ok(())
    }

    /// `ReadTitleFadingTextData`.
    fn read_fading_text(
        &self,
        records: usize,
        counter: u32,
        control: u32,
    ) -> Result<u32, PackError> {
        let mut at = records;
        loop {
            let key = byte(self.rom, at)?;
            if u32::from(key) == counter & 0xFF {
                let routine = byte(self.rom, at + 1)?;
                if routine > 5 {
                    return refuse(format!("0x{at:06X}: fading-text routine {routine}"));
                }
                return Ok(control | if routine < 2 { 1 } else { 1 << routine });
            }
            if key == 0xFF {
                return Ok(control);
            }
            at += 3;
        }
    }

    /// `RunTitleFadingText`: every eighth frame, step a line's text colour.
    fn run_fading_text(
        &mut self,
        text_entries: [usize; 2],
        mut control: u32,
    ) -> Result<u32, PackError> {
        let rom = self.rom;
        let mask = u32::from(imm(rom, 0x42B22, 0x0241, "andi.w #7, d1")?);
        let step_in = imm(rom, 0x42B2A, 0x0641, "addi.w #$222, d1")?;
        let full = u32::from(imm(rom, 0x42B2E, 0x0801, "btst #$C, d1")?);
        let step_out = imm(rom, 0x42B74, 0x0441, "subi.w #$222, d1")?;
        let snapshot = control;
        if self.mfc & 0xFF & mask != 0 {
            return Ok(control);
        }
        for (line, &e) in text_entries.iter().enumerate() {
            if snapshot >> (2 + line) & 1 != 0 {
                let word = self.pal[e].wrapping_add(step_in);
                if u32::from(word) >> full & 1 != 0 {
                    control &= !(1 << (2 + line));
                } else {
                    self.pal[e] = word;
                }
            }
        }
        for (line, &e) in text_entries.iter().enumerate() {
            if snapshot >> (4 + line) & 1 != 0 {
                if self.pal[e] != 0 {
                    self.pal[e] = self.pal[e].wrapping_sub(step_out);
                } else {
                    control = (control & !(1 << (4 + line))) | 1;
                }
            }
        }
        Ok(control)
    }

    /// `loc_427DA` to `loc_4295C`.
    fn reveal(&mut self) -> Result<(), PackError> {
        let rom = self.rom;
        self.enter("loc_427DA");
        let white = imm(rom, 0x427E6, 0x303C, "move.w #$EEE, d0")?;
        for at in [0x427EA, 0x427EE] {
            let e = entry(rom, at, 0x31C0, "move.w d0, (Palette_Table_Buffer+n).w")?;
            self.pal[e] = white;
        }
        self.wait();
        self.wait();
        let last = imm(rom, 0x428E8, 0x0C00, "cmpi.b #$18, d0")?;
        let mut step = 0;
        loop {
            step += 1;
            if step == last {
                break;
            }
            self.wait();
        }
        self.wait();
        Ok(())
    }

    /// `TitleRoutine_PressStartButton` with `DoPressStartButtonCyclingPal`.
    fn press_start(&mut self) -> Result<(), PackError> {
        let rom = self.rom;
        self.enter("TitleRoutine_PressStartButton");
        let mut count = i32::from(imm(rom, 0x42BDE, 0x303C, "move.w #$233, d0")?);
        expect_start_test(rom, 0x42BF0, "btst #ButtonStart (PressStartButtonLoop)")?;
        let table = imm_long(
            rom,
            0x42CC4,
            0x45F9,
            "lea (PressStartButtonCyclingPal).l, a2",
        )? as usize;
        let every = u32::from(imm(rom, 0x42CD0, 0x0241, "andi.w #3, d1")?);
        let phase = u32::from(imm(rom, 0x42CD6, 0x0240, "andi.w #$7C, d0")?);
        expect(rom, 0x42CDA, 0xE248, "lsr.w #1, d0")?;
        let at = usize::from(imm(rom, 0x42CE0, 0x3352, "move.w (a2), $4A(a1)")? / 2);
        loop {
            if self.mfc & every == 0 {
                self.pal[at] = w(rom, table + ((self.mfc & phase) >> 1) as usize)?;
            }
            self.wait();
            if self.pressed {
                return Ok(());
            }
            count -= 1;
            if count < 0 {
                return refuse(
                    "the prompt timed out into TitleRoutine_CharPortraits, which this model \
                     does not follow"
                        .into(),
                );
            }
        }
    }

    /// `loc_42C0E` to `loc_42C90`.
    fn enter_menu(&mut self) -> Result<(), PackError> {
        let rom = self.rom;
        self.enter("loc_42C0E");
        let last = imm(rom, 0x42C24, 0x0C00, "cmpi.b #9, d0")?;
        let mut step = 0;
        loop {
            step += 1;
            if step == last {
                break;
            }
            self.wait();
        }
        let source = imm_long(rom, 0x42C96, 0x41F9, "lea (Pal_Init_Line_3).l, a0")? as usize;
        let start = entry(rom, 0x42C9C, 0x43F8, "lea (Palette_Line_3).w, a1")?;
        let words = moveq(rom, 0x42CA0, 7, "moveq #$F, d7")? + 1;
        for i in 0..words {
            self.pal[start + i] = w(rom, source + 2 * i)?;
        }
        let loops = moveq(rom, 0x42CB2, 0, "moveq #9, d0")? + 1;
        for _ in 0..loops {
            self.held = false;
            self.wait();
        }
        self.wait();
        self.states.push(self.pal);
        Ok(())
    }
}

/// One sampled frame's 64 CRAM words.
pub type FrameWords = (u32, [u16; PALETTE_WORDS]);

/// `derive_palette_cycle`: the four CRAM lines at each sampled frame, and
/// each frame's words.
pub fn derive(rom: &[u8]) -> Result<(Json, Vec<FrameWords>), PackError> {
    let mut boot = Boot::new(rom);
    boot.sega()?;
    boot.title()?;
    let mut frames = Json::object();
    let mut words = Vec::new();
    let mut previous: Option<[u16; PALETTE_WORDS]> = None;
    for frame in FRAMES {
        let palette = boot.palette_at(frame)?;
        let mut lines = Json::object();
        for line in 0..4 {
            lines.insert(
                line.to_string(),
                palette[line * 16..(line + 1) * 16].to_vec(),
            );
        }
        let changed: Vec<usize> = previous.map_or_else(Vec::new, |old| {
            (0..PALETTE_WORDS)
                .filter(|&i| old[i] != palette[i])
                .collect()
        });
        frames.insert(
            frame.to_string(),
            Json::object()
                .with("lines", lines)
                .with("changed_entries_from_previous", changed),
        );
        words.push((frame, palette));
        previous = Some(palette);
    }
    Ok((
        Json::object()
            .with("format_version", 1u8)
            .with("kind", "psiv_title_palette_cycle")
            .with("line_count", 4u8)
            .with("colors_per_line", 16u8)
            .with("note", NOTE)
            .with("frames", frames),
        words,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_in_steps_each_nibble_toward_its_target() {
        let rom = [0u8; 4];
        let mut boot = Boot::new(&rom);
        boot.pal[0] = 0x0E42;
        boot.fade_in();
        assert_eq!(boot.pal[0], 0x0E42);
        // One wait for the display, then one per step of the largest nibble
        // (14), then the pass that changes nothing.
        assert_eq!(boot.states.len(), 1 + 14 + 1);
        assert_eq!(boot.states[1][0], 0x0111);
    }

    #[test]
    fn a_fade_out_ends_black_and_the_lag_table_delays_frames() {
        let rom = [0u8; 4];
        let mut boot = Boot::new(&rom);
        boot.pal[3] = 0x0002;
        boot.enter("GameMode_Sega");
        boot.fade_out();
        assert_eq!(boot.pal[3], 0);
        assert_eq!(boot.vint_frames[..3], [0, 11, 16]);
    }

    #[test]
    fn start_is_pressed_on_its_first_held_frame_only() {
        let rom = [0u8; 4];
        let mut boot = Boot::new(&rom);
        boot.frame = 399;
        boot.wait();
        assert!(!boot.pressed);
        boot.wait();
        assert!(boot.pressed);
        boot.wait();
        assert!(!boot.pressed && boot.held);
    }
}
