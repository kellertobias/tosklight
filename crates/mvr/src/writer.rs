use std::io::{Cursor, Write};

use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, Event},
};
use zip::{ZipWriter, write::SimpleFileOptions};

use super::{MvrDocument, MvrError};

fn element(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: &str) -> Result<(), MvrError> {
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
    xml.write_event(Event::Start(root))?;
    xml.write_event(Event::Start(BytesStart::new("Scene")))?;
    xml.write_event(Event::Start(BytesStart::new("Layers")))?;
    xml.write_event(Event::Start(BytesStart::new("Layer")))?;
    xml.write_event(Event::Start(BytesStart::new("ChildList")))?;
    write_fixtures(&mut xml, document)?;
    write_geometry(&mut xml, document)?;
    close_scene(&mut xml)?;
    write_archive(document, xml)
}

fn write_fixtures(
    xml: &mut Writer<Cursor<Vec<u8>>>,
    document: &MvrDocument,
) -> Result<(), MvrError> {
    for fixture in &document.fixtures {
        let mut node = BytesStart::new("Fixture");
        let uuid = fixture.uuid.to_string();
        node.push_attribute(("uuid", uuid.as_str()));
        node.push_attribute(("name", fixture.name.as_str()));
        xml.write_event(Event::Start(node))?;
        element(xml, "Matrix", &matrix_text(fixture.matrix))?;
        element(
            xml,
            "FixtureID",
            fixture.fixture_id.as_deref().unwrap_or(""),
        )?;
        element(xml, "GDTFSpec", &fixture.gdtf_spec)?;
        element(xml, "GDTFMode", &fixture.gdtf_mode)?;
        if let (Some(universe), Some(address)) = (fixture.universe, fixture.address) {
            xml.write_event(Event::Start(BytesStart::new("Addresses")))?;
            element(xml, "Address", &format!("{universe}.{address}"))?;
            xml.write_event(Event::End(BytesEnd::new("Addresses")))?;
        }
        xml.write_event(Event::End(BytesEnd::new("Fixture")))?;
    }
    Ok(())
}

fn write_geometry(
    xml: &mut Writer<Cursor<Vec<u8>>>,
    document: &MvrDocument,
) -> Result<(), MvrError> {
    for geometry in &document.geometry {
        let mut node = BytesStart::new("Geometry3D");
        let uuid = geometry.uuid.to_string();
        node.push_attribute(("uuid", uuid.as_str()));
        node.push_attribute(("name", geometry.name.as_str()));
        node.push_attribute(("fileName", geometry.file_name.as_str()));
        xml.write_event(Event::Start(node))?;
        element(xml, "Matrix", &matrix_text(geometry.matrix))?;
        xml.write_event(Event::End(BytesEnd::new("Geometry3D")))?;
    }
    Ok(())
}

fn matrix_text(matrix: [f64; 12]) -> String {
    matrix
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn close_scene(xml: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), MvrError> {
    for tag in [
        "ChildList",
        "Layer",
        "Layers",
        "Scene",
        "GeneralSceneDescription",
    ] {
        xml.write_event(Event::End(BytesEnd::new(tag)))?;
    }
    Ok(())
}

fn write_archive(
    document: &MvrDocument,
    xml: Writer<Cursor<Vec<u8>>>,
) -> Result<Vec<u8>, MvrError> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("GeneralSceneDescription.xml", options)?;
    zip.write_all(&xml.into_inner().into_inner())?;
    for (name, data) in &document.files {
        if name
            .to_ascii_lowercase()
            .ends_with("generalscenedescription.xml")
            || unsafe_archive_name(name)
        {
            continue;
        }
        zip.start_file(name, options)?;
        zip.write_all(data)?;
    }
    Ok(zip.finish()?.into_inner())
}

fn unsafe_archive_name(name: &str) -> bool {
    std::path::Path::new(name)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}
