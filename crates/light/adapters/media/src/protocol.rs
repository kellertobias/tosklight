use crate::{
    ImageFormat, LibraryId, MediaError, MediaImage, MediaLayerStatus, MediaLibraryElement,
    MediaLibraryFolder, MediaPreviewSource, MediaServerInformation,
};

pub(crate) const HEADER_BYTES: usize = 26;
pub(crate) const MAX_PACKET_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Packet {
    pub(crate) version: (u8, u8),
    pub(crate) request_index: u16,
    pub(crate) content: [u8; 4],
    pub(crate) payload: Vec<u8>,
}

pub(crate) struct Fragment {
    pub(crate) version: (u8, u8),
    pub(crate) request_index: u16,
    pub(crate) content: [u8; 4],
    pub(crate) part_count: u16,
    pub(crate) part: u16,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn encode_packet(
    version: (u8, u8),
    request_index: u16,
    content: [u8; 4],
    payload: &[u8],
) -> Vec<u8> {
    let size = HEADER_BYTES + payload.len();
    let mut output = Vec::with_capacity(size);
    output.extend_from_slice(b"CITP");
    output.push(1);
    output.push(0);
    output.extend_from_slice(&request_index.to_le_bytes());
    output.extend_from_slice(&(size as u32).to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(b"MSEX");
    output.push(version.0);
    output.push(version.1);
    output.extend_from_slice(&content);
    output.extend_from_slice(payload);
    output
}

pub(crate) fn parse_thumbnail(
    payload: &[u8],
    version: (u8, u8),
) -> Result<(LibraryId, (u8, MediaImage)), MediaError> {
    let (library, element_offset) = thumbnail_library(payload, version)?;
    let element = payload[element_offset];
    let format_offset = element_offset + 1;
    let image = parse_image(payload, format_offset, "EThn")?;
    Ok((library, (element, image)))
}

fn thumbnail_library(payload: &[u8], version: (u8, u8)) -> Result<(LibraryId, usize), MediaError> {
    if version >= (1, 1) {
        if payload.len() < 16 {
            return Err(MediaError::Invalid("truncated EThn packet".into()));
        }
        Ok((
            LibraryId {
                level: payload[1],
                ids: payload[2..5].try_into().unwrap(),
            },
            5,
        ))
    } else {
        if payload.len() < 13 {
            return Err(MediaError::Invalid("truncated EThn packet".into()));
        }
        Ok((
            LibraryId {
                level: 1,
                ids: [payload[1], 0, 0],
            },
            2,
        ))
    }
}

pub(crate) fn parse_stream_frame(
    payload: &[u8],
    version: (u8, u8),
) -> Result<(u16, MediaImage), MediaError> {
    let offset = if version >= (1, 2) { 36 } else { 0 };
    if payload.len() < offset + 12 {
        return Err(MediaError::Invalid("truncated StFr packet".into()));
    }
    let source = u16::from_le_bytes(payload[offset..offset + 2].try_into().unwrap());
    let image = parse_image(payload, offset + 2, "StFr")?;
    Ok((source, image))
}

pub(crate) fn parse_server_information(
    payload: &[u8],
) -> Result<MediaServerInformation, MediaError> {
    let (name, next) = ucs2(payload, 0, "SInf")?;
    let layer_count = *payload
        .get(next + 2)
        .ok_or_else(|| MediaError::Invalid("truncated SInf packet".into()))?;
    Ok(MediaServerInformation { name, layer_count })
}

pub(crate) fn parse_library_folders(
    payload: &[u8],
    version: (u8, u8),
) -> Result<Vec<MediaLibraryFolder>, MediaError> {
    if payload.first() != Some(&1) {
        return Err(MediaError::Invalid("ELIn is not a media library".into()));
    }
    let count = usize::from(
        *payload
            .get(1)
            .ok_or_else(|| MediaError::Invalid("truncated ELIn packet".into()))?,
    );
    let mut at = 2;
    let mut folders = Vec::with_capacity(count);
    for _ in 0..count {
        let id = if version >= (1, 1) {
            let id = *payload
                .get(at + 1)
                .ok_or_else(|| MediaError::Invalid("truncated ELIn library id".into()))?;
            at += 4;
            id
        } else {
            let id = *payload
                .get(at)
                .ok_or_else(|| MediaError::Invalid("truncated ELIn library id".into()))?;
            at += 1;
            id
        };
        at = at
            .checked_add(2)
            .ok_or_else(|| MediaError::Invalid("invalid ELIn".into()))?;
        let (name, next) = ucs2(payload, at, "ELIn")?;
        at = next;
        if version >= (1, 1) {
            at += 1;
        }
        let element_count = *payload
            .get(at)
            .ok_or_else(|| MediaError::Invalid("truncated ELIn element count".into()))?;
        at += 1;
        folders.push(MediaLibraryFolder {
            id,
            name,
            element_count,
        });
    }
    Ok(folders)
}

pub(crate) fn parse_library_elements(
    payload: &[u8],
    version: (u8, u8),
) -> Result<Vec<MediaLibraryElement>, MediaError> {
    let folder_id = *payload
        .get(if version >= (1, 1) { 1 } else { 0 })
        .ok_or_else(|| MediaError::Invalid("truncated MEIn library id".into()))?;
    let mut at = if version >= (1, 1) { 4 } else { 1 };
    let count = usize::from(
        *payload
            .get(at)
            .ok_or_else(|| MediaError::Invalid("truncated MEIn packet".into()))?,
    );
    at += 1;
    let mut elements = Vec::with_capacity(count);
    for _ in 0..count {
        let id = *payload
            .get(at)
            .ok_or_else(|| MediaError::Invalid("truncated MEIn element".into()))?;
        at += 3;
        let (name, next) = ucs2(payload, at, "MEIn")?;
        at = next;
        if payload.len() < at + 17 {
            return Err(MediaError::Invalid("truncated MEIn metadata".into()));
        }
        at += 8;
        let width = u16::from_le_bytes(payload[at..at + 2].try_into().unwrap());
        let height = u16::from_le_bytes(payload[at + 2..at + 4].try_into().unwrap());
        let length_frames = u32::from_le_bytes(payload[at + 4..at + 8].try_into().unwrap());
        let fps = payload[at + 8];
        at += 9;
        elements.push(MediaLibraryElement {
            folder_id,
            id,
            name,
            width,
            height,
            length_frames,
            fps,
        });
    }
    Ok(elements)
}

pub(crate) fn parse_preview_sources(payload: &[u8]) -> Result<Vec<MediaPreviewSource>, MediaError> {
    if payload.len() < 2 {
        return Err(MediaError::Invalid("truncated VSrc packet".into()));
    }
    let count = usize::from(u16::from_le_bytes(payload[0..2].try_into().unwrap()));
    let mut at = 2;
    let mut sources = Vec::with_capacity(count);
    for _ in 0..count {
        if payload.len() < at + 2 {
            return Err(MediaError::Invalid("truncated VSrc source".into()));
        }
        let id = u16::from_le_bytes(payload[at..at + 2].try_into().unwrap());
        at += 2;
        let (name, next) = ucs2(payload, at, "VSrc")?;
        at = next;
        if payload.len() < at + 8 {
            return Err(MediaError::Invalid("truncated VSrc metadata".into()));
        }
        let physical_output = payload[at];
        let raw_layer = payload[at + 1];
        let width = u16::from_le_bytes(payload[at + 4..at + 6].try_into().unwrap());
        let height = u16::from_le_bytes(payload[at + 6..at + 8].try_into().unwrap());
        at += 8;
        sources.push(MediaPreviewSource {
            id,
            name,
            physical_output,
            layer: (raw_layer != u8::MAX).then_some(raw_layer),
            width,
            height,
        });
    }
    Ok(sources)
}

pub(crate) fn parse_layer_status(payload: &[u8]) -> Result<Vec<MediaLayerStatus>, MediaError> {
    let count = usize::from(
        *payload
            .first()
            .ok_or_else(|| MediaError::Invalid("truncated LSta packet".into()))?,
    );
    let mut at = 1;
    let mut layers = Vec::with_capacity(count);
    for _ in 0..count {
        if payload.len() < at + 4 {
            return Err(MediaError::Invalid("truncated LSta layer".into()));
        }
        let layer = payload[at];
        let physical_output = payload[at + 1];
        let folder = payload[at + 2];
        let file = payload[at + 3];
        at += 4;
        let (name, next) = ucs2(payload, at, "LSta")?;
        at = next;
        if payload.len() < at + 13 {
            return Err(MediaError::Invalid("truncated LSta metadata".into()));
        }
        let position_frames = u32::from_le_bytes(payload[at..at + 4].try_into().unwrap());
        let length_frames = u32::from_le_bytes(payload[at + 4..at + 8].try_into().unwrap());
        let fps = payload[at + 8];
        let flags = u32::from_le_bytes(payload[at + 9..at + 13].try_into().unwrap());
        at += 13;
        layers.push(MediaLayerStatus {
            layer,
            physical_output,
            folder,
            file,
            name,
            position_frames,
            length_frames,
            fps,
            flags,
        });
    }
    Ok(layers)
}

fn ucs2(payload: &[u8], mut at: usize, packet_name: &str) -> Result<(String, usize), MediaError> {
    let mut units = Vec::new();
    loop {
        if payload.len() < at + 2 {
            return Err(MediaError::Invalid(format!(
                "truncated {packet_name} string"
            )));
        }
        let unit = u16::from_le_bytes(payload[at..at + 2].try_into().unwrap());
        at += 2;
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    Ok((String::from_utf16_lossy(&units), at))
}

fn parse_image(payload: &[u8], offset: usize, packet_name: &str) -> Result<MediaImage, MediaError> {
    let format = ImageFormat::parse(payload[offset..offset + 4].try_into().unwrap())?;
    let width = u16::from_le_bytes(payload[offset + 4..offset + 6].try_into().unwrap());
    let height = u16::from_le_bytes(payload[offset + 6..offset + 8].try_into().unwrap());
    let length = u16::from_le_bytes(payload[offset + 8..offset + 10].try_into().unwrap()) as usize;
    let data = &payload[offset + 10..];
    if data.len() != length {
        return Err(MediaError::Invalid(format!(
            "{packet_name} buffer length mismatch"
        )));
    }
    let image = MediaImage {
        format,
        width,
        height,
        bytes: data.to_vec(),
    };
    image.validate()?;
    Ok(image)
}
