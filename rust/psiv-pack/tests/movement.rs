//! #410: an attack object's `addq`/`subq.w #n,d16(a4)` coordinate writes keep
//! their own quick value (bits 11-9, 0 means 8) and direction (bit 8). The
//! decoder once read the effective-address bits, so every one came out
//! `addq.w #6`. Synthetic code; no ROM. `tests/test_battle_animation_movement.py`
//! holds the Python decoder to the same cases.

use psiv_pack::battle::animations::movement::coordinate_sites;

#[test]
fn addq_and_subq_decode_their_own_quick_value_and_direction() {
    let words: [u16; 8] = [
        0x5F6C, 0x002C, // subq.w #7,$2C(a4)
        0x536C, 0x002E, // subq.w #1,$2E(a4)
        0x506C, 0x002C, // addq.w #8,$2C(a4)
        0x5A6C, 0x002E, // addq.w #5,$2E(a4)
    ];
    let mut rom: Vec<u8> = words.iter().flat_map(|word| word.to_be_bytes()).collect();
    rom.resize(0x30, 0);
    let sites: Vec<(i64, String)> = coordinate_sites(&rom, 0, 0x20, 0x40)
        .iter()
        .map(serde_json::Value::from)
        .map(|site| {
            (
                site["delta"].as_i64().expect("a delta"),
                site["operation"]
                    .as_str()
                    .expect("an operation")
                    .to_string(),
            )
        })
        .collect();
    assert_eq!(
        sites,
        [
            (-7, "subq.w".to_string()),
            (-1, "subq.w".to_string()),
            (8, "addq.w".to_string()),
            (5, "addq.w".to_string()),
        ]
    );
}
