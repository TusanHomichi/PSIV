//! The instruction reader: one routine walked into the paths it can take, the
//! port of `psiv_tools/map_effects/decoder.py`, which owns the reasons.
//!
//! Not a 68000 emulator. It models immediates, `lea`, stores through an address
//! register, conditional branches, the flag tests and `GetMapLayoutOffset`;
//! steps over calls and bulk copies of known length, recording them as
//! `deferred`; and stops the entry on anything else, because an unknown opcode
//! has an unknown length. It forks at each conditional branch, and runs `dbf`
//! loops for real when the counter is known.
//!
//! The order of the tests in [`Decoder::walk`] is Python's, and it matters: the
//! masks overlap.

use std::collections::HashMap;

use super::anchors::{
    FIELD_OBJ_LAST, FIELD_OBJ_SECONDARY, FIELD_OBJ_STRIDE, GET_MAP_LAYOUT_OFFSET, MAP_LAYOUT_BG,
    MAP_LAYOUT_FG, OBJECT_DIALOGUE_OFFSET, OBJECT_X_OFFSET, OBJECT_Y_OFFSET, bank_address,
};
use crate::json::Json;
use crate::map_symbols::event_flag_symbol;
use crate::maps::layouts::Plane;
use crate::maps::records::hex6;
use crate::png::hex;

const STEPS: usize = 4000;
const MAX_PATHS: usize = 64;

/// What `GetMapLayoutOffset` left in a1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutTarget {
    pub plane: Plane,
    pub chunk_x: u16,
    pub chunk_y: u16,
}

/// One flag test a path passed through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate {
    pub bank: &'static str,
    pub flag: u16,
    pub required: bool,
}

impl Gate {
    pub fn to_json(&self) -> Json {
        let symbol = if self.bank == "event_flags" {
            event_flag_symbol(self.flag)
        } else {
            None
        };
        Json::object()
            .with("bank", self.bank)
            .with("flag", self.flag)
            .with("flag_hex", format!("0x{:02X}", self.flag))
            .with("symbol", symbol)
            .with("required", if self.required { "set" } else { "clear" })
    }
}

/// One effect. `layout` is set on a `layout_write`, which only becomes a cell
/// once a map's geometry is known; every other kind's fields are in `detail`.
#[derive(Clone, Debug, PartialEq)]
pub struct Write {
    pub kind: &'static str,
    pub at: i64,
    pub detail: Json,
    pub layout: Option<(LayoutTarget, i16, Option<u16>)>,
}

/// The writes a run of instructions reaches together, with what reaches them.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub gates: Vec<Gate>,
    pub writes: Vec<Write>,
    pub aborts: bool,
    pub conditions: Vec<String>,
    pub deferred: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Address {
    Plain(i64),
    Layout(LayoutTarget),
}

#[derive(Clone, Default)]
struct Regs {
    d: [Option<u16>; 8],
    a: [Option<Address>; 8],
    pending_gate: Option<(&'static str, u16)>,
    pending_compare: bool,
}

/// How a store is encoded.
struct StoreForm {
    register: usize,
    displaced: bool,
    source: Source,
}

enum Source {
    Clear,
    Immediate { word: bool },
    Register { index: usize, word: bool },
}

/// `STORES` and `REG_STORES`, for a0..a6.
fn store_form(op: u16) -> Option<StoreForm> {
    let low = usize::from(op & 7);
    let high = usize::from((op >> 9) & 7);
    let form = |register, displaced, source| {
        (register < 7).then_some(StoreForm {
            register,
            displaced,
            source,
        })
    };
    match op & 0xFFF8 {
        0x4250 => return form(low, false, Source::Clear),
        0x4268 => return form(low, true, Source::Clear),
        _ => {}
    }
    let immediate = |word| Source::Immediate { word };
    match op & 0xF1FF {
        0x10BC => return form(high, false, immediate(false)),
        0x117C => return form(high, true, immediate(false)),
        0x30BC => return form(high, false, immediate(true)),
        0x317C => return form(high, true, immediate(true)),
        _ => {}
    }
    let register = |word| Source::Register { index: low, word };
    match op & 0xF1F8 {
        0x1080 => form(high, false, register(false)),
        0x1140 => form(high, true, register(false)),
        0x3080 => form(high, false, register(true)),
        0x3140 => form(high, true, register(true)),
        _ => None,
    }
}

/// `DEFERRED_OPS`: recognised, not modelled, of known length.
fn deferred_op(op: u16) -> Option<(i64, &'static str)> {
    match op {
        0x4E41 => return Some((2, "trap #1 bulk copy")),
        0x4E40 => return Some((2, "trap #0 bulk clear")),
        0x08A8 | 0x08B8 => return Some((6, "bclr on a memory bit")),
        _ => {}
    }
    let copy = matches!(op & 0xF1F8, 0x10D8 | 0x20D8 | 0x30D8) && (op >> 9) & 7 < 7 && op & 7 < 7;
    copy.then_some((2, "postincrement bulk copy"))
}

/// `COMPARE_BRANCHES`: branches that follow an arithmetic compare.
fn compare_branch(op: u16) -> Option<&'static str> {
    Some(match op & 0xFF00 {
        0x6200 => "bhi",
        0x6300 => "bls",
        0x6400 => "bcc",
        0x6500 => "bcs",
        0x6A00 => "bpl",
        0x6B00 => "bmi",
        0x6C00 => "bge",
        0x6D00 => "blt",
        0x6E00 => "bgt",
        0x6F00 => "ble",
        _ => return None,
    })
}

