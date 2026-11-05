//! How a shop is reached, what it charges and what an inn does: the port of
//! `psiv_tools/shop_flow.py`, which owns the retail citations and the table of
//! routines. Each rule is read from the instruction that applies it, found by
//! a pinned signature, so `shops.json`'s `rules` is retail's, not a copy.

use crate::json::Json;
use crate::m68k::{expect, find_exactly, l, w};
use crate::{PackError, Rom};

use super::tables::{self, SHOP_GROUPS};

/// Signatures, each with the number of times retail holds it.
const COUNTER_SCAN: (&str, usize) = ("41f900068394", 1);
const COUNTER_SCAN_CALL: (&str, usize) = ("4eb900065d12", 1);
const BUY_PRICE: (&str, usize) = ("31f00014e3f8", 1);
const SELL_PRICE: (&str, usize) = ("30300014e248", 1);
const INN_RATES: (&str, usize) = ("43f900068112", 1);
const INN_PARTY_MULTIPLY: (&str, usize) = ("c6f9ffffe3f0", 1);
const GREETING_SELECTOR: (&str, usize) = ("41f900068136", 1);
const PORTRAIT_GROUPS: (&str, usize) = ("41f900067ffe", 1);
const RECOVER_HP: (&str, usize) = ("31680010000e", 5);
const REST_EVENT: (&str, usize) = ("0c780006ecd0", 1);

/// `Character_Stats` offsets `RecoverStats` writes.
const RECOVERY_FIELDS: [(u16, &str); 7] = [
    (0x0E, "curr_hp"),
    (0x10, "max_hp"),
    (0x12, "curr_tp"),
    (0x14, "max_tp"),
    (0x16, "status"),
    (0x6A, "curr_skill_uses"),
    (0x6B, "max_skill_uses"),
];
/// The item record offset holding the shop price.
pub const PRICE_OFFSET: usize = 0x14;
const PRICE_BYTES: usize = 2;
/// The most party slots `CalcWinIDAndCharNum` counts.
pub const MAX_PARTY: u32 = 5;
const SKILL_SLOTS: u16 = 8;

/// The decoded rules, and the numbers the rest of the build reads from them.
pub struct Flow {
    pub json: Json,
    pub sell_divisor: u32,
    pub inn_table: usize,
    pub greetings_table: usize,
    pub portrait_pointers: usize,
    /// Each portrait group's art addresses, group 0 the innkeepers.
    pub portrait_groups: [Vec<u32>; 2],
}

fn hex6(value: usize) -> String {
    format!("0x{value:06X}")
}

fn ram(word: u16) -> String {
    format!("0x{:08X}", 0xFFFF_0000 | u32::from(word))
}

fn sites(
    rom: &[u8],
    (signature, count): (&str, usize),
    label: &str,
) -> Result<Vec<usize>, PackError> {
    Ok(find_exactly(rom, signature, count, label)?)
}

fn site(rom: &[u8], pin: (&str, usize), label: &str) -> Result<usize, PackError> {
    Ok(sites(rom, pin, label)?[0])
}

fn fail(message: String) -> PackError {
    PackError(message)
}

