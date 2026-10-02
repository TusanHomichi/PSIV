//! Repair a copied pre-learning native slot without modifying its source file.
use psiv_runtime::tools::repair_legacy_progression;
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
    // The whole operation is the runtime's (`psiv_runtime::tools`): this
    // example only names the two directories and shows the report.
    let report = repair_legacy_progression(pack, source, output, 0).unwrap();
    for repair in &report.repairs {
        println!("{repair:?}");
    }
    assert_eq!(std::fs::read(source.join("slot_1.sram")).unwrap(), original);
    println!(
        "repaired {} characters into {}",
        report.repairs.len(),
        report.saved.display()
    );
}
