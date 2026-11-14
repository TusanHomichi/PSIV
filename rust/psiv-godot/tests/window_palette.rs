//! The native windows' palette-line binding (#533): every window family
//! draws its line-2 art through one [`WindowLine`], and a CRAM change reaches
//! the art on line 2 and nothing else. Imports the production module without
//! starting Godot; the pack-backed checks skip without a local pack.

#[allow(dead_code)]
#[path = "../src/window_palette.rs"]
mod window_palette;

use std::collections::BTreeSet;
use std::path::Path;

use psiv_data::color::cram_rgb;
use window_palette::{WINDOW_LINE, WINDOW_LINE_ROM, WindowFamily, WindowLine, WindowNode};

/// A binding under test: the line a window draws with from `words`
/// (`Palette_Table_Buffer`), whether they are live, and the baked words.
type Binding = fn(&[u16; 64], bool, &[u16]) -> Option<WindowLine>;

/// The production binding.
fn production(words: &[u16; 64], live: bool, baked: &[u16]) -> Option<WindowLine> {
    WindowLine::new(words, live, baked)
}

/// A deliberately wrong binding: the window art on line 3.
fn on_line_3(words: &[u16; 64], live: bool, baked: &[u16]) -> Option<WindowLine> {
    let line: [u16; 16] = words[48..64].try_into().ok()?;
    let baked: [u16; 16] = baked.get(..16)?.try_into().ok()?;
    Some(WindowLine {
        baked,
        live: if live { line } else { baked },
    })
}

/// Sixteen distinct CRAM words for `line`: word `i` is `0BBB0GGG0RRR0` with
/// red `i & 7`, green `i >> 3` and blue the line number, so no two of the
/// 64 share a colour.
fn line_words(line: u16) -> [u16; 16] {
    std::array::from_fn(|i| {
        let i = i as u16;
        ((i & 7) << 1) | ((i >> 3) << 5) | (line << 9)
    })
}

fn cram() -> [u16; 64] {
    let mut words = [0; 64];
    for line in 0..4 {
        words[line * 16..line * 16 + 16].copy_from_slice(&line_words(line as u16));
    }
    words
}

fn rgba(word: u16, alpha: u8) -> [u8; 4] {
    let [r, g, b] = cram_rgb(word);
    [r, g, b, alpha]
}

/// Art baked in line 2's words: one opaque pixel per index 1-15, a hole of
/// index 3's colour (alpha 0), and a colour outside line 2 (line 0's word 9).
fn art(baked: &[u16; 16]) -> Vec<[u8; 4]> {
    let mut pixels: Vec<[u8; 4]> = (1..16).map(|i| rgba(baked[i], 255)).collect();
    pixels.push(rgba(baked[3], 0));
    pixels.push(rgba(line_words(0)[9], 255));
    pixels
}

fn draw(line: &WindowLine, pixels: &[[u8; 4]]) -> Vec<[u8; 4]> {
    pixels.iter().map(|&pixel| line.colour(pixel)).collect()
}

/// What goes wrong when `bind` draws the art at rest, after a change to word
/// 5 of line 2, after a change to each other line, and with no live palette.
fn binding_failures(bind: Binding) -> Vec<&'static str> {
    let mut failures = Vec::new();
    let words = cram();
    let baked = line_words(2);
    let pixels = art(&baked);
    let Some(rest) = bind(&words, true, &baked) else {
        return vec!["no line"];
    };
    if draw(&rest, &pixels) != pixels {
        failures.push("at rest the art is not its baked colours");
    }

    let mut changed = words;
    changed[WINDOW_LINE * 16 + 5] = 0x0EEE;
    let drawn = bind(&changed, true, &baked).map(|line| draw(&line, &pixels));
    let expected: Vec<[u8; 4]> = pixels
        .iter()
        .map(|&pixel| {
            if pixel[..3] == rgba(baked[5], 0)[..3] {
                rgba(0x0EEE, pixel[3])
            } else {
                pixel
            }
        })
        .collect();
    if drawn.as_ref() != Some(&expected) {
        failures.push("a line-2 change does not reach exactly its index");
    }

    for line in [0, 1, 3] {
        let mut other = words;
        for word in &mut other[line * 16..line * 16 + 16] {
            *word ^= 0x0222;
        }
        if bind(&other, true, &baked).map(|line| draw(&line, &pixels)) != Some(pixels.clone()) {
            failures.push("another line's change reaches the window");
            break;
        }
    }

    if bind(&changed, false, &baked).map(|line| draw(&line, &pixels)) != Some(pixels.clone()) {
        failures.push("with no live palette the art is not its baked colours");
    }
    failures
}

