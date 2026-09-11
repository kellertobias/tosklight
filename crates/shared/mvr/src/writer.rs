use std::io::{Cursor, Write};

use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};

use super::{MvrDocument, MvrError, MvrFixture, MvrGeometry};

/// The application named in every archive this writes. MVR 1.6 makes the provider mandatory.
const PROVIDER: &str = "ToskLight";

/// The layer objects without a layer of their own are placed on.
const DEFAULT_LAYER: &str = "Default";

/// Namespace for layer UUIDs, so exporting the same rig twice names its layers identically.
const LAYER_NAMESPACE: Uuid = Uuid::from_u128(0x746f_736b_6c69_6768_745f_6d76_725f_6c79);

type XmlWriter = Writer<Cursor<Vec<u8>>>;

fn element(writer: &mut XmlWriter, name: &str, value: &str) -> Result<(), MvrError> {
    writer
        .create_element(name)
        .write_text_content(quick_xml::events::BytesText::new(value))?;
    Ok(())
}

pub fn write(document: &MvrDocument) -> Result<Vec<u8>, MvrError> {
    let mut xml = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    let mut root = BytesStart::new("GeneralSceneDescription");
    root.push_attribute(("verMajor", "1"));
    root.push_attribute(("verMinor", "6"));
    root.push_attribute(("provider", PROVIDER));
    root.push_attribute(("providerVersion", env!("CARGO_PKG_VERSION")));
    xml.write_event(Event::Start(root))?;
    xml.write_event(Event::Start(BytesStart::new("Scene")))?;
    xml.write_event(Event::Start(BytesStart::new("Layers")))?;
    for layer in layers(document) {
        write_layer(&mut xml, &layer)?;
    }
    for tag in ["Layers", "Scene", "GeneralSceneDescription"] {
        xml.write_event(Event::End(BytesEnd::new(tag)))?;
    }
    write_archive(document, xml)
}

struct Layer<'a> {
    name: &'a str,
    fixtures: Vec<&'a MvrFixture>,
    geometry: Vec<&'a MvrGeometry>,
}

/// Scene objects grouped by layer, in the order each layer first appears.
///
/// Every MVR object lives in a layer, and every layer needs a UUID; an empty rig still gets one.
fn layers(document: &MvrDocument) -> Vec<Layer<'_>> {
    fn layer<'a, 'b>(layers: &'b mut Vec<Layer<'a>>, name: Option<&'a str>) -> &'b mut Layer<'a> {
        let name = name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(DEFAULT_LAYER);
        let index = match layers.iter().position(|layer| layer.name == name) {
            Some(index) => index,
            None => {
                layers.push(Layer {
                    name,
                    fixtures: Vec::new(),
                    geometry: Vec::new(),
                });
                layers.len() - 1
            }
        };
        &mut layers[index]
    }
    let mut layers = Vec::new();
    for fixture in &document.fixtures {
        layer(&mut layers, fixture.layer.as_deref())
            .fixtures
            .push(fixture);
    }
    for geometry in &document.geometry {
        layer(&mut layers, geometry.layer.as_deref())
            .geometry
            .push(geometry);
    }
    if layers.is_empty() {
        layer(&mut layers, None);
    }
    layers
}

fn write_layer(xml: &mut XmlWriter, layer: &Layer<'_>) -> Result<(), MvrError> {
    let mut node = BytesStart::new("Layer");
    let uuid = Uuid::new_v5(&LAYER_NAMESPACE, layer.name.as_bytes()).to_string();
    node.push_attribute(("uuid", uuid.as_str()));
    node.push_attribute(("name", layer.name));
    xml.write_event(Event::Start(node))?;
    xml.write_event(Event::Start(BytesStart::new("ChildList")))?;
    for fixture in &layer.fixtures {
        write_fixture(xml, fixture)?;
    }
    for geometry in &layer.geometry {
        write_geometry(xml, geometry)?;
    }
    xml.write_event(Event::End(BytesEnd::new("ChildList")))?;
    xml.write_event(Event::End(BytesEnd::new("Layer")))?;
    Ok(())
}