/// `loc_65D12`: the scan that turns an object's position into a selector.
fn counter_scan(rom: &[u8]) -> Result<(Json, usize, usize), PackError> {
    let at0 = site(rom, COUNTER_SCAN, "CounterScan")?;
    let table = l(rom, at0 + 2)? as usize;
    expect(rom, at0 + 6, 0x3438, "move.w (Field_Map_Index).w, d2")?;
    let field_map_index = w(rom, at0 + 8)?;
    expect(rom, at0 + 10, 0x0C50, "cmpi.w #terminator, (a0)")?;
    let terminator = w(rom, at0 + 12)?;
    let mut compares = Vec::new();
    let mut at = at0 + 16;
    for register in ["d2", "d0", "d1"] {
        let opcode = w(rom, at)?;
        if opcode == 0xB450 {
            compares.push((0, register));
            at += 2;
        } else if opcode == 0xB068 || opcode == 0xB268 {
            compares.push((w(rom, at + 2)?, register));
            at += 4;
        } else {
            return Err(fail(format!(
                "0x{at:06X}: unexpected opcode 0x{opcode:04X} in the counter scan"
            )));
        }
        if rom.get(at) != Some(&0x66) {
            return Err(fail(format!(
                "0x{at:06X}: the counter scan's {register} compare is not followed by a `bne.s`"
            )));
        }
        at += 2;
    }
    expect(rom, at, 0x31E8, "move.w d16(a0), ($FFFFECD0).w")?;
    let selector_offset = w(rom, at + 2)?;
    let selector_ram = w(rom, at + 4)?;
    let stride = scan_stride(rom, at)?;
    if usize::from(selector_offset) + 2 != stride {
        return Err(fail(format!(
            "the counter scan reads its selector at +0x{selector_offset:X} of a {stride}-byte \
             entry; the selector is not the entry's last word"
        )));
    }
    let mut fields: Vec<Json> = compares
        .iter()
        .map(|(offset, register)| {
            Json::object()
                .with("offset", format!("0x{offset:02X}"))
                .with("compared_against", *register)
        })
        .collect();
    fields.push(
        Json::object()
            .with("offset", format!("0x{selector_offset:02X}"))
            .with("compared_against", Json::Null),
    );
    let call = site(rom, COUNTER_SCAN_CALL, "CounterScanCall")?;
    let json = Json::object()
        .with("routine", hex6(at0))
        .with("called_from", hex6(call))
        .with("table", hex6(table))
        .with("entry_bytes", stride)
        .with("terminator", format!("0x{terminator:04X}"))
        .with("field_map_index", ram(field_map_index))
        .with("selector_ram", ram(selector_ram))
        .with("fields", fields)
        .with("matches_on", "position")
        .with(
            "note",
            "the caller reaches this only after Interaction_ChkObjects has matched an object, \
             and passes that object's $30/$34 position. The scan never reads the collision \
             grid: a shop is bound to the shopkeeper's position, not to the counter tile in \
             front of them.",
        );
    Ok((json, table, stride))
}

/// The `addq.w #n, a0` that steps the scan by one entry.
fn scan_stride(rom: &[u8], at: usize) -> Result<usize, PackError> {
    for probe in (at..at + 0x20).step_by(2) {
        let opcode = w(rom, probe)?;
        if opcode & 0xF1FF == 0x5048 {
            let count = (opcode >> 9) & 7;
            return Ok(if count == 0 { 8 } else { usize::from(count) });
        }
    }
    Err(fail(format!(
        "0x{at:06X}: the counter scan never advances a0"
    )))
}

/// The buy and sell rules, from the two instructions that apply them.
fn prices(rom: &[u8]) -> Result<(Json, u32), PackError> {
    let buy = site(rom, BUY_PRICE, "BuyPrice")?;
    let sell = site(rom, SELL_PRICE, "SellPrice")?;
    for (at, what) in [(buy, "buy"), (sell, "sell")] {
        let offset = w(rom, at + 2)?;
        if usize::from(offset) != PRICE_OFFSET {
            return Err(fail(format!(
                "0x{at:06X}: the {what} price is read from item offset 0x{offset:02X}, not \
                 0x{PRICE_OFFSET:02X}"
            )));
        }
    }
    let price_ram = w(rom, buy + 4)?;
    let shift = w(rom, sell + 4)?;
    if shift & 0xFFC0 != 0xE240 || shift & 0x0007 != 0 {
        return Err(fail(format!(
            "0x{:06X}: the sell price is not adjusted by an `lsr.w` (found 0x{shift:04X})",
            sell + 4
        )));
    }
    let count = (shift >> 9) & 7;
    let places = if count == 0 { 8 } else { count };
    let divisor = 1u32 << places;
    let json = Json::object()
        .with("price_offset", format!("0x{PRICE_OFFSET:02X}"))
        .with("price_bytes", PRICE_BYTES)
        .with("encoding", "binary")
        .with("price_ram", ram(price_ram))
        .with(
            "buy",
            Json::object()
                .with("rom_offset", hex6(buy))
                .with("rule", "the item record's price word, spent unchanged")
                .with(
                    "affordability",
                    "sub.l against Current_Money; a negative result refuses the sale",
                ),
        )
        .with(
            "sell",
            Json::object()
                .with("rom_offset", hex6(sell))
                .with(
                    "rule",
                    format!("the same word divided by {divisor}, rounding down"),
                )
                .with("divisor", divisor)
                .with("arithmetic", format!("lsr.w #{places}")),
        )
        .with(
            "stock",
            Json::object().with("unlimited", true).with(
                "note",
                "the buy list is re-read from ShopInventories, which is ROM, on every visit, \
                 and the purchase path writes only the inventory slot and Current_Money. \
                 Nothing anywhere decrements a shop.",
            ),
        );
    Ok((json, divisor))
}

