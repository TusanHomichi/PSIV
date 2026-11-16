//! Exercise the test-only hooks in the actual decoder sources. Integration
//! tests compile the library without cfg(test), so load those same sources
//! here with their hooks enabled and share the library tests' collector.
use psiv_pack::decode::{Call, DecodeError, Decodes};

#[allow(dead_code)]
#[path = "../src/decode/enigma.rs"]
mod enigma;
#[allow(dead_code)]
#[path = "../src/decode/kosinski.rs"]
mod kosinski;
#[allow(dead_code)]
#[path = "../src/decode/nemesis.rs"]
mod nemesis;
#[path = "support/decoder_observed.rs"]
mod observed;
