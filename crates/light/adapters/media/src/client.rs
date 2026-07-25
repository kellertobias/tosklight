use crate::protocol::{
    Fragment, HEADER_BYTES, MAX_PACKET_BYTES, Packet, encode_packet, parse_stream_frame,
    parse_thumbnail,
};
use crate::{ImageFormat, LibraryId, MediaError, MediaImage};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

pub struct CitpClient {
    stream: TcpStream,
    request_index: u16,
    negotiated_version: (u8, u8),
    operation_timeout: Duration,
}

impl CitpClient {
    pub async fn connect(
        address: SocketAddr,
        operation_timeout: Duration,
    ) -> Result<Self, MediaError> {
        let stream = timeout(operation_timeout, TcpStream::connect(address))
            .await
            .map_err(|_| MediaError::Timeout)??;
        stream.set_nodelay(true)?;
        let mut client = Self {
            stream,
            request_index: 0,
            negotiated_version: (1, 2),
            operation_timeout,
        };
        let request = client.send(*b"CInf", vec![3, 1, 2, 1, 1, 1, 0]).await?;
        let server_info = client.receive_relevant(*b"SInf", request).await?;
        client.negotiated_version = server_info.version;
        Ok(client)
    }

    pub async fn request_thumbnail(
        &mut self,
        library_type: u8,
        library: LibraryId,
        elements: &[u8],
        width: u16,
        height: u16,
    ) -> Result<Vec<(u8, MediaImage)>, MediaError> {
        self.validate_thumbnail_request(elements, width, height)?;
        let payload = self.thumbnail_payload(library_type, library, elements, width, height);
        let request = self.send(*b"GETh", payload).await?;
        let mut images = Vec::with_capacity(elements.len());
        while images.len() < elements.len() {
            let packet = self.receive_relevant(*b"EThn", request).await?;
            images.push(parse_thumbnail(&packet.payload, packet.version)?.1);
        }
        Ok(images)
    }

    pub async fn request_preview(
        &mut self,
        source: u16,
        width: u16,
        height: u16,
    ) -> Result<MediaImage, MediaError> {
        validate_preview_bounds(width, height)?;
        let request = self
            .send(*b"RqSt", preview_payload(source, width, height))
            .await?;
        let packet = self.receive_relevant(*b"StFr", request).await?;
        let (received_source, image) = parse_stream_frame(&packet.payload, packet.version)?;
        if received_source != source {
            return Err(MediaError::Invalid(
                "media server returned a different preview source".into(),
            ));
        }
        Ok(image)
    }

    fn validate_thumbnail_request(
        &self,
        elements: &[u8],
        width: u16,
        height: u16,
    ) -> Result<(), MediaError> {
        if elements.is_empty()
            || elements.len() > 256
            || (self.negotiated_version < (1, 2) && elements.len() > 255)
            || width == 0
            || height == 0
            || width > 2048
            || height > 2048
        {
            return Err(MediaError::Invalid(
                "invalid thumbnail request bounds".into(),
            ));
        }
        Ok(())
    }

