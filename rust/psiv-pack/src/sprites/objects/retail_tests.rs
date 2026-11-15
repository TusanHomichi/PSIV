use super::*;
use crate::sprites::tests::retail;

#[test]
fn object_table_init_visibility_streaming_and_palette_extents() {
    let Some(r) = retail() else { return };
    let rom = r.rom.bytes();
    assert_eq!(r.routines.len(), 222);
    assert_eq!(
        (r.routines[0].symbol, r.routines[0].rom_offset),
        ("None", 0x4669e)
    );
    assert_eq!(u16_at(rom, 0x4669e).unwrap(), 0x4e75); // rts
    assert_eq!(
        (
            r.routines[1].symbol,
            r.routines[1].rom_offset,
            r.routines[1].object_id()
        ),
        ("Chaz", 0x466a0, 4)
    );
    let mut no_init = Vec::new();
    let mut hidden = Vec::new();
    let mut alternates = BTreeMap::new();
    let starts: BTreeSet<_> = r.routines.iter().map(|p| p.rom_offset).collect();
    for routine in &r.routines {
        let Some((start, end)) = init_bounds(rom, routine.rom_offset).unwrap() else {
            no_init.push(routine.symbol);
            continue;
        };
        let init = scan_init(rom, start, end).unwrap();
        if writes(&init, "bset", 1) {
            hidden.push(routine.symbol);
        }
        let body_end = starts
            .range(routine.rom_offset + 1..)
            .next()
            .copied()
            .unwrap_or(routine.rom_offset + 0x200);
        let body = scan_init(rom, end, body_end).unwrap();
        if let Some(values) = init
            .get(&("b", OFF_SPRITE_TILE_PROPS))
            .filter(|v| !v.is_empty())
            .or_else(|| body.get(&("b", OFF_SPRITE_TILE_PROPS)))
        {
            let extra: BTreeSet<_> = values
                .iter()
                .copied()
                .filter(|&v| Some(v as u8) != routine.sprite_tile_props)
                .collect();
            if !extra.is_empty() {
                alternates.insert(routine.symbol, extra);
            }
        }
    }
    assert_eq!(no_init, ["None"]);
    hidden.sort();
    assert_eq!(
        hidden,
        [
            "BarrierBeam2",
            "BarrierBeam4",
            "GyLaguiah",
            "InvisibleBlock",
            "KingRappyFlyingAway",
            "LandaleBeam"
        ]
    );
    assert_eq!(
        alternates,
        BTreeMap::from([
            ("TreasureChest", BTreeSet::from([0x20])),
            ("Elevator", BTreeSet::from([0x20]))
        ])
    );
    assert_eq!(
        r.routines
            .iter()
            .filter(|p| !p.builds_sprites)
            .map(|p| p.symbol)
            .collect::<Vec<_>>(),
        ["None", "InvisibleBlock", "KingRappyFlyingAway"]
    );
    assert_eq!(
        r.routines
            .iter()
            .filter(|p| p.streams_art != p.art_ptr.is_some())
            .map(|p| p.symbol)
            .collect::<Vec<_>>(),
        ["loc_483DC"]
    );
    assert_eq!(
        r.routines
            .iter()
            .filter_map(|p| p.sprite_tile_props)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 0x20, 0x40, 0x60])
    );
    for symbol in ["BarrierBeam2", "BarrierBeam4", "GyLaguiah", "LandaleBeam"] {
        assert!(
            r.routines
                .iter()
                .find(|p| p.symbol == symbol)
                .unwrap()
                .builds_sprites
        );
    }
    let extents = facing_table_extents(&r.routines);
    for (symbol, size) in [("Chaz", 4), ("Elevator", 2)] {
        assert_eq!(
            extents[&r
                .routines
                .iter()
                .find(|p| p.symbol == symbol)
                .unwrap()
                .mappings_addr
                .unwrap()],
            size
        );
    }
    assert!(extents.values().all(|v| (1..=4).contains(v)));
}

