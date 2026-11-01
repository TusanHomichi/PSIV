//! `FieldObjectsJmpTbl` and the one-time initialisation each routine runs:
//! `psiv_tools/sprites/objects.py`. Which table an object follows, which art it
//! draws, its CRAM line and whether it draws at all are immediates in that
//! block, so the block is read out of the cartridge.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::records::{FACINGS, hex6, s8_at, s16_at, tile_props_line, u8_at, u16_at, u32_at};
use crate::PackError;
use crate::map_symbols::FIELD_OBJECT_SYMBOLS;

pub const FIELD_OBJECTS_JMP_TBL: usize = 0x04AA6C;
pub const FIELD_OBJECT_COUNT: usize = 222;
const FIELD_OBJECT_STRIDE: usize = 4;
const FIELD_OBJ_ANIMATE: usize = 0x044728;
const FIELD_OBJ_ANIMATE_SIGNATURE: [u8; 14] = [
    0x08, 0xEC, 0x00, 0x05, 0x00, 0x02, 0x30, 0x2C, 0x00, 0x06, 0x22, 0x6C, 0x00, 0x08,
];
const FIELD_OBJ_ANIMATE2: usize = FIELD_OBJ_ANIMATE + 6;

const MOVE_L_TO_A4: u16 = 0x297C;
const MOVE_W_TO_A4: u16 = 0x397C;
const MOVE_B_TO_A4: u16 = 0x197C;
const BSET_TO_A4: u16 = 0x08EC;
const BCLR_TO_A4: u16 = 0x08AC;
const BCHG_TO_A4: u16 = 0x086C;
const BSET_A4_IND: u16 = 0x08D4;
const BRA_W: u16 = 0x6000;
const BSR_W: u16 = 0x6100;
const JMP_ABS_L: u16 = 0x4EF9;
const JSR_ABS_L: u16 = 0x4EB9;

const OFF_RENDER_FLAGS: u16 = 0x02;
const OFF_MAPPINGS_ADDR: u16 = 0x08;
const OFF_MAPPINGS: u16 = 0x0C;
const OFF_MAPPINGS_DURATION: u16 = 0x11;
const OFF_SPRITE_TILE_PROPS: u16 = 0x13;
const OFF_ART_TILE: u16 = 0x16;
const OFF_ART_PTR: u16 = 0x18;

/// `render_flags` bit 1: `Field_FillSpriteAttributes` returns on it.
const RENDER_FLAG_NO_SPRITES: u32 = 1;
/// `render_flags` bit 0: `FieldObj_CalcSpritePos` skips the camera subtraction.
pub const RENDER_FLAG_CAMERA_BYPASS: u32 = 0;
/// `render_flags` bit 3: tested by the talk probe and the walker's collision.
pub const RENDER_FLAG_INTERACTABLE: u32 = 3;
pub const INTERACTION_CHK_OBJECTS: usize = 0x058D50;
pub const FIELD_OBJ_DO_OBJ_COLLISION: usize = 0x047DA8;
const INTERACTION_LOOP_SIGNATURE: [u8; 6] = [0x7E, 0x33, 0x4A, 0x53, 0x67, 0x00];
const INTERACTION_BTST_SIGNATURE: [u8; 8] = [0x08, 0x2B, 0x00, 0x03, 0x00, 0x02, 0x67, 0x00];

/// What one routine sets up before it starts running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldObjectRoutine {
    pub index: usize,
    pub symbol: &'static str,
    pub rom_offset: usize,
    pub mappings_addr: Option<u32>,
    pub mappings: Option<u32>,
    pub art_ptr: Option<u32>,
    pub art_tile: Option<u16>,
    pub sprite_tile_props: Option<u8>,
    pub mappings_duration: Option<u8>,
    pub builds_sprites: bool,
    pub animates: bool,
    pub streams_art: bool,
    pub interactable: bool,
    pub interactable_source: String,
    pub interactable_changes_at_runtime: bool,
    pub camera_bypass: bool,
    pub camera_bypass_source: String,
    pub camera_bypass_changes_at_runtime: bool,
}

impl FieldObjectRoutine {
    pub fn object_id(&self) -> usize {
        self.index * FIELD_OBJECT_STRIDE
    }

    pub fn palette_line(&self) -> Option<u8> {
        self.sprite_tile_props.and_then(tile_props_line)
    }
}

