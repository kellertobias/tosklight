//! One console connection, library invalidation, and live stream delivery.

use super::*;

/// One console, for as long as it stays connected.
pub(super) async fn serve_console(
    service: Service,
    mut stream: tokio::net::TcpStream,
    peer: SocketAddr,
    shutdown: Shutdown,
    presence: ConsolePresence,
) {
    // Stream delivery runs independently of this short-lived request connection.
    let local_ip = stream
        .local_addr()
        .map_or(std::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED), |address| {
            address.ip()
        });
    let mut sessions = Sessions::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; READ_BUFFER];
    let started = std::time::Instant::now();
    let (preview_sender, mut preview_frames) = tokio::sync::mpsc::channel(16);
    let mut preview_tasks = std::collections::HashMap::new();

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
    let mut library_updates = LibraryUpdates::new(&service);
    let mut protocol_version = (1, 0);
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
                    if matches!(message.content_type, packet::content::GELI | packet::content::GEIN | packet::content::GETH | packet::content::GELT | packet::content::GVSR | packet::content::RQST | packet::content::CINF) {
                        tracing::debug!(%peer, content = %String::from_utf8_lossy(&message.content_type), version = ?message.version, request = ?message.body.iter().take(40).copied().collect::<Vec<_>>(), "CITP console request");
                    }
                    if message.layer == packet::MSEX {
                        protocol_version = media_citp::negotiate(message.version);
                    }
                    // MagicQ sends single-frame requests over short-lived TCP connections. The
                    // subscription belongs to its requested timeout, not to this control socket.
                    if message.content_type == packet::content::RQST
                        && let Some(request) = media_citp::read_stream_request(&message.body)
                        && let Some(preview) = service.previews.for_source(request.source)
                    {
                        let source = request.source;
                        let task = tokio::spawn(deliver_preview(
                            Arc::clone(preview), request, protocol_version, local_ip,
                            PreviewDestinations::for_peer(peer, preview_sender.clone()), shutdown.clone(),
                        ));
                        replace_preview_task(&mut preview_tasks, source, task.abort_handle());
                        continue;
                    }
                    let now = started.elapsed().as_millis() as u64;
                    let answered = answer_request(service.clone(), message, std::mem::take(&mut sessions), now).await;
                    let Ok((updated_sessions, replies)) = answered else {
                        tracing::warn!(%peer, "a CITP request worker stopped unexpectedly");
                        break 'connected;
                    };
                    sessions = updated_sessions;
                    for reply in replies {
                        if stream.write_all(&reply).await.is_err() {
                            break 'connected;
                        }
                    }
                }
            }
            frame = preview_frames.recv() => {
                if let Some(frame) = frame && stream.write_all(&frame).await.is_err() { break; }
            }
            _ = status.tick() => {
                for update in library_updates.messages(&service, protocol_version) {
                    if stream.write_all(&update).await.is_err() { break 'connected; }
                }
                if stream
                    .write_all(&media_citp::status(&service.layer_status()))
                    .await
                    .is_err()
                {
                    break 'connected;
                }


            }
            _ = &mut stopping => break 'connected,
        }
    }
    report_console_departure(peer, presence);
}

fn replace_preview_task(
    tasks: &mut std::collections::HashMap<u16, tokio::task::AbortHandle>,
    source: u16,
    next: tokio::task::AbortHandle,
) {
    if let Some(previous) = tasks.insert(source, next) {
        previous.abort();
    }
    // Dropping this map when TCP closes deliberately leaves the latest finite tasks alive.
}

/// A finite renderer lease whose cleanup also runs if a task is cancelled or panics.
struct PreviewLease(crate::preview::SharedPreview);

impl Drop for PreviewLease {
    fn drop(&mut self) {
        self.0.subscribed(false, None);
    }
}

