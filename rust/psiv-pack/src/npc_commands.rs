//! `npc_commands.json`, what a scene's `MoveActorCommand` byte means: the port of
//! `psiv_tools/npc_commands.py`, which owns the retail citations (#216, slice 5).
//!
//! `FieldObj_NPCMove` is the only place the speed table is named, so the code
//! pins it, and the table extents give both counts. No `psiv-data` type reads
//! this file: the game's scenes carry the commands themselves, so it stays `Json`.

use std::collections::BTreeSet;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::json::Json;
use crate::png::hex;
use crate::{PackError, Rom};

const RECORD_BYTES: usize = 8;
const FIXED_POINT: i32 = 256;
const FACING_UNCHANGED: u8 = 0xFF;
const COLLISION_CELL_PIXELS: u8 = 16;
/// `PACK_FORMAT_VERSION`.
const FORMAT_VERSION: u32 = crate::PACK_FORMAT_VERSION;
const NAME: &str = "npc_commands.json";

fn refuse<T>(message: String) -> Result<T, PackError> {
    Err(PackError(message))
}

fn facing_name(byte: u8) -> Option<&'static str> {
    match byte {
        0x0 => Some("down"),
        0x4 => Some("up"),
        0x8 => Some("right"),
        0xC => Some("left"),
        _ => None,
    }
}

/// One eight-byte movement record.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Command {
    index: usize,
    raw: [u8; RECORD_BYTES],
    x_pixels: u8,
    y_pixels: u8,
    x_velocity: i16,
    y_velocity: i16,
    facing: u8,
    unused: u8,
}

impl Command {
    fn moves(&self) -> bool {
        self.x_velocity != 0 || self.y_velocity != 0
    }

    /// `distance * 256 / |velocity|`, from `FieldObj_UpdateStepDuration`.
    fn frames(&self) -> Option<i32> {
        [
            (self.x_pixels, self.x_velocity),
            (self.y_pixels, self.y_velocity),
        ]
        .into_iter()
        .find(|(_, velocity)| *velocity != 0)
        .map(|(distance, velocity)| i32::from(distance) * FIXED_POINT / i32::from(velocity).abs())
    }

    fn direction(&self) -> Option<&'static str> {
        if !self.moves() {
            None
        } else if self.y_velocity != 0 {
            Some(if self.y_velocity > 0 { "down" } else { "up" })
        } else {
            Some(if self.x_velocity > 0 { "right" } else { "left" })
        }
    }

    fn facing_json(&self) -> Json {
        Json::object()
            .with("byte", self.facing)
            .with("name", facing_name(self.facing))
            .with("unchanged", self.facing == FACING_UNCHANGED)
    }

    fn to_json(&self) -> Json {
        Json::object()
            .with("id", self.index)
            .with("raw_hex", hex(&self.raw))
            .with("moves", self.moves())
            .with("direction", self.direction())
            .with("x_pixels", self.x_pixels)
            .with("y_pixels", self.y_pixels)
            .with("x_cells", self.x_pixels / COLLISION_CELL_PIXELS)
            .with("y_cells", self.y_pixels / COLLISION_CELL_PIXELS)
            .with("x_velocity", self.x_velocity)
            .with("y_velocity", self.y_velocity)
            .with("frames", self.frames())
            .with("facing", self.facing_json())
    }
}

fn decode(index: usize, raw: [u8; RECORD_BYTES]) -> Result<Command, PackError> {
    let command = Command {
        index,
        raw,
        x_pixels: raw[0],
        y_pixels: raw[1],
        x_velocity: i16::from_be_bytes([raw[2], raw[3]]),
        y_velocity: i16::from_be_bytes([raw[4], raw[5]]),
        facing: raw[6],
        unused: raw[7],
    };
    if command.facing != FACING_UNCHANGED && facing_name(command.facing).is_none() {
        return refuse(format!(
            "command {index} faces 0x{:02X}, which is neither a direction nor the negative that \
             leaves facing alone",
            command.facing
        ));
    }
    if command.x_velocity != 0 && command.y_velocity != 0 {
        return refuse(format!(
            "command {index} moves on both axes; the routine only ever produces one step at a time"
        ));
    }
    for (distance, velocity, axis) in [
        (command.x_pixels, command.x_velocity, "x"),
        (command.y_pixels, command.y_velocity, "y"),
    ] {
        if (distance != 0) != (velocity != 0) {
            return refuse(format!(
                "command {index} has {axis} distance {distance} and velocity {velocity}; one \
                 without the other never terminates or never moves"
            ));
        }
        if velocity != 0 && i32::from(distance) * FIXED_POINT % i32::from(velocity).abs() != 0 {
            return refuse(format!(
                "command {index}'s {axis} step is {distance} pixels at {velocity}/256 per frame, \
                 which is not a whole number of frames"
            ));
        }
    }
    Ok(command)
}