fn note_once(notes: &mut Vec<String>, note: String) {
    if !notes.contains(&note) {
        notes.push(note);
    }
}

fn object(kind: &'static str, at: i64, detail: Json) -> Write {
    Write {
        kind,
        at,
        detail,
        layout: None,
    }
}

/// The reader; a refusal is the reason the entry stays undecoded.
pub struct Decoder<'a> {
    pub rom: &'a [u8],
    pub start: i64,
    pub tests: &'a HashMap<i64, &'static str>,
    pub clears: &'a HashMap<i64, &'static str>,
    pub kos: i64,
    pub paths: Vec<Path>,
}

impl Decoder<'_> {
    /// Big-endian bytes from `at`; what lies past the image reads as absent.
    fn read(&self, at: i64, length: usize) -> u32 {
        let start = usize::try_from(at).unwrap_or(usize::MAX);
        self.rom
            .iter()
            .skip(start)
            .take(length)
            .fold(0, |value, &byte| (value << 8) | u32::from(byte))
    }

    fn w(&self, at: i64) -> u16 {
        self.read(at, 2) as u16
    }

    fn sw(&self, at: i64) -> i64 {
        i64::from(self.w(at) as i16)
    }

    fn fail<T>(&self, at: i64, why: String) -> Result<T, String> {
        let start = usize::try_from(at)
            .unwrap_or(self.rom.len())
            .min(self.rom.len());
        let end = (start + 6).min(self.rom.len());
        Err(format!(
            "+0x{:X}: {why} (bytes {})",
            at - self.start,
            hex(&self.rom[start..end])
        ))
    }

    pub fn run(&mut self) -> Result<(), String> {
        self.walk(
            self.start,
            Regs::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    /// A short or word branch's target and the address after it.
    fn branch_ends(&self, pc: i64, op: u16) -> (i64, i64) {
        if op & 0xFF != 0 {
            (pc + 2 + i64::from((op & 0xFF) as u8 as i8), pc + 2)
        } else {
            (pc + 2 + self.sw(pc + 2), pc + 4)
        }
    }

    #[allow(clippy::too_many_lines)]
    fn walk(
        &mut self,
        mut pc: i64,
        mut regs: Regs,
        gates: Vec<Gate>,
        mut writes: Vec<Write>,
        mut deferred: Vec<String>,
        mut conditions: Vec<String>,
    ) -> Result<(), String> {
        if self.paths.len() >= MAX_PATHS {
            return Err(format!("more than {MAX_PATHS} paths"));
        }
        let mut steps = 0;
        loop {
            steps += 1;
            if steps > STEPS {
                return self.fail(pc, format!("no rts within {STEPS} instructions"));
            }
            let op = self.w(pc);
            let dn = usize::from((op >> 9) & 7);
            let low = usize::from(op & 7);

            if op == 0x4E75 {
                self.paths.push(Path {
                    gates,
                    writes,
                    aborts: regs.d[7].is_some_and(|value| value != 0),
                    conditions,
                    deferred,
                });
                return Ok(());
            }

            // `dbf dN,disp`: a known counter runs the loop for real.
            if op & 0xFFF8 == 0x51C8 {
                match regs.d[low] {
                    None => {
                        note_once(&mut deferred, "loop".into());
                        pc += 4;
                    }
                    Some(value) => {
                        let value = value.wrapping_sub(1);
                        regs.d[low] = Some(value);
                        pc = if value != 0xFFFF {
                            pc + 2 + self.sw(pc + 2)
                        } else {
                            pc + 4
                        };
                    }
                }
                continue;
            }

            // addq/subq on a data register: the table walk's index step.
            if matches!(op & 0xF1C0, 0x5040 | 0x5140) {
                let Some(value) = regs.d[low] else {
                    return self.fail(pc, format!("addq/subq on d{low}, which is not tracked"));
                };
                let amount = match (op >> 9) & 7 {
                    0 => 8,
                    amount => amount,
                };
                regs.d[low] = Some(if op & 0x0100 != 0 {
                    value.wrapping_sub(amount)
                } else {
                    value.wrapping_add(amount)
                });
                pc += 2;
                continue;
            }

            if let Some((size, note)) = deferred_op(op) {
                note_once(&mut deferred, note.into());
                pc += size;
                continue;
            }

            // move.w #imm,dN / move.b #imm,dN / moveq #imm,dN
            if op & 0xF1FF == 0x303C {
                regs.d[dn] = Some(self.w(pc + 2));
                pc += 4;
                continue;
            }
            if op & 0xF1FF == 0x103C {
                regs.d[dn] = Some(self.w(pc + 2) & 0xFF);
                pc += 4;
                continue;
            }
            if op & 0xF100 == 0x7000 {
                regs.d[dn] = Some(op & 0xFF);
                pc += 2;
                continue;
            }

            // lea (abs).w / (abs).l / d16(aN) -> aM
            if op & 0xF1FF == 0x41F8 {
                regs.a[dn] = Some(Address::Plain(i64::from(self.w(pc + 2))));
                pc += 4;
                continue;
            }
            if op & 0xF1FF == 0x41F9 {
                regs.a[dn] = Some(Address::Plain(i64::from(self.read(pc + 2, 4))));
                pc += 6;
                continue;
            }
            if op & 0xF1F8 == 0x41E8 {
                let Some(Address::Plain(source)) = regs.a[low] else {
                    return self.fail(
                        pc,
                        "lea from an address register that is not a plain address".into(),
                    );
                };
                regs.a[dn] = Some(Address::Plain(source + self.sw(pc + 2)));
                pc += 4;
                continue;
            }

            // calls
            let call = if op == 0x4EB9 {
                Some((i64::from(self.read(pc + 2, 4)), 6))
            } else if op == 0x6100 {
                Some((pc + 2 + self.sw(pc + 2), 4))
            } else if op & 0xFF00 == 0x6100 {
                Some((pc + 2 + i64::from((op & 0xFF) as u8 as i8), 2))
            } else {
                None
            };
            if let Some((target, size)) = call {
                self.call(pc, target, &mut regs, &mut writes, &mut deferred)?;
                pc += size;
                continue;
            }

            // `cmpi.w #imm, d16(aN)`: a position compare, a condition this
            // slice does not model.
            if op & 0xFFF8 == 0x0C68 {
                note_once(
                    &mut conditions,
                    format!(
                        "position compare at {}: #{:#06x} against ${:02X}(a{low})",
                        hex6(pc as usize),
                        self.w(pc + 2),
                        self.w(pc + 4)
                    ),
                );
                regs.pending_compare = true;
                pc += 6;
                continue;
            }

            // A branch on that compare: both arms, ungated.
            if let Some(name) = compare_branch(op) {
                if !regs.pending_compare {
                    return self.fail(pc, format!("a {name} with no compare before it"));
                }
                regs.pending_compare = false;
                let (target, after) = self.branch_ends(pc, op);
                for destination in [target, after] {
                    self.walk(
                        destination,
                        regs.clone(),
                        gates.clone(),
                        writes.clone(),
                        deferred.clone(),
                        conditions.clone(),
                    )?;
                }
                return Ok(());
            }

            if matches!(op & 0xFF00, 0x6700 | 0x6600 | 0x6000) {
                let (target, after) = self.branch_ends(pc, op);
                if op & 0xFF00 == 0x6000 {
                    pc = target;
                    continue;
                }
                let bne = op & 0xFF00 == 0x6600;
                let Some((bank, flag)) = regs.pending_gate.take() else {
                    let kind = if bne { "bne" } else { "beq" };
                    return self.fail(pc, format!("{kind} with no flag test before it"));
                };
                // The test leaves Z set when the flag is CLEAR.
                for (destination, required) in [(target, bne), (after, !bne)] {
                    let arm = Gate {
                        bank,
                        flag,
                        required,
                    };
                    // An arm contradicting a gate this path carries cannot
                    // happen within one map load.
                    if gates
                        .iter()
                        .any(|g| g.bank == bank && g.flag == flag && g.required != required)
                    {
                        continue;
                    }
                    let mut extended = gates.clone();
                    if !extended.contains(&arm) {
                        extended.push(arm);
                    }
                    self.walk(
                        destination,
                        regs.clone(),
                        extended,
                        writes.clone(),
                        deferred.clone(),
                        conditions.clone(),
                    )?;
                }
                return Ok(());
            }

            // A store to an absolute short address.
            if matches!(op, 0x4278 | 0x11FC | 0x31FC) {
                let clear = op == 0x4278;
                let address = i64::from(self.w(if clear { pc + 2 } else { pc + 4 }));
                let value = (!clear).then(|| self.w(pc + 2));
                absolute(pc, address, value, &mut writes, &mut deferred);
                pc += if clear { 4 } else { 6 };
                continue;
            }

            // `move.b (aN,dM.w),dK`: with both known, a constant.
            if op & 0xF1F8 == 0x1030 {
                let extension = self.w(pc + 2);
                let index = regs.d[usize::from((extension >> 12) & 7)];
                let displacement = i64::from((extension & 0xFF) as u8 as i8);
                regs.d[dn] = match (regs.a[low], index) {
                    (Some(Address::Plain(base)), Some(index)) => {
                        let at = base + i64::from(index) + displacement;
                        (at > 0 && at < self.rom.len() as i64)
                            .then(|| u16::from(self.rom[at as usize]))
                    }
                    _ => None,
                };
                pc += 4;
                continue;
            }

            // Any other indexed or absolute load: forget the register, so a
            // later store using it fails instead of inventing a value.
            if op & 0xF1F8 == 0x3030 || matches!(op & 0xF1FF, 0x1038 | 0x3038) {
                regs.d[dn] = None;
                pc += 4;
                continue;
            }

            let Some(form) = store_form(op) else {
                return self.fail(
                    pc,
                    format!("opcode 0x{op:04X} is outside this decoder's vocabulary"),
                );
            };
            pc += self.store(pc, &form, &regs, &mut writes, &mut deferred)?;
        }
    }

    fn call(
        &self,
        pc: i64,
        target: i64,
        regs: &mut Regs,
        writes: &mut Vec<Write>,
        deferred: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(bank) = self.tests.get(&target) {
            let Some(flag) = regs.d[0] else {
                return self.fail(pc, "a flag test with no id in d0".into());
            };
            regs.pending_gate = Some((bank, flag));
            return Ok(());
        }
        if target == GET_MAP_LAYOUT_OFFSET {
            for register in [1, 2, 3] {
                if regs.d[register].is_none() {
                    return self.fail(pc, format!("GetMapLayoutOffset with d{register} unset"));
                }
            }
            regs.a[1] = Some(Address::Layout(LayoutTarget {
                plane: if regs.d[3] != Some(0) {
                    Plane::Bg
                } else {
                    Plane::Fg
                },
                chunk_x: regs.d[1].unwrap_or(0),
                chunk_y: regs.d[2].unwrap_or(0),
            }));
            return Ok(());
        }
        if target == self.kos {
            let plane = match regs.a[1] {
                Some(Address::Plain(MAP_LAYOUT_FG)) => Plane::Fg,
                Some(Address::Plain(MAP_LAYOUT_BG)) => Plane::Bg,
                _ => {
                    return self.fail(
                        pc,
                        "KosDecomp with an unrecognised source or destination".into(),
                    );
                }
            };
            let Some(Address::Plain(source)) = regs.a[0] else {
                return self.fail(
                    pc,
                    "KosDecomp with an unrecognised source or destination".into(),
                );
            };
            writes.push(object(
                "layout_replace",
                pc,
                Json::object()
                    .with("plane", plane.name())
                    .with("source", hex6(source as usize)),
            ));
            return Ok(());
        }
        if let Some(bank) = self.clears.get(&target) {
            let Some(flag) = regs.d[0] else {
                return self.fail(pc, "a flag clear with no id in d0".into());
            };
            writes.push(object(
                "flag_clear",
                pc,
                Json::object()
                    .with("bank", *bank)
                    .with("bank_address", format!("0xFFFF{:04X}", bank_address(bank)))
                    .with("flag", flag)
                    .with("flag_hex", format!("0x{flag:02X}"))
                    .with("door", hex6(target as usize)),
            ));
            return Ok(());
        }
        note_once(deferred, format!("call to {}", hex6(target as usize)));
        Ok(())
    }

    /// One store through an address register; returns its length.
    fn store(
        &self,
        pc: i64,
        form: &StoreForm,
        regs: &Regs,
        writes: &mut Vec<Write>,
        deferred: &mut Vec<String>,
    ) -> Result<i64, String> {
        // `word` is Python's value size, with a clear being neither.
        let (value, word, cleared, after) = match form.source {
            Source::Clear => (None, false, true, pc + 2),
            Source::Immediate { word: true } => (Some(self.w(pc + 2)), true, false, pc + 4),
            Source::Immediate { word: false } => {
                (Some(self.w(pc + 2) & 0xFF), false, false, pc + 4)
            }
            Source::Register { index, word } => {
                let Some(value) = regs.d[index] else {
                    return self.fail(
                        pc,
                        format!("a store sources d{index}, which is not tracked"),
                    );
                };
                (Some(value), word, false, pc + 2)
            }
        };
        let displacement = if form.displaced { self.sw(after) } else { 0 };
        let size = after - pc + if form.displaced { 2 } else { 0 };
        let Some(base) = regs.a[form.register] else {
            return self.fail(
                pc,
                format!("a store through a{} before it is loaded", form.register),
            );
        };
        let base = match base {
            Address::Layout(target) => {
                writes.push(Write {
                    kind: "layout_write",
                    at: pc,
                    detail: Json::object(),
                    layout: Some((target, displacement as i16, value)),
                });
                return Ok(size);
            }
            Address::Plain(base) => base,
        };
        let address = base + displacement;
        if (FIELD_OBJ_SECONDARY..=FIELD_OBJ_LAST + OBJECT_DIALOGUE_OFFSET).contains(&address) {
            let index = (address - FIELD_OBJ_SECONDARY) / FIELD_OBJ_STRIDE;
            let within = (address - FIELD_OBJ_SECONDARY) % FIELD_OBJ_STRIDE;
            let slot = Json::object().with("object_index", index as usize);
            if within == 0 && (cleared || (word && value == Some(0))) {
                writes.push(object("object_despawn", pc, slot));
            } else if within == 0 && word {
                writes.push(object("object_rewrite", pc, slot.with("object_id", value)));
            } else if within == OBJECT_DIALOGUE_OFFSET && !word && !cleared {
                writes.push(object(
                    "object_dialogue",
                    pc,
                    slot.with("dialogue_id", value),
                ));
            } else if within == OBJECT_X_OFFSET && word {
                writes.push(object("object_x", pc, slot.with("pixels", value)));
            } else if within == OBJECT_Y_OFFSET && word {
                writes.push(object("object_y", pc, slot.with("pixels", value)));
            } else {
                note_once(
                    deferred,
                    format!("write at +0x{within:X} of object slot {index}"),
                );
            }
            return Ok(size);
        }
        note_once(deferred, format!("write to 0xFFFF{address:04X}"));
        Ok(size)
    }
}

/// `_absolute`: a store straight to an absolute short address.
fn absolute(
    pc: i64,
    address: i64,
    value: Option<u16>,
    writes: &mut Vec<Write>,
    deferred: &mut Vec<String>,
) {
    if (FIELD_OBJ_SECONDARY..=FIELD_OBJ_LAST + OBJECT_DIALOGUE_OFFSET).contains(&address) {
        let index = (address - FIELD_OBJ_SECONDARY) / FIELD_OBJ_STRIDE;
        let within = (address - FIELD_OBJ_SECONDARY) % FIELD_OBJ_STRIDE;
        if within == 0 && value.unwrap_or(0) == 0 {
            writes.push(object(
                "object_despawn",
                pc,
                Json::object().with("object_index", index as usize),
            ));
            return;
        }
    }
    note_once(deferred, format!("write to 0xFFFF{address:04X}"));
}

#[cfg(test)]
mod tests;
