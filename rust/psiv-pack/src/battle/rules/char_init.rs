//! `InitializeCharStats`: the character record's layout.

use super::{
    CHARACTER_BYTES, CHARACTER_COUNT, CHARACTER_TABLE, byte, hex2, named_ram, ram_field, refuse,
    site, site_of_mod_stats,
};
use crate::PackError;
use crate::json::Json;
use crate::m68k::{expect, l, sw, w};

/// One field of the character record, as `_field` describes it.
pub struct Field {
    pub record_offset: usize,
    pub width: usize,
    pub count: usize,
    pub field: String,
    pub ram_offset: u16,
    pub mirrored_to: Option<String>,
}

impl Field {
    fn new(
        record_offset: usize,
        width: usize,
        count: usize,
        ram: u16,
        mirrored_to: Option<u16>,
    ) -> Result<Self, PackError> {
        Ok(Field {
            record_offset,
            width,
            count,
            field: named_ram(ram)?,
            ram_offset: ram,
            mirrored_to: mirrored_to.map(named_ram).transpose()?,
        })
    }

    fn to_json(&self) -> Json {
        Json::object()
            .with("record_offset", self.record_offset)
            .with("bytes", self.width * self.count)
            .with("width", self.width)
            .with("count", self.count)
            .with("field", self.field.as_str())
            .with("ram_offset", hex2(self.ram_offset))
            .with("mirrored_to", self.mirrored_to.clone())
    }
}

/// `read_char_init`.
pub struct CharInit {
    pub routine: usize,
    pub table: usize,
    pub record_bytes: usize,
    pub character_stats_ram: u32,
    pub struct_bytes: u16,
    pub fields: Vec<Field>,
}

impl CharInit {
    pub fn fields_json(&self) -> Vec<Json> {
        self.fields.iter().map(Field::to_json).collect()
    }
}

pub fn read_char_init(rom: &[u8]) -> Result<CharInit, PackError> {
    let site = site(rom, "InitializeCharStats")?;
    let character_stats_ram = 0xFFFF_0000 | u32::from(w(rom, site + 2)?);
    let table = l(rom, site + 6)? as usize;
    if byte(rom, site + 10)? != 0x7E {
        return refuse(format!(
            "0x{:06X}: InitializeCharStats does not set its loop counter with `moveq #imm, d7`",
            site + 10
        ));
    }
    let count = usize::from(byte(rom, site + 11)?) + 1;

    let mut fields: Vec<Field> = Vec::new();
    let mut at = site + 12;
    let mut consumed = 0;
    loop {
        let opcode = w(rom, at)?;
        match opcode {
            0x48E7 => break,
            0x1159 | 0x3159 | 0x2159 => {
                let width = match opcode {
                    0x1159 => 1,
                    0x3159 => 2,
                    _ => 4,
                };
                fields.push(Field::new(consumed, width, 1, w(rom, at + 2)?, None)?);
                consumed += width;
                at += 4;
            }
            0x3168 => {
                mirror(&mut fields, w(rom, at + 2)?, w(rom, at + 4)?)?;
                at += 6;
            }
            _ if opcode & 0xFF00 == 0x7C00 => {
                (at, consumed) = copy_block(rom, at, &mut fields, consumed)?;
            }
            _ => {
                return refuse(format!(
                    "0x{at:06X}: unknown opcode 0x{opcode:04X} in InitializeCharStats"
                ));
            }
        }
    }

    expect(rom, at + 4, 0x47E8, "lea equipment(a0), a3")?;
    expect(rom, at + 8, 0x4EB9, "jsr UpdateCharModStats")?;
    let mod_stats = l(rom, at + 10)? as usize;
    let update = site_of_mod_stats(rom)?;
    if mod_stats != update {
        return refuse(format!(
            "InitializeCharStats calls 0x{mod_stats:06X}, not the UpdateCharModStats at \
             0x{update:06X}"
        ));
    }
    if (table, count, consumed) != (CHARACTER_TABLE, CHARACTER_COUNT, CHARACTER_BYTES) {
        return refuse(format!(
            "InitializeCharStats reads {count} records of {consumed} bytes from 0x{table:06X}; \
             psiv_tools.core has {CHARACTER_COUNT} of {CHARACTER_BYTES} from \
             0x{CHARACTER_TABLE:06X}"
        ));
    }
    Ok(CharInit {
        routine: site,
        table,
        record_bytes: consumed,
        character_stats_ram,
        struct_bytes: struct_bytes(rom, at)?,
        fields,
    })
}

/// `_mirror`: the field just read is copied straight into another slot.
fn mirror(fields: &mut [Field], source: u16, destination: u16) -> Result<(), PackError> {
    if let Some(name) = ram_field(source)
        && let Some(field) = fields.iter_mut().rev().find(|f| f.field == name)
    {
        field.mirrored_to = Some(named_ram(destination)?);
        return Ok(());
    }
    refuse(format!(
        "InitializeCharStats mirrors 0x{source:02X}, which no read filled"
    ))
}

/// `_copy_block`: one `moveq / lea / move.b (a1)+, (a2)+ / dbf` array copy.
fn copy_block(
    rom: &[u8],
    mut at: usize,
    fields: &mut Vec<Field>,
    consumed: usize,
) -> Result<(usize, usize), PackError> {
    let iterations = usize::from(byte(rom, at + 1)?) + 1;
    expect(rom, at + 2, 0x45E8, "lea d16(a0), a2")?;
    let ram = w(rom, at + 4)?;
    expect(rom, at + 6, 0x14D9, "move.b (a1)+, (a2)+")?;
    at += 8;
    let mut mirrored = None;
    if w(rom, at)? == 0x14EA {
        let displacement = sw(rom, at + 2)?;
        if displacement != -1 {
            return refuse(format!(
                "0x{at:06X}: the array mirror reads {displacement}(a2), not -1(a2)"
            ));
        }
        mirrored = Some(ram + 1);
        at += 4;
    }
    expect(rom, at, 0x51CE, "dbf d6")?;
    fields.push(Field::new(consumed, 1, iterations, ram, mirrored)?);
    Ok((at + 4, consumed + iterations))
}

/// `_struct_bytes`: the `lea $80(a0), a0` that walks to the next struct.
fn struct_bytes(rom: &[u8], at: usize) -> Result<u16, PackError> {
    for probe in (at..at + 0x40).step_by(2) {
        if w(rom, probe)? == 0x41E8 && w(rom, probe + 4)? == 0x51CF {
            return Ok(w(rom, probe + 2)?);
        }
    }
    refuse(format!(
        "0x{at:06X}: InitializeCharStats' loop does not end in `lea d16(a0), a0` + `dbf d7`"
    ))
}
