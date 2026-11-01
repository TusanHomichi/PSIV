//! Field sprites, the port of `psiv_tools/sprites/` (#216, slice 4), whose
//! package docstring is the transcription: mapping records and animation tables
//! ([`records`]), `FieldObjectsJmpTbl`'s init blocks ([`objects`]), frames and
//! sheets ([`compose`]), palettes, the party, a map's staged art, vehicles and
//! chests ([`field`]), and writing `sprites/` ([`emit`]).

pub mod compose;
pub mod emit;
pub mod field;
pub mod objects;
pub mod records;
