use std::io::{Cursor, Write};

use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use super::*;

#[test]
fn round_trip() {
    let id = Uuid::new_v4();
    let doc = MvrDocument {
        fixtures: vec![MvrFixture {
            uuid: id,
            name: "Spot 1".into(),
            fixture_id: Some("1".into()),
            gdtf_spec: "spot.gdtf".into(),
            gdtf_mode: "Standard".into(),
            universe: Some(1),
            address: Some(101),
            matrix: matrix("1 0 0 0 1 0 0 0 1 1000 2000 3000"),
            layer: None,
            class: None,
        }],
        ..Default::default()
    };
    let parsed = read(&write(&doc).unwrap()).unwrap();
    assert_eq!(parsed.fixtures[0].uuid, id);
    assert_eq!(parsed.fixtures[0].address, Some(101));
    assert_eq!(parsed.fixtures[0].matrix[9], 1000.0);
}

#[test]
fn rejects_unsafe_paths() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "../GeneralSceneDescription.xml",
        SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"<GeneralSceneDescription/>").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(read(&bytes).is_err());
}