/// `pixels_per_frame`: a retail velocity is an exact binary fraction.
#[allow(clippy::float_arithmetic)]
fn per_frame(velocity: i32) -> f64 {
    f64::from(velocity) / f64::from(FIXED_POINT)
}

fn speed_json(
    selector: usize,
    offset: usize,
    commands: &[Command],
    block: &[u8],
) -> Result<Json, PackError> {
    let magnitudes: BTreeSet<i32> = commands
        .iter()
        .flat_map(|c| [c.x_velocity, c.y_velocity])
        .filter(|v| *v != 0)
        .map(|v| i32::from(v).abs())
        .collect();
    let [velocity] = magnitudes.iter().copied().collect::<Vec<_>>()[..] else {
        let listed: Vec<String> = magnitudes.iter().map(i32::to_string).collect();
        return refuse(format!(
            "speed table {selector} mixes velocities [{}]; a table is supposed to be one speed",
            listed.join(", ")
        ));
    };
    let frames: BTreeSet<i32> = commands
        .iter()
        .filter(|c| c.moves())
        .filter_map(Command::frames)
        .collect();
    Ok(Json::object()
        .with("selector", selector)
        .with("rom_offset", format!("0x{offset:06X}"))
        .with(
            "rom_end",
            format!("0x{:06X}", offset + commands.len() * RECORD_BYTES),
        )
        .with("sha256", hex(&Sha256::digest(block)))
        .with("velocity", velocity)
        .with("pixels_per_frame", per_frame(velocity))
        .with("frames_per_cell", frames.into_iter().collect::<Vec<_>>())
        .with(
            "commands",
            commands.iter().map(Command::to_json).collect::<Vec<_>>(),
        ))
}

/// `dispatch_site`: `lea (abs).l,a1 / add.w d7,d7 / add.w d7,d7 / movea.l
/// (a1,d7.w),a1`, once in the image. Returns the site and the table.
fn dispatch_site(rom: &[u8]) -> Result<(usize, usize), PackError> {
    let hits: Vec<usize> = rom
        .windows(14)
        .enumerate()
        .filter(|(_, w)| {
            w[..2] == [0x43, 0xF9] && w[6..14] == [0xDE, 0x47, 0xDE, 0x47, 0x22, 0x71, 0x70, 0x00]
        })
        .map(|(at, _)| at)
        .collect();
    let [site] = hits[..] else {
        return refuse(format!(
            "the NPC movement dispatch matches {} times, not once",
            hits.len()
        ));
    };
    let table = u32::from_be_bytes([rom[site + 2], rom[site + 3], rom[site + 4], rom[site + 5]]);
    Ok((site, table as usize))
}

fn long(rom: &[u8], at: usize) -> Result<usize, PackError> {
    rom.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(|| PackError(format!("a read at 0x{at:06X} runs past the end of the ROM")))
}

