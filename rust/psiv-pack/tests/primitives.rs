//! Frozen synthetic calls from TestEnigmaHeader, TestEnigmaDecoder and
//! TestNemesisDecoder (#558). No ROM, Python or reference checkout is needed.
//! The vectors retain each original test name, output and consumed length,
//! including refusals, straddling values, all inline widths and chained streams.

use psiv_pack::{
    decode::{enigma, nemesis},
    gfx, png,
};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/primitives.json")).unwrap()
}

fn bytes(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn python_decoder_acceptance_outputs_and_consumed_boundaries() {
    let fixture = fixture();
    let cases = fixture["decoders"].as_array().unwrap();
    assert_eq!(cases.len(), 99);
    let mut refused = 0;
    for case in cases {
        let data = bytes(case["data"].as_str().unwrap());
        let offset = case["offset"].as_u64().unwrap() as usize;
        let base = case["base_tile"].as_u64().unwrap_or(0) as u16;
        let result = match case["kind"].as_str().unwrap() {
            "enigma_header" => enigma::read_header(&data, offset, base).map(|h| {
                serde_json::json!({"header": [u16::from(h.inline_bits), u16::from(h.flags),
                    h.incremental_word, h.literal_word, h.base_tile]})
            }),
            "enigma" => enigma::decompress(
                &data,
                offset,
                base,
                case["max_words"].as_u64().map(|n| n as usize),
            )
            .map(|(out, n)| serde_json::json!({"output": out, "consumed": n})),
            "nemesis" => nemesis::decompress(&data, offset)
                .map(|(out, n)| serde_json::json!({"output": png::hex(&out), "consumed": n})),
            kind => panic!("unknown vector kind {kind}"),
        };
        if case["refused"] == true {
            refused += 1;
            assert!(
                result.is_err(),
                "{} accepted a refused stream",
                case["name"]
            );
        } else {
            let actual = result.unwrap_or_else(|e| panic!("{}: {e}", case["name"]));
            for (key, value) in actual.as_object().unwrap() {
                assert_eq!(value, &case[key], "{}: {key}", case["name"]);
            }
            if case["kind"] == "enigma" {
                let n = actual["output"].as_array().unwrap().len();
                assert!(
                    enigma::decompress(&data, offset, base, Some(n)).is_ok(),
                    "{} must fit its exact destination",
                    case["name"]
                );
                if n > 0 {
                    assert!(
                        enigma::decompress(&data, offset, base, Some(n - 1)).is_err(),
                        "{} must refuse a destination one word short",
                        case["name"]
                    );
                }
            }
        }
    }
    assert!(refused >= 14, "refusal cases must run too");
}

#[test]
fn image_identity_ignores_filters_compression_chunks_and_annotations() {
    let fixture = fixture();
    let cases = fixture["png"].as_array().unwrap();
    assert_eq!(cases.len(), 40);
    for case in cases {
        let data = bytes(case["data"].as_str().unwrap());
        assert_eq!(
            png::image_sha256(&data).unwrap(),
            case["digest"].as_str().unwrap(),
            "{}",
            case["name"]
        );
        let image = png::decode(&data).unwrap();
        let expected = if image.colour == 3 {
            vec![0, 1, 2, 3, 3, 2, 1, 0, 1, 1, 2, 2, 3, 3, 0, 0]
        } else {
            (0..12).collect()
        };
        assert_eq!(image.pixels, expected, "{}", case["name"]);
    }
}

#[test]
fn cram_conversion_preserves_levels_unused_bits_and_receipt_ramps() {
    // Literal receipt expectations, independent of the production ramp tables.
    let red_blue = [0, 32, 65, 98, 139, 172, 205, 238];
    let green = [0, 32, 68, 101, 137, 170, 206, 238];
    for word in 0..=u16::MAX {
        let color = gfx::Color::decode(word);
        let [r, g, b] = [
            ((word >> 1) & 7) as usize,
            ((word >> 5) & 7) as usize,
            ((word >> 9) & 7) as usize,
        ];
        assert_eq!(color.raw, word);
        assert_eq!(color.levels, [r as u8, g as u8, b as u8]);
        assert_eq!(color.rgb(), [red_blue[r], green[g], red_blue[b]]);
        let record = psiv_pack::json::dumps(&color.to_json(), true);
        assert_eq!(record.contains("unused_bits_set"), word & !0x0EEE != 0);
    }
    for level in 0..8 {
        for (channel, ramp) in [('r', red_blue), ('g', green), ('b', red_blue)] {
            assert_eq!(
                gfx::expand_channel(level, channel).unwrap(),
                ramp[level as usize]
            );
        }
    }
    assert!(gfx::expand_channel(8, 'r').is_err());
    assert!(gfx::expand_channel(1, 'x').is_err());
    assert_eq!(
        gfx::palette_rgb(&gfx::decode_palette(&[0, 0, 0x0E, 0xEE]).unwrap()),
        [[0, 0, 0], [238, 238, 238]]
    );
    assert!(gfx::decode_palette(&[0]).is_err());
}

#[test]
fn tile_plane_and_sheet_pixels_keep_order_flips_palette_and_vram_origin() {
    let packed: Vec<u8> = (0..32)
        .map(|i| ((i * 2 % 16) << 4) | ((i * 2 + 1) % 16))
        .collect();
    let tile = gfx::decode_tile(&packed).unwrap();
    assert_eq!(tile, std::array::from_fn(|i| (i % 16) as u8));
    assert!(gfx::decode_tile(&packed[..31]).is_err());
    assert!(gfx::decode_tiles(&[0; 33]).is_err());
    let tiles = [tile, [7; 64]];
    for line in 0..4 {
        for flip in [0, 0x800, 0x1000, 0x1800] {
            let word = 0x123 | (line << 13) | flip | 0x8000;
            let (w, h, pixels) = gfx::compose(&[word, 0x124], 2, &tiles, 0x123).unwrap();
            assert_eq!((w, h), (16, 8));
            for y in 0..8 {
                for x in 0..8 {
                    let sx = if flip & 0x800 != 0 { 7 - x } else { x };
                    let sy = if flip & 0x1000 != 0 { 7 - y } else { y };
                    let p = tile[sy * 8 + sx];
                    assert_eq!(
                        pixels[y * w + x],
                        if p == 0 { 0 } else { (line * 16) as u8 + p }
                    );
                    assert_eq!(pixels[y * w + x + 8], 7);
                }
            }
        }
    }
    for words in [&[0x122][..], &[0x125][..]] {
        assert!(gfx::compose(words, 1, &tiles, 0x123).is_err());
    }
    assert!(gfx::compose(&[0x123], 2, &tiles, 0x123).is_err());
    assert!(gfx::compose(&[0x123], 0, &tiles, 0x123).is_err());
    assert!(gfx::compose(&[], 1, &tiles, 0x123).is_err());
    let tiles: Vec<_> = (0..5).map(|n| [n; 64]).collect();
    let (w, h, pixels) = gfx::compose_sheet(&tiles, 2).unwrap();
    assert_eq!((w, h), (16, 24));
    for y in 0..h {
        for x in 0..w {
            let n = (y / 8) * 2 + x / 8;
            assert_eq!(pixels[y * w + x], if n < 5 { n as u8 } else { 0 });
        }
    }
    let data = gfx::render_sheet(&tiles, &[[1, 2, 3]], 2, Some(0)).unwrap();
    let image = png::decode(&data).unwrap();
    assert_eq!(image.pixels, pixels);
    assert_eq!(image.plte.len(), 16 * 3);
    assert_eq!(&image.plte[..3], &[1, 2, 3]);
    assert!(image.plte[3..].iter().all(|&b| b == 0));
    assert_eq!(image.trns, [0]);
}
