//! Checking that a longer route left an earlier run's play alone.
//!
//! A lane that appends chapters must not move a frame of the chapters before
//! them: the earlier run's tape is the route's evidence for those chapters, and
//! a native replay compares its saves byte for byte. [`check_prefix`] holds a
//! new run to that: its tape begins with every pad of the earlier one, and the
//! save each earlier chapter ended on hashes the same in the new run's save
//! directory. A difference names the first frame or the first chapter that
//! moved, which is where a fix changed earlier play.
//!
//! The new run need not have completed: a halted run still writes the saves of
//! the chapters it finished, which is where a prefix change shows first.

use std::path::Path;

use crate::split::{ChapterCut, SplitError};
use crate::tape::{Tape, fnv1a64};

/// What a passing check covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixMatch {
    /// The earlier run's chapters, every one matched.
    pub chapters: usize,
    /// The earlier run's frames, every pad matched.
    pub frames: usize,
}

/// The first frame at which two pad lists differ, comparing `base` against the
/// same length of `run`; `None` when `run` begins with all of `base`.
#[must_use]
pub fn first_divergence(base: &[u8], run: &[u8]) -> Option<usize> {
    base.iter()
        .zip(run)
        .position(|(a, b)| a != b)
        .or_else(|| (run.len() < base.len()).then_some(run.len()))
}

/// Holds `run` (its tape, and the save directory its chapters wrote) to the
/// earlier run `base` (its tape and its completed report's chapters).
///
/// # Errors
///
/// [`SplitError`] naming the first difference: the tapes' starts, the first
/// frame whose pad differs (or the run's tape ending early), or the first
/// chapter whose save is missing or hashes differently.
pub fn check_prefix(
    base: &Tape,
    chapters: &[ChapterCut],
    run: &Tape,
    run_saves: &Path,
) -> Result<PrefixMatch, SplitError> {
    if base.start != run.start {
        return Err(SplitError(format!(
            "the tapes start differently: {:?} and {:?}",
            base.start, run.start
        )));
    }
    if let Some(frame) = first_divergence(&base.pads, &run.pads) {
        let at = chapter_at(chapters, frame);
        return Err(SplitError(format!(
            "the pads differ from frame {frame}{at}: base {:?}, run {:?}",
            base.pads.get(frame),
            run.pads.get(frame)
        )));
    }
    for chapter in chapters {
        let path = run_saves
            .join(format!("{:02}-{}", chapter.index, chapter.id))
            .join("slot_1.sram");
        let bytes = std::fs::read(&path).map_err(|e| {
            SplitError(format!(
                "chapter {} `{}`: cannot read {}: {e}",
                chapter.index,
                chapter.id,
                path.display()
            ))
        })?;
        let hash = fnv1a64(&bytes);
        if hash != chapter.save_fnv {
            return Err(SplitError(format!(
                "chapter {} `{}` ends on save {hash:016x}, the base on {:016x}",
                chapter.index, chapter.id, chapter.save_fnv
            )));
        }
    }
    Ok(PrefixMatch {
        chapters: chapters.len(),
        frames: base.pads.len(),
    })
}

/// ", in chapter N `id`" for the chapter whose frames hold `frame`.
fn chapter_at(chapters: &[ChapterCut], frame: usize) -> String {
    let mut end = 0usize;
    for chapter in chapters {
        end = end.saturating_add(usize::try_from(chapter.frames).unwrap_or(usize::MAX));
        if frame < end {
            return format!(", in chapter {} `{}`", chapter.index, chapter.id);
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tape::TapeStart;

    fn tape(pads: &[u8]) -> Tape {
        Tape {
            start: TapeStart::NewGame,
            pads: pads.to_vec(),
        }
    }

    fn saves(name: &str, chapters: &[(&str, &[u8])]) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("psiv-prefix-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        for (at, (id, bytes)) in chapters.iter().enumerate() {
            let chapter = dir.join(format!("{at:02}-{id}"));
            std::fs::create_dir_all(&chapter).unwrap();
            std::fs::write(chapter.join("slot_1.sram"), bytes).unwrap();
        }
        dir
    }

    fn cuts() -> Vec<ChapterCut> {
        vec![
            ChapterCut {
                index: 0,
                id: "a".into(),
                frames: 2,
                save_fnv: fnv1a64(b"one"),
            },
            ChapterCut {
                index: 1,
                id: "b".into(),
                frames: 3,
                save_fnv: fnv1a64(b"two"),
            },
        ]
    }

    #[test]
    fn a_run_that_extends_the_base_matches() {
        let dir = saves("ok", &[("a", b"one"), ("b", b"two"), ("c", b"new")]);
        let found = check_prefix(
            &tape(&[0, 1, 2, 3, 4]),
            &cuts(),
            &tape(&[0, 1, 2, 3, 4, 9]),
            &dir,
        );
        assert_eq!(
            found.unwrap(),
            PrefixMatch {
                chapters: 2,
                frames: 5
            }
        );
    }

    #[test]
    fn a_moved_pad_names_its_frame_and_chapter() {
        let dir = saves("pad", &[("a", b"one"), ("b", b"two")]);
        let error = check_prefix(
            &tape(&[0, 1, 2, 3, 4]),
            &cuts(),
            &tape(&[0, 1, 2, 7, 4]),
            &dir,
        )
        .unwrap_err();
        assert!(error.0.contains("frame 3, in chapter 1 `b`"), "{error}");
        let short = check_prefix(&tape(&[0, 1, 2, 3, 4]), &cuts(), &tape(&[0, 1]), &dir);
        assert!(short.unwrap_err().0.contains("frame 2"));
    }

    #[test]
    fn a_moved_save_names_its_chapter() {
        let dir = saves("save", &[("a", b"one"), ("b", b"TWO")]);
        let error = check_prefix(
            &tape(&[0, 1, 2, 3, 4]),
            &cuts(),
            &tape(&[0, 1, 2, 3, 4]),
            &dir,
        )
        .unwrap_err();
        assert!(error.0.contains("chapter 1 `b` ends on save"), "{error}");
        let missing = saves("missing", &[("a", b"one")]);
        let error = check_prefix(
            &tape(&[0, 1, 2, 3, 4]),
            &cuts(),
            &tape(&[0, 1, 2, 3, 4]),
            &missing,
        )
        .unwrap_err();
        assert!(error.0.contains("cannot read"), "{error}");
    }

    #[test]
    fn tapes_from_different_starts_differ() {
        let dir = saves("start", &[("a", b"one"), ("b", b"two")]);
        let mut run = tape(&[0, 1, 2, 3, 4]);
        run.start = TapeStart::Save { hash: 1 };
        let error = check_prefix(&tape(&[0, 1, 2, 3, 4]), &cuts(), &run, &dir).unwrap_err();
        assert!(error.0.contains("start differently"), "{error}");
    }
}
