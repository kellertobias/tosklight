use std::io::{Cursor, Write};

use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use super::*;

fn spot(layer: Option<&str>) -> MvrFixture {
    MvrFixture {
        uuid: Uuid::new_v4(),
        name: "Spot 1".into(),
        fixture_id: Some("1".into()),
        gdtf_spec: "Acme@Spot.gdtf".into(),
        gdtf_mode: "Standard".into(),
        universe: Some(2),
        address: Some(101),
        matrix: matrix("1 0 0 0 1 0 0 0 1 1000 2000 3000"),
        layer: layer.map(Into::into),
        class: None,
    }
}

fn scene_xml(archive: &[u8]) -> String {
    let mut zip = ZipArchive::new(Cursor::new(archive)).unwrap();
    let mut xml = String::new();
    zip.by_name("GeneralSceneDescription.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

#[test]
fn round_trip() {
    let doc = MvrDocument {
        fixtures: vec![spot(None)],
        ..Default::default()
    };
    let parsed = read(&write(&doc).unwrap()).unwrap();
    assert_eq!(parsed.fixtures[0].uuid, doc.fixtures[0].uuid);
    assert_eq!(parsed.fixtures[0].universe, Some(2));
    assert_eq!(parsed.fixtures[0].address, Some(101));
    assert_eq!(parsed.fixtures[0].matrix, doc.fixtures[0].matrix);
    assert_eq!(parsed.fixtures[0].fixture_id.as_deref(), Some("1"));
}

/// Everything MVR 1.6 makes mandatory, which a strict reader refuses the archive without.
#[test]
fn the_scene_carries_every_node_mvr_makes_mandatory() {
    let doc = MvrDocument {
        fixtures: vec![spot(Some("Truss")), spot(None)],
        ..Default::default()
    };
    let xml = scene_xml(&write(&doc).unwrap());

    assert!(
        xml.contains("verMajor=\"1\" verMinor=\"6\" provider=\"ToskLight\" providerVersion=\"")
    );
    assert_eq!(xml.matches("<Layer uuid=\"").count(), 2, "{xml}");
    assert!(xml.contains("name=\"Truss\""));
    assert!(xml.contains("name=\"Default\""));
    assert!(
        xml.contains("<Matrix>{1,0,0}{0,1,0}{0,0,1}{1000,2000,3000}</Matrix>"),
        "{xml}"
    );
    assert!(xml.contains("<FixtureIDNumeric>1</FixtureIDNumeric>"));
    assert!(
        xml.contains("<Address break=\"0\">613</Address>"),
        "universe 2 address 101 is absolute address 613: {xml}"
    );
    let spec = xml.find("<GDTFSpec>").unwrap();
    assert!(xml.find("<Matrix>").unwrap() < spec && spec < xml.find("<FixtureID>").unwrap());
}

#[test]
fn layer_uuids_are_stable_between_exports() {
    let doc = MvrDocument {
        fixtures: vec![spot(Some("Truss"))],
        ..Default::default()
    };
    let layer = |xml: String| xml[xml.find("<Layer uuid=\"").unwrap()..][..50].to_owned();
    assert_eq!(
        layer(scene_xml(&write(&doc).unwrap())),
        layer(scene_xml(&write(&doc).unwrap()))
    );
}

#[test]
fn an_empty_rig_still_has_a_layer() {
    let xml = scene_xml(&write(&MvrDocument::default()).unwrap());
    assert!(xml.contains("<Layer uuid=\""), "{xml}");
}

#[test]
fn archive_members_keep_the_exact_name_the_scene_references() {
    let mut doc = MvrDocument {
        fixtures: vec![spot(None)],
        ..Default::default()
    };
    doc.files.insert("Acme@Spot.gdtf".into(), b"gdtf".to_vec());
    let archive = write(&doc).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(archive.as_slice())).unwrap();
    assert!(zip.by_name("Acme@Spot.gdtf").is_ok());
}

#[test]
fn reads_absolute_and_dotted_addresses() {
    let scene = |address: &str| {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("GeneralSceneDescription.xml", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(
            format!(
                "<GeneralSceneDescription><Scene><Layers><Layer uuid=\"{}\"><ChildList>\
                 <Fixture uuid=\"{}\" name=\"A\"><Addresses><Address break=\"0\">{address}\
                 </Address></Addresses></Fixture></ChildList></Layer></Layers></Scene>\
                 </GeneralSceneDescription>",
                Uuid::new_v4(),
                Uuid::new_v4()
            )
            .as_bytes(),
        )
        .unwrap();
        let fixture = read(&zip.finish().unwrap().into_inner()).unwrap().fixtures[0].clone();
        (fixture.universe, fixture.address)
    };
    assert_eq!(scene("613"), (Some(2), Some(101)));
    assert_eq!(scene("512"), (Some(1), Some(512)));
    assert_eq!(scene("2.101"), (Some(2), Some(101)));
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
