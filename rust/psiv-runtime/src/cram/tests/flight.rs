//! Helper copy destinations and both Snowstorm branches on synthetic palettes.

use super::*;

#[test]
fn helpers_copy_only_their_lines_and_missing_palettes_are_reported() {
    let addresses = [
        0x1D_E0D8, 0x1D_E0F8, 0x1D_E138, 0x1D_E118, 0x1D_E1D8, 0x1D_E1F8, 0x1D_E218, 0x1D_E198,
        0x1D_E1B8, 0x1D_E238,
    ];
    let palettes = ScenePalettes::from_parts(
        addresses
            .into_iter()
            .enumerate()
            .map(|(index, address)| (address, vec![index as u16 + 1; 16]))
            .collect(),
        BTreeMap::new(),
    );
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
