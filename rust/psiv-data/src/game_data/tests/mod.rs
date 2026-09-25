//! Unit tests for loading and validation.
//!
//! Fixtures are hand-written synthetic JSON -- 4x4 maps, invented symbols -- so
//! nothing Sega-derived is committed. They are shaped exactly like
//! `psiv_tools.pack` output, extra provenance fields included, so a field-name
//! drift between the two lanes fails here rather than at integration. The real
//! pack is exercised by `tests/runtime_pack.rs`, which is gated on it existing.
//!
//! Each case sits with the concern it checks: `accessors` is a pack that loads,
//! `pack` the manifest and cross-map references, `maps` one record's own numbers,
//! `overworld` its page hooks, `load` a pack directory on disk, and `schema` the
//! record's serde shape.

mod accessors;
mod fixtures;
mod load;
mod maps;
mod overworld;
mod pack;
mod schema;
