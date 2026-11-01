use super::*;

const AT: usize = 0x10;
const TREE: usize = 0x20;
const ART: [u8; 4] = [0, 0, 0, 0x30];

/// A minimal record for a non-world map, with `edit` applied to its parts
/// in the loader's order.
fn record_parts() -> Vec<Vec<u8>> {
    let mut layout = ART.to_vec();
    layout.extend(ART);
    vec![
        vec![7, 0x81],             // general var, music
        vec![0xFF, 0xFF],          // tilesets
        vec![0xFF, 0xFF],          // Nemesis sprites
        vec![0xFF, 0xFF],          // Kosinski sprites
        vec![1, 2, 3, 4],          // dimensions
        vec![0, 0, 0, 1, 0, 1],    // scroll: no step counters
        vec![0xFF, 0xFF],          // chunks
        vec![0xFF],                // map updates
        vec![0xFF, 0xFF],          // transitions
        vec![0xFF, 0xFF],          // transitions 2
        vec![0xFF, 0xFF],          // objects
        vec![0xFF, 0xFF],          // chests
        vec![0xFF, 0xFF],          // tile animations
        layout,                    // FG and BG layout
        vec![0, 0, 0, TREE as u8], // dialogue tree
        vec![0xFF, 0xFF],          // interaction areas
        vec![0xFF],                // events
        vec![0, 0, 0, 0x30],       // palette
        vec![0, 1, 2, 0x85],       // flags
        vec![0xFF, 0xFF],          // MapDataManager
    ]
}

fn walk(parts: &[Vec<u8>]) -> Result<MapRecord, PackError> {
    let mut rom = vec![0u8; AT];
    rom.extend(parts.concat());
    rom.resize(rom.len() + 0x100, 0);
    walk_map(&rom, 2, AT, &[(TREE, 5)])
}

#[test]
fn a_minimal_record_walks_to_its_terminator() {
    let parts = record_parts();
    let record = walk(&parts).unwrap();
    assert_eq!(record.end, AT + parts.concat().len());
    assert_eq!(record.general_var, 7);
    assert_eq!(record.dialogue.tree, 5);
    assert_eq!(record.layout.planes, Some((0x30, 0x30)));
    assert_eq!(record.flags.dungeon_teleport, 0x85);
}

#[test]
fn the_scroll_section_reads_counters_only_after_a_zero_mode() {
    let mut parts = record_parts();
    parts[5] = vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 1];
    let record = walk(&parts).unwrap();
    assert_eq!(record.scroll.fg.step_counters, Some((1, 2)));
    assert_eq!(record.scroll.bg.step_counters, None);
}

#[test]
fn fffe_in_the_nemesis_list_sends_an_entry_to_ram_and_goes_on() {
    let mut parts = record_parts();
    parts[2] = vec![
        0xFF, 0xFE, 0, 9, 0, 0, 0, 0x30, 0, 8, 0, 0, 0, 0x30, 0xFF, 0xFD,
    ];
    let record = walk(&parts).unwrap();
    let sprites = &record.sprites;
    assert_eq!(sprites.terminator, 0xFFFD);
    assert_eq!(sprites.entries.len(), 2);
    assert!(sprites.entries[0].to_ram);
    assert_eq!(sprites.entries[0].tile_number, 9);
    assert!(!sprites.entries[1].to_ram);
    assert_eq!(sprites.entries[1].tile_number, 8);
}

#[test]
fn only_some_lists_end_on_any_negative_word() {
    // Tilesets end on $8000; Kosinski sprites read it as a tile number.
    let mut parts = record_parts();
    parts[1] = vec![0x80, 0x00];
    assert_eq!(walk(&parts).unwrap().tilesets.terminator, 0x8000);
    let mut parts = record_parts();
    parts[3] = vec![0x80, 0x00, 0, 0, 0, 0x30, 0xFF, 0xFF];
    assert_eq!(walk(&parts).unwrap().sprite_data.entries[0].word, 0x8000);
}

#[test]
fn an_index_past_its_jump_table_is_refused() {
    let mut parts = record_parts();
    parts[7] = vec![0x40, 0xFF];
    let error = walk(&parts).unwrap_err().0;
    assert!(
        error.contains("map update index 0x40 is outside the 64-entry"),
        "{error}"
    );
    let mut parts = record_parts();
    parts[19] = vec![0x00, 0xA0, 0xFF, 0xFF];
    let error = walk(&parts).unwrap_err().0;
    assert!(error.contains("MapDataManager index 0x00A0"), "{error}");
}

#[test]
fn the_walk_and_the_terminator_scan_must_agree() {
    // An object holding $FFFF at an even offset: the loader's other path
    // would stop inside it.
    let mut parts = record_parts();
    parts[10] = vec![0, 4, 0, 0, 0xFF, 0xFF, 0, 1, 0, 1, 0xFF, 0xFF];
    let error = walk(&parts).unwrap_err().0;
    assert!(
        error.contains("objects: the record walk ends at"),
        "{error}"
    );
}

#[test]
fn a_pointer_outside_the_rom_and_an_unknown_tree_are_refused() {
    let mut parts = record_parts();
    parts[14] = vec![0, 0, 0, 0x21];
    let error = walk(&parts).unwrap_err().0;
    assert!(error.contains("does not match the start of any"), "{error}");
    let mut parts = record_parts();
    parts[17] = vec![0x7F, 0, 0, 0];
    let error = walk(&parts).unwrap_err().0;
    assert!(
        error.contains("palette pointer 0x7F000000 is outside"),
        "{error}"
    );
}

#[test]
fn a_routine_past_interaction_routines_is_refused() {
    let mut parts = record_parts();
    parts[15] = vec![0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0xFF, 0xFF];
    let error = walk(&parts).unwrap_err().0;
    assert!(error.contains("selects routine 8"), "{error}");
}

#[test]
fn a_rom_without_the_table_is_refused() {
    let error = check_pointer_table(&vec![0; 0x200000]).unwrap_err().0;
    assert!(
        error.starts_with("FieldMapPtrs signature mismatch at 0x100000"),
        "{error}"
    );
    let error = check_pointer_table(&[0; 16]).unwrap_err().0;
    assert!(error.ends_with(", got "), "{error}");
}

#[test]
fn an_object_off_the_table_stride_has_no_symbol() {
    let object = |object_id| Object {
        object_id,
        facing_dir: 0,
        dialogue_id: 0,
        art_tile: 0,
        x_tile: 0,
        y_tile: 0,
    };
    assert_eq!(object(0).symbol(), Some(FIELD_OBJECT_SYMBOLS[0]));
    assert_eq!(object(2).symbol(), None);
    assert_eq!(object(0xFFFC).symbol(), None);
}