/// `sell_price`: the buy word divided down.
pub fn sell_price(buy: u16, divisor: u32) -> u32 {
    u32::from(buy) / divisor
}

/// The inn rate table, the bill, and what a night restores.
fn inn_rules(rom: &[u8]) -> Result<(Json, usize), PackError> {
    let at = site(rom, INN_RATES, "InnRates")?;
    let table = l(rom, at + 2)? as usize;
    expect(rom, at + 6, 0x1038, "move.b ($FFFFECD1).w, d0")?;
    let index_ram = w(rom, at + 8)?;
    expect(rom, at + 10, 0xD040, "add.w d0, d0")?;
    expect(rom, at + 12, 0x43F1, "lea (a1,d0.w), a1")?;
    expect(rom, at + 18, 0x1629, "move.b d16(a1), d3")?;
    let rate_offset = w(rom, at + 20)?;

    let multiply = site(rom, INN_PARTY_MULTIPLY, "InnPartyMultiply")?;
    let party_ram = l(rom, multiply + 2)?;
    expect(rom, multiply + 6, 0x31C3, "move.w d3, (total).w")?;
    let total_ram = w(rom, multiply + 8)?;
    expect(rom, multiply + 10, 0x11D1, "move.b (a1), (variant).w")?;

    let event = site(rom, REST_EVENT, "RestEvent")?;
    let json = Json::object()
        .with("table", hex6(table))
        .with("entry_bytes", 2u8)
        .with("rate_offset", format!("0x{rate_offset:02X}"))
        .with("variant_offset", "0x00")
        .with("index_ram", ram(index_ram))
        .with("party_count_ram", format!("0x{party_ram:08X}"))
        .with("total_ram", ram(total_ram))
        .with("rom_offset", hex6(at))
        .with(
            "cost",
            Json::object()
                .with("formula", "rate * party_slots")
                .with(
                    "party_slots",
                    format!(
                        "CalcWinIDAndCharNum counts entries in Current_Party_Slots up to the \
                         {MAX_PARTY}-slot terminator; a dead member still counts"
                    ),
                )
                .with("max_party", MAX_PARTY),
        )
        .with("restores", recovery(rom)?)
        .with(
            "scripted_rest",
            Json::object()
                .with("rom_offset", hex6(event))
                .with("selector", format!("0x{:04X}", w(rom, event + 2)?))
                .with(
                    "note",
                    "one selector runs an event instead of an ordinary night, and only while \
                     both of its gate flags are clear",
                ),
        );
    Ok((json, table))
}

fn recovery_field(offset: u16) -> String {
    RECOVERY_FIELDS
        .iter()
        .find(|(at, _)| *at == offset)
        .map_or_else(|| format!("0x{offset:02X}"), |(_, name)| (*name).to_owned())
}