#[test]
fn a_window_draws_line_2_and_nothing_else() {
    assert_eq!(binding_failures(production), Vec::<&str>::new());
}

/// The negative control: the same checks reject art bound to another line.
#[test]
fn a_window_bound_to_another_line_is_rejected() {
    let failures = binding_failures(on_line_3);
    assert!(
        failures.contains(&"a line-2 change does not reach exactly its index"),
        "{failures:?}"
    );
    assert!(
        failures.contains(&"another line's change reaches the window"),
        "{failures:?}"
    );
}

#[test]
fn the_shader_rule_keeps_alpha_and_never_looks_up_index_0() {
    let mut baked = line_words(2);
    // Index 0 and index 1 share black, as `Pal_Init_Line_3` does.
    baked[0] = 0;
    baked[1] = 0;
    let mut live = baked;
    live[0] = 0x0E00;
    live[1] = 0x000E;
    let line = WindowLine { baked, live };
    assert_eq!(line.colour([0, 0, 0, 255]), rgba(0x000E, 255));
    assert_eq!(line.colour([0, 0, 0, 0]), rgba(0x000E, 0));
    assert_eq!(line.colour(rgba(baked[7], 128)), rgba(baked[7], 128));
    // Fewer than sixteen baked words: no binding.
    assert_eq!(WindowLine::new(&cram(), true, &baked[..15]), None);
}

/// Each family's node: the choice is part of the message box's draw list,
/// and every window node is some family's.
#[test]
fn each_window_family_is_drawn_by_a_node_carrying_the_line() {
    let nodes: Vec<(WindowFamily, WindowNode)> = WindowFamily::ALL
        .iter()
        .map(|&family| (family, family.node()))
        .collect();
    assert_eq!(
        nodes,
        [
            (WindowFamily::Dialogue, WindowNode::Dialogue),
            (WindowFamily::Choice, WindowNode::Dialogue),
            (WindowFamily::Camp, WindowNode::Camp),
            (WindowFamily::Shop, WindowNode::Shop),
        ]
    );
}

// The pack
// ---------------------------------------------------------------------------

fn pack() -> Option<&'static Path> {
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    if pack.join("manifest.json").is_file() {
        Some(pack)
    } else {
        eprintln!("runtime pack not present; skipping");
        None
    }
}

