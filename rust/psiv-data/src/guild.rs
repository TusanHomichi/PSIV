//! The Hunter's Guild job board, as the pack carries it
//! (`psiv_tools/guild_pack.py`): the job names, the list window's record and
//! the colour lines `Event_HuntersGuild` draws them in.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{DataError, RomHash, ShipWindow};

/// How many jobs the board lists.
pub const GUILD_JOBS: usize = 8;

/// Where the pack keeps the board's JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildFile {
    /// Relative JSON path.
    pub path: String,
    /// Extractor's provenance digest.
    pub sha256: String,
}

/// One 16-colour palette line, as RGB.
pub type GuildLine = Vec<[u8; 3]>;

#[derive(Deserialize)]
struct GuildJson {
    format_version: u32,
    kind: String,
    rom_sha256: RomHash,
    window: ShipWindow,
    names: Vec<String>,
    pending: String,
    rows: Vec<[u8; 8]>,
    money: Vec<u8>,
    font_line: GuildLine,
    done_line: GuildLine,
    closed_line: GuildLine,
}

/// The board's data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildBoard {
    /// The list window (`WinGroup_Event` record `$B`).
    pub window: ShipWindow,
    /// The job names, top to bottom.
    pub names: Vec<String>,
    /// `GuildText_ListingPending`, the row of a job not yet offered.
    pub pending: String,
    /// `HuntersGuildData`'s rows, byte for byte.
    pub rows: Vec<[u8; 8]>,
    /// `HuntersGuildMoneyTbl`, thousands of meseta.
    pub money: Vec<u8>,
    /// Line 2: the colours the window font is drawn in.
    pub font_line: GuildLine,
    /// Line 1: a finished job's row (`$A000`).
    pub done_line: GuildLine,
    /// Line 3: a job that is not open (`$E000`).
    pub closed_line: GuildLine,
}

impl GuildBoard {
    pub(crate) fn load(
        directory: &Path,
        file: &GuildFile,
        rom: &RomHash,
    ) -> Result<GuildBoard, DataError> {
        let path = directory.join(&file.path);
        let text = std::fs::read_to_string(&path).map_err(|e| DataError::io(&path, e))?;
        let json: GuildJson = serde_json::from_str(&text).map_err(|e| DataError::json(&path, e))?;
        let well_formed = json.format_version == 1
            && json.kind == "guild"
            && json.rom_sha256 == *rom
            && json.names.len() == GUILD_JOBS
            && json.rows.len() == GUILD_JOBS
            && json.money.len() == GUILD_JOBS
            && [&json.font_line, &json.done_line, &json.closed_line]
                .iter()
                .all(|line| line.len() == 16);
        if !well_formed {
            return Err(DataError::ManifestMismatch {
                path,
                field: "guild",
                manifest: "guild/1 with matching ROM, eight jobs, three 16-colour lines".into(),
                record: "wrong version, kind, ROM hash or a count".into(),
            });
        }
        Ok(GuildBoard {
            window: json.window,
            names: json.names,
            pending: json.pending,
            rows: json.rows,
            money: json.money,
            font_line: json.font_line,
            done_line: json.done_line,
            closed_line: json.closed_line,
        })
    }
}
