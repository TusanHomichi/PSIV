//! Dispatch selection and composition, using only synthetic compressed data.

use psiv_pack::maps::load_work::{AlgorithmWork, Destination, Stage, record_decompression_work};
use psiv_pack::maps::records::{MapRecord, walk_map};

const KOS: usize = 0x1000;
const NEM: usize = 0x1100;
const VEHICLES: usize = 0x51AA6;
const CHESTS: [usize; 3] = [0x2975AE, 0x297674, 0x2976FA];

fn append_art(list: &mut Vec<u8>, word: u16, source: usize) {
    list.extend(word.to_be_bytes());
    list.extend((source as u32).to_be_bytes());
}

fn fixture(id: usize, populated: bool) -> (Vec<u8>, MapRecord) {
    let mut rom = vec![0; 0x298000];
    // Two literal bytes then an end token: 380 intrinsic CPU clocks.
    rom[KOS..KOS + 7].copy_from_slice(&[0x0B, 0, b'a', b'b', 0, 0, 0]);
    // A single one-bit table code writes one solid tile: RAM8718 / VDP8740.
    let nem = [0, 1, 0x85, 0x71, 0, 0xFF, 0, 0, 0];
    for source in [NEM, NEM + 0x10, NEM + 0x20].into_iter().chain(CHESTS) {
        rom[source..source + nem.len()].copy_from_slice(&nem);
    }
    for index in 0..3 {
        rom[VEHICLES + index * 4..VEHICLES + index * 4 + 4]
            .copy_from_slice(&((NEM + index * 0x10) as u32).to_be_bytes());
    }
    let mut data = vec![7, 0x81];
    if populated {
        append_art(&mut data, 1, KOS);
        append_art(&mut data, 0x7FFF, KOS);
    }
    data.extend([0xFF, 0xFF]);
    if populated {
        append_art(&mut data, 0x7FFF, NEM);
        data.extend([0xFF, 0xFE]);
        append_art(&mut data, 0x8001, NEM);
    }
    data.extend([0xFF, 0xFF]);
    if populated {
        append_art(&mut data, 0x8000, KOS);
    }
    data.extend([0xFF, 0xFF]);
    data.extend([1, 2, 3, 4]); // dimensions
    data.extend([0, 0, 0, 1, 0, 1]); // scroll without step counters
    if populated {
        data.extend((KOS as u32).to_be_bytes());
        data.extend((KOS as u32).to_be_bytes());
    }
    data.extend([0xFF, 0xFF]); // chunks
    data.push(0xFF); // map updates
    for _ in 0..5 {
        data.extend([0xFF, 0xFF]); // transitions, transitions2, objects, chests, animations
    }
    if id > 1 {
        data.extend((KOS as u32).to_be_bytes());
        data.extend((KOS as u32).to_be_bytes());
    }
    data.extend((KOS as u32).to_be_bytes()); // dialogue
    data.extend([0xFF, 0xFF]); // interaction areas
    data.push(0xFF); // events
    data.extend((KOS as u32).to_be_bytes()); // palette
    data.extend([0, 1, 2, 0x85]); // flags
    data.extend([0xFF, 0xFF]); // map data manager
    rom[0x100..0x100 + data.len()].copy_from_slice(&data);
    let record = walk_map(&rom, id, 0x100, &[(KOS, 0)]).unwrap();
    (rom, record)
}

#[test]
fn orders_every_selected_call_and_preserves_destinations_and_repeated_sources() {
    let (rom, record) = fixture(2, true);
    let work = record_decompression_work(&rom, &record, 1).unwrap();
    assert!(!work.world_layout_pages_excluded);
    assert_eq!(
        work.streams
            .iter()
            .map(|stream| stream.stage)
            .collect::<Vec<_>>(),
        [
            Stage::Tilesets,
            Stage::Tilesets,
            Stage::Sprites,
            Stage::Sprites,
            Stage::SpriteData,
            Stage::Vehicle,
            Stage::Chunks,
            Stage::Chunks,
            Stage::ChestArt,
            Stage::ChestArt,
            Stage::ForegroundLayout,
            Stage::BackgroundLayout,
            Stage::Dialogue,
        ]
    );
    assert_eq!(work.streams[0].destination, Destination::Ram(0xFFFF6000));
    assert_eq!(work.streams[0].transfer_to_vram, Some(32));
    assert_eq!(work.streams[1].destination, Destination::Ram(0xFFFF6000));
    assert_eq!(work.streams[1].transfer_to_vram, Some(0xFFE0));
    assert_eq!(work.streams[2].destination, Destination::Vram(0xFFE0));
    assert_eq!(work.streams[3].destination, Destination::Ram(0xFFFF0020));
    assert_eq!(work.streams[2].cpu_cycles(), Some(8740));
    assert_eq!(work.streams[3].cpu_cycles(), Some(8718));
    assert_eq!(work.streams[4].destination, Destination::Ram(0xFFFF8000));
    assert_eq!(work.streams[5].destination, Destination::Ram(0xFFFF0000));
    assert_eq!(work.streams[6].destination, Destination::Ram(0xFFFF6000));
    assert_eq!(work.streams[7].destination, Destination::Ram(0xFFFF6002));
    assert_eq!(work.streams[10].destination, Destination::Ram(0xFFFFA000));
    assert_eq!(work.streams[11].destination, Destination::Ram(0xFFFFB000));
    assert_eq!(work.streams[12].destination, Destination::Ram(0xFFFF3000));
    for stream in &work.streams {
        match stream.work {
            AlgorithmWork::Kosinski(_) => {
                assert_eq!((stream.decoded_bytes, stream.consumed_bytes), (2, 7));
                assert_eq!(stream.cpu_cycles(), Some(380));
            }
            AlgorithmWork::Nemesis(_) => {
                assert_eq!((stream.decoded_bytes, stream.consumed_bytes), (32, 9));
                assert_eq!(stream.transfer_to_vram, None);
            }
        }
    }
    assert_eq!(work.cpu_cycles(), Some(46_696));
}

