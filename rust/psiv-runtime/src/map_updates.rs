//! Per-frame cartridge map programs. Source/order census: docs/field/MAP_UPDATES.md.

use psiv_core::battle::{Rng2, Rolls};
use psiv_core::{CameraPlane, Flag};
use psiv_data::{MapRecord, PalettePhase, PaletteWrite, ScrollMode, UpdateBank, UpdateProgram};

use crate::Runtime;

/// A source routine whose required native input does not exist (#41).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedMapUpdate {
    /// Table index.
    pub index: u8,
    /// Absent buffer/subsystem, as recorded by extraction.
    pub missing: String,
}

/// Exact sparse word writes to the scroll staging buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollWrite {
    /// Byte displacement in the retail staging buffer.
    pub offset: u16,
    /// Raw word, preserving signed two's-complement values.
    pub word: u16,
}

/// Read-only map-update presentation and volatile field state.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MapUpdateView {
    /// The 64-word CRAM shadow; empty for a legacy pack.
    pub palette: Vec<u16>,
    /// $ECF2, $ECF3 and $ECFF. Byte arithmetic always wraps.
    pub counters: [u8; 3],
    /// Last field frame that actually called RunMapUpdates.
    pub last_frame: Option<u16>,
    /// Number of calls since the map was loaded/refreshed.
    pub calls: u64,
    /// Changes to CRAM, for renderer upload suppression.
    pub palette_revision: u64,
    /// Scroll staging-buffer writes from the most recent call.
    pub scroll_writes: Vec<ScrollWrite>,
    /// The writes target VSRAM rather than the horizontal-scroll table.
    pub vertical_scroll: bool,
    /// Live Random_Battles_Flag override, if an update writes it.
    pub random_battles: Option<bool>,
    /// Missing native inputs, surfaced rather than approximated.
    pub unsupported: Vec<UnsupportedMapUpdate>,
}