async fn deliver_preview(
    preview: crate::preview::SharedPreview,
    request: media_citp::StreamRequest,
    version: (u8, u8),
    local_ip: std::net::IpAddr,
    destinations: PreviewDestinations,
    shutdown: Shutdown,
) {
    let Ok(socket) = preview_socket(local_ip) else {
        return;
    };
    let initial_sequence = preview.latest().map_or(0, |(sequence, _)| sequence);
    preview.subscribed(true, Some((request.width, request.height)));
    let _lease = PreviewLease(Arc::clone(&preview));
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(u64::from(
        request.timeout_seconds.clamp(1, 30),
    )));
    tokio::pin!(deadline);
    let mut watcher = shutdown.watcher();
    let stopping = watcher.wait();
    tokio::pin!(stopping);
    let mut poll = tokio::time::interval(std::time::Duration::from_millis(20));
    let interval = std::time::Duration::from_millis(1_000 / u64::from(request.fps.clamp(1, 10)));
    let mut next_frame = std::time::Instant::now();
    let mut last_sequence = initial_sequence;
    loop {
        tokio::select! {
            _ = &mut deadline => return,
            _ = &mut stopping => return,
            _ = poll.tick() => {
                if std::time::Instant::now() < next_frame { continue; }
                let Some((sequence, frame)) = preview.latest() else { continue };
                if sequence == last_sequence { continue; }
                let Some(frame) = requested_frame(&frame, &request) else { continue };
                let Some(message) = media_citp::message::stream_frame_format(request.source, &frame, request.format, version) else { continue };
                last_sequence = sequence;
                if let Err(error) = socket.send_to(&message, destinations.multicast).await {
                    tracing::debug!(%error, "CITP multicast preview frame was not delivered");
                }
                // A local console can bind 127.0.0.1 rather than wildcard and miss multicast.
                // This additional datagram reaches that explicitly bound local application.
                if let Some(peer) = destinations.loopback && let Err(error) = socket.send_to(&message, peer).await {
                    tracing::debug!(%error, "CITP loopback preview frame was not delivered");
                }
                tracing::debug!(source = request.source, sequence, width = frame.width, height = frame.height, bytes = message.len(), "CITP preview frame delivered");
                // Retain the existing ToskLight TCP client path, after UDP has been delivered.
                // A closed or slow TCP connection cannot prevent the requested UDP stream.
                if let Some(sender) = &destinations.tcp { let _ = sender.try_send(message); }
                if request.single_frame() { return; }
                next_frame = std::time::Instant::now() + interval;
            }
        }
    }
}

pub(super) fn preview_socket(local_ip: std::net::IpAddr) -> std::io::Result<UdpSocket> {
    let socket = std::net::UdpSocket::bind(SocketAddr::new(local_ip, 0))?;
    // macOS defaults UDP SO_SNDBUF to 9216; valid RGB8 preview datagrams exceed it.
    socket2::SockRef::from(&socket).set_send_buffer_size(256 * 1024)?;
    if let std::net::IpAddr::V4(interface) = local_ip {
        socket2::SockRef::from(&socket).set_multicast_if_v4(&interface)?;
    }
    socket.set_multicast_loop_v4(true)?;
    socket.set_nonblocking(true)?;
    UdpSocket::from_std(socket)
}

/// A console is reported as gone only once it has stayed away, so the short connections a desk
/// opens for one request each never appear as a disconnection.
fn report_console_departure(peer: SocketAddr, presence: ConsolePresence) {
    tracing::debug!(%peer, "a console connection closed");
    let Some(generation) = presence.departed(peer.ip()) else {
        return;
    };
    tokio::spawn(async move {
        tokio::time::sleep(PRESENCE_SETTLE).await;
        if presence.settled(peer.ip(), generation) {
            tracing::info!(console = %peer.ip(), "a console disconnected");
        }
    });
}

async fn answer_request(
    service: Service,
    message: packet::Message,
    mut sessions: Sessions,
    now: u64,
) -> Result<(Sessions, Vec<Vec<u8>>), tokio::task::JoinError> {
    tokio::task::spawn_blocking(move || {
        let replies = media_citp::respond(
            &message,
            &service.identity(),
            &service.library(),
            &mut sessions,
            now,
        );
        (sessions, replies)
    })
    .await
}

struct LibraryUpdates {
    catalog: Arc<CatalogSnapshot>,
    configuration: Arc<MediaConfiguration>,
}

impl LibraryUpdates {
    fn new(service: &Service) -> Self {
        Self {
            catalog: service.catalog.load_full(),
            configuration: service.configuration.load_full(),
        }
    }

    fn messages(&mut self, service: &Service, version: (u8, u8)) -> Vec<Vec<u8>> {
        let current = Self::new(service);
        if self.catalog.revision == current.catalog.revision
            && self.configuration.text == current.configuration.text
            && self.configuration.visualizers == current.configuration.visualizers
        {
            return Vec::new();
        }
        let old = PublishedLibrary {
            catalog: Arc::clone(&self.catalog),
            storage: service.storage.clone(),
            configuration: Arc::new(arc_swap::ArcSwap::from(self.configuration.clone())),
            fonts: Arc::clone(&service.fonts),
        };
        let mut folders = old
            .folders()
            .into_iter()
            .chain(service.library().folders())
            .map(|folder| folder.number)
            .collect::<Vec<_>>();
        folders.sort_unstable();
        folders.dedup();
        *self = current;
        folders
            .into_iter()
            .map(|folder| media_citp::message::library_updated(version, folder))
            .collect()
    }
}

