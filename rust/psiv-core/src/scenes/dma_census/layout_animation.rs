//! Seven layout/animation callers (#602). Explicit DMA frames are charged
//! once to the named waits below; CPU refresh boundaries remain #546's.
//! Offsets, repetitions and limits: `docs/scenes/DMA_FRAME_OWNERSHIP.md`.

use super::{DMA_HELPERS, Row, SceneOp, VINT_PREPARE, Why, jsr_target};
use crate::PresentationOp;

pub(super) const fn row(scene: &'static str, ops: usize, calls: usize) -> Row {
    Row {
        scene,
        ops,
        calls,
        why: Why::LayoutAnimationFrames,
    }
}

/// A missing refresh/DMA frame, not an animation wait. The owning issue is
/// explicit so a census exception cannot silently become a timing claim.
#[derive(Clone, Copy)]
struct RefreshGap {
    scene: &'static str,
    refresh: &'static [(usize, u32)],
    dma: usize,
    helper: u32,
    op: usize,
    issue: u32,
}

const REFRESH_GAPS: &[RefreshGap] = &[
    RefreshGap {
        scene: "Event_TylerGraveOpening",
        refresh: &[(0x06_FCF8, 0x05_40FA)],
        dma: 0x06_FCFE,
        helper: DMA_HELPERS[2],
        op: 6,
        issue: 546,
    },
    RefreshGap {
        scene: "Event_MeetingLeRoof",
        refresh: &[(0x07_0596, 0x05_4098), (0x07_059C, 0x05_40FA)],
        dma: 0x07_05A2,
        helper: DMA_HELPERS[0],
        op: 18,
        issue: 546,
    },
];

fn word(rom: &[u8], at: usize) -> u16 {
    u16::from_be_bytes(rom[at..at + 2].try_into().unwrap())
}

fn dbf(rom: &[u8], at: usize, register: u16, head: usize) -> bool {
    word(rom, at) == 0x51C8 + register
        && at as i64 + 2 + i64::from(word(rom, at + 2) as i16) == head as i64
}

/// Exact consuming ops, including their policy. Comparing the entire span
/// also rejects inserting a second DMA or wait between the named consumers.
fn waits(ops: &[SceneOp], span: std::ops::Range<usize>) -> Vec<(usize, u16, bool)> {
    span.filter_map(|i| match ops.get(i) {
        Some(SceneOp::WaitFrames { frames }) => Some((i, *frames, false)),
        Some(SceneOp::Wait { ticks }) => Some((i, *ticks, true)),
        Some(SceneOp::DmaPlanes) => Some((i, 1, false)),
        _ => None,
    })
    .collect()
}

