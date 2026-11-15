//! Symbol-oracle and retail instruction contracts from test_layouts.py.
//! Check individual operations/operands, never a copied cartridge fixture.
use super::*;

fn source(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/ps4disasm")
        .join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("local disassembly absent: layout symbol tests skipped");
            None
        }
        Err(e) => panic!("cannot read {}: {e}", path.display()),
    }
}

#[test]
fn reference_documents_layout_ram_chunk_geometry_and_collision_dispatch() {
    let Some(constants) = source("ps4.constants.asm") else {
        return;
    };
    assert!(constants.contains("Map_Layout_FG = ramaddr($FFFFA000)"));
    assert!(constants.contains("Map_Layout_BG = ramaddr($FFFFB000)"));
    let chunk = constants
        .lines()
        .find(|l| l.starts_with("Chunk_Table ="))
        .unwrap();
    for text in [
        "$FFFF6000",
        "32 bytes (16 words) per definition",
        "bit 6 is the collision flag",
    ] {
        assert!(chunk.contains(text));
    }
    let asm = source("ps4.asm").expect("reference source");
    let collision = asm
        .split_once("GetChunkAndCollision:")
        .unwrap()
        .1
        .split("RunMapTransitions:")
        .next()
        .unwrap();
    for text in [
        "1 = map change",
        "2 = recovery",
        "8 = solid; 9 = water; $A = sand; $B = ice block; $C = shop",
    ] {
        assert!(collision.contains(text));
    }
    let table = asm
        .split_once("TileCollNormalPtrs:")
        .unwrap()
        .1
        .split("; =")
        .next()
        .unwrap();
    let entries: Vec<_> = table
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("dc.l"))
        .map(|l| l.split_whitespace().next().unwrap().trim_end_matches(';'))
        .collect();
    assert_eq!(entries.len(), 16);
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(
            is_blocking(i as u8),
            matches!(*entry, "TileColl_Solid" | "TileColl_Shop")
        );
    }
    for value in [8, 9, 10, 11, 12] {
        assert!(collision.contains(&collision_type_name(value).replace('_', " ")));
    }
    assert!(asm.contains("andi.w\t#$BFFF, d3\t; remove collision bit"));
}

#[test]
fn reference_loader_uses_kosinski_and_reads_dimension_fields_in_record_order() {
    let Some(asm) = source("ps4.asm") else { return };
    let chunks = asm
        .split_once("Map_LoadChunks:")
        .unwrap()
        .1
        .split("loc_51AFC:")
        .next()
        .unwrap();
    assert!(chunks.contains("lea\t(Chunk_Table).l, a1"));
    assert!(chunks.contains("jsr\t(KosDecomp).l"));
    assert!(!chunks.contains("NemDecomp") && !chunks.contains("Enigma"));
    for (label, plane) in [
        ("loc_539E2:", "Map_Layout_FG"),
        ("loc_53A04:", "Map_Layout_BG"),
    ] {
        let body = &asm.split_once(label).unwrap().1[..400];
        assert!(body.contains("andi.w\t#$FFFE, d0"));
        assert!(body.contains(&format!("lea\t({plane}).w, a1")));
        assert!(body.contains("jsr\t(KosDecomp).l"));
    }
    let tiles = asm
        .split_once("loc_519D2:")
        .unwrap()
        .1
        .split("loc_51A14:")
        .next()
        .unwrap();
    for text in [
        "lea\t(Chunk_Table).l, a1",
        "jsr\t(KosDecomp).l",
        "lea\t$6(a0), a0",
    ] {
        assert!(tiles.contains(text));
    }
    let loader = asm
        .split_once("GameMode_LoadFieldMap:")
        .unwrap()
        .1
        .split("Map_LoadChunks).l")
        .next()
        .unwrap();
    let fields: Vec<_> = loader
        .lines()
        .filter(|l| l.contains("move.b\t(a0)+, (Map_"))
        .map(|l| l.split_once(", (").unwrap().1.split(')').next().unwrap())
        .collect();
    assert_eq!(
        fields,
        [
            "Map_General_Var",
            "Map_Row_Size_FG",
            "Map_Column_Size_FG",
            "Map_Row_Size_BG",
            "Map_Column_Size_BG"
        ]
    );
}

#[test]
fn retail_collision_operations_match_the_bit_weights_mask_and_plane_selection() {
    let Some(r) = super::retail_tests::retail() else {
        return;
    };
    let w = |at| crate::m68k::w(r.rom.bytes(), at).unwrap();
    let l = |at| crate::m68k::l(r.rom.bytes(), at).unwrap();
    // loc_45B0A: clear d4; four btst #6,disp(a1) / beq / addq.w; rts.
    assert_eq!(w(0x45b0a), 0x7800); // moveq #0,d4
    for (i, displacement) in [0, 2, 8, 10].into_iter().enumerate() {
        let at = 0x45b0c + i * 10;
        assert_eq!(w(at), 0x0829); // btst immediate, d16(a1)
        assert_eq!(w(at + 2), 6); // bit 6 of the high byte == bit 14
        assert_eq!(w(at + 4), displacement);
        assert_eq!(w(at + 6), 0x6702); // beq over the addq
        let weight = (1 << i) & 7; // ADDQ encodes 8 as zero
        assert_eq!(w(at + 8), 0x5044 | (weight << 9)); // addq.w #weight,d4
    }
    assert_eq!(w(0x45b34), 0x4e75); // rts
    for at in [0x54492, 0x544b2] {
        assert_eq!(w(at), 0x0243); // andi.w immediate,d3
        assert_eq!(w(at + 2), vdp_word(u16::MAX));
    }
    // loc_45AD0: tst.b EC24 / bne / addi.l FG / bra / addi.l BG.
    assert_eq!(w(0x45acc), 0x4a38);
    assert_eq!(w(0x45ace), 0xec24);
    assert_eq!(w(0x45ad0), 0x6608);
    assert_eq!(w(0x45ad2), 0x0683);
    assert_eq!(l(0x45ad4), 0xffffa000);
    assert_eq!(w(0x45ad8), 0x6006);
    assert_eq!(w(0x45ada), 0x0683);
    assert_eq!(l(0x45adc), 0xffffb000);
    assert_eq!(LAYOUT_RAM_BYTES, (l(0x45adc) - l(0x45ad4)) as usize);
    // GetChunkAndCollision crosses row sizes; SetupChunks draws each plane
    // with its own. This dormant retail quirk is documented in EXTRACTION.
    assert_eq!(w(0x45ab8), 0x1838); // move.b abs.w,d4
    assert_eq!(w(0x45aba), 0xec64);
    assert_eq!(w(0x45abc), 0x4a38); // tst.b EC24
    assert_eq!(w(0x45abe), 0xec24);
    assert_eq!(w(0x45ac0), 0x6604); // bne over alternate row size
    assert_eq!(w(0x45ac2), 0x1838);
    assert_eq!(w(0x45ac4), 0xec66);
    for (at, row, base) in [(0x54356, 0xec64, 0xffffa000), (0x543a6, 0xec66, 0xffffb000)] {
        assert_eq!(w(at), 0x1838);
        assert_eq!(w(at + 2), row);
        assert_eq!(w(at + 4), 0x5244); // addq.w #1,d4
        assert_eq!(w(at + 6), 0xc6c4); // mulu.w d4,d3
        assert_eq!(w(at + 8), 0xd645); // add.w d5,d3
        assert_eq!(w(at + 10), 0x0683); // addi.l base,d3
        assert_eq!(l(at + 12), base);
    }
    for m in &r.maps {
        assert_eq!(m.fg.width_chunks, m.bg.width_chunks);
    }
}