/// `extract_npc_commands`: the tables, as the pack emits them.
pub fn extract_npc_commands(rom: &[u8]) -> Result<Json, PackError> {
    let (site, table) = dispatch_site(rom)?;
    if table == 0 || table >= rom.len() {
        return refuse(format!(
            "the speed table address 0x{table:06X} is outside the ROM"
        ));
    }
    // The tables sit flush behind the pointers, so the gap is the pointer count.
    let first = long(rom, table)?;
    if first <= table || !(first - table).is_multiple_of(4) {
        return refuse(format!(
            "the speed table at 0x{table:06X} points at 0x{first:06X}, which is not a whole \
             number of longs past it; the tables are supposed to be flush"
        ));
    }
    let pointers = (0..(first - table) / 4)
        .map(|i| long(rom, table + i * 4))
        .collect::<Result<Vec<_>, _>>()?;
    let strides: BTreeSet<i64> = pointers
        .windows(2)
        .map(|pair| pair[1] as i64 - pair[0] as i64)
        .collect();
    let [stride] = strides.iter().copied().collect::<Vec<_>>()[..] else {
        let listed: Vec<String> = strides.iter().map(i64::to_string).collect();
        return refuse(format!(
            "the command tables are [{}] bytes apart; they are supposed to be equal in length",
            listed.join(", ")
        ));
    };
    if stride <= 0 || stride % RECORD_BYTES as i64 != 0 {
        return refuse(format!(
            "a command table is {stride} bytes, not a whole number of {RECORD_BYTES}-byte records"
        ));
    }
    let stride = stride as usize;
    let count = stride / RECORD_BYTES;

    let mut tables = Vec::with_capacity(pointers.len());
    for (index, &offset) in pointers.iter().enumerate() {
        let Some(block) = rom.get(offset..offset + stride) else {
            return refuse(format!(
                "command table {index} runs past the end of the ROM"
            ));
        };
        let commands = block
            .as_chunks::<RECORD_BYTES>()
            .0
            .iter()
            .enumerate()
            .map(|(i, raw)| decode(i, *raw))
            .collect::<Result<Vec<_>, _>>()?;
        tables.push((index, offset, commands, block));
    }
    let shape = |commands: &[Command]| -> Vec<(u8, u8, Option<&'static str>, u8)> {
        commands
            .iter()
            .map(|c| (c.x_pixels, c.y_pixels, c.direction(), c.facing))
            .collect()
    };
    let reference = &tables[0].2;
    if tables.iter().any(|t| shape(&t.2) != shape(reference)) {
        return refuse(
            "the speed tables do not hold the same commands; the id cannot be read as a \
             direction independent of the speed"
                .into(),
        );
    }
    if tables.iter().any(|t| t.2.iter().any(|c| c.unused != 0)) {
        return refuse("a command's trailing byte is not zero".into());
    }

    let directions: BTreeSet<&str> = reference.iter().filter_map(Command::direction).collect();
    let mut duplicates = Json::object();
    for direction in &directions {
        let ids: Vec<usize> = reference
            .iter()
            .filter(|c| c.direction() == Some(direction))
            .map(|c| c.index)
            .collect();
        duplicates.insert(*direction, ids);
    }
    let pixels: BTreeSet<u8> = reference
        .iter()
        .filter(|c| c.moves())
        .map(|c| c.x_pixels.max(c.y_pixels))
        .collect();
    let speeds = tables
        .iter()
        .map(|(index, offset, commands, block)| speed_json(*index, *offset, commands, block))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json::object()
        .with("kind", "npc_movement_commands")
        .with(
            "note",
            "What a scene's MoveActorCommand byte means. `commands` is indexed by the command \
             byte and `speeds` by the routine's d7 selector; the same eleven commands appear at \
             all three speeds. A refused move still applies the command's facing and still \
             resets the animation, so a blocked NPC turns on the spot.",
        )
        .with(
            "dispatch",
            Json::object()
                .with("routine", "FieldObj_NPCMove")
                .with("site", format!("0x{site:06X}"))
                .with("speed_table", format!("0x{table:06X}"))
                .with("speed_count", tables.len())
                .with("selector", "d7")
                .with("command_index", "d0")
                .with("bounds_checked", false),
        )
        .with(
            "record",
            Json::object()
                .with("bytes", RECORD_BYTES)
                .with("fixed_point", FIXED_POINT)
                .with(
                    "fields",
                    Json::object()
                        .with("0", "x distance in pixels")
                        .with("1", "y distance in pixels")
                        .with("2", "x velocity, signed word, pixels per frame times 256")
                        .with("4", "y velocity, signed word, pixels per frame times 256")
                        .with("6", "facing direction, or 0xFF to leave facing unchanged")
                        .with("7", "unused, zero in every retail record"),
                )
                .with(
                    "frames",
                    "distance * 256 / abs(velocity), from FieldObj_UpdateStepDuration",
                ),
        )
        .with("command_count", count)
        .with(
            "commands",
            reference
                .iter()
                .map(|c| {
                    Json::object()
                        .with("id", c.index)
                        .with("direction", c.direction())
                        .with("cells", c.x_pixels.max(c.y_pixels) / COLLISION_CELL_PIXELS)
                        .with("facing", c.facing_json())
                })
                .collect::<Vec<_>>(),
        )
        .with("speeds", speeds)
        .with(
            "census",
            Json::object()
                .with(
                    "distinct_directions",
                    directions
                        .iter()
                        .copied()
                        .map(str::to_owned)
                        .collect::<Vec<_>>(),
                )
                .with(
                    "standing_still",
                    reference
                        .iter()
                        .filter(|c| !c.moves())
                        .map(|c| c.index)
                        .collect::<Vec<_>>(),
                )
                .with("duplicate_directions", duplicates)
                .with("pixels_per_command", pixels.into_iter().collect::<Vec<_>>())
                .with(
                    "note",
                    "Every moving command covers exactly one collision cell, and three of the \
                     eleven ids stand still. Left and right each have three ids that decode \
                     identically, which is why a consumer must read the id through this table \
                     rather than deriving a direction from its value.",
                ),
        ))
}