fn check_jump_table(rom: &[u8]) -> Result<(), PackError> {
    for index in 0..FIELD_OBJECT_COUNT {
        let entry = FIELD_OBJECTS_JMP_TBL + index * 4;
        if u16_at(rom, entry)? != BRA_W {
            return Err(PackError(format!(
                "FieldObjectsJmpTbl entry {index} at {} is not a bra.w; the table is not where \
                 this build thinks it is",
                hex6(entry)
            )));
        }
    }
    if u16_at(rom, FIELD_OBJECTS_JMP_TBL + FIELD_OBJECT_COUNT * 4)? != BRA_W {
        return Err(PackError(format!(
            "FieldObjectsJmpTbl is longer than {FIELD_OBJECT_COUNT} entries"
        )));
    }
    let signature = &FIELD_OBJ_ANIMATE_SIGNATURE;
    if rom.get(FIELD_OBJ_ANIMATE..FIELD_OBJ_ANIMATE + signature.len()) != Some(signature) {
        return Err(PackError(format!(
            "FieldObj_Animate at {} does not match its signature",
            hex6(FIELD_OBJ_ANIMATE)
        )));
    }
    Ok(())
}

fn check_interaction_readers(rom: &[u8]) -> Result<(), PackError> {
    for (label, offset) in [
        ("Interaction_ChkObjects", INTERACTION_CHK_OBJECTS),
        ("FieldObj_DoObjCollision", FIELD_OBJ_DO_OBJ_COLLISION),
    ] {
        let head = offset + 4;
        if rom.get(head..head + INTERACTION_LOOP_SIGNATURE.len())
            != Some(&INTERACTION_LOOP_SIGNATURE)
        {
            return Err(PackError(format!(
                "{label} at {} does not open its slot loop",
                hex6(offset)
            )));
        }
        let btst = head + 8;
        if rom.get(btst..btst + INTERACTION_BTST_SIGNATURE.len())
            != Some(&INTERACTION_BTST_SIGNATURE)
        {
            return Err(PackError(format!(
                "{label} at {} does not test render_flags bit {RENDER_FLAG_INTERACTABLE}",
                hex6(offset)
            )));
        }
    }
    Ok(())
}