    fn thumbnail_payload(
        &self,
        library_type: u8,
        library: LibraryId,
        elements: &[u8],
        width: u16,
        height: u16,
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(16 + elements.len());
        payload.extend_from_slice(&ImageFormat::Jpeg.cookie());
        payload.extend_from_slice(&width.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.push(1);
        payload.push(library_type);
        self.encode_thumbnail_address(&mut payload, library, elements.len());
        payload.extend_from_slice(elements);
        payload
    }

    fn encode_thumbnail_address(
        &self,
        payload: &mut Vec<u8>,
        library: LibraryId,
        element_count: usize,
    ) {
        if self.negotiated_version == (1, 0) {
            payload.push(library.ids[0]);
            payload.push(element_count as u8);
            return;
        }
        library.encode(payload);
        if self.negotiated_version >= (1, 2) {
            payload.extend_from_slice(&(element_count as u16).to_le_bytes());
        } else {
            payload.push(element_count as u8);
        }
    }

    async fn receive_relevant(
        &mut self,
        wanted: [u8; 4],
        request_index: u16,
    ) -> Result<Packet, MediaError> {
        for _ in 0..64 {
            let packet = self.read_packet().await?;
            if packet.content == *b"Nack" {
                return Err(MediaError::Rejected(
                    String::from_utf8_lossy(packet.payload.get(..4).unwrap_or_default())
                        .into_owned(),
                ));
            }
            if packet.content == wanted
                && (packet.request_index == 0 || packet.request_index == request_index)
            {
                return Ok(packet);
            }
        }
        Err(MediaError::Invalid(
            "too many unrelated CITP messages".into(),
        ))
    }

    async fn send(&mut self, content: [u8; 4], payload: Vec<u8>) -> Result<u16, MediaError> {
        self.request_index = self.request_index.wrapping_add(1).max(1);
        let version = if content == *b"CInf" {
            (1, 2)
        } else {
            self.negotiated_version
        };
        let bytes = encode_packet(version, self.request_index, content, &payload);
        timeout(self.operation_timeout, self.stream.write_all(&bytes))
            .await
            .map_err(|_| MediaError::Timeout)??;
        Ok(self.request_index)
    }

    async fn read_packet(&mut self) -> Result<Packet, MediaError> {
        let mut first = self.read_fragment().await?;
        validate_first_fragment(&first)?;
        let mut payload = std::mem::take(&mut first.payload);
        for expected in 1..first.part_count {
            let fragment = self.read_fragment().await?;
            validate_following_fragment(&first, &fragment, expected, payload.len())?;
            payload.extend_from_slice(&fragment.payload);
        }
        Ok(Packet {
            version: first.version,
            request_index: first.request_index,
            content: first.content,
            payload,
        })
    }

    async fn read_fragment(&mut self) -> Result<Fragment, MediaError> {
        let mut header = [0_u8; 20];
        timeout(self.operation_timeout, self.stream.read_exact(&mut header))
            .await
            .map_err(|_| MediaError::Timeout)??;
        validate_header(&header)?;
        let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if !(HEADER_BYTES..=MAX_PACKET_BYTES).contains(&size) {
            return Err(MediaError::Invalid("invalid message size".into()));
        }
        let mut rest = vec![0; size - 20];
        timeout(self.operation_timeout, self.stream.read_exact(&mut rest))
            .await
            .map_err(|_| MediaError::Timeout)??;
        let version = (rest[0], rest[1]);
        validate_version(version)?;
        Ok(Fragment {
            version,
            request_index: u16::from_le_bytes(header[6..8].try_into().unwrap()),
            content: rest[2..6].try_into().unwrap(),
            part_count: u16::from_le_bytes(header[12..14].try_into().unwrap()),
            part: u16::from_le_bytes(header[14..16].try_into().unwrap()),
            payload: rest[6..].to_vec(),
        })
    }
}

fn preview_payload(source: u16, width: u16, height: u16) -> Vec<u8> {
    let mut payload = Vec::with_capacity(13);
    payload.extend_from_slice(&source.to_le_bytes());
    payload.extend_from_slice(&ImageFormat::Jpeg.cookie());
    payload.extend_from_slice(&width.to_le_bytes());
    payload.extend_from_slice(&height.to_le_bytes());
    payload.push(1);
    payload.push(0);
    payload
}

fn validate_preview_bounds(width: u16, height: u16) -> Result<(), MediaError> {
    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return Err(MediaError::Invalid("invalid preview request bounds".into()));
    }
    Ok(())
}

fn validate_first_fragment(fragment: &Fragment) -> Result<(), MediaError> {
    if fragment.part != 0 || fragment.part_count == 0 || fragment.part_count > 256 {
        return Err(MediaError::Invalid("invalid CITP fragment sequence".into()));
    }
    Ok(())
}

fn validate_following_fragment(
    first: &Fragment,
    fragment: &Fragment,
    expected: u16,
    current_size: usize,
) -> Result<(), MediaError> {
    let metadata_matches = fragment.part == expected
        && fragment.part_count == first.part_count
        && fragment.version == first.version
        && fragment.request_index == first.request_index
        && fragment.content == first.content;
    if !metadata_matches {
        return Err(MediaError::Invalid(
            "inconsistent CITP fragment sequence".into(),
        ));
    }
    if current_size.saturating_add(fragment.payload.len()) > MAX_PACKET_BYTES {
        return Err(MediaError::Invalid(
            "reassembled CITP message exceeds size limit".into(),
        ));
    }
    Ok(())
}

fn validate_header(header: &[u8; 20]) -> Result<(), MediaError> {
    if &header[..4] != b"CITP" || &header[16..20] != b"MSEX" {
        return Err(MediaError::Invalid(
            "invalid CITP/MSEX header cookie".into(),
        ));
    }
    Ok(())
}

fn validate_version(version: (u8, u8)) -> Result<(), MediaError> {
    if version.0 != 1 || version.1 > 2 {
        return Err(MediaError::Invalid(format!(
            "unsupported MSEX version {}.{}",
            version.0, version.1
        )));
    }
    Ok(())
}
