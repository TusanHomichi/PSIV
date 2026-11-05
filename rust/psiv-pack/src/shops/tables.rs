//! The two shop tables: the port of `psiv_tools/shops.py`, which owns the
//! retail citations. `ShopInventories` is 49 `$FF`-terminated item lists with
//! no pointer table in front (`GetOffsetByID_FF_Delim` counts terminators);
//! `loc_68394` is the `$FFFF`-terminated run of eight-byte counter rows,
//! `(map id, x, y, group << 8 | index)`.

use crate::PackError;
use crate::map_symbols::{ITEM_SYMBOLS, MAP_SYMBOLS};

const TERMINATOR: u8 = 0xFF;
/// `ShopInventories`, end-exclusive: `0x682A3` is the `even` pad byte.
pub const INVENTORIES_LABEL: &str = "ShopInventories";
pub const INVENTORIES_START: usize = 0x681A4;
pub const INVENTORIES_END: usize = 0x682A3;
/// The shop index space three tables agree on.
pub const SHOP_COUNT: usize = 49;
/// The first two lists, unique in retail: it pins the address and the item-id
/// convention before anything is decoded.
const SIGNATURE: [u8; 8] = [0x7D, 0xFF, 0x01, 0x02, 0x08, 0x03, 0x09, 0xFF];
const PAD_BYTE: u8 = 0x00;
const NEXT_LABEL_FIRST_LONG: u32 = 0x002A_C470;
pub const INDEXING: &str = "GetOffsetByID_FF_Delim: shop N is the Nth $FF-terminated list, \
counted from the table start. No pointer table, no bounds check.";

pub const LOCATIONS_LABEL: &str = "loc_68394";
pub const LOCATIONS_START: usize = 0x68394;
pub const LOCATION_ENTRY_BYTES: usize = 8;
const LOCATION_TERMINATOR: u16 = 0xFFFF;

/// The high byte of `$FFFFECD0`. Only groups 1 and 2 index `ShopInventories`;
/// group 0 is an inn, with its own index space.
pub const SHOP_GROUPS: [(u8, &str); 3] = [(0, "inn"), (1, "shop"), (2, "shop_alternate_dialogue")];
pub const INN_GROUP: u8 = 0;

/// One `ShopInventories` list.
pub struct Inventory {
    pub index: usize,
    pub rom_offset: usize,
    pub raw: Vec<u8>,
    /// Item ids, 1-based.
    pub items: Vec<u8>,
}

/// One `loc_68394` row.
pub struct Location {
    pub rom_offset: usize,
    pub raw: [u8; LOCATION_ENTRY_BYTES],
    pub map_id: u16,
    pub x: u16,
    pub y: u16,
    pub selector: u16,
}

impl Location {
    pub fn group_id(&self) -> u8 {
        (self.selector >> 8) as u8
    }

    pub fn index(&self) -> usize {
        usize::from(self.selector & 0xFF)
    }

    pub fn is_inn(&self) -> bool {
        self.group_id() == INN_GROUP
    }

    pub fn group_name(&self) -> Option<&'static str> {
        group_name(self.group_id())
    }

    pub fn map_symbol(&self) -> Option<&'static str> {
        MAP_SYMBOLS.get(usize::from(self.map_id)).copied()
    }

    /// Inns never read `ShopInventories`.
    pub fn shop_inventory_index(&self) -> Option<usize> {
        (!self.is_inn()).then(|| self.index())
    }
}

pub fn group_name(id: u8) -> Option<&'static str> {
    SHOP_GROUPS
        .iter()
        .find(|(group, _)| *group == id)
        .map(|(_, name)| *name)
}

/// Item ids are 1-based: `ItemID_Dagger = 1`.
pub fn item_symbol(id: u8) -> Option<&'static str> {
    usize::from(id)
        .checked_sub(1)
        .and_then(|index| ITEM_SYMBOLS.get(index))
        .copied()
}

fn error(message: String) -> PackError {
    PackError(message)
}

fn hex(bytes: &[u8]) -> String {
    crate::png::hex(bytes)
}

