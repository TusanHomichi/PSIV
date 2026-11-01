//! The event patches: what the hook routine each page copy jumps to writes.
//! [`PatchDecoder`] reads those routines out of the cartridge. It accepts the
//! encodings they use, and an unrecognised opcode is a failure rather than a
//! silently truncated patch.

use super::*;

#[cfg(test)]
mod tests;

/// One store into `Map_Layout`, resolved to plane and chunk coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchWrite {
    pub plane: Plane,
    pub chunk_x: usize,
    pub chunk_y: usize,
    pub chunk_ids: Vec<u8>,
    pub displacement: i16,
}

impl PatchWrite {
    pub fn to_json(&self) -> Json {
        Json::object()
            .with("plane", self.plane.name())
            .with("chunk_x", self.chunk_x)
            .with("chunk_y", self.chunk_y)
            .with("cell_x", self.chunk_x * COLLISION_CELL_TILES)
            .with("cell_y", self.chunk_y * COLLISION_CELL_TILES)
            .with(
                "chunk_ids",
                self.chunk_ids
                    .iter()
                    .map(|value| format!("0x{value:02X}"))
                    .collect::<Vec<_>>(),
            )
            .with("displacement", self.displacement)
    }
}

/// One event-flag-gated block of writes inside one page's hook routine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutPatch {
    pub plane: Plane,
    pub page: usize,
    pub routine: usize,
    pub event_flag: u16,
    pub writes: Vec<PatchWrite>,
}

impl LayoutPatch {
    pub fn event_symbol(&self) -> Option<&'static str> {
        event_flag_symbol(self.event_flag)
    }

    pub fn to_json(&self) -> Json {
        Json::object()
            .with("triggered_by_plane", self.plane.name())
            .with("page", self.page)
            .with("routine", hex6(self.routine))
            .with(
                "event_flag",
                Json::object()
                    .with("id", self.event_flag)
                    .with("id_hex", format!("0x{:02X}", self.event_flag))
                    .with("symbol", self.event_symbol()),
            )
            .with(
                "writes",
                self.writes
                    .iter()
                    .map(PatchWrite::to_json)
                    .collect::<Vec<_>>(),
            )
    }
}

const RTS: u16 = 0x4E75;
const MOVE_W_IMM_D0: u16 = 0x303C;
const MOVE_W_IMM_D1: u16 = 0x323C;
const JSR_ABS_L: u16 = 0x4EB9;
const BSR_W: u16 = 0x6100;
const MOVEQ_0_D0: u16 = 0x7000;
const BEQ_SHORT: u16 = 0x67;

/// A store into `(a1)`: its size, whether its source is an immediate rather
/// than d0, and whether it carries a displacement. A byte immediate is still
/// encoded as a word, high half ignored.
fn store_form(opcode: u16) -> Option<(usize, bool, bool)> {
    Some(match opcode {
        0x22BC => (4, true, false),  // move.l #imm,(a1)
        0x237C => (4, true, true),   // move.l #imm,d(a1)
        0x2280 => (4, false, false), // move.l d0,(a1)
        0x2340 => (4, false, true),  // move.l d0,d(a1)
        0x12BC => (1, true, false),  // move.b #imm,(a1)
        0x137C => (1, true, true),   // move.b #imm,d(a1)
        0x1280 => (1, false, false), // move.b d0,(a1)
        0x1340 => (1, false, true),  // move.b d0,d(a1)
        _ => return None,
    })
}

/// A reader for the instruction forms the hook routines are made of. Not a
/// 68000 disassembler: it refuses everything they don't use.
struct PatchDecoder<'rom> {
    rom: &'rom [u8],
    plane: Plane,
    page: usize,
    routine: usize,
    pos: usize,
}

