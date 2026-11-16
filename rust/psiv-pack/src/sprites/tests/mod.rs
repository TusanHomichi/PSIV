//! Sprite retirement checks (#556). Inputs and expected images stay local.

use super::{compose::*, field::*, objects::*, records::*};
use std::sync::OnceLock;

mod contracts;
mod retail;
mod source;

pub(crate) struct Retail {
    pub rom: crate::Rom,
    pub routines: Vec<FieldObjectRoutine>,
    pub party: Vec<PartySprite>,
}

pub(crate) fn retail() -> Option<&'static Retail> {
    static RETAIL: OnceLock<Option<Retail>> = OnceLock::new();
    RETAIL
        .get_or_init(|| {
            let rom = crate::test_support::retail_rom("sprite tests")?;
            let routines = scan_field_objects(rom.bytes()).unwrap();
            let party = party_sprites(rom.bytes(), &routines).unwrap();
            Some(Retail {
                rom,
                routines,
                party,
            })
        })
        .as_ref()
}
