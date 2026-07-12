#![forbid(unsafe_code)]
//! Bounded CITP/MSEX 1.2 client primitives for media thumbnails and output previews.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    io,
    net::SocketAddr,
    time::{Duration, SystemTime},
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

pub const DEFAULT_CITP_PORT: u16 = 4811;
const HEADER_BYTES: usize = 26;
const MAX_PACKET_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("CITP I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("CITP operation timed out")]
    Timeout,
    #[error("invalid CITP packet: {0}")]
    Invalid(String),
    #[error("media server rejected {0}")]
    Rejected(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct LibraryId {
    pub level: u8,
    pub ids: [u8; 3],
}
impl LibraryId {
    pub const ROOT: Self = Self {
        level: 0,
        ids: [0; 3],
    };
    fn encode(self, output: &mut Vec<u8>) {
        output.push(self.level.min(3));
        output.extend_from_slice(&self.ids);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Rgb8,
}
impl ImageFormat {
    fn cookie(self) -> [u8; 4] {
        match self {
            Self::Jpeg => *b"JPEG",
            Self::Png => *b"PNG ",
            Self::Rgb8 => *b"RGB8",
        }
    }
    fn parse(value: [u8; 4]) -> Result<Self, MediaError> {
        match &value {
            b"JPEG" => Ok(Self::Jpeg),
            b"PNG " => Ok(Self::Png),
            b"RGB8" => Ok(Self::Rgb8),
            _ => Err(MediaError::Invalid(format!(
                "unsupported image format {:?}",
                String::from_utf8_lossy(&value)
            ))),
        }
    }
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Rgb8 => "application/x-citp-rgb8",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaImage {
    pub format: ImageFormat,
    pub width: u16,
    pub height: u16,
    pub bytes: Vec<u8>,
}
impl MediaImage {
    fn validate(&self) -> Result<(), MediaError> {
        if self.width == 0 || self.height == 0 || self.width > 4096 || self.height > 4096 {
            return Err(MediaError::Invalid(
                "image dimensions are outside 1-4096".into(),
            ));
        }
        if self.bytes.is_empty() || self.bytes.len() > MAX_IMAGE_BYTES {
            return Err(MediaError::Invalid(
                "image payload is empty or exceeds the cache limit".into(),
            ));
        }
        if self.format == ImageFormat::Rgb8
            && self.bytes.len() != usize::from(self.width) * usize::from(self.height) * 3
        {
            return Err(MediaError::Invalid(
                "RGB8 payload size does not match dimensions".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ThumbnailKey {
    pub fixture: String,
    pub library_type: u8,
    pub library: LibraryId,
    pub element: u8,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PreviewKey {
    pub fixture: String,
    pub source: u16,
}

#[derive(Clone, Debug)]
pub struct CachedImage {
    pub image: MediaImage,
    pub received_at: SystemTime,
}

pub struct MediaCache {
    thumbnails: HashMap<ThumbnailKey, CachedImage>,
    previews: HashMap<PreviewKey, CachedImage>,
    thumbnail_order: VecDeque<ThumbnailKey>,
    preview_order: VecDeque<PreviewKey>,
    thumbnail_limit: usize,
    preview_limit: usize,
}
impl Default for MediaCache {
    fn default() -> Self {
        Self::new(512, 32)
    }
}
impl MediaCache {
    pub fn new(thumbnail_limit: usize, preview_limit: usize) -> Self {
        Self {
            thumbnails: HashMap::new(),
            previews: HashMap::new(),
            thumbnail_order: VecDeque::new(),
            preview_order: VecDeque::new(),
            thumbnail_limit: thumbnail_limit.max(1),
            preview_limit: preview_limit.max(1),
        }
    }
    pub fn put_thumbnail(
        &mut self,
        key: ThumbnailKey,
        image: MediaImage,
    ) -> Result<(), MediaError> {
        image.validate()?;
        touch(&mut self.thumbnail_order, &key);
        self.thumbnails.insert(
            key,
            CachedImage {
                image,
                received_at: SystemTime::now(),
            },
        );
        evict(
            &mut self.thumbnails,
            &mut self.thumbnail_order,
            self.thumbnail_limit,
        );
        Ok(())
    }
    pub fn put_preview(&mut self, key: PreviewKey, image: MediaImage) -> Result<(), MediaError> {
        image.validate()?;
        touch(&mut self.preview_order, &key);
        self.previews.insert(
            key,
            CachedImage {
                image,
                received_at: SystemTime::now(),
            },
        );
        evict(
            &mut self.previews,
            &mut self.preview_order,
            self.preview_limit,
        );
        Ok(())
    }
    pub fn thumbnail(&mut self, key: &ThumbnailKey) -> Option<CachedImage> {
        let value = self.thumbnails.get(key)?.clone();
        touch(&mut self.thumbnail_order, key);
        Some(value)
    }
    pub fn preview(&mut self, key: &PreviewKey) -> Option<CachedImage> {
        let value = self.previews.get(key)?.clone();
        touch(&mut self.preview_order, key);
        Some(value)
    }
    pub fn clear_fixture(&mut self, fixture: &str) {
        self.thumbnails.retain(|key, _| key.fixture != fixture);
        self.previews.retain(|key, _| key.fixture != fixture);
        self.thumbnail_order.retain(|key| key.fixture != fixture);
        self.preview_order.retain(|key| key.fixture != fixture);
    }
    pub fn retain_fixtures(&mut self, fixtures: &std::collections::HashSet<String>) {
        self.thumbnails
            .retain(|key, _| fixtures.contains(&key.fixture));
        self.previews
            .retain(|key, _| fixtures.contains(&key.fixture));
        self.thumbnail_order
            .retain(|key| fixtures.contains(&key.fixture));
        self.preview_order
            .retain(|key| fixtures.contains(&key.fixture));
    }
}
fn touch<K: Eq + Clone>(order: &mut VecDeque<K>, key: &K) {
    if let Some(index) = order.iter().position(|candidate| candidate == key) {
        order.remove(index);
    }
    order.push_back(key.clone());
}
fn evict<K: Eq + std::hash::Hash + Clone, V>(
    values: &mut HashMap<K, V>,
    order: &mut VecDeque<K>,
    limit: usize,
) {
    while values.len() > limit {
        if let Some(key) = order.pop_front() {
            values.remove(&key);
        } else {
            break;
        }
    }
}

#[derive(Clone, Debug)]
struct Packet {
    version: (u8, u8),
    request_index: u16,
    content: [u8; 4],
    payload: Vec<u8>,
}
struct Fragment {
    version: (u8, u8),
    request_index: u16,
    content: [u8; 4],
    part_count: u16,
    part: u16,
    payload: Vec<u8>,
}

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
        let mut payload = Vec::with_capacity(16 + elements.len());
        payload.extend_from_slice(&ImageFormat::Jpeg.cookie());
        payload.extend_from_slice(&width.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.push(1);
        payload.push(library_type);
        if self.negotiated_version == (1, 0) {
            payload.push(library.ids[0]);
            payload.push(elements.len() as u8);
        } else {
            library.encode(&mut payload);
            if self.negotiated_version >= (1, 2) {
                payload.extend_from_slice(&(elements.len() as u16).to_le_bytes());
            } else {
                payload.push(elements.len() as u8);
            }
        }
        payload.extend_from_slice(elements);
        let request = self.send(*b"GETh", payload).await?;
        let expected = elements.len();
        let mut images = Vec::with_capacity(expected);
        while images.len() < expected {
            let packet = self.receive_relevant(*b"EThn", request).await?;
            images.push(parse_thumbnail(&packet.payload, packet.version)?.1);
        }
        Ok(images.into_iter().map(|image| (image.0, image.1)).collect())
    }
    pub async fn request_preview(
        &mut self,
        source: u16,
        width: u16,
        height: u16,
    ) -> Result<MediaImage, MediaError> {
        if width == 0 || height == 0 || width > 2048 || height > 2048 {
            return Err(MediaError::Invalid("invalid preview request bounds".into()));
        }
        let mut payload = Vec::with_capacity(13);
        payload.extend_from_slice(&source.to_le_bytes());
        payload.extend_from_slice(&ImageFormat::Jpeg.cookie());
        payload.extend_from_slice(&width.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.push(1);
        payload.push(0);
        let request = self.send(*b"RqSt", payload).await?;
        let packet = self.receive_relevant(*b"StFr", request).await?;
        let (received_source, image) = parse_stream_frame(&packet.payload, packet.version)?;
        if received_source != source {
            return Err(MediaError::Invalid(
                "media server returned a different preview source".into(),
            ));
        }
        Ok(image)
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
        let first = self.read_fragment().await?;
        if first.part != 0 || first.part_count == 0 || first.part_count > 256 {
            return Err(MediaError::Invalid("invalid CITP fragment sequence".into()));
        }
        let mut payload = first.payload;
        for expected in 1..first.part_count {
            let fragment = self.read_fragment().await?;
            if fragment.part != expected
                || fragment.part_count != first.part_count
                || fragment.version != first.version
                || fragment.request_index != first.request_index
                || fragment.content != first.content
            {
                return Err(MediaError::Invalid(
                    "inconsistent CITP fragment sequence".into(),
                ));
            }
            if payload.len().saturating_add(fragment.payload.len()) > MAX_PACKET_BYTES {
                return Err(MediaError::Invalid(
                    "reassembled CITP message exceeds size limit".into(),
                ));
            }
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
        if &header[..4] != b"CITP" || &header[16..20] != b"MSEX" {
            return Err(MediaError::Invalid(
                "invalid CITP/MSEX header cookie".into(),
            ));
        }
        let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if !(HEADER_BYTES..=MAX_PACKET_BYTES).contains(&size) {
            return Err(MediaError::Invalid("invalid message size".into()));
        }
        let parts = u16::from_le_bytes(header[12..14].try_into().unwrap());
        let part = u16::from_le_bytes(header[14..16].try_into().unwrap());
        let mut rest = vec![0; size - 20];
        timeout(self.operation_timeout, self.stream.read_exact(&mut rest))
            .await
            .map_err(|_| MediaError::Timeout)??;
        let version = (rest[0], rest[1]);
        if version.0 != 1 || version.1 > 2 {
            return Err(MediaError::Invalid(format!(
                "unsupported MSEX version {}.{}",
                version.0, version.1
            )));
        }
        let content = rest[2..6].try_into().unwrap();
        Ok(Fragment {
            version,
            request_index: u16::from_le_bytes(header[6..8].try_into().unwrap()),
            content,
            part_count: parts,
            part,
            payload: rest[6..].to_vec(),
        })
    }
}

fn encode_packet(
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

fn parse_thumbnail(
    payload: &[u8],
    version: (u8, u8),
) -> Result<(LibraryId, (u8, MediaImage)), MediaError> {
    let (library, element_offset) = if version >= (1, 1) {
        if payload.len() < 16 {
            return Err(MediaError::Invalid("truncated EThn packet".into()));
        }
        (
            LibraryId {
                level: payload[1],
                ids: payload[2..5].try_into().unwrap(),
            },
            5,
        )
    } else {
        if payload.len() < 13 {
            return Err(MediaError::Invalid("truncated EThn packet".into()));
        }
        (
            LibraryId {
                level: 1,
                ids: [payload[1], 0, 0],
            },
            2,
        )
    };
    let element = payload[element_offset];
    let format_offset = element_offset + 1;
    let format = ImageFormat::parse(
        payload[format_offset..format_offset + 4]
            .try_into()
            .unwrap(),
    )?;
    let width = u16::from_le_bytes(
        payload[format_offset + 4..format_offset + 6]
            .try_into()
            .unwrap(),
    );
    let height = u16::from_le_bytes(
        payload[format_offset + 6..format_offset + 8]
            .try_into()
            .unwrap(),
    );
    let length = u16::from_le_bytes(
        payload[format_offset + 8..format_offset + 10]
            .try_into()
            .unwrap(),
    ) as usize;
    let data = &payload[format_offset + 10..];
    if data.len() != length {
        return Err(MediaError::Invalid("EThn buffer length mismatch".into()));
    }
    let image = MediaImage {
        format,
        width,
        height,
        bytes: data.to_vec(),
    };
    image.validate()?;
    Ok((library, (element, image)))
}
fn parse_stream_frame(payload: &[u8], version: (u8, u8)) -> Result<(u16, MediaImage), MediaError> {
    let offset = if version >= (1, 2) { 36 } else { 0 };
    if payload.len() < offset + 12 {
        return Err(MediaError::Invalid("truncated StFr packet".into()));
    }
    let source = u16::from_le_bytes(payload[offset..offset + 2].try_into().unwrap());
    let format = ImageFormat::parse(payload[offset + 2..offset + 6].try_into().unwrap())?;
    let width = u16::from_le_bytes(payload[offset + 6..offset + 8].try_into().unwrap());
    let height = u16::from_le_bytes(payload[offset + 8..offset + 10].try_into().unwrap());
    let length = u16::from_le_bytes(payload[offset + 10..offset + 12].try_into().unwrap()) as usize;
    let data = &payload[offset + 12..];
    if data.len() != length {
        return Err(MediaError::Invalid("StFr buffer length mismatch".into()));
    }
    let image = MediaImage {
        format,
        width,
        height,
        bytes: data.to_vec(),
    };
    image.validate()?;
    Ok((source, image))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn read_wire_packet(stream: &mut TcpStream) -> Vec<u8> {
        let mut header = [0; 20];
        stream.read_exact(&mut header).await.unwrap();
        let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        let mut packet = header.to_vec();
        packet.resize(size, 0);
        stream.read_exact(&mut packet[20..]).await.unwrap();
        packet
    }
    #[tokio::test]
    async fn negotiates_and_retrieves_thumbnail_and_preview() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let cinf = read_wire_packet(&mut stream).await;
            assert_eq!(&cinf[22..26], b"CInf");
            stream
                .write_all(&encode_packet((1, 2), 1, *b"SInf", &[]))
                .await
                .unwrap();
            let geth = read_wire_packet(&mut stream).await;
            assert_eq!(&geth[22..26], b"GETh");
            let mut thumbnail = vec![1, 0, 0, 0, 0, 7];
            thumbnail.extend_from_slice(b"JPEG");
            thumbnail.extend_from_slice(&2_u16.to_le_bytes());
            thumbnail.extend_from_slice(&1_u16.to_le_bytes());
            thumbnail.extend_from_slice(&3_u16.to_le_bytes());
            thumbnail.extend_from_slice(&[1, 2, 3]);
            stream
                .write_all(&encode_packet((1, 2), 2, *b"EThn", &thumbnail))
                .await
                .unwrap();
            let rqst = read_wire_packet(&mut stream).await;
            assert_eq!(&rqst[22..26], b"RqSt");
            let mut frame = vec![b'a'; 36];
            frame.extend_from_slice(&4_u16.to_le_bytes());
            frame.extend_from_slice(b"JPEG");
            frame.extend_from_slice(&2_u16.to_le_bytes());
            frame.extend_from_slice(&1_u16.to_le_bytes());
            frame.extend_from_slice(&3_u16.to_le_bytes());
            frame.extend_from_slice(&[4, 5, 6]);
            stream
                .write_all(&encode_packet((1, 2), 3, *b"StFr", &frame))
                .await
                .unwrap();
        });
        let mut client = CitpClient::connect(address, Duration::from_secs(1))
            .await
            .unwrap();
        let thumbnails = client
            .request_thumbnail(1, LibraryId::ROOT, &[7], 64, 64)
            .await
            .unwrap();
        assert_eq!(thumbnails[0].0, 7);
        assert_eq!(thumbnails[0].1.bytes, [1, 2, 3]);
        let preview = client.request_preview(4, 64, 64).await.unwrap();
        assert_eq!(preview.bytes, [4, 5, 6]);
        server.await.unwrap();
    }
    #[tokio::test]
    async fn honors_legacy_msex_version_and_thumbnail_layout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_wire_packet(&mut stream).await;
            stream
                .write_all(&encode_packet((1, 0), 1, *b"SInf", &[]))
                .await
                .unwrap();
            let geth = read_wire_packet(&mut stream).await;
            assert_eq!(&geth[20..22], &[1, 0]);
            assert_eq!(&geth[35..39], &[1, 9, 1, 4]);
            let mut thumbnail = vec![1, 9, 4];
            thumbnail.extend_from_slice(b"JPEG");
            thumbnail.extend_from_slice(&1_u16.to_le_bytes());
            thumbnail.extend_from_slice(&1_u16.to_le_bytes());
            thumbnail.extend_from_slice(&1_u16.to_le_bytes());
            thumbnail.push(42);
            stream
                .write_all(&encode_packet((1, 0), 2, *b"EThn", &thumbnail))
                .await
                .unwrap();
        });
        let mut client = CitpClient::connect(address, Duration::from_secs(1))
            .await
            .unwrap();
        let images = client
            .request_thumbnail(
                1,
                LibraryId {
                    level: 1,
                    ids: [9, 0, 0],
                },
                &[4],
                64,
                64,
            )
            .await
            .unwrap();
        assert_eq!(images[0].0, 4);
        assert_eq!(images[0].1.bytes, [42]);
        server.await.unwrap();
    }
    #[tokio::test]
    async fn reassembles_ordered_citp_fragments() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_wire_packet(&mut stream).await;
            stream
                .write_all(&encode_packet((1, 2), 1, *b"SInf", &[]))
                .await
                .unwrap();
            let _ = read_wire_packet(&mut stream).await;
            let mut payload = vec![1, 0, 0, 0, 0, 7];
            payload.extend_from_slice(b"JPEG");
            payload.extend_from_slice(&1_u16.to_le_bytes());
            payload.extend_from_slice(&1_u16.to_le_bytes());
            payload.extend_from_slice(&4_u16.to_le_bytes());
            payload.extend_from_slice(&[1, 2, 3, 4]);
            let split = 9;
            for (part, bytes) in [payload[..split].to_vec(), payload[split..].to_vec()]
                .into_iter()
                .enumerate()
            {
                let mut packet = encode_packet((1, 2), 2, *b"EThn", &bytes);
                packet[12..14].copy_from_slice(&2_u16.to_le_bytes());
                packet[14..16].copy_from_slice(&(part as u16).to_le_bytes());
                stream.write_all(&packet).await.unwrap();
            }
        });
        let mut client = CitpClient::connect(address, Duration::from_secs(1))
            .await
            .unwrap();
        let images = client
            .request_thumbnail(1, LibraryId::ROOT, &[7], 64, 64)
            .await
            .unwrap();
        assert_eq!(images[0].1.bytes, [1, 2, 3, 4]);
        server.await.unwrap();
    }
    #[test]
    fn cache_is_bounded_and_fixture_scoped() {
        let image = || MediaImage {
            format: ImageFormat::Jpeg,
            width: 1,
            height: 1,
            bytes: vec![1],
        };
        let mut cache = MediaCache::new(1, 1);
        let a = ThumbnailKey {
            fixture: "a".into(),
            library_type: 1,
            library: LibraryId::ROOT,
            element: 1,
        };
        let b = ThumbnailKey {
            fixture: "b".into(),
            library_type: 1,
            library: LibraryId::ROOT,
            element: 2,
        };
        cache.put_thumbnail(a.clone(), image()).unwrap();
        cache.put_thumbnail(b.clone(), image()).unwrap();
        assert!(cache.thumbnail(&a).is_none());
        assert!(cache.thumbnail(&b).is_some());
        cache.clear_fixture("b");
        assert!(cache.thumbnail(&b).is_none());
    }
    #[test]
    fn rejects_malformed_image_lengths() {
        let mut payload = vec![1, 0, 0, 0, 0, 7];
        payload.extend_from_slice(b"JPEG");
        payload.extend_from_slice(&2_u16.to_le_bytes());
        payload.extend_from_slice(&1_u16.to_le_bytes());
        payload.extend_from_slice(&4_u16.to_le_bytes());
        payload.extend_from_slice(&[1, 2, 3]);
        assert!(parse_thumbnail(&payload, (1, 2)).is_err());
    }
}