/// `extract_shop_inventories`: the signature, the 49 lists, the boundary.
pub fn inventories(rom: &[u8]) -> Result<Vec<Inventory>, PackError> {
    let start = INVENTORIES_START;
    let found = rom.get(start..start + SIGNATURE.len());
    if found != Some(&SIGNATURE[..]) {
        return Err(error(format!(
            "ShopInventories signature mismatch at 0x{start:06X}: expected {}, got {}",
            hex(&SIGNATURE),
            hex(found.unwrap_or_default())
        )));
    }
    let count = rom
        .windows(SIGNATURE.len())
        .filter(|w| *w == SIGNATURE)
        .count();
    if count != 1 {
        return Err(error(format!(
            "ShopInventories signature {} occurs {count} times in this ROM; it must be unique \
             for the offset to be self-verifying",
            hex(&SIGNATURE)
        )));
    }
    let block = rom
        .get(start..INVENTORIES_END)
        .ok_or_else(|| error("ShopInventories runs past the end of the ROM".into()))?;
    let mut lists = Vec::new();
    let mut from = 0;
    for (cursor, &value) in block.iter().enumerate() {
        if value == TERMINATOR {
            lists.push((from, &block[from..=cursor]));
            from = cursor + 1;
        }
    }
    if from != block.len() {
        return Err(error(format!(
            "ShopInventories ends with {} unterminated bytes: {}",
            block.len() - from,
            hex(&block[from..])
        )));
    }
    if lists.len() != SHOP_COUNT {
        return Err(error(format!(
            "ShopInventories holds {} $FF-terminated lists between 0x{start:06X} and \
             0x{INVENTORIES_END:06X}; the shop index space is {SHOP_COUNT}",
            lists.len()
        )));
    }
    check_boundary(rom)?;
    lists
        .into_iter()
        .enumerate()
        .map(|(index, (offset, record))| {
            let body = &record[..record.len() - 1];
            if body.is_empty() {
                return Err(error(
                    "Empty shop record: a shop with nothing to sell".into(),
                ));
            }
            if let Some(bad) = body.iter().find(|&&v| item_symbol(v).is_none()) {
                return Err(error(format!(
                    "Shop record {} references item id {bad}, which is outside the 1..{} \
                     inventory table",
                    hex(record),
                    ITEM_SYMBOLS.len()
                )));
            }
            Ok(Inventory {
                index,
                rom_offset: start + offset,
                raw: record.to_vec(),
                items: body.to_vec(),
            })
        })
        .collect()
}

/// The end of the table, proven against the block that follows it.
fn check_boundary(rom: &[u8]) -> Result<(), PackError> {
    let end = INVENTORIES_END;
    let pad = rom.get(end).copied();
    if pad != Some(PAD_BYTE) {
        return Err(error(format!(
            "Expected the `even` pad byte 0x{PAD_BYTE:02X} at 0x{end:06X}, got {pad:?}"
        )));
    }
    let following = crate::m68k::l(rom, end + 1)?;
    if following != NEXT_LABEL_FIRST_LONG {
        return Err(error(format!(
            "Expected loc_682A4 to begin with 0x{NEXT_LABEL_FIRST_LONG:08X} at 0x{:06X}, got \
             0x{following:08X}",
            end + 1
        )));
    }
    Ok(())
}

