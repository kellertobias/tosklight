#![forbid(unsafe_code)]
//! Bounded, transport-neutral MVR archive reader and writer.

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
};
use thiserror::Error;
use uuid::Uuid;
use zip::ZipArchive;

mod writer;
pub use writer::write;

pub const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum MvrError {
    #[error("invalid MVR archive: {0}")]
    Invalid(String),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MvrDocument {
    pub fixtures: Vec<MvrFixture>,
    pub geometry: Vec<MvrGeometry>,
    #[serde(skip)]
    pub files: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MvrFixture {
    pub uuid: Uuid,
    pub name: String,
    pub fixture_id: Option<String>,
    pub gdtf_spec: String,
    pub gdtf_mode: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    pub matrix: [f64; 12],
    pub layer: Option<String>,
    pub class: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MvrGeometry {
    pub uuid: Uuid,
    pub name: String,
    pub file_name: String,
    pub matrix: [f64; 12],
    pub layer: Option<String>,
    pub class: Option<String>,
}

#[derive(Clone, Debug)]
pub struct GdtfMode {
    pub manufacturer: String,
    pub model: String,
    pub name: String,
    pub channels: Vec<GdtfChannel>,
}
#[derive(Clone, Debug)]
pub struct GdtfChannel {
    pub attribute: String,
    pub offsets: Vec<u16>,
}

pub fn read_gdtf(bytes: &[u8]) -> Result<Vec<GdtfMode>, MvrError> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let mut xml = None;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        if entry
            .name()
            .to_ascii_lowercase()
            .ends_with("description.xml")
        {
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            xml = Some(data);
            break;
        }
    }
    let xml = xml.ok_or_else(|| MvrError::Invalid("GDTF description.xml is missing".into()))?;
    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut manufacturer = "Unknown".to_owned();
    let mut model = "Unknown".to_owned();
    let mut modes = Vec::new();
    let mut current: Option<GdtfMode> = None;
    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => {
                let tag = String::from_utf8_lossy(local(e.name().as_ref())).to_ascii_lowercase();
                if tag == "fixturetype" {
                    manufacturer = attr(&e, b"manufacturer").unwrap_or_else(|| "Unknown".into());
                    model = attr(&e, b"shortname")
                        .or_else(|| attr(&e, b"name"))
                        .unwrap_or_else(|| "Unknown".into());
                } else if tag == "dmxmode" {
                    if let Some(mode) = current.take() {
                        modes.push(mode);
                    }
                    current = Some(GdtfMode {
                        manufacturer: manufacturer.clone(),
                        model: model.clone(),
                        name: attr(&e, b"name").unwrap_or_else(|| "Standard".into()),
                        channels: Vec::new(),
                    });
                } else if tag == "dmxchannel"
                    && let Some(mode) = current.as_mut()
                {
                    let offsets = attr(&e, b"offset")
                        .unwrap_or_else(|| "1".into())
                        .split(',')
                        .filter_map(|v| v.trim().parse::<u16>().ok())
                        .map(|v| v.saturating_sub(1))
                        .collect::<Vec<_>>();
                    let attribute = attr(&e, b"name").unwrap_or_else(|| {
                        format!("channel.{}", offsets.first().copied().unwrap_or(0) + 1)
                    });
                    mode.channels.push(GdtfChannel { attribute, offsets });
                }
            }
            Event::End(e) if local(e.name().as_ref()).eq_ignore_ascii_case(b"dmxmode") => {
                if let Some(mode) = current.take() {
                    modes.push(mode);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some(mode) = current {
        modes.push(mode);
    }
    if modes.is_empty() {
        return Err(MvrError::Invalid("GDTF contains no DMX modes".into()));
    }
    Ok(modes)
}

fn local(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}
fn attr(start: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|a| local(a.key.as_ref()).eq_ignore_ascii_case(key))
        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
}
fn matrix(text: &str) -> [f64; 12] {
    let mut result = [0.0; 12];
    result[0] = 1.0;
    result[4] = 1.0;
    result[8] = 1.0;
    for (slot, value) in result.iter_mut().zip(
        text.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter_map(|v| v.parse().ok()),
    ) {
        *slot = value;
    }
    result
}

pub fn read(bytes: &[u8]) -> Result<MvrDocument, MvrError> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(MvrError::Invalid("archive exceeds 256 MiB".into()));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let mut files = HashMap::new();
    let mut xml = None;
    let mut expanded = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(path) = entry.enclosed_name() else {
            return Err(MvrError::Invalid("archive contains an unsafe path".into()));
        };
        if entry.is_dir() {
            continue;
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_EXPANDED_BYTES {
            return Err(MvrError::Invalid("expanded archive exceeds 512 MiB".into()));
        }
        let name = path.to_string_lossy().replace('\\', "/");
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        if name
            .to_ascii_lowercase()
            .ends_with("generalscenedescription.xml")
        {
            xml = Some(data.clone());
        }
        files.insert(name.to_ascii_lowercase(), data);
    }
    let xml =
        xml.ok_or_else(|| MvrError::Invalid("GeneralSceneDescription.xml is missing".into()))?;
    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut doc = MvrDocument {
        files,
        ..Default::default()
    };
    let mut stack: Vec<String> = Vec::new();
    let mut current_fixture: Option<MvrFixture> = None;
    let mut current_geometry: Option<MvrGeometry> = None;
    let mut text = String::new();
    let mut uuids = HashSet::new();
    loop {
        match reader.read_event()? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(local(e.name().as_ref())).to_ascii_lowercase();
                stack.push(tag.clone());
                text.clear();
                if tag == "fixture" {
                    let uuid = attr(&e, b"uuid")
                        .and_then(|v| Uuid::parse_str(&v).ok())
                        .unwrap_or_else(Uuid::new_v4);
                    current_fixture = Some(MvrFixture {
                        uuid,
                        name: attr(&e, b"name").unwrap_or_else(|| "Fixture".into()),
                        fixture_id: None,
                        gdtf_spec: String::new(),
                        gdtf_mode: String::new(),
                        universe: None,
                        address: None,
                        matrix: matrix(""),
                        layer: attr(&e, b"layer"),
                        class: attr(&e, b"class"),
                    });
                }
                if tag == "geometry3d" {
                    let uuid = attr(&e, b"uuid")
                        .and_then(|v| Uuid::parse_str(&v).ok())
                        .unwrap_or_else(Uuid::new_v4);
                    current_geometry = Some(MvrGeometry {
                        uuid,
                        name: attr(&e, b"name").unwrap_or_else(|| "Geometry".into()),
                        file_name: attr(&e, b"filename").unwrap_or_default(),
                        matrix: matrix(""),
                        layer: attr(&e, b"layer"),
                        class: attr(&e, b"class"),
                    });
                }
            }
            Event::Text(e) => {
                text.push_str(&e.decode().map_err(|e| MvrError::Invalid(e.to_string()))?)
            }
            Event::End(e) => {
                let tag = String::from_utf8_lossy(local(e.name().as_ref())).to_ascii_lowercase();
                let value = text.trim();
                if let Some(f) = current_fixture.as_mut() {
                    match tag.as_str() {
                        "fixtureid" => f.fixture_id = Some(value.into()),
                        "gdtfspec" => f.gdtf_spec = value.into(),
                        "gdtfmode" => f.gdtf_mode = value.into(),
                        "matrix" => f.matrix = matrix(value),
                        "address" => {
                            let nums: Vec<u16> = value
                                .split(|c: char| !c.is_ascii_digit())
                                .filter_map(|v| v.parse().ok())
                                .collect();
                            if nums.len() >= 2 {
                                f.universe = Some(nums[0]);
                                f.address = Some(nums[1]);
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(g) = current_geometry.as_mut()
                    && tag == "matrix"
                {
                    g.matrix = matrix(value);
                }
                if tag == "fixture" {
                    let f = current_fixture.take().unwrap();
                    if !uuids.insert(f.uuid) {
                        return Err(MvrError::Invalid(format!("duplicate UUID {}", f.uuid)));
                    }
                    doc.fixtures.push(f);
                }
                if tag == "geometry3d" {
                    let g = current_geometry.take().unwrap();
                    if !uuids.insert(g.uuid) {
                        return Err(MvrError::Invalid(format!("duplicate UUID {}", g.uuid)));
                    }
                    doc.geometry.push(g);
                }
                stack.pop();
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(doc)
}

#[cfg(test)]
mod tests;