pub(super) fn problems(rom: &[u8], name: &str, ops: &[SceneOp]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut check = |holds: bool, what: &str| {
        if !holds {
            errors.push(format!("{name}: {what}"));
        }
    };
    if let Some(gap) = REFRESH_GAPS.iter().find(|g| g.scene == name) {
        check(gap.issue == 546, "refresh gap must belong to #546");
        check(
            jsr_target(rom, gap.dma) == Some(gap.helper),
            "refresh DMA call changed",
        );
        for &(at, target) in gap.refresh {
            check(
                jsr_target(rom, at) == Some(target),
                "refresh CPU call changed (#546)",
            );
        }
        let expected = if name == "Event_MeetingLeRoof" {
            SceneOp::Presentation {
                op: PresentationOp::ReloadMapChunks,
            }
        } else {
            SceneOp::WriteMapChunks {
                chunks: std::borrow::Cow::Borrowed(&[(10, 12, 0x47)]),
            }
        };
        check(
            ops.get(gap.op) == Some(&expected),
            "refresh-owned op changed (#546)",
        );
        check(
            !ops.get(gap.op + 1).is_some_and(|op| {
                matches!(
                    op,
                    SceneOp::DmaPlanes | SceneOp::WaitFrames { .. } | SceneOp::Wait { .. }
                )
            }),
            "refresh gap acquired an unverified wait (#546)",
        );
        return errors;
    }
    match name {
        "Event_RuneFlaeli" => {
            // One B-plane DMA per pass, 240 passes of loc_6D932. The
            // ObjectAnimation record is nonblocking; these four waits own it.
            check(
                jsr_target(rom, 0x06_D954) == Some(DMA_HELPERS[2])
                    && word(rom, 0x06_D92E) == 0x303C
                    && word(rom, 0x06_D930) + 1 == 240
                    && dbf(rom, 0x06_D964, 0, 0x06_D932),
                "240-pass Flaeli DMA loop changed",
            );
            check(
                waits(ops, 21..32)
                    == [
                        (23, 70, false),
                        (27, 26, false),
                        (29, 10, false),
                        (31, 134, false),
                    ],
                "Flaeli waits must spend exactly 240 DMA frames",
            );
            check(
                matches!(
                    ops.get(21),
                    Some(SceneOp::ObjectAnimation { frames: 240, .. })
                ) && matches!(ops.get(30), Some(SceneOp::RestoreMapChunks { .. })),
                "Flaeli animation/refresh beat changed",
            );
            // RefreshPlaneBG is a tail jump, conditional on ECF3 == $32.
            check(
                word(rom, 0x06_DBD2) == 0x4EF9
                    && rom[0x06_DBD4..0x06_DBD8] == 0x05_40FA_u32.to_be_bytes(),
                "Flaeli conditional CPU refresh changed (#546)",
            );
        }
        "Event_MachineCenterAppearing" => {
            check(
                jsr_target(rom, 0x06_B536) == Some(DMA_HELPERS[2])
                    && word(rom, 0x06_B510) == 0x3E3C
                    && word(rom, 0x06_B512) + 1 == 369
                    && dbf(rom, 0x06_B54A, 7, 0x06_B514),
                "369-pass Machine Center DMA loop changed",
            );
            check(
                waits(ops, 5..8) == [(5, 308, true), (7, 61, true)],
                "Machine Center's split waits must spend exactly 369 DMA frames",
            );
            check(
                jsr_target(rom, 0x06_B5D8) == Some(0x05_40FA)
                    && matches!(ops.get(6), Some(SceneOp::WriteMapChunks { chunks })
                    if chunks.as_ref() == [(57, 90, 0xD3)]),
                "Machine Center refresh beat changed (#546)",
            );
        }
        "Event_EclipseTorchUsed" => {
            check(
                jsr_target(rom, 0x07_043C) == Some(DMA_HELPERS[2])
                    && jsr_target(rom, 0x07_0436) == Some(0x05_40FA),
                "Eclipse Torch refresh/DMA changed",
            );
            check(
                matches!(
                    ops.get(36),
                    Some(SceneOp::CopyChunkDefinition {
                        source_addr: 0xFFFF_6100,
                        dest_addr: 0xFFFF_6960,
                        words: 16,
                    })
                ) && matches!(
                    ops.get(37),
                    Some(SceneOp::Presentation {
                        op: PresentationOp::ReloadMapChunks
                    })
                ) && matches!(
                    ops.get(43),
                    Some(SceneOp::CopyChunkDefinition {
                        source_addr: 0xFFFF_6100,
                        dest_addr: 0xFFFF_6960,
                        words: 16,
                    })
                ),
                "Eclipse Torch chunk copy / RefreshPlaneBG ops changed",
            );
            check(
                waits(ops, 38..40) == [(38, 1, false)],
                "Eclipse Torch's WaitFrames(1) already spends the DMA frame",
            );
        }
        "Event_WreckageEngine" => {
            check(
                jsr_target(rom, 0x06_DD3E) == Some(DMA_HELPERS[1])
                    && jsr_target(rom, 0x06_DD34) == Some(0x05_4098)
                    && word(rom, 0x06_DD06) == 0x7007
                    && dbf(rom, 0x06_DD62, 0, 0x06_DD08),
                "eight-pass Wreckage DMA loop changed",
            );
            for at in [0x06_DD44, 0x06_DD4A, 0x06_DD50] {
                check(
                    jsr_target(rom, at) == Some(VINT_PREPARE),
                    "Wreckage phase must have three bare VInts",
                );
            }
            check(
                waits(ops, 23..39) == (0..8).map(|i| (24 + i * 2, 4, false)).collect::<Vec<_>>(),
                "Wreckage waits must spend DMA + three VInts, eight times",
            );
            check(
                (0..8).all(|i| matches!(ops.get(23 + i * 2), Some(SceneOp::WriteMapChunks { .. }))),
                "Wreckage must write before each four-frame phase",
            );
        }
        "Event_JuzaDefeated" => {
            check(
                jsr_target(rom, 0x06_FBB0) == Some(DMA_HELPERS[2])
                    && dbf(rom, 0x06_FBB6, 0, 0x06_FBB0)
                    && jsr_target(rom, 0x06_FBA6) == Some(0x05_40FA),
                "Juza table refresh/DMA loop changed",
            );
            check(
                ops.len() == 18,
                "Juza must have eight write/wait phases before the flag",
            );
            for i in 0..8 {
                let table = 0x06_FBC4 + i * 5;
                let frames = u16::from(rom[table]) + 1;
                let chunks: Vec<_> = [(12, 9), (13, 9), (12, 10), (13, 10)]
                    .into_iter()
                    .enumerate()
                    .map(|(j, (x, y))| (x, y, u16::from(rom[table + 1 + j])))
                    .collect();
                let correct = match ops.get(1 + i * 2) {
                    Some(SceneOp::WriteMapChunks { chunks: got }) if i % 2 == 0 => {
                        got.as_ref() == chunks.as_slice()
                    }
                    Some(SceneOp::RestoreMapChunks { chunks: got }) if i % 2 == 1 => {
                        got.as_ref() == chunks.as_slice()
                    }
                    _ => false,
                };
                check(
                    correct,
                    "Juza table row must write/restore the US chunks in order",
                );
                check(
                    ops.get(2 + i * 2) == Some(&SceneOp::WaitFrames { frames }),
                    "Juza table row must spend delay + 1 DMA frames without updates",
                );
            }
            check(rom[0x06_FBEC] == 0xFF, "Juza table terminator changed");
            check(
                matches!(ops.last(), Some(SceneOp::SetFlag { flag, value: true })
                if *flag == crate::Flag::event(0x48)),
                "Juza flag must follow the final delay",
            );
        }
        _ => check(false, "no layout/animation frame classification"),
    }
    errors
}

#[cfg(test)]
mod tests;
