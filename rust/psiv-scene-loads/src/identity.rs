//! US routine identities. Only offsets, lengths and SHA-256 digests are
//! stored here; the instruction bytes remain in the user's verified ROM.

use psiv_pack::png::hex;
use sha2::{Digest, Sha256};

const BOUNDARIES: [(usize, usize, &str); 9] = [
    (
        0x06AF9C,
        6,
        "43ceb3ff56d33140d3e81abff6847517cc69df9548caef72c939d3b94036d9cd",
    ),
    (
        0x06B004,
        6,
        "f65594f3860ed31f2bf272331ab96bef48a841e485d563293331f1acfd211c91",
    ),
    (
        0x041740,
        4,
        "eece4fd26a8ece8593cdb628bb0833a3ef1e0a06ea97e3ac8e4a63148496ffd7",
    ),
    (
        0x041784,
        6,
        "5d9e7a8ff20b0aec61176df77d1e367c54aeb10475b80b09c5bd1bb445374570",
    ),
    (
        0x041A90,
        4,
        "9587699b3d0cc8d5fd7a9dc0e1d0f501f40a559c4e2f8370dc85f24add409a31",
    ),
    (
        0x041B50,
        6,
        "885c298ca64aaf4859c62f95289b2d9ac8e7767fd794bf221b5e57b14cead7ad",
    ),
    (
        0x0702CE,
        10,
        "55a91b3ed4f0ca3a7d0c7d088a3750abc4fe2c3f3379c82f6e0813c54f415307",
    ),
    (
        0x0702DE,
        6,
        "f972b611fbd10de64eaab2e0f8ff844c1dd06bbe184f26b38f832d6f90fa16ed",
    ),
    (
        0x070430,
        18,
        "1a0ee4bb910e5f790d97c3503c839dfac1a8e0c496656769ff7689aa9847d237",
    ),
];

fn check(data: &[u8], windows: &[(usize, usize, &str)]) -> Result<(), String> {
    for &(offset, length, expected) in windows {
        let bytes = data
            .get(offset..offset + length)
            .ok_or_else(|| format!("routine boundary {offset:06X} is outside the image"))?;
        if hex(&Sha256::digest(bytes)) != expected {
            return Err(format!(
                "US routine boundary digest mismatch at {offset:06X}"
            ));
        }
    }
    Ok(())
}

pub fn verify(data: &[u8]) -> Result<(), String> {
    check(data, &BOUNDARIES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_or_truncated_window_fails_identity_verification() {
        let data = b"synthetic boundary";
        let digest = hex(&Sha256::digest(&data[2..6]));
        let windows = [(2, 4, digest.as_str())];
        assert!(check(data, &windows).is_ok());
        let mut changed = data.to_vec();
        changed[3] ^= 1;
        assert!(
            check(&changed, &windows)
                .unwrap_err()
                .contains("digest mismatch")
        );
        assert!(check(&data[..5], &windows).unwrap_err().contains("outside"));
    }
}
