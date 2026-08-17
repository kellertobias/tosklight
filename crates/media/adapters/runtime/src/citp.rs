//! The CITP service: sockets, and what this server publishes on them.
//!
//! Discovery is a UDP announcement on the CITP group; requests arrive on a TCP connection per
//! console. Everything a message *means* is decided in `media-citp`, which has no sockets, so what
//! is left here is carrying bytes and answering from the live catalog and state.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use media_application::MediaConfiguration;
use media_citp::message::{LibraryElement, LibraryFolder, Thumbnail, ThumbnailRequest};
use media_citp::server::{Identity, Library, Sessions};
use media_citp::{MULTICAST_GROUP, packet};
use media_domain::catalog::CatalogSnapshot;
use media_domain::{Countdown, MediaAddress, Size, SourceStatus, VisualizerKind};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, UdpSocket};

use crate::dmx::SharedState;
use crate::presentation::{SharedCatalog, SharedConfiguration};
use crate::shutdown::Shutdown;

/// How often a media server announces itself, matching what consoles expect to hear.
const ANNOUNCE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
/// How often a connected console is told what the layers are doing.
const STATUS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
/// A console that sends more than this without a complete message is not talking CITP.
const READ_BUFFER: usize = 8192;
const CONSOLE_IDENTITY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// The most recent running Light Desk identity received over CITP discovery.
#[derive(Clone, Default)]
pub struct ConsoleIdentity {
    latest: Arc<Mutex<Option<(String, std::time::Instant)>>>,
}

impl ConsoleIdentity {
    fn observe(&self, message: &media_citp::Message) -> bool {
        let Some(peer) = media_citp::read_peer_location(message) else {
            return false;
        };
        if peer.kind != "LightingConsole" || peer.state != "Running" || peer.name.trim().is_empty()
        {
            return false;
        }
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some((peer.name, std::time::Instant::now()));
        }
        true
    }

    fn observe_datagram(&self, datagram: &[u8]) -> bool {
        let mut bytes = datagram.to_vec();
        let mut console = false;
        if let Ok(messages) = packet::take_messages(&mut bytes) {
            for message in messages {
                console |= self.observe(&message);
            }
        }
        console
    }

    pub fn snapshot(&self) -> Option<media_http::DeskIdentityTelemetry> {
        let latest = self.latest.lock().ok()?;
        let (show_name, seen) = latest.as_ref()?;
        (seen.elapsed() < CONSOLE_IDENTITY_TIMEOUT).then(|| media_http::DeskIdentityTelemetry {
            show_name: show_name.clone(),
        })
    }
}

/// The library, answered from the published catalog and the thumbnails on disk.
struct PublishedLibrary {
    catalog: Arc<CatalogSnapshot>,
    storage: media_library::LibraryStorage,
    configuration: SharedConfiguration,
    fonts: Arc<Mutex<Option<media_text::Fonts>>>,
}

impl Library for PublishedLibrary {
    fn folders(&self) -> Vec<LibraryFolder> {
        let mut folders = self
            .catalog
            .folders
            .iter()
            .filter(|folder| folder.folder <= 199)
            .map(|folder| LibraryFolder {
                number: folder.folder as u8,
                name: folder
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Folder {:03}", folder.folder)),
                element_count: folder.items.len().min(255) as u8,
            })
            .collect::<Vec<_>>();
        let configuration = self.configuration.load();
        for folder in 200..=249 {
            let count = configuration
                .text
                .slots
                .iter()
                .filter(|slot| slot.address.folder == folder)
                .count();
            if count > 0 {
                folders.push(LibraryFolder {
                    number: folder,
                    name: format!("Text {folder:03}"),
                    element_count: count.min(255) as u8,
                });
            }
        }
        for folder in 250..=255 {
            let count = configuration
                .visualizers
                .entries
                .iter()
                .filter(|entry| entry.address.folder == folder)
                .count();
            if count > 0 {
                folders.push(LibraryFolder {
                    number: folder,
                    name: format!("Visualizers {folder:03}"),
                    element_count: count.min(255) as u8,
                });
            }
        }
        folders
    }