#[test]
fn fixed_chests_are_loaded_even_when_the_record_has_no_objects_or_chests() {
    let (mut rom, mut record) = fixture(2, false);
    let normal = record_decompression_work(&rom, &record, 0).unwrap();
    assert_eq!(normal.streams.len(), 5);
    assert_eq!(normal.streams[0].source, CHESTS[0]);
    assert_eq!(normal.streams[1].source, CHESTS[1]);
    assert_eq!(normal.streams[0].destination, Destination::Vram(0x9B80));
    assert_eq!(normal.streams[1].destination, Destination::Vram(0x9CC0));
    assert_eq!(normal.cpu_cycles(), Some(18_620));
    record.general_var = 0x32;
    rom[CHESTS[2]] = 0x80; // XOR alternate normal-chest art adds 74 clocks.
    let special = record_decompression_work(&rom, &record, 0).unwrap();
    assert_eq!(special.streams[0].source, CHESTS[2]);
    assert_eq!(special.streams[1].source, CHESTS[1]);
    assert_eq!(special.cpu_cycles(), Some(18_694));
}

#[test]
fn each_retained_vehicle_indexes_the_rom_pointer_table_and_zero_skips_it() {
    let (rom, record) = fixture(2, false);
    let baseline = record_decompression_work(&rom, &record, 0).unwrap();
    assert!(
        baseline
            .streams
            .iter()
            .all(|stream| stream.stage != Stage::Vehicle)
    );
    for vehicle in 1_u16..=3 {
        let work = record_decompression_work(&rom, &record, vehicle).unwrap();
        assert_eq!(work.streams[0].stage, Stage::Vehicle);
        assert_eq!(
            work.streams[0].source,
            NEM + usize::from(vehicle - 1) * 0x10
        );
        assert_eq!(work.cpu_cycles(), Some(27_338));
    }
    assert!(record_decompression_work(&[], &record, 4).is_err());
}

#[test]
fn world_planes_are_explicitly_excluded_raw_copies_not_fabricated_kosinski_calls() {
    for id in [0, 1] {
        let (rom, record) = fixture(id, false);
        let work = record_decompression_work(&rom, &record, 0).unwrap();
        assert!(work.world_layout_pages_excluded);
        assert_eq!(work.streams.len(), 3);
        assert_eq!(work.streams[2].stage, Stage::Dialogue);
        assert_eq!(work.cpu_cycles(), Some(17_860));
    }
    let (rom, mut record) = fixture(2, false);
    record.layout.planes = None;
    assert!(record_decompression_work(&rom, &record, 0).is_err());
}

#[test]
fn unsupported_wrapped_nemesis_rows_make_the_sum_unsupported() {
    let (mut rom, record) = fixture(2, true);
    rom[NEM..NEM + 3].copy_from_slice(&[0x20, 0, 0xFF]);
    let work = record_decompression_work(&rom, &record, 0).unwrap();
    let sprite = &work.streams[2];
    assert_eq!(sprite.decoded_bytes, 0);
    assert_eq!(sprite.consumed_bytes, 3);
    assert_eq!(sprite.cpu_cycles(), None);
    assert_eq!(work.cpu_cycles(), None);
    assert!(
        work.streams
            .iter()
            .any(|stream| stream.stage == Stage::Dialogue)
    );
}

#[test]
fn decoder_and_pointer_failures_propagate_with_the_map_stage_and_source() {
    let (rom, mut record) = fixture(2, false);
    record.dialogue.pointer = rom.len();
    let error = record_decompression_work(&rom, &record, 0).unwrap_err().0;
    assert!(
        error.contains("map 0x002 Dialogue stream at 0x298000"),
        "{error}"
    );
    let (mut rom, record) = fixture(2, true);
    rom[NEM..NEM + 5].copy_from_slice(&[0, 1, 0xFF, 0, 0]);
    let error = record_decompression_work(&rom, &record, 0).unwrap_err().0;
    assert!(error.contains("Sprites stream at 0x001100"), "{error}");
    assert!(error.contains("Undefined code"), "{error}");
    let (rom, record) = fixture(2, false);
    let error = record_decompression_work(&rom[..VEHICLES], &record, 1)
        .unwrap_err()
        .0;
    assert!(
        error.contains("vehicle pointer table is truncated"),
        "{error}"
    );
}
