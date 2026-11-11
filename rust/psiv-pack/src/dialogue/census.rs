//! `Census` in `psiv_tools/dialogue_pack/flow.py`: what the retail script
//! contains, as opposed to what the codes permit. It walks the tree records
//! `trees.json` holds, entry by entry, as `emit_dialogue` does.

use std::collections::{BTreeMap, BTreeSet};

use crate::PackError;
use crate::json::Json;
use crate::text::{TEXT_ACTIONS, dialogue_charset};

const LINES_PER_WINDOW: usize = 2;

/// A tally whose keys print as Python's `str(key)` of an int.
type Tally<K> = BTreeMap<K, usize>;

fn bump<K: Ord>(tally: &mut Tally<K>, key: K) {
    *tally.entry(key).or_insert(0) += 1;
}

fn tally<K: ToString>(tally: &Tally<K>) -> Json {
    Json::Object(
        tally
            .iter()
            .map(|(key, count)| (key.to_string(), Json::from(*count)))
            .collect(),
    )
}

fn text(value: &Json, key: &str) -> Option<String> {
    match value.get(key) {
        Some(Json::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

fn int(value: &Json, key: &str) -> Result<i128, PackError> {
    match value.get(key) {
        Some(Json::Int(n)) => Ok(*n),
        _ => Err(PackError(format!("a dialogue record has no integer {key}"))),
    }
}

fn list<'a>(value: &'a Json, key: &str) -> Result<&'a [Json], PackError> {
    match value.get(key) {
        Some(Json::Array(items)) => Ok(items),
        _ => Err(PackError(format!("a dialogue record has no list {key}"))),
    }
}

#[derive(Default)]
pub struct Census {
    control_codes: Tally<String>,
    control_codes_by_tree: BTreeMap<i128, Tally<String>>,
    actions: Tally<String>,
    portrait_ids: Tally<i128>,
    portrait_positions: Tally<i128>,
    delays: Tally<i128>,
    line_lengths: Tally<usize>,
    page_endings: Tally<String>,
    /// Counted from decoded characters rather than raw bytes.
    glyphs: BTreeSet<char>,
    empty_entries: usize,
    entries: usize,
    pages: usize,
    longest_line: String,
    longest_line_at: Option<Json>,
    preamble_events: usize,
    preamble_keep_facing: usize,
    codes_outside_preamble: Vec<Json>,
    unreachable_tails: Vec<Json>,
    line_overflow: Vec<Json>,
}

impl Census {
    /// `add_entry`: one entry of tree `tree`, its segments and pages.
    pub fn add_entry(&mut self, tree: i128, entry: &Json) -> Result<(), PackError> {
        let id = int(entry, "id")?;
        let segments = list(entry, "segments")?;
        self.entries += 1;
        let by_tree = self.control_codes_by_tree.entry(tree).or_default();
        if segments.is_empty() {
            self.empty_entries += 1;
        }
        for (index, segment) in segments.iter().enumerate() {
            if let Some(chars) = text(segment, "text") {
                self.glyphs.extend(chars.chars());
                continue;
            }
            let code = text(segment, "code")
                .ok_or_else(|| PackError("a control segment has no code".into()))?;
            bump(&mut self.control_codes, code.clone());
            bump(by_tree, code);
            let name = text(segment, "ctrl").unwrap_or_default();
            match name.as_str() {
                "action" => bump(
                    &mut self.actions,
                    text(segment, "action").unwrap_or_default(),
                ),
                "portrait" => {
                    bump(&mut self.portrait_ids, int(segment, "id")?);
                    if let Some(Json::Int(position)) = segment.get("position") {
                        bump(&mut self.portrait_positions, *position);
                    }
                }
                "delay" => bump(&mut self.delays, int(segment, "frames")?),
                "event" | "keep_npc_facing" => {
                    // Both are preamble-only: everything before them must be
                    // a flag check, or the interaction scanner never reaches
                    // them.
                    let preamble = segments[..index]
                        .iter()
                        .all(|other| text(other, "ctrl").as_deref() == Some("flag_check"));
                    if preamble && name == "event" {
                        self.preamble_events += 1;
                    } else if preamble {
                        self.preamble_keep_facing += 1;
                    } else {
                        self.codes_outside_preamble.push(
                            Json::object()
                                .with("tree", tree)
                                .with("entry", id)
                                .with("segment", index)
                                .with("ctrl", name.as_str()),
                        );
                    }
                }
                _ => {}
            }
            if name == "yes_no" && index != segments.len() - 1 {
                self.unreachable_tails.push(
                    Json::object()
                        .with("tree", tree)
                        .with("entry", id)
                        .with("segment", index)
                        .with("ctrl", name.as_str())
                        .with("trailing_segments", segments.len() - index - 1),
                );
            }
        }
        for page in list(entry, "pages")? {
            self.pages += 1;
            bump(
                &mut self.page_endings,
                text(page, "end").unwrap_or_default(),
            );
            let lines = list(page, "lines")?;
            if lines.len() > LINES_PER_WINDOW {
                self.line_overflow.push(
                    Json::object()
                        .with("tree", tree)
                        .with("entry", id)
                        .with("lines", lines.len()),
                );
            }
            for line in lines {
                let Json::Str(line) = line else {
                    return Err(PackError("a page line is not text".into()));
                };
                let length = line.chars().count();
                bump(&mut self.line_lengths, length);
                if length > self.longest_line.chars().count() {
                    self.longest_line = line.clone();
                    self.longest_line_at =
                        Some(Json::object().with("tree", tree).with("entry", id));
                }
            }
        }
        Ok(())
    }

    /// `to_json`, given the portrait table's ids, the font's blank glyph
    /// bytes and the control codes.
    pub fn to_json(
        &self,
        portrait_ids: &BTreeSet<i128>,
        blank_glyphs: &BTreeSet<u8>,
        control_codes: &[u8],
    ) -> Json {
        let used: BTreeSet<i128> = self.portrait_ids.keys().copied().collect();
        let charset = dialogue_charset();
        let glyphs: Vec<(u8, char)> = (0..=255u8)
            .filter_map(|byte| charset.glyph(byte).map(|c| (byte, c)))
            .collect();
        let unused_glyphs: Vec<Json> = glyphs
            .iter()
            .filter(|(_, c)| !self.glyphs.contains(c))
            .map(|&(byte, c)| {
                Json::object()
                    .with("byte", byte)
                    .with("byte_hex", format!("0x{byte:02X}"))
                    .with("char", c.to_string())
            })
            .collect();
        let blank: Vec<Json> = glyphs
            .iter()
            .filter(|&&(byte, c)| blank_glyphs.contains(&byte) && c != ' ')
            .map(|&(byte, c)| {
                Json::object()
                    .with("byte", byte)
                    .with("byte_hex", format!("0x{byte:02X}"))
                    .with("char", c.to_string())
                    .with("used", self.glyphs.contains(&c))
            })
            .collect();
        let mut unused_codes: Vec<String> = control_codes
            .iter()
            .map(|code| format!("0x{code:02X}"))
            .filter(|code| !self.control_codes.contains_key(code))
            .collect();
        unused_codes.sort();
        let mut unused_actions: Vec<&str> = TEXT_ACTIONS
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !self.actions.contains_key(*name))
            .collect();
        unused_actions.sort_unstable();
        Json::object()
            .with("entries", self.entries)
            .with("empty_entries", self.empty_entries)
            .with("pages", self.pages)
            .with("control_codes", tally(&self.control_codes))
            .with(
                "control_codes_by_tree",
                Json::Object(
                    self.control_codes_by_tree
                        .iter()
                        .map(|(tree, codes)| (tree.to_string(), tally(codes)))
                        .collect(),
                ),
            )
            .with("unused_control_codes", unused_codes)
            .with("text_actions", tally(&self.actions))
            .with("unused_text_actions", unused_actions)
            .with("portrait_ids", tally(&self.portrait_ids))
            .with(
                "portrait_ids_unused",
                portrait_ids.difference(&used).copied().collect::<Vec<_>>(),
            )
            .with(
                "portrait_ids_outside_table",
                used.iter()
                    .filter(|id| !portrait_ids.contains(id) && **id != 0)
                    .copied()
                    .collect::<Vec<_>>(),
            )
            .with("portrait_positions", tally(&self.portrait_positions))
            .with("delay_frames", tally(&self.delays))
            .with("line_lengths", tally(&self.line_lengths))
            .with("longest_line", self.longest_line.chars().count())
            .with("longest_line_text", self.longest_line.as_str())
            .with("longest_line_at", self.longest_line_at.clone())
            .with("page_endings", tally(&self.page_endings))
            .with(
                "preamble",
                Json::object()
                    .with("events", self.preamble_events)
                    .with("keep_npc_facing", self.preamble_keep_facing)
                    .with("outside_preamble", self.codes_outside_preamble.clone()),
            )
            .with("unreachable_tails", self.unreachable_tails.clone())
            .with("lines_past_the_window", self.line_overflow.clone())
            .with("unused_charset_bytes", unused_glyphs)
            .with("blank_glyphs_in_charset", blank)
            .with(
                "note",
                "Observed, not permitted. The script uses 10 of the 16 control codes and 66 of \
                 the 78 charset bytes; $FB is not a retail code at all; four charset bytes have \
                 an empty glyph in the font and none of the four is ever used; portrait id 0 \
                 means hide and is not a table entry; and no line anywhere exceeds 32 \
                 characters or spills past the window's two lines.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: usize, segments: Vec<Json>, lines: &[&[&str]]) -> Json {
        let pages: Vec<Json> = lines
            .iter()
            .map(|page| {
                Json::object().with("end", "end").with(
                    "lines",
                    page.iter().map(|l| Json::from(*l)).collect::<Vec<_>>(),
                )
            })
            .collect();
        Json::object()
            .with("id", id)
            .with("segments", segments)
            .with("pages", pages)
    }

    fn ctrl(code: &str, name: &str) -> Json {
        Json::object().with("code", code).with("ctrl", name)
    }

    #[test]
    fn preamble_codes_tails_and_the_longest_line_are_found_in_walk_order() {
        let mut census = Census::default();
        let flag = ctrl("0xFA", "flag_check");
        let text = Json::object().with("text", "ab");
        census
            .add_entry(
                1,
                &entry(0, vec![flag.clone(), ctrl("0xF6", "event")], &[&["abc"]]),
            )
            .unwrap();
        census
            .add_entry(
                1,
                &entry(
                    1,
                    vec![
                        text,
                        ctrl("0xF3", "keep_npc_facing"),
                        ctrl("0xF5", "yes_no"),
                        flag,
                    ],
                    &[&["abcd", "x", "y"]],
                ),
            )
            .unwrap();
        census
            .add_entry(2, &entry(0, vec![], &[&["wxyz"]]))
            .unwrap();
        let json =
            serde_json::Value::from(&census.to_json(&BTreeSet::new(), &BTreeSet::new(), &[]));
        assert_eq!(json["entries"], 3);
        assert_eq!(json["empty_entries"], 1);
        assert_eq!(json["preamble"]["events"], 1);
        assert_eq!(json["preamble"]["keep_npc_facing"], 0);
        assert_eq!(json["preamble"]["outside_preamble"][0]["entry"], 1);
        assert_eq!(json["unreachable_tails"][0]["trailing_segments"], 1);
        assert_eq!(json["lines_past_the_window"][0]["lines"], 3);
        assert_eq!(
            json["longest_line_text"], "abcd",
            "the first of the longest stays"
        );
        assert_eq!(json["longest_line_at"]["tree"], 1);
        assert_eq!(json["control_codes_by_tree"]["1"]["0xFA"], 2);
    }
}
