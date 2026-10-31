//! Just enough 68000 to read tables and short routines out of retail:
//! `psiv_tools/m68k.py`, which owns the reasons. Signature counts, big-endian
//! fields, an opcode at an address, a `bra.w` target, a PC-relative jump
//! table and a self-bounded `bra.w` table. Deliberately not a disassembler.

/// A retail image that did not hold what a reader required.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct M68kError(pub String);

impl std::fmt::Display for M68kError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for M68kError {}

impl From<M68kError> for crate::PackError {
    fn from(error: M68kError) -> Self {
        crate::PackError(error.0)
    }
}

fn hex_bytes(signature: &str) -> Vec<u8> {
    assert!(
        signature.len().is_multiple_of(2),
        "a signature is whole bytes: {signature}"
    );
    (0..signature.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&signature[i..i + 2], 16).expect("a hex signature"))
        .collect()
}

/// Every offset at which `signature` (hex) occurs, as `re.finditer` finds
/// them: left to right, and a match resumes the search after itself.
pub fn find_all(rom: &[u8], signature: &str) -> Vec<usize> {
    let pattern = hex_bytes(signature);
    let mut hits = Vec::new();
    if pattern.is_empty() {
        return (0..=rom.len()).collect();
    }
    let mut at = 0;
    while at + pattern.len() <= rom.len() {
        if rom[at..at + pattern.len()] == pattern[..] {
            hits.push(at);
            at += pattern.len();
        } else {
            at += 1;
        }
    }
    hits
}

/// [`find_all`], refusing any count but `expected`: the pin discipline.
pub fn find_exactly(
    rom: &[u8],
    signature: &str,
    expected: usize,
    label: &str,
) -> Result<Vec<usize>, M68kError> {
    let hits = find_all(rom, signature);
    if hits.len() != expected {
        return Err(M68kError(format!(
            "{label}: signature {signature} occurs {} times, not the {expected} retail has",
            hits.len()
        )));
    }
    Ok(hits)
}

fn field<const N: usize>(rom: &[u8], at: usize) -> Result<[u8; N], M68kError> {
    rom.get(at..at + N)
        .map(|bytes| bytes.try_into().expect("N bytes"))
        .ok_or_else(|| {
            M68kError(format!(
                "0x{at:06X}: a {N}-byte read runs past the end of the image"
            ))
        })
}

/// The big-endian word at `at`.
pub fn w(rom: &[u8], at: usize) -> Result<u16, M68kError> {
    field(rom, at).map(u16::from_be_bytes)
}

/// The signed big-endian word at `at`.
pub fn sw(rom: &[u8], at: usize) -> Result<i16, M68kError> {
    field(rom, at).map(i16::from_be_bytes)
}

/// The big-endian long at `at`.
pub fn l(rom: &[u8], at: usize) -> Result<u32, M68kError> {
    field(rom, at).map(u32::from_be_bytes)
}

/// Require `opcode` as the word at `at`.
pub fn expect(rom: &[u8], at: usize, opcode: u16, what: &str) -> Result<(), M68kError> {
    let found = w(rom, at)?;
    if found != opcode {
        return Err(M68kError(format!(
            "0x{at:06X}: expected {what} (0x{opcode:04X}), found 0x{found:04X}"
        )));
    }
    Ok(())
}

fn offset(at: usize, displacement: i64) -> Result<usize, M68kError> {
    usize::try_from(at as i64 + displacement)
        .map_err(|_| M68kError(format!("0x{at:06X}: a branch target before the image")))
}

/// The target of the `bra.w` at `at`.
pub fn bra_target(rom: &[u8], at: usize) -> Result<usize, M68kError> {
    expect(rom, at, 0x6000, "bra.w")?;
    offset(at + 2, i64::from(sw(rom, at + 2)?))
}

