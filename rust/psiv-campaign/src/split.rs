//! Cutting a run's tape at its chapter boundaries.
//!
//! A completed run's report names every chapter, the frames it took and the
//! FNV-1a 64 hash of the save it ended on. [`split_tape`] cuts the run's tape at
//! those frames: the first piece keeps the run's own start, and every later
//! piece is a save-start tape whose save is the chapter before it. The pieces
//! go through the shared tape codec, so a native replay opens them like any
//! other tape and `TapeFeed` checks each start save against its bytes.
//!
//! The pieces concatenate back to the run's tape, and that is all they promise.
//! A piece replayed from its predecessor's save is **not** the run's chapter: a
//! slot carries no RNG state and no frame counter, so a loaded game restarts
//! both and the same pads play other battles (the chapter-one-to-Holt piece
//! ends on different bytes). The route's evidence is the whole tape from New
//! Game; `tools/verify_native_route.py` replays that once and compares each
//! chapter's save at the frame the chapter ended on.

use serde_json::Value;

use crate::runner::ChapterSummary;
use crate::tape::{Tape, TapeStart, fnv1a64};

/// One chapter as the completed report records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterCut {
    /// Position in the route, which names the chapter's save directory.
    pub index: usize,
    /// The chapter id.
    pub id: String,
    /// Frames the chapter took, including the settling before its save.
    pub frames: u64,
    /// FNV-1a 64 of the chapter-boundary save the chapter ended on.
    pub save_fnv: u64,
}

/// Why a tape could not be split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitError(pub String);

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SplitError {}

/// The chapters of a completed run's report.
///
/// # Errors
///
/// [`SplitError`] when the report is not a completed run's, or a chapter
/// record is malformed.
pub fn chapters_of(report: &Value) -> Result<Vec<ChapterCut>, SplitError> {
    if report["result"].as_str() != Some("completed") {
        return Err(SplitError(
            "the report is not a completed run's (a halted run has no chapter list)".into(),
        ));
    }
    let list = report["chapters"]
        .as_array()
        .ok_or_else(|| SplitError("the report has no `chapters` list".into()))?;
    list.iter()
        .enumerate()
        .map(|(at, chapter)| {
            let bad = |field: &str| SplitError(format!("chapter record {at} lacks `{field}`"));
            let save_fnv = chapter["save_fnv"]
                .as_str()
                .and_then(|text| u64::from_str_radix(text, 16).ok())
                .ok_or_else(|| bad("save_fnv"))?;
            Ok(ChapterCut {
                index: chapter["index"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| bad("index"))?,
                id: chapter["id"].as_str().ok_or_else(|| bad("id"))?.to_owned(),
                frames: chapter["frames"].as_u64().ok_or_else(|| bad("frames"))?,
                save_fnv,
            })
        })
        .collect()
}

/// Cuts `run` into one tape per chapter.
///
/// # Errors
///
/// [`SplitError`] when no chapter is named, or the chapters' frames do not add
/// up to the tape's.
pub fn split_tape(run: &Tape, chapters: &[ChapterCut]) -> Result<Vec<Tape>, SplitError> {
    if chapters.is_empty() {
        return Err(SplitError("the run has no chapters to cut".into()));
    }
    let total: u64 = chapters.iter().map(|chapter| chapter.frames).sum();
    if usize::try_from(total).ok() != Some(run.pads.len()) {
        return Err(SplitError(format!(
            "the chapters add up to {total} frames and the tape has {}",
            run.pads.len()
        )));
    }
    let mut pieces = Vec::with_capacity(chapters.len());
    let mut at = 0;
    for (n, chapter) in chapters.iter().enumerate() {
        let end = at + usize::try_from(chapter.frames).unwrap_or(usize::MAX);
        let start = if n == 0 {
            run.start.clone()
        } else {
            TapeStart::Save {
                hash: chapters[n - 1].save_fnv,
            }
        };
        pieces.push(Tape {
            start,
            pads: run.pads[at..end].to_vec(),
        });
        at = end;
    }
    Ok(pieces)
}

