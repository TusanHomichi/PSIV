//! Repair a copied pre-learning native slot without modifying its source file.
use psiv_core::StepFrames;
use psiv_data::{BattleFiles, GameData};
use psiv_runtime::Runtime;
use std::path::Path;

fn main() {
    let source = std::env::args()
        .nth(1)
        .expect("source save directory (slot 1)");
    let output = std::env::args()
        .nth(2)
        .expect("new repaired output directory");
    let source = Path::new(&source);
    let output = Path::new(&output);
    assert!(
        !output.join("slot_1.sram").exists(),
        "output slot must be new"
    );
    let original = std::fs::read(source.join("slot_1.sram")).unwrap();
    let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime-pack"));
    let mut rt = Runtime::load_slot(
        GameData::load(pack).unwrap(),
        source,
        0,
        StepFrames::default(),
    )
    .unwrap();
    rt.enable_battles(&BattleFiles::load(pack).unwrap())
        .unwrap();
    let repairs = rt.repair_legacy_progression().unwrap();
    for repair in &repairs {
        println!("{repair:?}");
    }
    assert!(
        rt.repair_legacy_progression().unwrap().is_empty(),
        "repair must be idempotent"
    );
    let saved = rt.save_slot(output, 0).unwrap();
    assert_eq!(std::fs::read(source.join("slot_1.sram")).unwrap(), original);
    println!(
        "repaired {} characters into {}",
        repairs.len(),
        saved.display()
    );
}