    fn elements(&self, folder: u8) -> Vec<LibraryElement> {
        if folder <= 199 {
            return self
                .catalog
                .folder(u16::from(folder))
                .map_or_else(Vec::new, |found| {
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
                });
        }
        let configuration = self.configuration.load();
        let dimensions = configuration.outputs.first().map_or((640, 360), |output| {
            (
                output.resolution.width.min(u32::from(u16::MAX)) as u16,
                output.resolution.height.min(u32::from(u16::MAX)) as u16,
            )
        });
        if folder <= 249 {
            return configuration
                .text
                .slots
                .iter()
                .filter(|slot| slot.address.folder == folder)
                .map(|slot| LibraryElement {
                    number: slot.address.file,
                    name: slot.name.clone(),
                    width: dimensions.0,
                    height: dimensions.1,
                    length_frames: 1,
                    fps: 25,
                })
                .collect();
        }
        configuration
            .visualizers
            .entries
            .iter()
            .filter(|entry| entry.address.folder == folder)
            .map(|entry| LibraryElement {
                number: entry.address.file,
                name: entry.configuration.name.clone(),
                width: dimensions.0,
                height: dimensions.1,
                length_frames: 1,
                fps: 25,
            })
            .collect()
    }

    fn thumbnail(
        &self,
        folder: u8,
        element: Option<u8>,
        request: &ThumbnailRequest,
    ) -> Option<Thumbnail> {
        if folder >= 250 {
            let configuration = self.configuration.load();
            let entry = configuration.visualizers.entries.iter().find(|entry| {
                entry.address.folder == folder
                    && element.is_none_or(|file| entry.address.file == file)
            })?;
            return visualizer_thumbnail(entry.configuration.kind, request);
        }
        if folder >= 200 {
            let configuration = self.configuration.load();
            let slot = configuration.text.slots.iter().find(|slot| {
                slot.address.folder == folder
                    && element.is_none_or(|file| slot.address.file == file)
            })?;
            return text_thumbnail(slot, request, &self.fonts);
        }
        // A folder's own picture is its first item's, which is what an operator recognises the
        // folder by. Resizing to the console's exact request would need a decoder in the request
        // path; CITP carries the real dimensions, so the stored thumbnail is sent as it is.
        let file = element.or_else(|| {
            self.catalog
                .folder(u16::from(folder))
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

fn visualizer_thumbnail(kind: VisualizerKind, request: &ThumbnailRequest) -> Option<Thumbnail> {
    let decoder = png::Decoder::new(std::io::Cursor::new(visualizer_preview_png(kind)));
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    let pixels = &pixels[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        _ => return None,
    };
    crate::preview::encode(
        &rgba,
        Size::new(info.width, info.height),
        thumbnail_size(request, info.width, info.height),
    )
    .ok()
}

fn text_thumbnail(
    slot: &media_domain::TextSlot,
    request: &ThumbnailRequest,
    fonts: &Arc<Mutex<Option<media_text::Fonts>>>,
) -> Option<Thumbnail> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as i64;
    let words = media_domain::text::render(&slot.entry, &Countdown::new(), now, 0)?;
    let mut fonts = fonts.lock().ok()?;
    let rendered = media_text::render_line(fonts.as_mut()?, &words, &slot.style, 320, 180).ok()?;
    crate::preview::encode(
        &rendered.pixels,
        Size::new(rendered.width, rendered.height),
        thumbnail_size(request, 320, 180),
    )
    .ok()
}

fn thumbnail_size(request: &ThumbnailRequest, width: u32, height: u32) -> Size {
    let requested = Size::new(u32::from(request.width), u32::from(request.height));
    if !request.preserve_aspect || width == 0 || height == 0 {
        return requested;
    }
    let scale =
        (requested.width as f64 / width as f64).min(requested.height as f64 / height as f64);
    Size::new(
        (width as f64 * scale).round().max(1.0) as u32,
        (height as f64 * scale).round().max(1.0) as u32,
    )
}

fn visualizer_preview_png(kind: VisualizerKind) -> &'static [u8] {
    match kind {
        VisualizerKind::EqualizerBars => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/000-equalizer-bars.png"
        ),
        VisualizerKind::WaveformOscilloscope => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/001-waveform-oscilloscope.png"
        ),
        VisualizerKind::CircularSpectrum => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/002-circular-spectrum.png"
        ),
        VisualizerKind::WaveTerrain => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/003-wave-terrain.png"
        ),
        VisualizerKind::PulsingCircles => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/010-pulsing-circles.png"
        ),
        VisualizerKind::MorphingPolygon => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/011-morphing-polygon.png"
        ),
        VisualizerKind::MinimalistShapes => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/012-minimalist-shapes.png"
        ),
        VisualizerKind::Kaleidoscope => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/013-kaleidoscope.png"
        ),
        VisualizerKind::BeatExplosions => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/020-beat-explosions.png"
        ),
        VisualizerKind::DancingSwarm => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/021-dancing-swarm.png"
        ),
        VisualizerKind::Starfield => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/022-starfield.png"
        ),
        VisualizerKind::LightningTendrils => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/023-lightning-tendrils.png"
        ),
        VisualizerKind::RadiatingRays => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/030-radiating-rays.png"
        ),
        VisualizerKind::StrobeFlash => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/031-strobe-flash.png"
        ),
        VisualizerKind::ColorCycling => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/032-color-cycling.png"
        ),
        VisualizerKind::CrossingLines => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/033-crossing-lines.png"
        ),
        VisualizerKind::DigitalGlitch => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/040-digital-glitch.png"
        ),
        VisualizerKind::CrtScanline => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/041-crt-scanline.png"
        ),
        VisualizerKind::MatrixDigitalRain => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/042-matrix-digital-rain.png"
        ),
        VisualizerKind::RotatingShape => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/050-rotating-3d-shape.png"
        ),
        VisualizerKind::FractalMorph => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/051-fractal-morph.png"
        ),
        VisualizerKind::CityTunnel => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/052-city-tunnel.png"
        ),
        VisualizerKind::GridLandscape => include_bytes!(
            "../../../../../apps/media/src/features/visualizers/previews/053-grid-landscape.png"
        ),
    }
}

