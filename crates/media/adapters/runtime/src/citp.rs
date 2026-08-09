//! The CITP service: sockets, and what this server publishes on them.
//!
//! Discovery is a UDP announcement on the CITP group; requests arrive on a TCP connection per
//! console. Everything a message *means* is decided in `media-citp`, which has no sockets, so what
//! is left here is carrying bytes and answering from the live catalog and state.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use media_application::MediaConfiguration;
use media_citp::message::{LibraryElement, LibraryFolder, Thumbnail, ThumbnailRequest};
use media_citp::server::{Identity, Library, Sessions};
use media_citp::{MULTICAST_GROUP, packet};
use media_domain::catalog::CatalogSnapshot;
use media_domain::{MediaAddress, SourceStatus};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, UdpSocket};

use crate::dmx::SharedState;
use crate::presentation::SharedCatalog;
use crate::shutdown::Shutdown;

/// How often a media server announces itself, matching what consoles expect to hear.
const ANNOUNCE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
/// How often a connected console is told what the layers are doing.
const STATUS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
/// A console that sends more than this without a complete message is not talking CITP.
const READ_BUFFER: usize = 8192;

/// The library, answered from the published catalog and the thumbnails on disk.
struct PublishedLibrary {
    catalog: Arc<CatalogSnapshot>,
    storage: media_library::LibraryStorage,
}

impl Library for PublishedLibrary {
    fn folders(&self) -> Vec<LibraryFolder> {
        self.catalog
            .folders
            .iter()
            .map(|folder| LibraryFolder {
                number: folder.folder,
                name: folder
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Folder {:03}", folder.folder)),
                element_count: folder.items.len().min(255) as u8,
            })
            .collect()
    }

    fn elements(&self, folder: u8) -> Vec<LibraryElement> {
        self.catalog.folder(folder).map_or_else(Vec::new, |found| {
            found
                .items
                .iter()
                .map(|item| LibraryElement {
                    number: item.file,
                    name: item.name.clone(),
                    width: item.width.min(u32::from(u16::MAX)) as u16,
                    height: item.height.min(u32::from(u16::MAX)) as u16,
                    length_frames: item.frames.unwrap_or(0),
                    fps: 25,
                })
                .collect()
        })
    }

    fn thumbnail(
        &self,
        folder: u8,
        element: Option<u8>,
        _request: &ThumbnailRequest,
    ) -> Option<Thumbnail> {
        // A folder's own picture is its first item's, which is what an operator recognises the
        // folder by. Resizing to the console's exact request would need a decoder in the request
        // path; CITP carries the real dimensions, so the stored thumbnail is sent as it is.
        let file = element.or_else(|| {
            self.catalog
                .folder(folder)
                .and_then(|found| found.items.first())
                .map(|item| item.file)
        })?;
        let path = self.storage.thumbnail_path(MediaAddress::new(folder, file));
        let jpeg = std::fs::read(path).ok()?;
        let (width, height) = jpeg_size(&jpeg)?;
        let thumbnail = Thumbnail {
            width,
            height,
            jpeg,
        };
        thumbnail.fits().then_some(thumbnail)
    }

    fn timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs())
    }
}

/// The dimensions a JPEG declares, read from its frame header.
///
/// CITP publishes a thumbnail's real size, and a console lays out its grid from it, so a guess
/// would visibly distort every picture.
fn jpeg_size(jpeg: &[u8]) -> Option<(u16, u16)> {
    let mut at = 2; // past the start-of-image marker
    while at + 9 < jpeg.len() {
        if jpeg[at] != 0xFF {
            at += 1;
            continue;
        }
        let marker = jpeg[at + 1];
        // The start-of-frame markers, excluding the ones that are not frames.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = u16::from_be_bytes([jpeg[at + 5], jpeg[at + 6]]);
            let width = u16::from_be_bytes([jpeg[at + 7], jpeg[at + 8]]);
            return Some((width, height));
        }
        let length = u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]) as usize;
        at += 2 + length.max(2);
    }
    None
}