/// Stores into the object structure, by form and field offset. `"l"`, `"w"` and
/// `"b"` are moves; `"bset"`, `"bclr"` and `"bchg"` are bit instructions.
type Stores = BTreeMap<(&'static str, u16), Vec<u32>>;

fn writes(stores: &Stores, kind: &'static str, bit: u32) -> bool {
    stores
        .get(&(kind, OFF_RENDER_FLAGS))
        .is_some_and(|values| values.contains(&bit))
}

/// `_interactable` and `_camera_bypass`: a render-flags bit, and how we know. A
/// routine that writes neither leaves it clear, because `GameMode_LoadFieldMap`
/// zero-fills the object area before parsing the record.
fn flag_bit(
    symbol: &str,
    init: &Stores,
    body: &Stores,
    bit: u32,
    words: &str,
) -> Result<(bool, String, bool), PackError> {
    let (init_set, init_clear) = (writes(init, "bset", bit), writes(init, "bclr", bit));
    let later = ["bset", "bclr", "bchg"]
        .iter()
        .any(|kind| writes(body, kind, bit));
    if init_set && init_clear {
        return Err(PackError(format!(
            "FieldObj_{symbol} both sets and clears {words} {bit} in one init block"
        )));
    }
    let source = if init_set {
        format!("bset #{bit}, $2(a4) in the init block")
    } else if init_clear {
        format!("bclr #{bit}, $2(a4) in the init block")
    } else {
        "no write; GameMode_LoadFieldMap zero-fills the object area before LoadMapObjects, so the \
         bit is clear"
            .to_owned()
    };
    Ok((init_set, source, later))
}

/// `_init_bounds`: `bset #7,(a4) / bne.s <main>`; the block between runs once.
fn init_bounds(rom: &[u8], routine: usize) -> Result<Option<(usize, usize)>, PackError> {
    if u16_at(rom, routine)? != BSET_A4_IND
        || u16_at(rom, routine + 2)? != 0x0007
        || u8_at(rom, routine + 4)? != 0x66
    {
        return Ok(None);
    }
    let displacement = s8_at(rom, routine + 5)?;
    if displacement <= 0 {
        return Ok(None);
    }
    Ok(Some((routine + 6, routine + 6 + displacement as usize)))
}

/// `_scan_init`: stores over one span, following a register whose value came
/// from a `moveq`. Any other write to a register makes it unknown again.
fn scan_init(rom: &[u8], start: usize, end: usize) -> Result<Stores, PackError> {
    let mut found = Stores::new();
    let mut known: HashMap<u16, u32> = HashMap::new();
    let mut offset = start;
    while offset + 2 < end {
        let word = u16_at(rom, offset)?;
        if word == MOVE_L_TO_A4 && offset + 8 <= end {
            let key = ("l", u16_at(rom, offset + 6)?);
            found.entry(key).or_default().push(u32_at(rom, offset + 2)?);
            offset += 8;
            continue;
        }
        if word == MOVE_W_TO_A4 && offset + 6 <= end {
            let key = ("w", u16_at(rom, offset + 4)?);
            found
                .entry(key)
                .or_default()
                .push(u32::from(u16_at(rom, offset + 2)?));
            offset += 6;
            continue;
        }
        if word == MOVE_B_TO_A4 && offset + 6 <= end {
            let key = ("b", u16_at(rom, offset + 4)?);
            found
                .entry(key)
                .or_default()
                .push(u32::from(u8_at(rom, offset + 3)?));
            offset += 6;
            continue;
        }
        let bit_kind = match word {
            BSET_TO_A4 => Some("bset"),
            BCLR_TO_A4 => Some("bclr"),
            BCHG_TO_A4 => Some("bchg"),
            _ => None,
        };
        if let Some(kind) = bit_kind
            && offset + 6 <= end
        {
            let key = (kind, u16_at(rom, offset + 4)?);
            found
                .entry(key)
                .or_default()
                .push(u32::from(u16_at(rom, offset + 2)?));
            offset += 6;
            continue;
        }
        if word & 0xF100 == 0x7000 {
            known.insert((word >> 9) & 7, u32::from(word & 0xFF));
            offset += 2;
            continue;
        }
        let register_store = match word & 0xFFF8 {
            0x1940 => Some("b"),
            0x3940 => Some("w"),
            0x2940 => Some("l"),
            _ => None,
        };
        if let Some(size) = register_store
            && offset + 4 <= end
        {
            if let Some(&value) = known.get(&(word & 7)) {
                let key = (size, u16_at(rom, offset + 2)?);
                found.entry(key).or_default().push(value);
            }
            offset += 4;
            continue;
        }
        // `move.<size> <ea>, Dn`: destination mode 0, register in bits 11-9.
        if word & 0xC000 == 0 && (word >> 12) & 3 != 0 && word & 0x01C0 == 0 {
            known.remove(&((word >> 9) & 7));
        }
        offset += 2;
    }
    Ok(found)
}

/// `_reaches_animate`: does this body branch to `FieldObj_Animate` / `_Animate2`?
fn reaches_animate(rom: &[u8], start: usize, end: usize) -> Result<(bool, bool), PackError> {
    let (mut animate, mut animate2) = (false, false);
    let mut offset = start;
    while offset + 4 <= end {
        let word = u16_at(rom, offset)?;
        let target = if matches!(word, BRA_W | BSR_W) {
            Some((offset as i64 + 2 + i64::from(s16_at(rom, offset + 2)?)) as usize)
        } else if matches!(word, JMP_ABS_L | JSR_ABS_L) && offset + 6 <= end {
            Some(u32_at(rom, offset + 2)? as usize)
        } else {
            None
        };
        animate |= target == Some(FIELD_OBJ_ANIMATE);
        animate2 |= target == Some(FIELD_OBJ_ANIMATE2);
        offset += 2;
    }
    Ok((animate, animate2))
}

fn last(stores: &Stores, kind: &'static str, field: u16) -> Option<u32> {
    stores
        .get(&(kind, field))
        .and_then(|values| values.last())
        .copied()
}

/// `scan_field_objects`: every routine's one-time initialisation. A routine's
/// body runs to the next routine start in address order, which bounds the
/// branch scan.
pub fn scan_field_objects(rom: &[u8]) -> Result<Vec<FieldObjectRoutine>, PackError> {
    check_jump_table(rom)?;
    check_interaction_readers(rom)?;
    let mut targets = Vec::with_capacity(FIELD_OBJECT_COUNT);
    for index in 0..FIELD_OBJECT_COUNT {
        let entry = FIELD_OBJECTS_JMP_TBL + index * 4;
        targets.push((entry as i64 + 2 + i64::from(s16_at(rom, entry + 2)?)) as usize);
    }
    let starts: BTreeSet<usize> = targets.iter().copied().collect();
    let mut routines = Vec::with_capacity(FIELD_OBJECT_COUNT);
    for (index, &routine) in targets.iter().enumerate() {
        let symbol = FIELD_OBJECT_SYMBOLS[index];
        let body_end = starts
            .range(routine + 1..)
            .next()
            .copied()
            .unwrap_or(routine + 0x200);
        let (animate, animate2) = reaches_animate(rom, routine, body_end)?;
        let Some((start, end)) = init_bounds(rom, routine)? else {
            routines.push(FieldObjectRoutine {
                index,
                symbol,
                rom_offset: routine,
                mappings_addr: None,
                mappings: None,
                art_ptr: None,
                art_tile: None,
                sprite_tile_props: None,
                mappings_duration: None,
                builds_sprites: false,
                animates: animate || animate2,
                streams_art: animate,
                interactable: false,
                interactable_source: "routine has no init block".into(),
                interactable_changes_at_runtime: false,
                camera_bypass: false,
                camera_bypass_source: "routine has no init block".into(),
                camera_bypass_changes_at_runtime: false,
            });
            continue;
        };
        let stores = scan_init(rom, start, end)?;
        let body = scan_init(rom, end, body_end)?;
        // Three routines store their tile properties in the per-frame part,
        // because the value depends on a flag. The first store is unconditional.
        let key = ("b", OFF_SPRITE_TILE_PROPS);
        let props = stores
            .get(&key)
            .filter(|values| !values.is_empty())
            .or_else(|| body.get(&key))
            .map_or(&[][..], Vec::as_slice);
        for &value in props {
            if tile_props_line(value as u8).is_none() || value > 0xFF {
                return Err(PackError(format!(
                    "{symbol} stores sprite tile properties 0x{value:02X}, which is not one of \
                     the four CRAM-line values retail uses"
                )));
            }
        }
        let (interactable, interactable_source, interactable_changes_at_runtime) = flag_bit(
            symbol,
            &stores,
            &body,
            RENDER_FLAG_INTERACTABLE,
            "render_flags bit",
        )?;
        let (camera_bypass, camera_bypass_source, camera_bypass_changes_at_runtime) = flag_bit(
            symbol,
            &stores,
            &body,
            RENDER_FLAG_CAMERA_BYPASS,
            "render-flags bit",
        )?;
        let hidden = writes(&stores, "bset", RENDER_FLAG_NO_SPRITES);
        let unhides = writes(&body, "bclr", RENDER_FLAG_NO_SPRITES)
            || writes(&body, "bchg", RENDER_FLAG_NO_SPRITES);
        routines.push(FieldObjectRoutine {
            index,
            symbol,
            rom_offset: routine,
            mappings_addr: last(&stores, "l", OFF_MAPPINGS_ADDR),
            mappings: last(&stores, "l", OFF_MAPPINGS),
            art_ptr: last(&stores, "l", OFF_ART_PTR),
            art_tile: last(&stores, "w", OFF_ART_TILE).map(|tile| tile as u16),
            sprite_tile_props: props.first().map(|&value| value as u8),
            mappings_duration: last(&stores, "b", OFF_MAPPINGS_DURATION).map(|value| value as u8),
            builds_sprites: !hidden || unhides,
            animates: animate || animate2,
            streams_art: animate,
            interactable,
            interactable_source,
            interactable_changes_at_runtime,
            camera_bypass,
            camera_bypass_source,
            camera_bypass_changes_at_runtime,
        });
    }
    Ok(routines)
}

/// `facing_table_extents`: the tables are packed with no count, so each runs to
/// the next any routine references, capped at the four facings.
pub fn facing_table_extents(routines: &[FieldObjectRoutine]) -> HashMap<u32, usize> {
    let addresses: BTreeSet<u32> = routines.iter().filter_map(|r| r.mappings_addr).collect();
    let listed: Vec<u32> = addresses.into_iter().collect();
    listed
        .iter()
        .enumerate()
        .map(|(position, &address)| {
            let following = listed.get(position + 1).copied().unwrap_or(address + 16);
            let extent = ((following - address) / 4) as usize;
            (address, extent.clamp(1, FACINGS.len()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_are_read_by_form_and_a_moveq_is_followed_until_overwritten() {
        let code = [
            0x29, 0x7C, 0, 1, 2, 3, 0, 0x08, // move.l #$10203,8(a4)
            0x19, 0x7C, 0, 0x40, 0, 0x13, // move.b #$40,$13(a4)
            0x08, 0xEC, 0, 3, 0, 2, // bset #3,2(a4)
            0x70, 0x05, // moveq #5,d0
            0x39, 0x40, 0, 0x16, // move.w d0,$16(a4)
            0x30, 0x38, 0x12, 0x34, // move.w $1234.w,d0: forgotten
            0x39, 0x40, 0, 0x18, // move.w d0,$18(a4): unknown
            0x4E, 0x71, // padding: the scan stops two short of the end
        ];
        let stores = scan_init(&code, 0, code.len()).unwrap();
        assert_eq!(last(&stores, "l", OFF_MAPPINGS_ADDR), Some(0x10203));
        assert_eq!(last(&stores, "b", OFF_SPRITE_TILE_PROPS), Some(0x40));
        assert!(writes(&stores, "bset", RENDER_FLAG_INTERACTABLE));
        assert_eq!(last(&stores, "w", OFF_ART_TILE), Some(5));
        assert_eq!(last(&stores, "w", OFF_ART_PTR), None);
    }

    #[test]
    fn a_routine_without_the_opening_has_no_init_block() {
        assert_eq!(
            init_bounds(&[0x08, 0xD4, 0, 7, 0x66, 4], 0).unwrap(),
            Some((6, 10))
        );
        assert_eq!(init_bounds(&[0x08, 0xD4, 0, 6, 0x66, 4], 0).unwrap(), None);
        assert_eq!(init_bounds(&[0x08, 0xD4, 0, 7, 0x67, 4], 0).unwrap(), None);
        assert_eq!(
            init_bounds(&[0x08, 0xD4, 0, 7, 0x66, 0xFE], 0).unwrap(),
            None
        );
    }

    #[test]
    fn a_flag_bit_says_how_it_is_known_and_refuses_both_writes() {
        let store = |kind: &'static str| {
            Stores::from([((kind, OFF_RENDER_FLAGS), vec![RENDER_FLAG_INTERACTABLE])])
        };
        let none = Stores::new();
        let check = |init: &Stores, body: &Stores| {
            flag_bit(
                "X",
                init,
                body,
                RENDER_FLAG_INTERACTABLE,
                "render_flags bit",
            )
        };
        assert_eq!(
            check(&store("bset"), &none).unwrap(),
            (true, "bset #3, $2(a4) in the init block".to_owned(), false)
        );
        let (set, source, later) = check(&none, &store("bchg")).unwrap();
        assert!(!set && later);
        assert!(source.starts_with("no write;"));
        let both = Stores::from([
            (("bset", OFF_RENDER_FLAGS), vec![3]),
            (("bclr", OFF_RENDER_FLAGS), vec![3]),
        ]);
        assert!(check(&both, &none).is_err());
    }

    #[test]
    fn a_table_runs_to_the_next_and_is_capped_at_four_facings() {
        let routine = |mappings_addr| FieldObjectRoutine {
            index: 0,
            symbol: "X",
            rom_offset: 0,
            mappings_addr: Some(mappings_addr),
            mappings: None,
            art_ptr: None,
            art_tile: None,
            sprite_tile_props: None,
            mappings_duration: None,
            builds_sprites: true,
            animates: true,
            streams_art: false,
            interactable: false,
            interactable_source: String::new(),
            interactable_changes_at_runtime: false,
            camera_bypass: false,
            camera_bypass_source: String::new(),
            camera_bypass_changes_at_runtime: false,
        };
        let extents = facing_table_extents(&[routine(100), routine(104), routine(200)]);
        assert_eq!(extents[&100], 1);
        assert_eq!(extents[&104], 4);
        assert_eq!(extents[&200], 4);
    }
}