/// Children follow the order the MVR 1.6 fixture node lists them in.
fn write_fixture(xml: &mut XmlWriter, fixture: &MvrFixture) -> Result<(), MvrError> {
    let mut node = BytesStart::new("Fixture");
    let uuid = fixture.uuid.to_string();
    node.push_attribute(("uuid", uuid.as_str()));
    node.push_attribute(("name", fixture.name.as_str()));
    xml.write_event(Event::Start(node))?;
    element(xml, "Matrix", &matrix_text(fixture.matrix))?;
    element(xml, "GDTFSpec", &fixture.gdtf_spec)?;
    element(xml, "GDTFMode", &fixture.gdtf_mode)?;
    let fixture_id = fixture.fixture_id.as_deref().unwrap_or("");
    element(xml, "FixtureID", fixture_id)?;
    // Mandatory beside FixtureID. A virtual `0.x` number has no integer form, so it is 0.
    let numeric = fixture_id.trim().parse::<u32>().unwrap_or(0);
    element(xml, "FixtureIDNumeric", &numeric.to_string())?;
    if let (Some(universe), Some(address)) = (fixture.universe, fixture.address) {
        xml.write_event(Event::Start(BytesStart::new("Addresses")))?;
        let mut node = BytesStart::new("Address");
        node.push_attribute(("break", "0"));
        xml.write_event(Event::Start(node))?;
        xml.write_event(Event::Text(quick_xml::events::BytesText::new(
            &absolute_address(universe, address).to_string(),
        )))?;
        xml.write_event(Event::End(BytesEnd::new("Address")))?;
        xml.write_event(Event::End(BytesEnd::new("Addresses")))?;
    }
    xml.write_event(Event::End(BytesEnd::new("Fixture")))?;
    Ok(())
}

/// The absolute DMX address MVR counts from universe 1, address 1. Every reader accepts this form;
/// the dotted `universe.address` form is optional in the standard.
fn absolute_address(universe: u16, address: u16) -> u32 {
    (u32::from(universe.max(1)) - 1) * 512 + u32::from(address)
}

/// Scenery is a scene object carrying its geometry file; a bare `Geometry3D` is not a valid
/// child of a layer.
fn write_geometry(xml: &mut XmlWriter, geometry: &MvrGeometry) -> Result<(), MvrError> {
    let mut node = BytesStart::new("SceneObject");
    let uuid = geometry.uuid.to_string();
    node.push_attribute(("uuid", uuid.as_str()));
    node.push_attribute(("name", geometry.name.as_str()));
    xml.write_event(Event::Start(node))?;
    element(xml, "Matrix", &matrix_text(geometry.matrix))?;
    xml.write_event(Event::Start(BytesStart::new("Geometries")))?;
    let mut file = BytesStart::new("Geometry3D");
    file.push_attribute(("fileName", geometry.file_name.as_str()));
    xml.write_event(Event::Empty(file))?;
    xml.write_event(Event::End(BytesEnd::new("Geometries")))?;
    xml.write_event(Event::End(BytesEnd::new("SceneObject")))?;
    Ok(())
}

/// `{u}{v}{w}{o}`: three rotation rows and the offset in millimetres, as MVR writes a 4x3 matrix.
fn matrix_text(matrix: [f64; 12]) -> String {
    matrix
        .chunks(3)
        .map(|row| {
            let row = row.iter().map(ToString::to_string).collect::<Vec<_>>();
            format!("{{{}}}", row.join(","))
        })
        .collect()
}

fn write_archive(document: &MvrDocument, xml: XmlWriter) -> Result<Vec<u8>, MvrError> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("GeneralSceneDescription.xml", options)?;
    zip.write_all(&xml.into_inner().into_inner())?;
    // Sorted, so the same rig always produces the same archive.
    let mut files: Vec<_> = document.files.iter().collect();
    files.sort_by_key(|(name, _)| *name);
    for (name, data) in files {
        if name
            .to_ascii_lowercase()
            .ends_with("generalscenedescription.xml")
            || unsafe_archive_name(name)
        {
            continue;
        }
        zip.start_file(name.as_str(), options)?;
        zip.write_all(data)?;
    }
    Ok(zip.finish()?.into_inner())
}

fn unsafe_archive_name(name: &str) -> bool {
    std::path::Path::new(name)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}