struct PreviewDestinations {
    multicast: SocketAddr,
    loopback: Option<SocketAddr>,
    tcp: Option<tokio::sync::mpsc::Sender<Vec<u8>>>,
}

impl PreviewDestinations {
    fn for_peer(peer: SocketAddr, tcp: tokio::sync::mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            multicast: SocketAddr::from((Ipv4Addr::from(MULTICAST_GROUP), media_citp::CITP_PORT)),
            loopback: peer
                .ip()
                .is_loopback()
                .then(|| SocketAddr::new(peer.ip(), media_citp::CITP_PORT)),
            tcp: Some(tcp),
        }
    }
}

fn requested_frame(frame: &Thumbnail, request: &media_citp::StreamRequest) -> Option<Thumbnail> {
    // UDP's maximum payload includes both the CITP/MSEX and StFr headers. Its limit is
    // slightly smaller than the image-length field's u16 maximum.
    const MAX_JPEG_BYTES: usize = 65_507 - packet::MSEX_HEADER - 12;
    let mut width = request.width.clamp(1, 640).min(frame.width);
    let mut height = request.height.clamp(1, 360).min(frame.height);
    if frame.width <= width && frame.height <= height && frame.jpeg.len() <= MAX_JPEG_BYTES {
        return Some(frame.clone());
    }
    let image = image::load_from_memory_with_format(&frame.jpeg, image::ImageFormat::Jpeg)
        .ok()?
        .to_rgba8();
    loop {
        let frame = crate::preview::encode(
            image.as_raw(),
            Size::new(image.width(), image.height()),
            Size::new(u32::from(width), u32::from(height)),
        )
        .ok()?;
        if frame.jpeg.len() <= MAX_JPEG_BYTES {
            return Some(frame);
        }
        if width <= 1 && height <= 1 {
            return None;
        }
        width = (width * 3 / 4).max(1);
        height = (height * 3 / 4).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noisy_jpeg_previews_fit_the_udp_payload_including_protocol_headers() {
        let mut seed = 1u32;
        let pixels = (0..640 * 360)
            .flat_map(|_| {
                let mut pixel = [0, 0, 0, 255];
                for channel in &mut pixel[..3] {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    *channel = (seed >> 24) as u8;
                }
                pixel
            })
            .collect::<Vec<_>>();
        let source =
            crate::preview::encode(&pixels, Size::new(640, 360), Size::new(640, 360)).unwrap();
        assert!(
            source.jpeg.len() > 65_535,
            "exercise a detailed frame previously silently discarded"
        );
        let request = media_citp::StreamRequest {
            source: 1,
            format: packet::FORMAT_JPEG,
            width: 640,
            height: 360,
            fps: 1,
            timeout_seconds: 0,
        };
        let bounded = requested_frame(&source, &request).unwrap();
        let wire =
            media_citp::message::stream_frame_format(1, &bounded, packet::FORMAT_JPEG, (1, 0))
                .unwrap();
        assert!(wire.len() <= 65_507);
        assert!(bounded.width > 0 && bounded.height > 0);
        assert!(image::load_from_memory(&bounded.jpeg).is_ok());
        let mut boundary = bounded;
        boundary.jpeg.resize(65_470, 0); // valid JPEG with padding after EOI
        let corrected = requested_frame(&boundary, &request).unwrap();
        assert!(
            media_citp::message::stream_frame_format(1, &corrected, packet::FORMAT_JPEG, (1, 0))
                .unwrap()
                .len()
                <= 65_507
        );
    }

    #[tokio::test]
    async fn single_frame_rgb_preview_survives_tcp_close_and_exceeds_macos_default_send_buffer() {
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let destination = udp.local_addr().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let preview = Arc::new(crate::preview::Preview::new());
        let demanded = Arc::clone(&preview);
        let accept = tokio::spawn(async move {
            let (mut control, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 38];
            control.read_exact(&mut bytes).await.unwrap();
            let message = packet::parse(&bytes).unwrap();
            let request = media_citp::read_stream_request(&message.body).unwrap();
            let worker = tokio::spawn(deliver_preview(
                demanded,
                request,
                message.version,
                Ipv4Addr::LOCALHOST.into(),
                PreviewDestinations {
                    multicast: destination,
                    loopback: None,
                    tcp: None,
                },
                Shutdown::new(),
            ));
            drop(control);
            // Hand ownership back before publishing: awaiting here would deadlock
            // the renderer's demand-driven frame publication below.
            (worker,)
        });
        let mut control = tokio::net::TcpStream::connect(address).await.unwrap();
        let mut body = packet::Body::new();
        body.u16(38763)
            .four_cc(packet::FORMAT_RGB8)
            .u16(100)
            .u16(100)
            .u8(24)
            .u8(0);
        control
            .write_all(&packet::msex_message(
                packet::content::RQST,
                (1, 0),
                body.as_slice(),
            ))
            .await
            .unwrap();
        drop(control);
        let (worker,) = accept.await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !preview.wanted() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // The renderer publishes only after the TCP control connection has gone away.
        preview.publish(
            crate::preview::encode(
                &[240, 20, 10, 255].repeat(320 * 180),
                Size::new(320, 180),
                Size::new(320, 180),
            )
            .unwrap(),
        );
        let mut bytes = vec![0; 65535];
        let (length, _) =
            tokio::time::timeout(std::time::Duration::from_secs(1), udp.recv_from(&mut bytes))
                .await
                .expect("frame arrives after TCP close")
                .unwrap();
        let frame = packet::parse(&bytes[..length]).unwrap();
        assert_eq!(frame.content_type, packet::content::STFR);
        assert_eq!(frame.version, (1, 0));
        let reader = packet::Reader::new(&frame.body);
        assert_eq!(reader.u16(0), 38763);
        assert_eq!(reader.four_cc(2), packet::FORMAT_RGB8);
        assert_eq!((reader.u16(6), reader.u16(8)), (100, 100));
        assert_eq!(reader.u16(10), 30_000);
        assert!(length > 9_216);
        worker.await.unwrap();
        assert!(!preview.wanted(), "single-frame lease is released");
    }

    #[tokio::test]
    async fn a_preview_with_no_rendered_frame_releases_its_lease_on_shutdown() {
        let preview = Arc::new(crate::preview::Preview::new());
        let shutdown = Shutdown::new();
        let request = media_citp::StreamRequest {
            source: 1,
            format: packet::FORMAT_JPEG,
            width: 100,
            height: 100,
            fps: 10,
            timeout_seconds: 30,
        };
        let worker = tokio::spawn(deliver_preview(
            Arc::clone(&preview),
            request,
            (1, 0),
            Ipv4Addr::LOCALHOST.into(),
            PreviewDestinations {
                multicast: "127.0.0.1:1".parse().unwrap(),
                loopback: None,
                tcp: None,
            },
            shutdown.clone(),
        ));
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !preview.wanted() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        shutdown.request(crate::shutdown::ShutdownReason::Requested);
        worker.await.unwrap();
        assert!(!preview.wanted());
    }

    #[tokio::test]
    async fn renewed_requests_cancel_the_previous_lease_but_connection_drop_does_not() {
        let preview = Arc::new(crate::preview::Preview::new());
        let shutdown = Shutdown::new();
        let mut tasks = std::collections::HashMap::new();
        let mut workers = Vec::new();
        for _ in 0..125 {
            let request = media_citp::StreamRequest {
                source: 1,
                format: packet::FORMAT_JPEG,
                width: 100,
                height: 100,
                fps: 24,
                timeout_seconds: 10,
            };
            let task = tokio::spawn(deliver_preview(
                Arc::clone(&preview),
                request,
                (1, 0),
                Ipv4Addr::LOCALHOST.into(),
                PreviewDestinations {
                    multicast: "127.0.0.1:1".parse().unwrap(),
                    loopback: None,
                    tcp: None,
                },
                shutdown.clone(),
            ));
            replace_preview_task(&mut tasks, 1, task.abort_handle());
            workers.push(task);
        }
        assert_eq!(tasks.len(), 1);
        let latest = workers.pop().unwrap();
        for worker in workers {
            assert!(worker.await.unwrap_err().is_cancelled());
        }
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !preview.wanted() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(tasks);
        assert!(
            !latest.is_finished(),
            "TCP close leaves the finite subscription alive"
        );
        shutdown.request(crate::shutdown::ShutdownReason::Requested);
        latest.await.unwrap();
        assert!(
            !preview.wanted(),
            "no superseded subscription leaks a renderer lease"
        );
    }
}