/// Everything the CITP tasks share.
#[derive(Clone)]
struct Service {
    name: String,
    listening_port: u16,
    previews: crate::preview::SharedPreviews,
    state: SharedState,
    catalog: SharedCatalog,
    configuration: SharedConfiguration,
    storage: media_library::LibraryStorage,
    fonts: Arc<Mutex<Option<media_text::Fonts>>>,
    layers: u8,
    preview_sources: Vec<media_citp::VideoSource>,
}

impl Service {
    fn identity(&self) -> Identity {
        Identity {
            name: self.name.clone(),
            listening_port: self.listening_port,
            layers: self.layers,
            preview_sources: self.preview_sources.clone(),
        }
    }

    fn library(&self) -> PublishedLibrary {
        PublishedLibrary {
            catalog: Arc::clone(&self.catalog.load_full()),
            configuration: Arc::clone(&self.configuration),
            storage: self.storage.clone(),
            fonts: Arc::clone(&self.fonts),
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
    live_configuration: SharedConfiguration,
    previews: crate::preview::SharedPreviews,
    shutdown: Shutdown,
    console_identity: ConsoleIdentity,
) {
    let listen = configuration.network.resolved().citp_listen;
    let preview_sources = configured_preview_sources(configuration, &previews);
    let service = Service {
        name: format!("ToskLight Media — {}", configuration.instance_id.as_str()),
        listening_port: listen.port(),
        previews,
        state,
        catalog,
        configuration: live_configuration,
        storage: media_library::LibraryStorage::new(configuration.library.root.clone()),
        fonts: Arc::new(Mutex::new(media_text::Fonts::load().ok())),
        layers: configuration
            .outputs
            .iter()
            .map(|output| output.personality.layer_count())
            .max()
            .unwrap_or(8)
            .min(255) as u8,
        preview_sources,
    };
    tokio::spawn(announce(
        service.clone(),
        shutdown.clone(),
        console_identity,
        listen,
    ));
    tokio::spawn(listen_for_consoles(service, listen, shutdown));
}

fn configured_preview_sources(
    configuration: &MediaConfiguration,
    previews: &crate::preview::SharedPreviews,
) -> Vec<media_citp::VideoSource> {
    let mut sources = Vec::new();
    for (physical_output, output) in configuration
        .outputs
        .iter()
        .filter(|output| output.enabled)
        .enumerate()
    {
        let Some(id) = previews.source_for_output(output.id) else {
            continue;
        };
        let physical_output = physical_output.min(u8::MAX as usize) as u8;
        sources.push(media_citp::VideoSource {
            id,
            name: output
                .citp
                .source_name
                .clone()
                .unwrap_or_else(|| format!("{} Program", output.name)),
            physical_output,
            layer: None,
            width: output.resolution.width.min(u32::from(u16::MAX)) as u16,
            height: output.resolution.height.min(u32::from(u16::MAX)) as u16,
        });
        for layer in 0..usize::from(output.personality.layer_count()) {
            let Some(id) = previews.source_for_layer(output.id, layer) else {
                continue;
            };
            sources.push(media_citp::VideoSource {
                id,
                name: format!("{} Layer {}", output.name, layer + 1),
                physical_output,
                layer: Some(layer.min(u8::MAX as usize) as u8),
                width: output.resolution.width.min(u32::from(u16::MAX)) as u16,
                height: output.resolution.height.min(u32::from(u16::MAX)) as u16,
            });
        }
    }
    sources
}

/// Announces this server, and answers a console that announced itself.
async fn announce(
    service: Service,
    shutdown: Shutdown,
    console_identity: ConsoleIdentity,
    listen: SocketAddr,
) {
    let socket = match UdpSocket::bind(listen).await {
        Ok(socket) => socket,
        Err(error) => {
            tracing::error!(%error, "CITP discovery could not open a socket; consoles will not find this server");
            return;
        }
    };
    if let Err(error) = socket.set_broadcast(true) {
        tracing::warn!(%error, "CITP discovery cannot broadcast");
    }

    let multicast = Ipv4Addr::from(MULTICAST_GROUP);
    let interface = match listen.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() => ip,
        _ => Ipv4Addr::UNSPECIFIED,
    };
    if let Err(error) = socket.join_multicast_v4(multicast, interface) {
        tracing::warn!(%error, %multicast, %interface, "CITP discovery cannot join its multicast group");
    }

    let group = SocketAddr::from((multicast, media_citp::CITP_PORT));
    let announcement = media_citp::announcement(&service.name, service.listening_port);
    let mut ticker = tokio::time::interval(ANNOUNCE_INTERVAL);
    let mut buffer = vec![0_u8; READ_BUFFER];
    let mut watcher = shutdown.watcher();
    let mut stopping = Box::pin(watcher.wait());

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(error) = socket.send_to(&announcement, group).await {
                    tracing::debug!(%error, "CITP announcement could not be sent");
                }
            }
            received = socket.recv_from(&mut buffer) => {
                match received {
                    Ok((count, peer)) => {
                        if console_identity.observe_datagram(&buffer[..count])
                            && let Err(error) = socket.send_to(&announcement, peer).await
                        {
                            tracing::debug!(%error, %peer, "CITP discovery reply could not be sent");
                        }
                    }
                    Err(error) => tracing::debug!(%error, "CITP discovery datagram could not be read"),
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
    // Some CITP consumers receive StFr on the standard multicast group even though the RqSt
    // lifecycle travels over TCP. Multicast is additive: the requesting TCP peer always receives
    // the same bounded frame directly as well.
    let multicast = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))
        .await
        .ok();
    let multicast_group =
        SocketAddr::from((Ipv4Addr::from(MULTICAST_GROUP), media_citp::CITP_PORT));
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
    let mut subscribed_sources = std::collections::BTreeSet::new();
    let mut sent_sequences = std::collections::BTreeMap::new();
    let mut watcher = shutdown.watcher();
    let mut stopping = Box::pin(watcher.wait());