/// The `chapters` list a completed run's report carries: each chapter's route
/// position, id, frames, battles, save file and the save's FNV-1a 64.
///
/// # Errors
///
/// The I/O error of reading a chapter save back to hash it.
pub fn report_chapters(
    first_index: usize,
    chapters: &[ChapterSummary],
) -> Result<Value, std::io::Error> {
    let mut list = Vec::with_capacity(chapters.len());
    for (offset, chapter) in chapters.iter().enumerate() {
        let bytes = std::fs::read(&chapter.save)?;
        list.push(serde_json::json!({
            "index": first_index + offset,
            "id": chapter.id,
            "frames": chapter.frames,
            "battles": chapter.battles,
            "save": chapter.save.display().to_string(),
            "save_fnv": format!("{:016x}", fnv1a64(&bytes)),
        }));
    }
    Ok(Value::Array(list))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cut(index: usize, frames: u64, save_fnv: u64) -> ChapterCut {
        ChapterCut {
            index,
            id: format!("c{index}"),
            frames,
            save_fnv,
        }
    }

    fn run() -> Tape {
        Tape {
            start: TapeStart::NewGame,
            pads: vec![0, 0, 1, 1, 2, 0x80, 0x80],
        }
    }

    #[test]
    fn pieces_concatenate_to_the_run() {
        let chapters = [cut(0, 2, 0xaa), cut(1, 3, 0xbb), cut(2, 2, 0xcc)];
        let pieces = split_tape(&run(), &chapters).unwrap();
        assert_eq!(pieces.len(), 3);
        let joined: Vec<u8> = pieces.iter().flat_map(|p| p.pads.clone()).collect();
        assert_eq!(joined, run().pads);
    }

    #[test]
    fn each_piece_starts_from_the_save_before_it() {
        let chapters = [cut(0, 2, 0xaa), cut(1, 3, 0xbb), cut(2, 2, 0xcc)];
        let pieces = split_tape(&run(), &chapters).unwrap();
        assert_eq!(pieces[0].start, TapeStart::NewGame);
        assert_eq!(pieces[1].start, TapeStart::Save { hash: 0xaa });
        assert_eq!(pieces[2].start, TapeStart::Save { hash: 0xbb });
    }

    #[test]
    fn a_run_that_started_from_a_save_keeps_that_start() {
        let mut from_save = run();
        from_save.start = TapeStart::Save { hash: 0x99 };
        let pieces = split_tape(&from_save, &[cut(5, 7, 0xaa)]).unwrap();
        assert_eq!(pieces[0].start, TapeStart::Save { hash: 0x99 });
    }

    #[test]
    fn frames_that_do_not_add_up_are_refused() {
        for frames in [1, 8] {
            let error = split_tape(&run(), &[cut(0, 3, 1), cut(1, frames, 2)]).unwrap_err();
            assert!(error.0.contains("add up to"), "{error}");
        }
        assert!(split_tape(&run(), &[]).is_err());
    }

    #[test]
    fn a_report_without_a_completed_run_is_refused() {
        let halted = json!({"result": "halted", "chapter": "academy"});
        assert!(
            chapters_of(&halted)
                .unwrap_err()
                .0
                .contains("not a completed")
        );
        let bare = json!({"result": "completed", "digest": "00"});
        assert!(chapters_of(&bare).unwrap_err().0.contains("no `chapters`"));
        let malformed = json!({"result": "completed", "chapters": [{"id": "a"}]});
        assert!(chapters_of(&malformed).is_err());
    }

    #[test]
    fn a_report_round_trips_its_chapter_records() {
        let report = json!({"result": "completed", "chapters": [
            {"index": 3, "id": "x", "frames": 10, "save_fnv": "00000000000000ff"},
        ]});
        let expected = ChapterCut {
            index: 3,
            id: "x".into(),
            frames: 10,
            save_fnv: 0xff,
        };
        assert_eq!(chapters_of(&report).unwrap(), vec![expected]);
    }
}