fn json(pack: &Path, file: &str) -> serde_json::Value {
    let bytes = std::fs::read(pack.join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"))
}

/// An indexed PNG's `PLTE` colours.
fn plte(path: &Path) -> Vec<[u8; 3]> {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut at = 8;
    while at + 8 <= data.len() {
        let size = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        if &data[at + 4..at + 8] == b"PLTE" {
            let (colours, _) = data[at + 8..at + 8 + size].as_chunks::<3>();
            return colours.to_vec();
        }
        at += size + 12;
    }
    panic!("{} has no palette", path.display());
}

/// Every PNG a family draws, with the CRAM line its pack record names: the
/// art's sixteen colours sit at `16 * line` in its palette when the PNG is
/// laid out by CRAM slot (the STATUS portraits), at 0 otherwise.
fn family_art(pack: &Path, family: WindowFamily) -> Vec<(String, usize, bool)> {
    let window = json(pack, "dialogue/window.json");
    let line = |record: &serde_json::Value| {
        record["cram_line"]
            .as_u64()
            .unwrap_or_else(|| panic!("no cram_line in {record}")) as usize
    };
    let window_line = line(&window["palette"]);
    let strip = window["png"].as_str().unwrap().to_owned();
    let mut art = vec![(strip, window_line, false)];
    match family {
        WindowFamily::Dialogue => {
            let font = json(pack, "dialogue/font.json");
            art.push((font["png"].as_str().unwrap().to_owned(), window_line, false));
            art.push((
                window["scroll_arrow"]["png"].as_str().unwrap().to_owned(),
                window_line,
                false,
            ));
            let portraits = json(pack, "dialogue/portraits.json");
            let portrait_line = line(&portraits["palette"]);
            for portrait in portraits["portraits"].as_array().unwrap() {
                art.push((
                    portrait["png"].as_str().unwrap().to_owned(),
                    portrait_line,
                    false,
                ));
            }
        }
        WindowFamily::Choice => {
            art.push(("dialogue/menu_font.png".to_owned(), window_line, false));
        }
        WindowFamily::Camp => {
            art.push(("dialogue/menu_font.png".to_owned(), window_line, false));
            let characters = json(pack, "battle/characters.json");
            for character in characters["characters"].as_array().unwrap() {
                let portrait = &character["status_portrait"];
                if portrait.is_object() {
                    art.push((
                        portrait["png"].as_str().unwrap().to_owned(),
                        line(portrait),
                        true,
                    ));
                }
            }
        }
        WindowFamily::Shop => {
            art.push(("dialogue/menu_font.png".to_owned(), window_line, false));
            for portrait in json(pack, "shops.json")["portraits"].as_array().unwrap() {
                art.push((
                    portrait["png"].as_str().unwrap().to_owned(),
                    line(portrait),
                    false,
                ));
            }
        }
    }
    art
}

/// Every family's art is baked in the words the window shader is given as
/// `baked` (`Pal_Init_Line_3`), on the line the binding reads, and those
/// words' colours 1-15 are distinct, so each pixel finds its own index.
#[test]
fn every_family_draws_art_baked_in_the_bound_line() {
    let Some(pack) = pack() else {
        return;
    };
    let data = psiv_data::GameData::load(pack).expect("pack loads");
    let baked = data
        .scene_palettes()
        .words(WINDOW_LINE_ROM)
        .expect("the pack carries Pal_Init_Line_3");
    let colours: Vec<[u8; 3]> = baked.iter().take(16).map(|&w| cram_rgb(w)).collect();
    assert_eq!(colours.len(), 16);
    let distinct: BTreeSet<[u8; 3]> = colours[1..].iter().copied().collect();
    assert_eq!(distinct.len(), 15, "line 2's colours 1-15 must be distinct");

    let mut seen = 0;
    for family in WindowFamily::ALL {
        let art = family_art(pack, family);
        assert!(art.len() >= 2, "{family:?} draws no art");
        for (png, line, by_slot) in art {
            assert_eq!(line, WINDOW_LINE, "{family:?} {png} is on line {line}");
            let palette = plte(&pack.join(&png));
            let start = if by_slot { line * 16 } else { 0 };
            assert_eq!(
                palette.get(start..start + 16),
                Some(colours.as_slice()),
                "{family:?} {png} is not baked in Pal_Init_Line_3"
            );
            seen += 1;
        }
    }
    assert!(seen > 30, "only {seen} PNGs checked");
}

/// After the field boot's map load, line 2 holds the words the art is baked
/// in, so the windows draw their baked colours at rest; line 3, where the
/// wrong binding reads, does not.
#[test]
fn a_map_load_puts_the_baked_words_on_the_bound_line() {
    let Some(pack) = pack() else {
        return;
    };
    let data = psiv_data::GameData::load(pack).expect("pack loads");
    let baked = data
        .scene_palettes()
        .words(WINDOW_LINE_ROM)
        .expect("the pack carries Pal_Init_Line_3")
        .to_vec();
    let session = psiv_runtime::Session::start(data)
        .field()
        .expect("field boots");
    let view = session.runtime().cram();
    assert!(view.live, "the field boot's map has a live palette");
    let line = WindowLine::new(&view.words, view.live, &baked).expect("bound");
    assert_eq!(
        line.live, line.baked,
        "the windows are not their baked colours at rest"
    );
    let wrong = on_line_3(&view.words, view.live, &baked).expect("bound");
    assert_ne!(
        wrong.live, wrong.baked,
        "line 3 holds the window colours too"
    );
}