impl MapUpdateView {
    pub(crate) fn loaded(record: &MapRecord) -> Self {
        Self {
            palette: record.map_update_palette.clone().unwrap_or_default(),
            unsupported: record
                .map_updates
                .iter()
                .flatten()
                .filter_map(|entry| {
                    if let UpdateProgram::Unsupported { missing } = &entry.program {
                        Some(UnsupportedMapUpdate {
                            index: entry.index,
                            missing: missing.clone(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            ..Self::default()
        }
    }

    fn write(&mut self, slot: usize, word: u16) {
        if self.palette[slot] != word {
            self.palette[slot] = word;
            self.palette_revision = self.palette_revision.wrapping_add(1);
        }
    }

    fn writes(&mut self, writes: &[PaletteWrite]) {
        for write in writes {
            self.write(usize::from(write.slot), write.word);
        }
    }

    fn scroll(&mut self, offset: u16, word: u16) {
        self.scroll_writes.push(ScrollWrite { offset, word });
    }
}

fn flag(bank: UpdateBank, id: u8) -> Flag {
    match bank {
        UpdateBank::Event => Flag::event(u16::from(id)),
        UpdateBank::Chest => Flag::chest(u16::from(id)),
        UpdateBank::Temp => Flag::temp(u16::from(id)),
    }
}

impl Runtime {
    /// Run once at the retail update point, never as a vblank side effect.
    pub(crate) fn run_map_updates(&mut self) {
        let y = self.scene_party_actor(0).map_or_else(
            || (crate::geometry::driver_of(self.party.leader()).y >> 16) as u16,
            |actor| {
                ((i32::from(actor.cell.y) - 1) * 16
                    + actor
                        .render_offset_16ths(self.party.leader().step_frames())
                        .1) as u16
            },
        );
        let Some(record) = self.data.map(psiv_data::MapId(self.map.id().0)) else {
            return;
        };
        let Some(updates) = &record.map_updates else {
            return;
        };
        let view = &mut self.effects.updates;
        view.last_frame = Some(self.frames);
        view.calls += 1;
        view.scroll_writes.clear();
        view.vertical_scroll = false;
        for entry in updates {
            match &entry.program {
                UpdateProgram::Noop | UpdateProgram::Unsupported { .. } => {}
                UpdateProgram::Palette {
                    gates,
                    bypass,
                    cycles,
                    stopped,
                } => {
                    let active = bypass
                        .is_some_and(|id| self.game.is_set(Flag::event(u16::from(id))))
                        || gates
                            .iter()
                            .all(|g| self.game.is_set(flag(g.bank, g.flag)) == g.set);
                    if !active {
                        view.writes(stopped);
                        continue;
                    }
                    for cycle in cycles {
                        let clock = if cycle.timer {
                            view.counters[0] = view.counters[0].wrapping_add(1);
                            u16::from(view.counters[0])
                        } else {
                            self.frames
                        };
                        if clock & cycle.frame_mask != 0 {
                            continue;
                        }
                        let phase = match cycle.phase {
                            PalettePhase::Frame { mask, shift } => {
                                usize::from((clock & mask) >> shift)
                            }
                            PalettePhase::Counter { slot } => {
                                let counter = &mut view.counters[usize::from(slot)];
                                if usize::from(*counter) >= cycle.frames.len() {
                                    *counter = 0;
                                }
                                let phase = usize::from(*counter);
                                *counter = counter.wrapping_add(1);
                                phase
                            }
                        };
                        view.writes(&cycle.frames[phase]);
                    }
                }
                UpdateProgram::Flags {
                    chests,
                    flag: id,
                    clear_chest,
                } => {
                    if *clear_chest {
                        let _ = self.game.clear(Flag::chest(u16::from(*id)));
                    } else if chests
                        .iter()
                        .all(|id| self.game.is_set(Flag::chest(u16::from(*id))))
                    {
                        let _ = self.game.set(Flag::event(u16::from(*id)));
                    }
                }
                UpdateProgram::Camera {
                    foreground,
                    x_shift,
                    y_shift,
                } => {
                    let plane = if *foreground {
                        CameraPlane::Foreground
                    } else {
                        CameraPlane::Background
                    };
                    self.camera.damp_step(plane, *x_shift, *y_shift);
                }
                UpdateProgram::Tunnels => {
                    let (fg_x, _) = self.camera.position_on(CameraPlane::Foreground);
                    let (bg_x, _) = self.camera.position_on(CameraPlane::Background);
                    for row in 0..32 {
                        view.scroll(row * 32, (fg_x as i16).wrapping_neg() as u16);
                    }
                    for row in 0..32 {
                        let word = (bg_x as i16).wrapping_mul(if row < 16 { -1 } else { -2 });
                        view.scroll(2 + row * 32, word as u16);
                    }
                    self.camera.shift_x_step(CameraPlane::Background, 2);
                }
                UpdateProgram::Scroll { mode, samples } => {
                    let counter = if *mode == ScrollMode::Unsigned { 0 } else { 2 };
                    view.counters[counter] = view.counters[counter].wrapping_add(1);
                    let phase = view.counters[counter];
                    let (fg_x, _) = self.camera.position_on(CameraPlane::Foreground);
                    let (_, bg_y) = self.camera.position_on(CameraPlane::Background);
                    view.vertical_scroll = *mode == ScrollMode::Vertical;
                    let count = if view.vertical_scroll { 20 } else { 256 };
                    for row in 0..count {
                        let stride = match mode {
                            ScrollMode::Unsigned => 0x60,
                            ScrollMode::Signed => 4,
                            ScrollMode::Vertical => 0x2C,
                        };
                        let sample = samples[usize::from(phase.wrapping_add((row * stride) as u8))];
                        let displacement = match mode {
                            ScrollMode::Unsigned => i16::from(sample >> 3),
                            ScrollMode::Signed => (i16::from(sample as i8) >> 4) - 16,
                            ScrollMode::Vertical => i16::from(sample as i8) >> 5,
                        };
                        if view.vertical_scroll {
                            view.scroll(row * 4, displacement as u16);
                            view.scroll(row * 4 + 2, bg_y as u16);
                        } else {
                            view.scroll(row * 4, (fg_x as i16).wrapping_neg() as u16);
                            view.scroll(row * 4 + 2, displacement as u16);
                        }
                    }
                    if view.vertical_scroll {
                        self.camera
                            .write_y_word(CameraPlane::Foreground, view.scroll_writes[0].word);
                    }
                }
                UpdateProgram::Crystals { colors, rest } => {
                    view.counters[0] = view.counters[0].wrapping_sub(1);
                    if view.counters[0] != 0 {
                        view.write(2, *rest);
                        view.write(18, *rest);
                    } else if self.frames & 3 != 0 {
                        view.counters[0] = 1;
                    } else if view.counters[1] == 4 {
                        let mut rng2 = Rng2::with_surrogate(&mut self.rng, self.frames);
                        view.counters[0] = rng2.next_roll() as u8;
                        view.counters[1] = 0;
                    } else {
                        let color = colors[usize::from(view.counters[1])];
                        view.write(2, color);
                        view.write(18, color);
                        view.counters[1] = view.counters[1].wrapping_add(1);
                        view.counters[0] = 1;
                    }
                }
                UpdateProgram::Edge { colors } => {
                    for slot in 0..14 {
                        view.write(slot, 0);
                    }
                    if self.frames & 1 == 0 {
                        if view.counters[0] >= 13 {
                            view.counters[0] = 0;
                        }
                        for (i, &color) in colors.iter().enumerate() {
                            view.write(1 + (usize::from(view.counters[0]) + i) % 13, color);
                        }
                        view.counters[0] = view.counters[0].wrapping_add(1);
                    }
                }
                UpdateProgram::EdgeLine { frames, .. } => {
                    if self.frames & 3 == 0 {
                        view.writes(&frames[usize::from(view.counters[1])]);
                        view.counters[1] = view.counters[1].wrapping_add(1);
                        if usize::from(view.counters[1]) >= frames.len() {
                            view.counters[1] = 0;
                        }
                    }
                }
                UpdateProgram::Music {
                    secondary_map,
                    sound,
                } => {
                    self.saved_map_index_2 = *secondary_map;
                    self.saved_sound_index = *sound;
                }
                UpdateProgram::Encounters { y_max } => {
                    view.random_battles = Some(y > *y_max);
                }
            }
        }
    }
}