/// Everything the CITP tasks share.
#[derive(Clone)]
struct Service {
    name: String,
    listening_port: u16,
    preview: crate::preview::SharedPreview,
    state: SharedState,
    catalog: SharedCatalog,
    storage: media_library::LibraryStorage,
    layers: u8,
}

impl Service {
    fn identity(&self) -> Identity<'_> {
        Identity {
            name: &self.name,
            listening_port: self.listening_port,
            layers: self.layers,
            // The preview size a console is offered. The program output is read back at whatever
            // a subscriber asks for; this is only what the source advertises.
            preview_width: 320,
            preview_height: 180,
        }
    }

    fn library(&self) -> PublishedLibrary {
        PublishedLibrary {
            catalog: Arc::clone(&self.catalog.load_full()),
            storage: self.storage.clone(),
        }
    }

    /// What every layer of the first output is doing, as a console reads it.
    fn layer_status(&self) -> Vec<media_citp::LayerStatus> {
        let state = self.state.load();
        let catalog = self.catalog.load();
        let Some(output) = state.outputs.first() else {
            return Vec::new();
        };

        output
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| media_citp::LayerStatus {
                layer: index.min(255) as u8,
                physical_output: 0,
                folder: layer.address.folder,
                file: layer.address.file,
                name: catalog
                    .resolve(layer.address)
                    .map(|item| item.name.clone())
                    .unwrap_or_default(),
                position_frames: 0,
                length_frames: catalog
                    .resolve(layer.address)
                    .and_then(|item| item.frames)
                    .unwrap_or(0),
                fps: 25,
                status: layer.source_status,
                playing: layer.draws() && layer.source_status == SourceStatus::Ready,
            })
            .collect()
    }
}

/// Starts discovery and the request listener.
///
/// A bind failure is reported and the rest of the server keeps running: a console that cannot find
/// this machine is a problem, and a program output that stopped because of it would be a disaster.
pub fn spawn(
    configuration: &MediaConfiguration,
    state: SharedState,
    catalog: SharedCatalog,
    preview: crate::preview::SharedPreview,
    shutdown: Shutdown,
) {
    let listen = configuration.network.resolved().citp_listen;
    let service = Service {
        name: format!("ToskLight Media — {}", configuration.instance_id.as_str()),
        listening_port: listen.port(),
        preview,
        state,
        catalog,
        storage: media_library::LibraryStorage::new(configuration.library.root.clone()),
        layers: configuration
            .outputs
            .first()
            .map_or(8, |output| output.personality.layer_count().min(255) as u8),
    };
    tokio::spawn(announce(service.clone(), shutdown.clone()));
    tokio::spawn(listen_for_consoles(service, listen, shutdown));
}

/// Announces this server, and answers a console that announced itself.
async fn announce(service: Service, shutdown: Shutdown) {
    let socket = match UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!(%error, "CITP discovery could not open a socket; consoles will not find this server");
            return;
        }
    };
    if let Err(error) = socket.set_broadcast(true) {
        tracing::warn!(%error, "CITP discovery cannot broadcast");
    }

    let group = SocketAddr::from((Ipv4Addr::from(MULTICAST_GROUP), media_citp::CITP_PORT));
    let announcement = media_citp::announcement(&service.name, service.listening_port);
    let mut ticker = tokio::time::interval(ANNOUNCE_INTERVAL);
    let mut watcher = shutdown.watcher();
    let mut stopping = Box::pin(watcher.wait());

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(error) = socket.send_to(&announcement, group).await {
                    tracing::debug!(%error, "CITP announcement could not be sent");
                }
            }
            _ = &mut stopping => return,
        }
    }
}

/// Accepts consoles and serves each one.
async fn listen_for_consoles(service: Service, listen: SocketAddr, shutdown: Shutdown) {
    let listener = match TcpListener::bind(listen).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!(
                address = %listen,
                %error,
                "cannot listen for CITP consoles; another process already holds that port"
            );
            return;
        }
    };
    tracing::info!(address = %listen, "listening for CITP consoles");

    let mut watcher = shutdown.watcher();
    let mut stopping = Box::pin(watcher.wait());
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, peer)) => {
                    tracing::info!(%peer, "a console connected");
                    tokio::spawn(serve_console(service.clone(), stream, peer, shutdown.clone()));
                }
                Err(error) => tracing::warn!(%error, "a console could not be accepted"),
            },
            _ = &mut stopping => return,
        }
    }
}

