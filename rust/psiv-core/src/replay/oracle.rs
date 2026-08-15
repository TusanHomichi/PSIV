//! The oracle's own log, and the comparison against it.
//!
//! [`OracleLog`] parses a hardware CSV and indexes it by frame and by column so
//! a diff is a lookup rather than a scan. [`DiffReport`] keeps *compared* and
//! *unavailable* columns apart, which is the difference between a verdict and a
//! false pass.

use core::fmt;
use std::collections::BTreeMap;

use super::columns::all_modelled_columns;
use super::row::ReplayRow;

/// The outcome of a comparison: what disagreed, and over which columns.
#[derive(Debug, Clone)]
pub struct DiffReport {
    /// Every disagreement, ordered by frame then column.
    pub divergences: Vec<Divergence>,
    /// Columns both sides carried, so a clean result over them means something.
    pub compared: Vec<&'static str>,
    /// Columns the engine emits that the oracle log does not carry. These were
    /// not compared, and reporting them as agreement is how a false pass
    /// happens — a whole column group once vanished into a silent `continue`
    /// and the run announced itself clean over columns it had never read.
    pub unavailable: Vec<&'static str>,
}

/// One frame's disagreement between engine and oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    /// The oracle frame it happened on.
    pub frame: u32,
    /// Which column.
    pub column: String,
    /// What the engine said.
    pub engine: String,
    /// What the hardware said.
    pub oracle: String,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frame {}: {} engine={} oracle={}",
            self.frame, self.column, self.engine, self.oracle
        )
    }
}

/// An oracle log, indexed by frame.
///
/// Both indexes are built at parse time. A linear scan per lookup is fine at
/// thirty-odd columns and quadratic at three hundred — the object group made
/// that difference the gap between a second and an afternoon.
#[derive(Debug, Clone, Default)]
pub struct OracleLog {
    rows: Vec<Vec<String>>,
    by_frame: BTreeMap<u32, usize>,
    by_column: BTreeMap<String, usize>,
}

impl OracleLog {
    /// Parses an oracle CSV: `#` comment lines, then a header row, then data.
    #[must_use]
    pub fn parse(text: &str) -> OracleLog {
        let mut header = Vec::new();
        let mut rows = Vec::new();
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let fields: Vec<String> = line.split(',').map(str::to_string).collect();
            if header.is_empty() {
                header = fields;
            } else {
                rows.push(fields);
            }
        }
        let by_column = header
            .iter()
            .enumerate()
            .map(|(index, name)| (name.clone(), index))
            .collect();
        let frame_index = header.iter().position(|name| name == "frame");
        let by_frame = frame_index
            .map(|frame_index| {
                rows.iter()
                    .enumerate()
                    .filter_map(|(row, fields)| {
                        let frame = fields.get(frame_index)?.parse::<u32>().ok()?;
                        Some((frame, row))
                    })
                    .collect()
            })
            .unwrap_or_default();
        OracleLog {
            rows,
            by_frame,
            by_column,
        }
    }

    /// The value of `column` on `frame`, if the log has both.
    #[must_use]
    pub fn get(&self, frame: u32, column: &str) -> Option<&str> {
        let column = *self.by_column.get(column)?;
        let row = *self.by_frame.get(&frame)?;
        self.rows.get(row)?.get(column).map(String::as_str)
    }

    /// How many data rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the log has no data rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Compares engine rows against this log over the modelled columns.
    ///
    /// `skip` names columns to leave out of the comparison — the caller's
    /// escape hatch for a column known to diverge for a documented reason,
    /// rather than this module quietly excluding it.
    ///
    /// Returns divergences in frame order, first one first.
    #[must_use]
    pub fn diff(&self, rows: &[ReplayRow], skip: &[&str]) -> Vec<Divergence> {
        self.compare(rows, skip).divergences
    }

    /// The full comparison: what diverged, and what was actually looked at.
    ///
    /// A column both sides carry is compared; one the oracle log does not carry
    /// is *unavailable*, not clean. Keeping the two apart is the difference
    /// between a verdict and a false pass — an engine column the log never had
    /// would otherwise vanish into a silent `continue` and be counted as
    /// agreement.
    #[must_use]
    pub fn compare(&self, rows: &[ReplayRow], skip: &[&str]) -> DiffReport {
        let mut divergences = Vec::new();
        let mut compared = Vec::new();
        let mut unavailable = Vec::new();
        for column in all_modelled_columns() {
            if skip.contains(&column) || column == "mark" || column == "frame" {
                continue;
            }
            let mut seen = false;
            for row in rows {
                let (Some(engine), Some(oracle)) = (row.field(column), self.get(row.frame, column))
                else {
                    continue;
                };
                seen = true;
                if engine != oracle {
                    divergences.push(Divergence {
                        frame: row.frame,
                        column: column.to_string(),
                        engine,
                        oracle: oracle.to_string(),
                    });
                }
            }
            if seen {
                compared.push(column);
            } else {
                unavailable.push(column);
            }
        }
        divergences.sort_by(|a, b| a.frame.cmp(&b.frame).then(a.column.cmp(&b.column)));
        DiffReport {
            divergences,
            compared,
            unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_oracle_log_is_indexed_by_frame_and_column() {
        let log = OracleLog::parse(
            "# provenance\n\
             frame,mark,buttons,c1_x_px\n\
             1,start,.,768\n\
             2,,D,770\n",
        );
        assert_eq!(log.len(), 2);
        assert_eq!(log.get(1, "c1_x_px"), Some("768"));
        assert_eq!(log.get(2, "buttons"), Some("D"));
        assert_eq!(log.get(3, "c1_x_px"), None);
        assert_eq!(log.get(1, "nonesuch"), None);
    }
}