/// The table a `jmp`/`jsr (d8,PC,dN.w)` at `at` dispatches through: only
/// the plain word-indexed form with an eight-bit displacement.
pub fn pc_relative_table(
    rom: &[u8],
    at: usize,
    opcode: u16,
    what: &str,
) -> Result<usize, M68kError> {
    expect(rom, at, opcode, what)?;
    let extension = w(rom, at + 2)?;
    if extension & 0xFF00 != 0 {
        return Err(M68kError(format!(
            "0x{at:06X}: {what} extension 0x{extension:04X} is not the plain word-indexed \
             form the dispatchers use"
        )));
    }
    Ok(at + 2 + usize::from(extension & 0xFF))
}

/// How many `bra.w` entries a table has, from its own lowest target: every
/// entry jumps forward past the table, so it ends where the nearest target
/// begins.
pub fn self_bounded_bra_table(
    rom: &[u8],
    table: usize,
    what: &str,
    limit: usize,
) -> Result<usize, M68kError> {
    let mut lowest: Option<usize> = None;
    let mut count = 0;
    while lowest.is_none_or(|lowest| table + count * 4 < lowest) {
        let entry = table + count * 4;
        let target = bra_target(rom, entry)?;
        if target <= entry {
            return Err(M68kError(format!(
                "{what}: entry {count} jumps backwards to 0x{target:06X}"
            )));
        }
        lowest = Some(lowest.map_or(target, |lowest| lowest.min(target)));
        count += 1;
        if count > limit {
            return Err(M68kError(format!("{what}: no entry bounds the table")));
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_all_matches_finditer() {
        // Non-overlapping, left to right: "aaaa" holds "aa" twice, not three times.
        assert_eq!(find_all(&[0xAA; 4], "aaaa"), [0, 2]);
        assert_eq!(find_all(&[1, 2, 3, 1, 2], "0102"), [0, 3]);
        assert!(find_all(&[1, 2], "0103").is_empty());
        assert!(find_exactly(&[1, 2, 1, 2], "0102", 2, "pin").is_ok());
        let error = find_exactly(&[1, 2], "0102", 2, "pin").unwrap_err();
        assert_eq!(
            error.0,
            "pin: signature 0102 occurs 1 times, not the 2 retail has"
        );
    }

    #[test]
    fn fields_are_big_endian_and_bounded() {
        let rom = [0x12, 0x34, 0xFF, 0xFE];
        assert_eq!(w(&rom, 0), Ok(0x1234));
        assert_eq!(sw(&rom, 2), Ok(-2));
        assert_eq!(l(&rom, 0), Ok(0x1234_FFFE));
        assert!(w(&rom, 3).is_err());
        assert!(l(&rom, 1).is_err());
    }

    #[test]
    fn branches_and_tables() {
        // bra.w +6 at 0 -> 8; bra.w +2 at 4 -> 8.
        let rom = [0x60, 0x00, 0x00, 0x06, 0x60, 0x00, 0x00, 0x02, 0x4E, 0x75];
        assert_eq!(bra_target(&rom, 0), Ok(8));
        assert_eq!(self_bounded_bra_table(&rom, 0, "table", 64), Ok(2));
        assert!(bra_target(&rom, 8).is_err());
        // A backwards entry is refused.
        let back = [0x60, 0x00, 0xFF, 0xFC];
        assert!(self_bounded_bra_table(&back, 0, "table", 64).is_err());
        // jmp (d8,PC,d0.w): opcode $4EFB, extension $0004 -> at + 2 + 4.
        let jump = [0x4E, 0xFB, 0x00, 0x04];
        assert_eq!(pc_relative_table(&jump, 0, 0x4EFB, "jmp"), Ok(6));
        let wide = [0x4E, 0xFB, 0x10, 0x04];
        assert!(pc_relative_table(&wide, 0, 0x4EFB, "jmp").is_err());
        assert!(expect(&jump, 0, 0x4EB9, "jsr").is_err());
    }
}