impl PatchDecoder<'_> {
    fn fail(&self, message: String) -> PackError {
        PackError(format!(
            "{} page {} hook at {}, offset {}: {message}",
            upper(self.plane),
            self.page,
            hex6(self.routine),
            hex6(self.pos)
        ))
    }

    /// `length` big-endian bytes; bytes past the end of the ROM aren't there,
    /// and the opcode tests refuse what they leave.
    fn read(&mut self, length: usize) -> u32 {
        let value = self
            .rom
            .iter()
            .skip(self.pos)
            .take(length)
            .fold(0u32, |value, &byte| (value << 8) | u32::from(byte));
        self.pos += length;
        value
    }

    fn word(&mut self) -> u16 {
        self.read(2) as u16
    }

    fn signed_word(&mut self) -> i16 {
        self.word() as i16
    }

    fn decode(&mut self) -> Result<Vec<LayoutPatch>, PackError> {
        let mut patches = Vec::new();
        loop {
            let opcode = self.word();
            if opcode == RTS {
                return Ok(patches);
            }
            if opcode != MOVE_W_IMM_D0 {
                return Err(self.fail(format!(
                    "opcode 0x{opcode:04X} is not `move.w #imm,d0` or `rts`"
                )));
            }
            let event_flag = self.word();
            if self.word() != JSR_ABS_L || self.read(4) != EVENT_FLAGS_TEST {
                return Err(
                    self.fail("the flag load is not followed by `jsr (EventFlags_Test).l`".into())
                );
            }
            let branch = self.word();
            if branch >> 8 != BEQ_SHORT || branch & 0xFF == 0 {
                return Err(self.fail(format!(
                    "expected a short `beq` after the flag test, got 0x{branch:04X}"
                )));
            }
            let end = self.pos + usize::from(branch & 0xFF);
            let writes = self.block(end)?;
            patches.push(LayoutPatch {
                plane: self.plane,
                page: self.page,
                routine: self.routine,
                event_flag,
                writes,
            });
        }
    }

    /// One flag's writes, up to where its `beq` lands.
    fn block(&mut self, end: usize) -> Result<Vec<PatchWrite>, PackError> {
        let (mut column, mut row) = (None, None);
        let mut base = None;
        let mut d0_is_zero = false;
        let mut writes = Vec::new();
        while self.pos < end {
            let opcode = self.word();
            match opcode {
                MOVE_W_IMM_D0 => column = Some(self.word()),
                MOVE_W_IMM_D1 => row = Some(self.word()),
                BSR_W => {
                    let from = self.pos as i64;
                    let target = (from + i64::from(self.signed_word())) as usize;
                    base = Some(match target {
                        GET_MAP_LAYOUT_CHUNK_FG => Plane::Fg,
                        GET_MAP_LAYOUT_CHUNK_BG => Plane::Bg,
                        _ => {
                            return Err(self.fail(format!(
                                "`bsr.w {}` is not GetMapLayoutChunkFG/BG",
                                hex6(target)
                            )));
                        }
                    });
                    if column.is_none() || row.is_none() {
                        return Err(self.fail(
                            "GetMapLayoutChunk is reached without both coordinates set".into(),
                        ));
                    }
                    d0_is_zero = false;
                }
                MOVEQ_0_D0 => d0_is_zero = true,
                _ => {
                    let Some(form) = store_form(opcode) else {
                        return Err(self.fail(format!(
                            "opcode 0x{opcode:04X} is outside this decoder's vocabulary"
                        )));
                    };
                    let (Some(base), Some(column), Some(row)) = (base, column, row) else {
                        return Err(
                            self.fail("a store runs before GetMapLayoutChunk has set a1".into())
                        );
                    };
                    writes.push(self.store(form, base, column, row, d0_is_zero)?);
                }
            }
        }
        if self.pos != end {
            return Err(self.fail(format!("the block overruns its `beq` target {}", hex6(end))));
        }
        Ok(writes)
    }

    fn store(
        &mut self,
        (size, immediate, has_displacement): (usize, bool, bool),
        base: Plane,
        column: u16,
        row: u16,
        d0_is_zero: bool,
    ) -> Result<PatchWrite, PackError> {
        let value = if immediate {
            if size == 4 {
                self.read(4)
            } else {
                self.read(2) & 0xFF
            }
        } else {
            if !d0_is_zero {
                return Err(self.fail("a store reads d0 without a preceding `moveq #0,d0`".into()));
            }
            0
        };
        let displacement = if has_displacement {
            self.signed_word()
        } else {
            0
        };
        let bytes = value.to_be_bytes();
        self.resolve(base, column, row, displacement, bytes[4 - size..].to_vec())
    }

    /// `_resolve`: turn `d(a1)` into a plane and a chunk coordinate. A
    /// displacement of $F000 reaches the same cell of the other plane, so the
    /// only reading that works is the RAM address.
    fn resolve(
        &self,
        base: Plane,
        column: u16,
        row: u16,
        displacement: i16,
        chunk_ids: Vec<u8>,
    ) -> Result<PatchWrite, PackError> {
        let window_rows = WINDOW_ROWS as i64;
        let row_bytes = ROW_BYTES as i64;
        let window_row = i64::from(row) & (window_rows - 1);
        let origin = match base {
            Plane::Fg => MAP_LAYOUT_FG,
            Plane::Bg => MAP_LAYOUT_BG,
        };
        let address = origin + window_row * row_bytes + i64::from(column) + i64::from(displacement);
        let plane = if (MAP_LAYOUT_FG..MAP_LAYOUT_FG + LAYOUT_RAM_SPAN).contains(&address) {
            Plane::Fg
        } else if (MAP_LAYOUT_BG..MAP_LAYOUT_BG + LAYOUT_RAM_SPAN).contains(&address) {
            Plane::Bg
        } else {
            return Err(self.fail(format!(
                "displacement {displacement} puts the store at 0x{address:04X}, outside both \
                 Map_Layout regions"
            )));
        };
        let offset = address & (LAYOUT_RAM_SPAN - 1);
        // The window is modular, so take the row move nearest zero.
        let delta = (offset / row_bytes - window_row + window_rows / 2).rem_euclid(window_rows)
            - window_rows / 2;
        let chunk_x = offset.rem_euclid(row_bytes);
        let chunk_y = i64::from(row) + delta;
        if !(0..WIDTH_CHUNKS as i64).contains(&chunk_x)
            || !(0..HEIGHT_CHUNKS as i64).contains(&chunk_y)
        {
            return Err(self.fail(format!(
                "the store resolves to chunk ({chunk_x}, {chunk_y}), off the map"
            )));
        }
        let length = chunk_ids.len();
        if chunk_x as usize + length > WIDTH_CHUNKS {
            return Err(self.fail(format!(
                "a {length}-byte store at column {chunk_x} runs past the row"
            )));
        }
        Ok(PatchWrite {
            plane,
            chunk_x: chunk_x as usize,
            chunk_y: chunk_y as usize,
            chunk_ids,
            displacement,
        })
    }
}