#[test]
fn interaction_readers_zeroed_slots_and_all_routine_answers() {
    let Some(r) = retail() else { return };
    let rom = r.rom.bytes();
    // Check the two readers and reject mutations in their loop and bit test.
    for site in [INTERACTION_CHK_OBJECTS, FIELD_OBJ_DO_OBJ_COLLISION] {
        assert_eq!(u32_at(rom, site).unwrap(), 0x47f8c300); // lea $C300.w,a3
        for delta in [4, 12] {
            let mut bad = rom.to_vec();
            bad[site + delta] ^= 1;
            assert!(scan_field_objects(&bad).is_err());
        }
    }
    assert_eq!(RENDER_FLAG_INTERACTABLE, 3);
    // GameMode_LoadFieldMap: move.w #$3ff,d7; lea $c000.w,a0; trap #0.
    assert_eq!(u16_at(rom, 0x517a4).unwrap(), 0x3e3c);
    let longs = usize::from(u16_at(rom, 0x517a6).unwrap()) + 1;
    assert_eq!(longs, 1024);
    assert_eq!(u32_at(rom, 0x517a8).unwrap(), 0x41f8c000);
    assert_eq!(u16_at(rom, 0x517ac).unwrap(), 0x4e40);
    assert!(0xc000 + longs * 4 >= 0xcfc0 + 0x40);
    assert_eq!(r.routines.iter().filter(|p| p.interactable).count(), 104);
    let mut sources = BTreeMap::<&str, usize>::new();
    for p in &r.routines {
        *sources.entry(&p.interactable_source).or_default() += 1;
    }
    assert_eq!(sources["bset #3, $2(a4) in the init block"], 104);
    assert_eq!(sources["bclr #3, $2(a4) in the init block"], 74);
    assert_eq!(sources["routine has no init block"], 1);
    let silent: Vec<_> = r
        .routines
        .iter()
        .filter(|p| p.interactable_source.starts_with("no write"))
        .collect();
    assert_eq!(silent.len(), 43);
    assert!(silent.iter().all(|p| !p.interactable));
    let changing: Vec<_> = r
        .routines
        .iter()
        .filter(|p| p.interactable_changes_at_runtime)
        .collect();
    assert_eq!(
        changing.iter().map(|p| p.symbol).collect::<Vec<_>>(),
        ["FellowPenguin"]
    );
    assert!(changing[0].interactable);
    for symbol in ["Xanafalgue", "Igglanova"] {
        let p = r.routines.iter().find(|p| p.symbol == symbol).unwrap();
        assert!(!p.interactable);
        assert!(p.interactable_source.contains("bclr"));
    }
    assert!(
        r.routines
            .iter()
            .find(|p| p.symbol == "InvisibleBlock")
            .unwrap()
            .interactable
    );
    // Table and Animate signatures are checked by the scanner itself.
    for at in [FIELD_OBJECTS_JMP_TBL, FIELD_OBJ_ANIMATE] {
        let mut bad = rom.to_vec();
        bad[at] ^= 1;
        assert!(scan_field_objects(&bad).is_err());
    }
}

#[test]
fn the_flags_byte_has_no_unrecognised_writer_forms() {
    let Some(r) = retail() else { return };
    let rom = r.rom.bytes();
    // Enumerate all byte writers to d16(a4), independently of scan_init.
    let mut forms = BTreeMap::from([
        (0x086c, true),
        (0x08ac, true),
        (0x08ec, true),
        (0x002c, true),
        (0x022c, true),
        (0x0a2c, true),
        (0x422c, false),
        (0x462c, false),
        (0x197c, true),
    ]);
    for n in 0..8 {
        for base in [0x016c, 0x01ac, 0x01ec, 0x812c, 0xc12c, 0xb12c] {
            forms.insert(base | (n << 9), false);
        }
        forms.insert(0x1940 | n, false);
    }
    let lo = r.routines.iter().map(|p| p.rom_offset).min().unwrap();
    let hi = r.routines.iter().map(|p| p.rom_offset).max().unwrap() + 0x200;
    let mut operations = BTreeSet::new();
    let mut bit3 = BTreeSet::new();
    for at in (lo..hi - 6).step_by(2) {
        let op = u16_at(rom, at).unwrap();
        if let Some(&immediate) = forms.get(&op) {
            let displacement = at + if immediate { 4 } else { 2 };
            if u16_at(rom, displacement).unwrap() == 2 {
                operations.insert(op);
                if immediate && u16_at(rom, at + 2).unwrap() == 3 {
                    bit3.insert(op);
                }
            }
        }
    }
    assert_eq!(operations, BTreeSet::from([0x086c, 0x08ac, 0x08ec]));
    assert_eq!(bit3, BTreeSet::from([0x08ac, 0x08ec]));
}
