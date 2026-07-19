use crate::{ImageFormat, LibraryId, MediaError, MediaImage};

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