/// A copy's field name: the eight skill-use slots are curr/max byte pairs
/// from `$6A`, checked before the flat table.
fn copied_field(offset: u16) -> String {
    if (0x6A..0x6A + SKILL_SLOTS * 2).contains(&offset) {
        let (slot, half) = ((offset - 0x6A) / 2, (offset - 0x6A) % 2);
        let field = if half == 0 {
            "curr_skill_uses"
        } else {
            "max_skill_uses"
        };
        return format!("{field}[{slot}]");
    }
    recovery_field(offset)
}

/// `DoCharRecovery`, walked to its `rts`: the writes a night makes.
fn recovery(rom: &[u8]) -> Result<Json, PackError> {
    let start = sites(rom, RECOVER_HP, "RecoverHp")?[3];
    let mut writes = Vec::new();
    let mut at = start;
    loop {
        let opcode = w(rom, at)?;
        if opcode == 0x4E75 {
            break;
        }
        let (first, second) = (w(rom, at + 2)?, w(rom, at + 4)?);
        let write = match opcode {
            0x3168 | 0x1168 => Json::object()
                .with("field", copied_field(second))
                .with("offset", format!("0x{second:02X}"))
                .with("bytes", if opcode == 0x3168 { 2u8 } else { 1 })
                .with("from", copied_field(first))
                .with("value", Json::Null),
            0x117C => Json::object()
                .with("field", recovery_field(second))
                .with("offset", format!("0x{second:02X}"))
                .with("bytes", 1u8)
                .with("from", Json::Null)
                .with("value", first),
            _ => {
                return Err(fail(format!(
                    "0x{at:06X}: unexpected opcode 0x{opcode:04X} in DoCharRecovery"
                )));
            }
        };
        writes.push(write);
        at += 6;
        if at - start > 0x80 {
            return Err(fail(format!("0x{start:06X}: DoCharRecovery does not end")));
        }
    }
    Ok(Json::object()
        .with("routine", hex6(start))
        .with("per_character", writes)
        .with(
            "applies_to",
            "every occupied party slot, then the three vehicles",
        ))
}

/// The portrait groups and the greeting selector, their sizes proven: the
/// second group ends where the inn rate table begins.
/// [`presentation`]'s rules, and the tables it found.
struct Presentation {
    json: Json,
    pointers: usize,
    art: [Vec<u32>; 2],
    greetings: usize,
}

fn presentation(rom: &[u8], rates: usize) -> Result<Presentation, PackError> {
    let at = site(rom, PORTRAIT_GROUPS, "PortraitGroups")?;
    let pointers = l(rom, at + 2)? as usize;
    expect(rom, at + 8, 0x1038, "move.b ($FFFFECD0).w, d0")?;
    expect(rom, at + 12, 0x0C00, "cmpi.b #group, d0")?;
    let alternate = w(rom, at + 14)?;
    let groups = [l(rom, pointers)? as usize, l(rom, pointers + 4)? as usize];
    if groups[0] != pointers + 8 {
        return Err(fail(format!(
            "the first portrait group is at 0x{:06X}, not immediately after the two pointers \
             at 0x{:06X}",
            groups[0],
            pointers + 8
        )));
    }
    let bounds = [groups[1], rates];
    if (0..2).any(|i| bounds[i] < groups[i] + 4) {
        return Err(fail("the portrait groups do not run in table order".into()));
    }
    let mut art: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
    let mut listed = Vec::new();
    for i in 0..2 {
        let count = (bounds[i] - groups[i]) / 4;
        art[i] = (0..count)
            .map(|n| l(rom, groups[i] + n * 4))
            .collect::<Result<_, _>>()?;
        listed.push(
            Json::object()
                .with("group", i)
                .with("rom_offset", hex6(groups[i]))
                .with("count", count)
                .with(
                    "art",
                    art[i]
                        .iter()
                        .map(|a| format!("0x{a:06X}"))
                        .collect::<Vec<_>>(),
                ),
        );
    }
    let selector = site(rom, GREETING_SELECTOR, "GreetingSelector")?;
    let greetings = l(rom, selector + 2)? as usize;
    let json = Json::object()
        .with(
            "portraits",
            Json::object()
                .with("pointers", hex6(pointers))
                .with("rom_offset", hex6(at))
                .with("groups", listed)
                .with("alternate_group_uses", i64::from(alternate) - 1)
                .with(
                    "note",
                    format!(
                        "the selector's high byte picks the group, except that group \
                         {alternate} reads group {}'s portraits; the low byte indexes within it",
                        i64::from(alternate) - 1
                    ),
                ),
        )
        .with(
            "greetings",
            Json::object()
                .with("table", hex6(greetings))
                .with("rom_offset", hex6(selector))
                .with("entry_bytes", 2u8)
                .with(
                    "note",
                    "one word per shop: the high byte picks which of two greeting texts is used \
                     and the low byte picks the noun fragment naming the shop's trade. Inns \
                     ignore it and use their rate entry's high byte instead. The fragments are \
                     window text, not dialogue tree entries -- talking to a shopkeeper never \
                     runs the tree.",
                ),
        );
    Ok(Presentation {
        json,
        pointers,
        art,
        greetings,
    })
}