/// `read_patch_table`: entries are `dc.w routine - table`.
fn read_patch_table(rom: &[u8], map_id: usize, plane: Plane) -> Result<Vec<usize>, PackError> {
    require_overworld(map_id)?;
    let site = patch_table_site(plane);
    let what = format!("{} patch table selector", upper(plane));
    check_bytes(rom, site.rom_offset, &site.signature(), &what)?;
    let base = site.table_for(map_id);
    Ok((0..PAGE_COUNT)
        .map(|page| {
            let at = base + page * 2;
            let offset = i16::from_be_bytes([rom[at], rom[at + 1]]);
            (base as i64 + i64::from(offset)) as usize
        })
        .collect())
}

/// `decode_patches`: every event-gated layout write both planes' page hooks
/// perform, at absolute chunk coordinates.
pub fn decode_patches(rom: &[u8], map_id: usize) -> Result<Vec<LayoutPatch>, PackError> {
    require_overworld(map_id)?;
    check_bytes(
        rom,
        GET_MAP_LAYOUT_CHUNK_FG,
        &get_map_layout_chunk_signature(),
        "GetMapLayoutChunkFG",
    )?;
    let mut patches = Vec::new();
    for plane in PLANES {
        for (page, routine) in read_patch_table(rom, map_id, plane)?
            .into_iter()
            .enumerate()
        {
            let mut decoder = PatchDecoder {
                rom,
                plane,
                page,
                routine,
                pos: routine,
            };
            patches.extend(decoder.decode()?);
        }
    }
    Ok(patches)
}