/// `extract_shop_locations`: walk `loc_68394` to its `$FFFF`, and require the
/// shop rows to cover exactly the inventories' index space.
pub fn locations(rom: &[u8], shop_count: usize) -> Result<Vec<Location>, PackError> {
    let mut rows = Vec::new();
    let mut offset = LOCATIONS_START;
    loop {
        let Some(raw) = rom.get(offset..offset + LOCATION_ENTRY_BYTES) else {
            return Err(error(format!(
                "{LOCATIONS_LABEL} at 0x{LOCATIONS_START:06X} is not terminated before the end \
                 of the ROM"
            )));
        };
        let word = |at: usize| u16::from_be_bytes([raw[at], raw[at + 1]]);
        let map_id = word(0);
        if map_id == LOCATION_TERMINATOR {
            break;
        }
        let row = Location {
            rom_offset: offset,
            raw: raw.try_into().expect("an eight-byte row"),
            map_id,
            x: word(2),
            y: word(4),
            selector: word(6),
        };
        if row
            .shop_inventory_index()
            .is_some_and(|index| index >= shop_count)
        {
            return Err(error(format!(
                "{LOCATIONS_LABEL} entry at 0x{offset:06X} selects shop index 0x{:02X}, but \
                 ShopInventories only holds {shop_count} lists",
                row.index()
            )));
        }
        rows.push(row);
        offset += LOCATION_ENTRY_BYTES;
    }
    if rows.is_empty() {
        return Err(error(format!(
            "{LOCATIONS_LABEL} at 0x{LOCATIONS_START:06X} is empty"
        )));
    }
    let mut referenced: Vec<usize> = rows
        .iter()
        .filter_map(Location::shop_inventory_index)
        .collect();
    referenced.sort_unstable();
    referenced.dedup();
    if referenced != (0..shop_count).collect::<Vec<_>>() {
        return Err(error(format!(
            "loc_68394 and ShopInventories disagree about the shop index space: it references \
             {referenced:?} of 0..{shop_count}"
        )));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> Vec<u8> {
        let mut rom = vec![0u8; 0x70000];
        let mut at = INVENTORIES_START;
        rom[at..at + SIGNATURE.len()].copy_from_slice(&SIGNATURE);
        at += SIGNATURE.len();
        for _ in 2..SHOP_COUNT - 1 {
            rom[at] = 0x7D;
            rom[at + 1] = TERMINATOR;
            at += 2;
        }
        // The 49th list runs to the end of the block.
        while at < INVENTORIES_END - 1 {
            rom[at] = 0x01;
            at += 1;
        }
        rom[INVENTORIES_END - 1] = TERMINATOR;
        rom[INVENTORIES_END + 1..INVENTORIES_END + 5]
            .copy_from_slice(&NEXT_LABEL_FIRST_LONG.to_be_bytes());
        rom
    }

    #[test]
    fn the_lists_are_counted_by_their_terminators() {
        let rom = image();
        let lists = inventories(&rom).unwrap();
        assert_eq!(lists.len(), SHOP_COUNT);
        assert_eq!(lists[0].items, [0x7D]);
        assert_eq!(lists[1].items, [1, 2, 8, 3, 9]);
        assert_eq!(lists[1].rom_offset, INVENTORIES_START + 2);
    }

    #[test]
    fn a_changed_boundary_or_an_extra_list_is_refused() {
        let mut rom = image();
        rom[INVENTORIES_END] = 0x01;
        assert!(inventories(&rom).is_err(), "pad byte");
        let mut rom = image();
        rom[INVENTORIES_END - 3] = TERMINATOR;
        assert!(inventories(&rom).is_err(), "a fiftieth list");
        let mut rom = image();
        rom[INVENTORIES_START + 3] = 0xA1;
        assert!(inventories(&rom).is_err(), "an item id past the table");
    }

    #[test]
    fn locations_must_cover_every_shop_and_end_in_a_terminator() {
        let mut rom = vec![0u8; 0x70000];
        let mut at = LOCATIONS_START;
        let mut row = |map: u16, selector: u16| {
            for (i, word) in [map, 16, 32, selector].into_iter().enumerate() {
                rom[at + i * 2..at + i * 2 + 2].copy_from_slice(&word.to_be_bytes());
            }
            at += LOCATION_ENTRY_BYTES;
        };
        row(25, 0x0000);
        row(26, 0x0100);
        row(27, 0x0201);
        rom[at..at + 2].copy_from_slice(&LOCATION_TERMINATOR.to_be_bytes());
        let rows = locations(&rom, 2).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows[0].is_inn());
        assert_eq!(rows[0].shop_inventory_index(), None);
        assert_eq!(rows[2].shop_inventory_index(), Some(1));
        assert!(locations(&rom, 3).is_err(), "shop 2 unreachable");
        assert!(locations(&rom, 1).is_err(), "shop 1 out of range");
    }
}