/// `read_flow`: every rule `shops.json` depends on, with the counter scan
/// held to the location table it walks.
pub fn read_flow(rom: &Rom) -> Result<Flow, PackError> {
    let data = rom.bytes();
    let (interaction, table, stride) = counter_scan(data)?;
    if table != tables::LOCATIONS_START {
        return Err(fail(format!(
            "the counter scan walks 0x{table:06X}, but the location table is decoded at \
             0x{:06X}",
            tables::LOCATIONS_START
        )));
    }
    if stride != tables::LOCATION_ENTRY_BYTES {
        return Err(fail(format!(
            "the counter scan steps {stride} bytes; the location table is decoded with {}-byte \
             entries",
            tables::LOCATION_ENTRY_BYTES
        )));
    }
    let (prices, sell_divisor) = prices(data)?;
    let (inn, inn_table) = inn_rules(data)?;
    let Presentation {
        json: presentation,
        pointers: portrait_pointers,
        art: portrait_groups,
        greetings: greetings_table,
    } = presentation(data, inn_table)?;
    let groups = SHOP_GROUPS.iter().fold(Json::object(), |out, (id, name)| {
        out.with(id.to_string(), *name)
    });
    let json = Json::object()
        .with("interaction", interaction)
        .with("prices", prices)
        .with("inn", inn)
        .with("presentation", presentation)
        .with("groups", groups);
    Ok(Flow {
        json,
        sell_divisor,
        inn_table,
        greetings_table,
        portrait_pointers,
        portrait_groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_slot_copy_names_its_slot_and_half() {
        assert_eq!(copied_field(0x0E), "curr_hp");
        assert_eq!(copied_field(0x6A), "curr_skill_uses[0]");
        assert_eq!(copied_field(0x6B), "max_skill_uses[0]");
        assert_eq!(copied_field(0x79), "max_skill_uses[7]");
        assert_eq!(copied_field(0x7A), "0x7A");
        assert_eq!(recovery_field(0x6B), "max_skill_uses");
    }

    #[test]
    fn the_scan_stride_is_the_first_addq_to_a0() {
        let mut rom = vec![0u8; 0x40];
        rom[0x0A..0x0C].copy_from_slice(&0x5048u16.to_be_bytes());
        assert_eq!(scan_stride(&rom, 0).unwrap(), 8);
        rom[0x04..0x06].copy_from_slice(&0x5848u16.to_be_bytes());
        assert_eq!(scan_stride(&rom, 0).unwrap(), 4);
        assert!(scan_stride(&[0u8; 0x40], 0).is_err());
    }

    #[test]
    fn a_sale_pays_the_word_divided_down() {
        assert_eq!(sell_price(21, 2), 10);
        assert_eq!(sell_price(0, 2), 0);
    }
}