    'connected: loop {
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
                            break 'connected;
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
                    break 'connected;
                }

                let now = started.elapsed().as_millis() as u64;
                let active = sessions.active_sources(now).into_iter().collect::<std::collections::BTreeSet<_>>();
                for source in subscribed_sources.difference(&active) {
                    if let Some(preview) = service.previews.for_source(*source) {
                        preview.subscribed(false, None);
                    }
                }
                for source in active.difference(&subscribed_sources) {
                    if let Some(preview) = service.previews.for_source(*source) {
                        preview.subscribed(true, sessions.requested_size_for(*source, now));
                        if let Some((sequence, _)) = preview.latest() {
                            // A browser refresh asks for one frame at a time. Do not satisfy that
                            // request with the cached frame from before it subscribed; wait for
                            // the renderer capture triggered above so moving output stays live.
                            sessions.acknowledge_cached_sequence(*source, sequence);
                            sent_sequences.insert(*source, sequence);
                        }
                    }
                }
                for source in &active {
                    let Some(preview) = service.previews.for_source(*source) else { continue };
                    preview.requested_size_is(sessions.requested_size_for(*source, now));
                    let Some((sequence, frame)) = preview.latest() else { continue };
                    if sent_sequences.get(source) == Some(&sequence) { continue; }
                    sent_sequences.insert(*source, sequence);
                    for message in sessions.frames_for_source(*source, &frame, sequence, now) {
                        if stream.write_all(&message).await.is_err() {
                            break 'connected;
                        }
                        if let Some(socket) = multicast.as_ref()
                            && let Err(error) = socket.send_to(&message, multicast_group).await
                        {
                            tracing::debug!(%error, "CITP multicast preview frame was not delivered");
                        }
                    }
                }
                subscribed_sources = active;
            }
            _ = &mut stopping => break 'connected,
        }
    }
    for source in subscribed_sources {
        if let Some(preview) = service.previews.for_source(source) {
            preview.subscribed(false, None);
        }
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

    #[test]
    fn generated_text_and_visualizers_are_real_citp_library_entries() {
        let configuration = MediaConfiguration::default();
        let library = PublishedLibrary {
            catalog: Arc::new(CatalogSnapshot::default()),
            storage: media_library::LibraryStorage::new(std::path::PathBuf::new()),
            configuration: Arc::new(arc_swap::ArcSwap::from_pointee(configuration)),
            fonts: Arc::new(Mutex::new(media_text::Fonts::load().ok())),
        };

        let folders = library.folders();
        assert!(folders.iter().any(|folder| folder.number == 200));
        assert!(folders.iter().any(|folder| folder.number == 250));
        assert_eq!(library.elements(200)[0].name, "Clock");
        assert_eq!(library.elements(250)[0].name, "Equalizer Bars");

        let thumbnail = library
            .thumbnail(
                250,
                Some(1),
                &ThumbnailRequest {
                    width: 160,
                    height: 90,
                    preserve_aspect: true,
                    folder: 250,
                    elements: vec![1],
                    folders: Vec::new(),
                },
            )
            .expect("a rendered built-in visualizer has a CITP thumbnail");
        assert_eq!((thumbnail.width, thumbnail.height), (160, 90));
        assert_eq!(&thumbnail.jpeg[..2], &[0xFF, 0xD8]);
    }

    #[tokio::test]
    async fn a_light_console_udp_announcement_identifies_its_active_show() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let announcement = media_citp::peer_location(&media_citp::Presence {
            listening_port: 0,
            kind: "LightingConsole",
            name: "The Tempest",
            state: "Running",
        });

        sender
            .send_to(&announcement, receiver.local_addr().unwrap())
            .await
            .unwrap();
        let mut buffer = [0_u8; 512];
        let (count, _) = receiver.recv_from(&mut buffer).await.unwrap();
        let identity = ConsoleIdentity::default();
        identity.observe_datagram(&buffer[..count]);

        assert_eq!(
            identity.snapshot(),
            Some(media_http::DeskIdentityTelemetry {
                show_name: "The Tempest".to_owned(),
            })
        );
    }

    #[test]
    fn a_stale_console_announcement_is_not_reported_as_connected() {
        let identity = ConsoleIdentity::default();
        *identity.latest.lock().unwrap() = Some((
            "Closed show".to_owned(),
            std::time::Instant::now() - CONSOLE_IDENTITY_TIMEOUT,
        ));

        assert_eq!(identity.snapshot(), None);
    }

    #[test]
    fn every_enabled_logical_output_is_a_stable_named_preview_source() {
        let mut main = media_application::configuration::OutputConfiguration::new("Main");
        main.resolution.width = 1920;
        main.resolution.height = 1080;
        main.citp.source_name = Some("Program A".to_owned());
        let auxiliary = media_application::configuration::OutputConfiguration::new("Aux");
        let configuration = MediaConfiguration {
            outputs: vec![main.clone(), auxiliary.clone()],
            ..Default::default()
        };
        let previews = crate::preview::SharedPreviews::configured(&configuration);
        let sources = configured_preview_sources(&configuration, &previews);
        assert_eq!(
            sources.len(),
            2 + 2 * usize::from(main.personality.layer_count())
        );
        assert_eq!(sources[0].name, "Program A");
        assert_eq!(sources[0].layer, None);
        assert_eq!(sources[1].name, "Main Layer 1");
        assert_eq!(sources[1].layer, Some(0));
        assert_eq!((sources[0].width, sources[0].height), (1920, 1080));
        assert_eq!(sources[0].id, previews.source_for_output(main.id).unwrap());
        assert_ne!(sources[0].id, sources[1].id);
    }
}
