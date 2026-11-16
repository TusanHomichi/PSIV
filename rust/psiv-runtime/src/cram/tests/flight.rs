//! Helper copy destinations and both Snowstorm branches on synthetic palettes.

use super::*;

#[test]
fn helpers_copy_only_their_lines_and_missing_palettes_are_reported() {
    let mut words: BTreeMap<u32, Vec<u16>> = [0x1D_E0D8, 0x1D_E0F8, 0x1D_E138, 0x1D_E118]
        .into_iter()
        .enumerate()
        .map(|(index, address)| (address, vec![index as u16 + 1; 16]))
        .collect();
    let mut table = Vec::new();
    for world in 0..6u16 {
        // Fictional, deliberately non-ROM pointers and palette words.
        let address = 0x10000 + u32::from(world) * 0x100;
        table.extend([(address >> 16) as u16, address as u16]);
        words.insert(address, vec![world + 5; 16]);
    }
    words.insert(0x06_4550, table);
    let palettes = ScenePalettes::from_parts(words, BTreeMap::new());
    for snow in [false, true] {
        let mut cram = cram(filled(0xEEE), filled(0));
        cram.flight_ship(&palettes, snow);
        let mut expected = filled(0xEEE);
        expected[32..48].fill(if snow { 1 } else { 2 });
        assert_eq!(cram.view().words, expected);
    }
    for world in 0..6 {
        let mut cram = cram(filled(0xEEE), filled(0));
        cram.flight_planet(&palettes, world);
        let mut expected = filled(0xEEE);
        expected[..16].fill(u16::from(world) + 5);
        expected[16..32].fill(4);
        expected[48..64].fill(3);
        assert_eq!(cram.view().words, expected);
        // Empty pack is a negative control, never a silent baked-palette pass.
        let mut missing = super::cram(filled(0), filled(0));
        missing.flight_planet(&ScenePalettes::default(), world);
        assert_eq!(missing.view().missing.len(), 3);
        assert_ne!(missing.view().words, cram.view().words);
    }
}
