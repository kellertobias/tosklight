#![forbid(unsafe_code)]
//! Bounded CITP/MSEX 1.2 client primitives for media thumbnails and output previews.

mod cache;
mod client;
mod model;
mod protocol;

pub use cache::MediaCache;
pub use client::{CitpClient, CitpPreviewSubscription};
pub use model::{
    CachedImage, ImageFormat, LibraryId, MediaControlCapability, MediaError, MediaImage,
    MediaLayerCapabilities, MediaLayerStatus, MediaLibraryElement, MediaLibraryFolder,
    MediaPreviewSource, MediaProviderCapabilities, MediaServerInformation, MediaServerSnapshot,
    PreviewKey, ThumbnailKey,
};

pub const DEFAULT_CITP_PORT: u16 = 4809;
pub const CITP_MULTICAST_GROUP: [u8; 4] = [224, 0, 0, 180];

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredCitpServer {
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// Actively asks the CITP group for Media Server identities and listens on the probe's ephemeral
/// port for bounded replies. It never claims the shared 4809 receive port, so discovery can run
/// beside a local server and several planning windows.
pub async fn discover_servers(
    wait: std::time::Duration,
) -> Result<Vec<DiscoveredCitpServer>, MediaError> {
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    let group = std::net::SocketAddr::from((
        std::net::Ipv4Addr::from(CITP_MULTICAST_GROUP),
        DEFAULT_CITP_PORT,
    ));
    let announcement = console_announcement("ToskLight Control");
    socket.send_to(&announcement, group).await?;
    // The Media Server's same-computer preset deliberately binds CITP to loopback. Multicast
    // does not cross that boundary on every operating system, so probe the standard local port
    // as well. A server listening on all interfaces may answer both; the endpoint map below
    // deduplicates those replies.
    let _ = socket
        .send_to(
            &announcement,
            std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, DEFAULT_CITP_PORT)),
        )
        .await;
    let deadline = tokio::time::Instant::now() + wait;
    let mut found = std::collections::BTreeMap::new();
    let mut bytes = [0_u8; 2048];
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let received = tokio::time::timeout(remaining, socket.recv_from(&mut bytes)).await;
        let Ok(Ok((count, peer))) = received else {
            break;
        };
        let Some((port, kind, name, state)) = parse_peer_location(&bytes[..count]) else {
            continue;
        };
        if kind != "MediaServer" || state != "Running" || name.trim().is_empty() {
            continue;
        }
        let port = if port == 0 { DEFAULT_CITP_PORT } else { port };
        let server = DiscoveredCitpServer {
            name,
            host: peer.ip().to_string(),
            port,
        };
        found.insert((server.host.clone(), server.port), server);
    }
    Ok(found.into_values().collect())
}

fn parse_peer_location(bytes: &[u8]) -> Option<(u16, String, String, String)> {
    if bytes.len() < 26 || &bytes[..4] != b"CITP" || &bytes[16..24] != b"PINFPLoc" {
        return None;
    }
    let port = u16::from_le_bytes(bytes[24..26].try_into().ok()?);
    let mut at = 26;
    let mut string = || {
        let tail = bytes.get(at..)?;
        let length = tail.iter().position(|byte| *byte == 0)?;
        let value = String::from_utf8_lossy(&tail[..length]).into_owned();
        at += length + 1;
        Some(value)
    };
    Some((port, string()?, string()?, string()?))
}

/// Announces the Light Desk's active show as a CITP lighting-console identity.
pub fn console_announcement(show_name: &str) -> Vec<u8> {
    fn ucs1(target: &mut Vec<u8>, value: &str) {
        target.extend(
            value
                .bytes()
                .map(|byte| if byte.is_ascii() { byte } else { b'?' }),
        );
        target.push(0);
    }

    let mut body = Vec::new();
    body.extend_from_slice(&0_u16.to_le_bytes());
    ucs1(&mut body, "LightingConsole");
    ucs1(&mut body, show_name);
    ucs1(&mut body, "Running");
    let size = 24 + body.len();
    let mut message = Vec::with_capacity(size);
    message.extend_from_slice(b"CITP");
    message.extend_from_slice(&[1, 0]);
    message.extend_from_slice(&0_u16.to_le_bytes());
    message.extend_from_slice(&(size as u32).to_le_bytes());
    message.extend_from_slice(&1_u16.to_le_bytes());
    message.extend_from_slice(&0_u16.to_le_bytes());
    message.extend_from_slice(b"PINFPLoc");
    message.extend_from_slice(&body);
    message
}

#[cfg(test)]
mod tests;