/// The section.
pub fn build(rom: &Rom, root: &Path) -> Result<Json, PackError> {
    let mut file = Json::object().with("format_version", FORMAT_VERSION);
    crate::json::overlay(&mut file, &extract_npc_commands(rom.bytes())?);
    let sha = crate::write_json(root, NAME, &file)?;
    Ok(Json::object().with("npc_commands", headline(&file, sha)?))
}

/// The manifest's headline of the table: the counts and each speed's
/// frames per cell.
fn headline(file: &Json, sha256: String) -> Result<Json, PackError> {
    let missing = |what: &str| PackError(format!("npc_commands.json has no {what}"));
    let speeds = match file.get("speeds") {
        Some(Json::Array(speeds)) => speeds,
        _ => return Err(missing("speeds")),
    };
    let first_frames = speeds
        .iter()
        .map(|speed| match speed.get("frames_per_cell") {
            Some(Json::Array(frames)) => frames.first().cloned().ok_or_else(|| missing("frames")),
            _ => Err(missing("frames_per_cell")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json::object()
        .with("file", NAME)
        .with("sha256", sha256)
        .with(
            "command_count",
            file.get("command_count")
                .cloned()
                .ok_or_else(|| missing("command_count"))?,
        )
        .with(
            "speed_count",
            file.get("dispatch")
                .and_then(|d| d.get("speed_count"))
                .cloned()
                .ok_or_else(|| missing("dispatch.speed_count"))?,
        )
        .with("frames_per_cell", Json::Array(first_frames)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(x: u8, y: u8, xv: i16, yv: i16, facing: u8) -> [u8; 8] {
        let (a, b) = (xv.to_be_bytes(), yv.to_be_bytes());
        [x, y, a[0], a[1], b[0], b[1], facing, 0]
    }

    /// A ROM: the dispatch, two pointers, two tables of two commands.
    fn image(first: &[[u8; 8]; 2], second: &[[u8; 8]; 2]) -> Vec<u8> {
        let table = 14u32;
        let mut rom = vec![0x43, 0xF9];
        rom.extend_from_slice(&table.to_be_bytes());
        rom.extend_from_slice(&[0xDE, 0x47, 0xDE, 0x47, 0x22, 0x71, 0x70, 0x00]);
        rom.extend_from_slice(&(table + 8).to_be_bytes());
        rom.extend_from_slice(&(table + 24).to_be_bytes());
        for r in first.iter().chain(second) {
            rom.extend_from_slice(r);
        }
        rom
    }

    fn decoded(raw: [u8; 8]) -> serde_json::Value {
        serde_json::Value::from(&decode(0, raw).unwrap().to_json())
    }

    /// Ported from tests/test_npc_commands.py's record-format cases (#216).
    #[test]
    fn a_record_decodes_its_direction_frames_and_facing() {
        // 16 pixels at 256/256 is 16 frames, what `FieldObj_UpdateStepDuration`
        // counts down.
        let down = decoded(record(0, 16, 0, 256, 0));
        assert_eq!(
            (
                &down["direction"],
                &down["frames"],
                &down["y_pixels"],
                &down["facing"]["byte"]
            ),
            (&"down".into(), &16.into(), &16.into(), &0.into())
        );
        let up = decoded(record(0, 16, 0, -256, 4));
        assert_eq!(
            (&up["direction"], &up["y_velocity"], &up["frames"]),
            (&"up".into(), &(-256).into(), &16.into())
        );
        let left = decoded(record(16, 0, -256, 0, 0xC));
        assert_eq!(
            (&left["direction"], &left["x_velocity"]),
            (&"left".into(), &(-256).into())
        );
        let idle = decoded(record(0, 0, 0, 0, FACING_UNCHANGED));
        assert_eq!(idle["moves"], false);
        assert!(idle["direction"].is_null() && idle["frames"].is_null());
        assert_eq!(idle["facing"]["unchanged"], true);
        assert_eq!((RECORD_BYTES, FIXED_POINT), (8, 256));
        // A velocity without a distance never arrives.
        assert!(decode(0, record(0, 0, 256, 0, 0)).is_err());
    }

    fn retail() -> Option<(Vec<u8>, serde_json::Value)> {
        let rom = crate::test_support::retail_rom("npc commands retail test")?;
        let data = rom.bytes().to_vec();
        let payload = serde_json::Value::from(&extract_npc_commands(&data).unwrap());
        Some((data, payload))
    }

    /// Ported from tests/test_npc_commands.py's ROM cases (#216).
    #[test]
    fn retail_dispatch_speeds_and_commands() {
        let Some((data, payload)) = retail() else {
            return;
        };
        // The table is named by the routine that reads it:
        // `lea (abs).l,a1 / add.w d7,d7 / add.w d7,d7 / movea.l (a1,d7.w),a1`.
        assert_eq!(dispatch_site(&data).unwrap(), (0x04A168, 0x04A202));
        assert_eq!(&data[0x04A168..0x04A16A], [0x43, 0xF9]);
        assert_eq!(long(&data, 0x04A16A).unwrap(), 0x04A202);
        assert_eq!(payload["dispatch"]["speed_table"], "0x04A202");
        // Three speeds of eleven commands, the tables flush against each other.
        assert_eq!(payload["dispatch"]["speed_count"], 3);
        assert_eq!(payload["command_count"], 11);
        let speeds = payload["speeds"].as_array().unwrap();
        assert_eq!(speeds[0]["rom_offset"], "0x04A20E");
        for pair in speeds.windows(2) {
            assert_eq!(pair[0]["rom_end"], pair[1]["rom_offset"]);
        }
        assert_eq!(speeds[2]["rom_end"], "0x04A316");
        // Half, one and two pixels a frame; the fastest is the party's $0200.
        let field = |key: &str| speeds.iter().map(|s| s[key].clone()).collect::<Vec<_>>();
        assert_eq!(
            field("velocity"),
            [128, 256, 512].map(serde_json::Value::from)
        );
        assert_eq!(
            field("pixels_per_frame"),
            [0.5, 1.0, 2.0].map(serde_json::Value::from)
        );
        assert_eq!(
            field("frames_per_cell"),
            [[32], [16], [8]].map(|f| serde_json::json!(f))
        );
        // Command two is one step down at every speed: what sent this
        // extraction looking.
        for speed in speeds {
            let down = &speed["commands"][2];
            assert_eq!(
                (&down["direction"], &down["y_cells"], &down["x_pixels"]),
                (&"down".into(), &1.into(), &0.into())
            );
            assert_eq!(down["y_pixels"], COLLISION_CELL_PIXELS);
            assert_eq!(
                down["facing"],
                serde_json::json!({"byte": 0, "name": "down", "unchanged": false})
            );
            assert!(down["y_velocity"].as_i64().unwrap() > 0);
        }
        // Four directions and three idles; every move crosses one cell.
        let census = &payload["census"];
        assert_eq!(census["standing_still"], serde_json::json!([0, 3, 7]));
        assert_eq!(
            census["duplicate_directions"],
            serde_json::json!({"up": [1], "down": [2], "left": [4, 5, 6], "right": [8, 9, 10]})
        );
        assert_eq!(census["pixels_per_command"], serde_json::json!([16]));
        // The same commands at every speed.
        let shapes: Vec<Vec<String>> = speeds
            .iter()
            .map(|s| {
                s["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|c| {
                        format!(
                            "{} {} {} {}",
                            c["direction"], c["x_pixels"], c["y_pixels"], c["facing"]["byte"]
                        )
                    })
                    .collect()
            })
            .collect();
        assert!(shapes.windows(2).all(|pair| pair[0] == pair[1]));
        // A moved dispatch is refused, and so are tables of unequal length.
        let mut moved = data.clone();
        moved[0x04A168..0x04A170].fill(0);
        assert!(dispatch_site(&moved).is_err());
        assert!(extract_npc_commands(&moved).is_err());
        let mut unequal = data.clone();
        unequal[0x04A20A..0x04A20E].copy_from_slice(&0x04A400u32.to_be_bytes());
        assert!(extract_npc_commands(&unequal).is_err());
    }

    /// Ported from tests/test_npc_commands.py's pack case (#216): the
    /// manifest's headline names the file it wrote.
    #[test]
    fn retail_manifest_headline_names_the_file() {
        use sha2::Digest;
        let Some(rom) = crate::test_support::retail_rom("npc commands part") else {
            return;
        };
        let root = crate::test_support::scratch("npc-commands");
        let part = serde_json::Value::from(&build(&rom, &root).unwrap());
        let headline = &part["npc_commands"];
        let blob = std::fs::read(root.join(NAME)).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(headline["file"], NAME);
        assert_eq!(
            headline["sha256"].as_str().unwrap(),
            crate::png::hex(&sha2::Sha256::digest(&blob))
        );
        let file: serde_json::Value = serde_json::from_slice(&blob).unwrap();
        assert_eq!(
            (&file["format_version"], &file["kind"]),
            (&FORMAT_VERSION.into(), &"npc_movement_commands".into())
        );
        assert_eq!(
            (&headline["command_count"], &headline["speed_count"]),
            (&11.into(), &3.into())
        );
        assert_eq!(headline["frames_per_cell"], serde_json::json!([32, 16, 8]));
        let down = &file["speeds"][1]["commands"][2];
        assert_eq!(
            (&down["direction"], &down["y_cells"]),
            (&"down".into(), &1.into())
        );
    }

    #[test]
    fn a_table_reads_its_speeds_and_counts_from_its_extents() {
        let still = record(0, 0, 0, 0, FACING_UNCHANGED);
        let rom = image(
            &[record(16, 0, 256, 0, 8), still],
            &[record(16, 0, 512, 0, 8), still],
        );
        let text = crate::json::dumps(&extract_npc_commands(&rom).unwrap(), true);
        assert!(text.contains("\"speed_count\": 2"), "{text}");
        assert!(text.contains("\"command_count\": 2"), "{text}");
        assert!(text.contains("\"pixels_per_frame\": 1.0"), "{text}");
        assert!(text.contains("\"pixels_per_frame\": 2.0"), "{text}");
        assert!(text.contains("\"frames\": 8"), "{text}");
        assert!(text.contains("\"standing_still\": [\n"), "{text}");
    }

    #[test]
    fn a_record_or_table_off_the_shape_is_refused() {
        let good = record(16, 0, 256, 0, 8);
        let bad = |raw, wanted: &str| {
            let error = decode(0, raw).unwrap_err();
            assert!(error.0.contains(wanted), "{}", error.0);
        };
        bad(record(0, 0, 0, 0, 1), "neither a direction");
        bad(record(16, 16, 256, 256, 0), "both axes");
        bad(record(16, 0, 0, 0, 0), "one without the other");
        bad(record(16, 0, 300, 0, 0), "not a whole number of frames");
        let other = image(&[good, good], &[record(0, 16, 0, 256, 0), good]);
        assert!(
            extract_npc_commands(&other)
                .unwrap_err()
                .0
                .contains("do not hold the same")
        );
        let mixed = image(&[good, record(16, 0, 512, 0, 8)], &[good, good]);
        assert!(extract_npc_commands(&mixed).is_err());
        let mut trailing = image(&[good, good], &[good, good]);
        *trailing.last_mut().unwrap() = 1;
        assert!(
            extract_npc_commands(&trailing)
                .unwrap_err()
                .0
                .contains("trailing byte")
        );
        assert!(dispatch_site(&[0; 20]).unwrap_err().0.contains("0 times"));
    }
}
