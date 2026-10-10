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

fn layer(id: &str, name: &str) -> MvrLayer {
    MvrLayer {
        id: id.into(),
        name: name.into(),
    }
}

fn layer_names(xml: &str) -> Vec<String> {
    xml.match_indices("<Layer uuid=\"")
        .map(|(index, _)| {
            let rest = &xml[index..];
            let start = rest.find("name=\"").unwrap() + 6;
            let end = rest[start..].find('"').unwrap();
            rest[start..start + end].to_owned()
        })
        .collect()
}

#[test]
fn layers_carry_the_names_and_order_the_document_declares() {
    let mut on_truss = spot(Some("truss-id"));
    on_truss.name = "On truss".into();
    let doc = MvrDocument {
        layers: vec![
            layer("floor-id", "Floor"),
            layer("truss-id", "Truss"),
            layer("spare-id", "Spare"),
        ],
        fixtures: vec![on_truss, spot(Some("floor-id")), spot(Some("undeclared"))],
        ..Default::default()
    };
    let xml = scene_xml(&write(&doc).unwrap());

    assert_eq!(
        layer_names(&xml),
        ["Floor", "Truss", "Spare", "undeclared"],
        "declared layers keep their order, an empty one included"
    );
    let truss = xml.find("name=\"Truss\"").unwrap();
    let fixture = xml.find("name=\"On truss\"").unwrap();
    assert!(truss < fixture && fixture < xml[truss..].find("</Layer>").unwrap() + truss);
    assert!(
        !xml.contains("truss-id"),
        "an identity is not a name: {xml}"
    );
}

#[test]
fn layers_that_share_a_name_stay_separate_and_the_default_reads_as_default() {
    let doc = MvrDocument {
        layers: vec![
            layer("a", "Truss"),
            layer("b", "Truss"),
            layer("default", ""),
        ],
        fixtures: vec![spot(Some("a")), spot(Some("b")), spot(None)],
        ..Default::default()
    };
    let xml = scene_xml(&write(&doc).unwrap());
    assert_eq!(layer_names(&xml), ["Truss", "Truss", "Default"]);
}

#[test]
fn reading_keeps_each_layers_name_and_the_fixtures_inside_it() {
    let mut on_truss = spot(Some("truss-id"));
    on_truss.name = "On truss".into();
    let doc = MvrDocument {
        layers: vec![layer("floor-id", "Floor"), layer("truss-id", "Front Truss")],
        fixtures: vec![spot(Some("floor-id")), on_truss],
        ..Default::default()
    };
    let read_back = read(&write(&doc).unwrap()).unwrap();

    let names: Vec<_> = read_back
        .layers
        .iter()
        .map(|layer| layer.name.as_str())
        .collect();
    assert_eq!(names, ["Floor", "Front Truss"]);
    let truss = &read_back.layers[1];
    let fixture = read_back
        .fixtures
        .iter()
        .find(|fixture| fixture.name == "On truss")
        .unwrap();
    assert_eq!(fixture.layer.as_deref(), Some(truss.id.as_str()));
}

#[test]
fn case_colliding_members_cannot_shadow_gdtf_source() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    zip.start_file("GeneralSceneDescription.xml", options)
        .unwrap();
    zip.write_all(b"<GeneralSceneDescription verMajor=\"1\" verMinor=\"6\"><Scene><Layers/></Scene></GeneralSceneDescription>").unwrap();
    zip.start_file("fixture.gdtf", options).unwrap();
    zip.write_all(b"one").unwrap();
    zip.start_file("Fixture.GDTF", options).unwrap();
    zip.write_all(b"two").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(
        read(&bytes)
            .unwrap_err()
            .to_string()
            .contains("duplicate archive member")
    );
}

fn xml_archive(member: &str, xml: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(member, SimpleFileOptions::default())
        .unwrap();
    zip.write_all(xml.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn attribute_entities_round_trip_literal_layer_and_fixture_names() {
    let mut fixture = spot(Some("stage"));
    fixture.name = "Spot \"A\" & 'B' <Main>".into();
    let doc = MvrDocument {
        fixtures: vec![fixture],
        layers: vec![layer("stage", "Stage & Venue \"Main\" 'A'")],
        ..Default::default()
    };
    let once = read(&write(&doc).unwrap()).unwrap();
    let twice = read(&write(&once).unwrap()).unwrap();
    for imported in [&once, &twice] {
        assert_eq!(imported.layers[0].name, doc.layers[0].name);
        assert_eq!(imported.fixtures[0].name, doc.fixtures[0].name);
    }
}

#[test]
fn attribute_numeric_entities_decode_unicode_and_ampersand_once() {
    let fixture_id = Uuid::new_v4();
    let geometry_id = Uuid::new_v4();
    let xml = format!(
        r#"<GeneralSceneDescription><Scene><Layers><Layer uuid="stage" name="Stage &#38; &#x56;enue &#x1F4A1;"><ChildList><Fixture uuid="{fixture_id}" name="Spot &#34;A&#34; &amp;amp;" class="Rig &#x26; show"></Fixture><Geometry3D uuid="{geometry_id}" name="Mask &#39;A&#39;" fileName="mask&#38;logo.glb"></Geometry3D></ChildList></Layer></Layers></Scene></GeneralSceneDescription>"#
    );
    let doc = read(&xml_archive("GeneralSceneDescription.xml", &xml)).unwrap();
    assert_eq!(doc.layers[0].name, "Stage & Venue 💡");
    assert_eq!(doc.fixtures[0].name, "Spot \"A\" &amp;");
    assert_eq!(doc.fixtures[0].class.as_deref(), Some("Rig & show"));
    assert_eq!(doc.geometry[0].name, "Mask 'A'");
    assert_eq!(doc.geometry[0].file_name, "mask&logo.glb");
}

#[test]
fn malformed_attribute_entities_and_syntax_are_actionable_errors() {
    for attributes in [
        r#"name="Stage &missing;""#,
        r#"name="Stage &#xZZ;""#,
        r#"name="Stage &amp""#,
        r#"name="Stage" name="Other""#,
        r#"name="Stage" broken"#,
    ] {
        let xml = format!(
            "<GeneralSceneDescription><Scene><Layers><Layer uuid=\"stage\" {attributes}></Layer></Layers></Scene></GeneralSceneDescription>"
        );
        let error = read(&xml_archive("GeneralSceneDescription.xml", &xml)).unwrap_err();
        assert!(
            error.to_string().contains("XML attribute"),
            "{attributes}: {error}"
        );
    }
}

#[test]
fn gdtf_uses_the_same_attribute_entity_decoder() {
    let xml = r#"<GDTF><FixtureType Manufacturer="Acme &amp; Company" Name="Lamp &#34;A&#34;"><DMXModes><DMXMode Name="Mode &#x31;"><DMXChannels><DMXChannel Name="Pan &amp; Tilt" Offset="&#49;,&#51;"/></DMXChannels></DMXMode></DMXModes></FixtureType></GDTF>"#;
    let modes = read_gdtf(&xml_archive("description.xml", xml)).unwrap();
    assert_eq!(modes[0].manufacturer, "Acme & Company");
    assert_eq!(modes[0].model, "Lamp \"A\"");
    assert_eq!(modes[0].name, "Mode 1");
    assert_eq!(modes[0].channels[0].attribute, "Pan & Tilt");
    assert_eq!(modes[0].channels[0].offsets, vec![0, 2]);
}