/// One console, for as long as it stays connected.
async fn serve_console(
    service: Service,
    mut stream: tokio::net::TcpStream,
    peer: SocketAddr,
    shutdown: Shutdown,
) {
    let mut sessions = Sessions::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; READ_BUFFER];
    let started = std::time::Instant::now();

    // The greeting goes out before the console asks anything, so it knows what it reached.
    if stream
        .write_all(&media_citp::greeting(&service.identity()))
        .await
        .is_err()
    {
        return;
    }

    let mut status = tokio::time::interval(STATUS_INTERVAL);
    // What this connection last told the outputs it wanted, so a change is reported once rather
    // than every tick.
    let mut subscribed = false;
    let mut sent_sequence = 0u64;
    let mut watcher = shutdown.watcher();
    let mut stopping = Box::pin(watcher.wait());

    loop {
        tokio::select! {
            read = stream.read(&mut buffer) => {
                let Ok(count) = read else { break };
                if count == 0 {
                    break; // the console hung up
                }
                pending.extend_from_slice(&buffer[..count]);

                let messages = match packet::take_messages(&mut pending) {
                    Ok(messages) => messages,
                    Err(error) => {
                        tracing::warn!(%peer, %error, "a console sent something that is not CITP");
                        break;
                    }
                };
                for message in messages {
                    let now = started.elapsed().as_millis() as u64;
                    let replies = media_citp::respond(
                        &message,
                        &service.identity(),
                        &service.library(),
                        &mut sessions,
                        now,
                    );
                    for reply in replies {
                        if stream.write_all(&reply).await.is_err() {
                            return;
                        }
                    }
                }
            }
            _ = status.tick() => {
                if stream
                    .write_all(&media_citp::status(&service.layer_status()))
                    .await
                    .is_err()
                {
                    return;
                }

                let now = started.elapsed().as_millis() as u64;
                let wanted = sessions.anyone_subscribed(now);
                if wanted != subscribed {
                    subscribed = wanted;
                    service.preview.subscribed(wanted, sessions.requested_size(now));
                }

                // A frame is sent only when there is a new one: a console asked for ten a second,
                // not for the same picture sixty times.
                if let Some((sequence, frame)) = service.preview.latest()
                    && sequence != sent_sequence
                {
                    sent_sequence = sequence;
                    for message in sessions.frames_for(&frame, sequence, now) {
                        if stream.write_all(&message).await.is_err() {
                            return;
                        }
                    }
                }
            }
            _ = &mut stopping => return,
        }
    }
    if subscribed {
        // A console that closed without unsubscribing must not leave the outputs capturing.
        service.preview.subscribed(false, None);
    }
    tracing::info!(%peer, "a console disconnected");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest possible JPEG frame header: SOI, a start-of-frame with one 16×8 image.
    fn tiny_jpeg() -> Vec<u8> {
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        jpeg.extend_from_slice(&8u16.to_be_bytes());
        jpeg.extend_from_slice(&16u16.to_be_bytes());
        jpeg.extend_from_slice(&[0; 8]);
        jpeg
    }

    #[test]
    fn a_thumbnail_publishes_the_size_it_really_is() {
        assert_eq!(jpeg_size(&tiny_jpeg()), Some((16, 8)));
    }

    #[test]
    fn something_that_is_not_a_jpeg_publishes_no_size_rather_than_a_wrong_one() {
        assert_eq!(jpeg_size(&[]), None);
        assert_eq!(jpeg_size(&[0xFF, 0xD8]), None);
        assert_eq!(jpeg_size(&[0; 64]), None);
    }

    #[test]
    fn a_marker_that_is_not_a_frame_is_skipped_rather_than_read_as_one() {
        // 0xC4 is a Huffman table, not a frame; reading it as one would publish nonsense.
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xC4, 0x00, 0x06, 1, 2, 3, 4];
        jpeg.extend_from_slice(&tiny_jpeg()[2..]);
        assert_eq!(jpeg_size(&jpeg), Some((16, 8)));
    }
}
