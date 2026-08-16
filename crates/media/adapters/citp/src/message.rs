//! The MSEX messages a media server sends and receives.
//!
//! Layouts are ported field for field from the C++ application this rebuild replaces. Where MSEX
//! 1.0 and 1.1 differ, both shapes are produced, because a desk that negotiated 1.0 will not read
//! a 1.1 library identifier.

use media_domain::{SourceFailure, SourceStatus};

use crate::packet::{
    Body, FORMAT_JPEG, LIBRARY_TYPE_MEDIA, MSEX_HEADER, Reader, content, msex_message, pinf_message,
};

/// What a server publishes about itself on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence<'a> {
    pub listening_port: u16,
    /// The peer type. Consoles look for `MediaServer`.
    pub kind: &'a str,
    pub name: &'a str,
    pub state: &'a str,
}

/// An owned peer announcement received from another CITP product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerLocation {
    pub listening_port: u16,
    pub kind: String,
    pub name: String,
    pub state: String,
}

/// Reads a PINF `PLoc` announcement without borrowing the receive buffer.
pub fn read_peer_location(message: &crate::packet::Message) -> Option<PeerLocation> {
    if message.layer != crate::packet::PINF || message.content_type != content::PLOC {
        return None;
    }
    let reader = Reader::new(&message.body);
    let mut at = 2;
    let kind = read_ucs1(&message.body, &mut at)?;
    let name = read_ucs1(&message.body, &mut at)?;
    let state = read_ucs1(&message.body, &mut at)?;
    Some(PeerLocation {
        listening_port: reader.u16(0),
        kind,
        name,
        state,
    })
}

fn read_ucs1(bytes: &[u8], at: &mut usize) -> Option<String> {
    let tail = bytes.get(*at..)?;
    let length = tail.iter().position(|byte| *byte == 0)?;
    let value = String::from_utf8_lossy(&tail[..length]).into_owned();
    *at += length + 1;
    Some(value)
}

/// The discovery announcement, sent to the CITP multicast group and in reply to a peer's own.
pub fn peer_location(presence: &Presence<'_>) -> Vec<u8> {
    let mut body = Body::new();
    body.u16(presence.listening_port)
        .ucs1(presence.kind)
        .ucs1(presence.name)
        .ucs1(presence.state);
    pinf_message(content::PLOC, body.as_slice())
}

/// What this server is, sent as soon as a console connects and again whenever it asks.
pub fn server_information(name: &str, version: (u8, u8), layers: u8) -> Vec<u8> {
    let mut body = Body::new();
    body.ucs2(name).u8(0).u8(1).u8(layers);
    // One DMX source string per layer. Empty means "this layer is not separately patched", which
    // is true: one Media personality covers every layer of an output.
    for _ in 0..layers {
        body.ucs1("");
    }
    msex_message(content::SINF, version, body.as_slice())
}

/// One layer, as a console's status display reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStatus {
    pub layer: u8,
    pub physical_output: u8,
    pub folder: u8,
    pub file: u8,
    pub name: String,
    pub position_frames: u32,
    pub length_frames: u32,
    pub fps: u8,
    pub status: SourceStatus,
    /// Whether this layer is contributing pixels right now.
    pub playing: bool,
}

/// The MSEX layer-status flags this server sets.
///
/// The lifecycle is published, not inferred: a console must be able to tell a layer that is still
/// loading from one that failed, and a layer that finished a single pass from one that is empty.
pub mod flags {
    pub const PLAYING: u32 = 0x0001;
    pub const PLAYBACK_FLAGS: u32 = 0x0002;
    pub const MEDIA_LOADING: u32 = 0x0004;
    pub const MEDIA_FAILED: u32 = 0x0008;
    pub const MEDIA_COMPLETED: u32 = 0x0010;
}

/// The flags one layer's lifecycle produces.
pub const fn status_flags(status: SourceStatus, playing: bool) -> u32 {
    let lifecycle = match status {
        SourceStatus::Loading => flags::MEDIA_LOADING,
        SourceStatus::Failed { .. } => flags::MEDIA_FAILED,
        SourceStatus::Completed => flags::MEDIA_COMPLETED,
        SourceStatus::Ready | SourceStatus::Unselected => 0,
    };
    if playing {
        lifecycle | flags::PLAYING
    } else {
        lifecycle
    }
}

/// Operator-safe text for a failed layer, published in the layer's name field.
///
/// A console shows this beside the layer, so it names the problem without an absolute path or a
/// decoder's own words.
pub const fn failure_text(failure: SourceFailure) -> &'static str {
    match failure {
        SourceFailure::MissingFile => "missing file",
        SourceFailure::UnsupportedCodec => "unplayable format",
        SourceFailure::DecodeFailed => "damaged file",
        SourceFailure::GpuUploadFailed => "graphics error",
    }
}

/// The periodic status of every layer of one output.
pub fn layer_status(layers: &[LayerStatus]) -> Vec<u8> {
    let mut body = Body::new();
    body.u8(layers.len().min(255) as u8);
    for layer in layers.iter().take(255) {
        let name = match layer.status {
            SourceStatus::Failed { failure } => failure_text(failure).to_owned(),
            _ => layer.name.clone(),
        };
        body.u8(layer.layer)
            .u8(layer.physical_output)
            .u8(layer.folder)
            .u8(layer.file)
            .ucs2(&name)
            .u32(layer.position_frames)
            .u32(layer.length_frames)
            .u8(layer.fps)
            .u32(status_flags(layer.status, layer.playing));
    }
    msex_message(content::LSTA, (1, 0), body.as_slice())
}

/// One published library folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFolder {
    pub number: u8,
    pub name: String,
    pub element_count: u8,
}

/// The folders a console asked about.
pub fn element_library_information(version: (u8, u8), folders: &[LibraryFolder]) -> Vec<u8> {
    let detailed = version >= (1, 1);
    let mut body = Body::new();
    body.u8(LIBRARY_TYPE_MEDIA).u8(folders.len().min(255) as u8);

    for folder in folders.iter().take(255) {
        if detailed {
            body.library_id(1, folder.number);
        } else {
            body.u8(folder.number);
        }
        // The DMX range a folder occupies is the folder number itself: one folder, one value.
        body.u8(folder.number).u8(folder.number).ucs2(&folder.name);
        if detailed {
            body.u8(0); // no nested libraries
        }
        body.u8(folder.element_count);
    }
    msex_message(
        content::ELIN,
        (1, if detailed { 1 } else { 0 }),
        body.as_slice(),
    )
}

/// One published element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryElement {
    pub number: u8,
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub length_frames: u32,
    pub fps: u8,
}

/// The elements of one folder.
pub fn media_element_information(
    version: (u8, u8),
    folder: u8,
    elements: &[LibraryElement],
    timestamp: u64,
) -> Vec<u8> {
    let detailed = version >= (1, 1);
    let mut body = Body::new();
    if detailed {
        body.library_id(1, folder);
    } else {
        body.u8(folder);
    }
    body.u8(elements.len().min(255) as u8);

    for element in elements.iter().take(255) {
        body.u8(element.number)
            .u8(element.number)
            .u8(element.number)
            .ucs2(&element.name)
            .u64(timestamp)
            .u16(element.width)
            .u16(element.height)
            .u32(element.length_frames)
            .u8(element.fps);
    }
    msex_message(
        content::MEIN,
        (1, if detailed { 1 } else { 0 }),
        body.as_slice(),
    )
}

/// An encoded image on its way to a console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub width: u16,
    pub height: u16,
    pub jpeg: Vec<u8>,
}

impl Thumbnail {
    /// Whether this image fits the 16-bit length CITP gives it.
    ///
    /// Silently truncating would send a console a corrupt picture, so an oversized thumbnail is
    /// not sent at all.
    pub fn fits(&self) -> bool {
        self.jpeg.len() <= usize::from(u16::MAX)
    }
}

/// A folder's representative thumbnail.
pub fn element_library_thumbnail(
    version: (u8, u8),
    folder: u8,
    thumbnail: &Thumbnail,
) -> Option<Vec<u8>> {
    if !thumbnail.fits() {
        return None;
    }
    let detailed = version >= (1, 1);
    let mut body = Body::new();
    body.u8(LIBRARY_TYPE_MEDIA);
    if detailed {
        body.library_id(1, folder);
    } else {
        body.u8(folder);
    }
    append_image(&mut body, thumbnail);
    Some(msex_message(
        content::ELTH,
        (1, if detailed { 1 } else { 0 }),
        body.as_slice(),
    ))
}

/// One element's thumbnail.
pub fn element_thumbnail(
    version: (u8, u8),
    folder: u8,
    element: u8,
    thumbnail: &Thumbnail,
) -> Option<Vec<u8>> {
    if !thumbnail.fits() {
        return None;
    }
    let detailed = version >= (1, 1);
    let mut body = Body::new();
    body.u8(LIBRARY_TYPE_MEDIA);
    if detailed {
        body.library_id(1, folder);
    } else {
        body.u8(folder);
    }
    body.u8(element);
    append_image(&mut body, thumbnail);
    Some(msex_message(
        content::ETHN,
        (1, if detailed { 1 } else { 0 }),
        body.as_slice(),
    ))
}

/// One stable logical output advertised as a CITP preview source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoSource {
    pub id: u16,
    pub name: String,
    pub physical_output: u8,
    pub width: u16,
    pub height: u16,
}

/// The video sources a console may subscribe to.
pub fn video_sources(version: (u8, u8), sources: &[VideoSource]) -> Vec<u8> {
    let mut body = Body::new();
    body.u16(sources.len().min(u16::MAX as usize) as u16);
    for source in sources.iter().take(u16::MAX as usize) {
        body.u16(source.id)
            .ucs2(&source.name)
            .u8(source.physical_output)
            .u8(0xFF) // not tied to one layer
            .u16(0) // no flags
            .u16(source.width)
            .u16(source.height);
    }
    msex_message(content::VSRC, (1, version.1.min(2)), body.as_slice())
}

/// One preview frame.
pub fn stream_frame(source: u16, frame: &Thumbnail) -> Option<Vec<u8>> {
    if !frame.fits() || frame.jpeg.is_empty() {
        return None;
    }
    let mut body = Body::new();
    body.u16(source);
    append_image(&mut body, frame);
    Some(msex_message(content::STFR, (1, 1), body.as_slice()))
}

fn append_image(body: &mut Body, image: &Thumbnail) {
    body.four_cc(FORMAT_JPEG)
        .u16(image.width)
        .u16(image.height)
        .u16(image.jpeg.len() as u16)
        .bytes(&image.jpeg);
}

// Requests, read from what a console sent.

/// What a console asked for when it requested library information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRequest {
    /// Empty means "everything".
    pub folders: Vec<u8>,
}

pub fn read_library_request(version: (u8, u8), body: &[u8]) -> Option<LibraryRequest> {
    let reader = Reader::new(body);
    if reader.u8(0) != LIBRARY_TYPE_MEDIA {
        return None;
    }
    if version >= (1, 1) {
        // A parent level above the root names a nested library, and media libraries are flat.
        if reader.u8(1) != 0 {
            return None;
        }
        let count = reader.u8(5);
        Some(LibraryRequest {
            folders: reader.list(6, count),
        })
    } else {
        let count = reader.u8(1);
        Some(LibraryRequest {
            folders: reader.list(2, count),
        })
    }
}

/// What a console asked for when it requested elements of one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRequest {
    pub folder: u8,
    /// Empty means "everything in that folder".
    pub elements: Vec<u8>,
}

pub fn read_element_request(version: (u8, u8), body: &[u8]) -> Option<ElementRequest> {
    let reader = Reader::new(body);
    if reader.u8(0) != LIBRARY_TYPE_MEDIA {
        return None;
    }
    if version >= (1, 1) {
        let count = reader.u8(5);
        Some(ElementRequest {
            folder: reader.u8(2),
            elements: reader.list(6, count),
        })
    } else {
        let count = reader.u8(2);
        Some(ElementRequest {
            folder: reader.u8(1),
            elements: reader.list(3, count),
        })
    }
}

/// What a console asked for when it requested thumbnails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailRequest {
    pub width: u16,
    pub height: u16,
    pub preserve_aspect: bool,
    pub folder: u8,
    /// Empty means every element of the folder. Ignored by a library-thumbnail request.
    pub elements: Vec<u8>,
    /// For a library request: which folders. Empty means all.
    pub folders: Vec<u8>,
}

pub fn read_library_thumbnail_request(version: (u8, u8), body: &[u8]) -> Option<ThumbnailRequest> {
    let reader = Reader::new(body);
    if reader.u8(9) != LIBRARY_TYPE_MEDIA {
        return None;
    }
    let count = reader.u8(10);
    let folders = if version >= (1, 1) {
        reader.library_ids(11, count)
    } else {
        reader.list(11, count)
    };
    Some(ThumbnailRequest {
        width: reader.u16(4),
        height: reader.u16(6),
        preserve_aspect: reader.u8(8) & 0x01 != 0,
        folder: 0,
        elements: Vec::new(),
        folders,
    })
}

pub fn read_element_thumbnail_request(version: (u8, u8), body: &[u8]) -> Option<ThumbnailRequest> {
    let reader = Reader::new(body);
    if reader.u8(9) != LIBRARY_TYPE_MEDIA {
        return None;
    }
    let (folder, count, elements_at) = if version >= (1, 2) {
        (reader.u8(11), reader.u16(14) as u8, 16)
    } else if version >= (1, 1) {
        (reader.u8(11), reader.u8(14), 15)
    } else {
        (reader.u8(10), reader.u8(11), 12)
    };
    Some(ThumbnailRequest {
        width: reader.u16(4),
        height: reader.u16(6),
        preserve_aspect: reader.u8(8) & 0x01 != 0,
        folder,
        elements: reader.list(elements_at, count),
        folders: Vec::new(),
    })
}

/// A console's subscription to the live output preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRequest {
    pub source: u16,
    pub format: [u8; 4],
    pub width: u16,
    pub height: u16,
    pub fps: u8,
    /// Seconds the subscription lasts. Zero means one frame and nothing more.
    pub timeout_seconds: u8,
}

impl StreamRequest {
    pub const fn single_frame(&self) -> bool {
        self.timeout_seconds == 0
    }
}

pub fn read_stream_request(body: &[u8]) -> Option<StreamRequest> {
    if body.len() < 12 {
        return None;
    }
    let reader = Reader::new(body);
    Some(StreamRequest {
        source: reader.u16(0),
        format: reader.four_cc(2),
        width: reader.u16(6),
        height: reader.u16(8),
        fps: reader.u8(10).max(1),
        timeout_seconds: reader.u8(11),
    })
}

/// Where a message's body begins inside a whole MSEX message. Exposed for tests that build a
/// request the way a console would put it on the wire.
pub const BODY_AT: usize = MSEX_HEADER;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{Message, parse};

    fn body_of(message: &[u8]) -> Message {
        parse(message).expect("a built message frames")
    }

    #[test]
    fn discovery_announces_the_port_a_console_should_connect_to() {
        let message = peer_location(&Presence {
            listening_port: 14_809,
            kind: "MediaServer",
            name: "ToskLight Media",
            state: "Running",
        });
        let framed = body_of(&message);

        assert_eq!(framed.content_type, content::PLOC);
        assert_eq!(Reader::new(&framed.body).u16(0), 14_809);
        assert!(
            framed
                .body
                .windows(11)
                .any(|window| window == b"MediaServer"),
            "a console filters on the peer type"
        );
    }

    #[test]
    fn a_console_announcement_round_trips_as_owned_identity() {
        let framed = body_of(&peer_location(&Presence {
            listening_port: 0,
            kind: "LightingConsole",
            name: "The Tempest",
            state: "Running",
        }));

        assert_eq!(
            read_peer_location(&framed),
            Some(PeerLocation {
                listening_port: 0,
                kind: "LightingConsole".into(),
                name: "The Tempest".into(),
                state: "Running".into(),
            })
        );
    }

    #[test]
    fn a_failed_layer_publishes_safe_text_and_a_failure_flag() {
        let status = layer_status(&[LayerStatus {
            layer: 0,
            physical_output: 0,
            folder: 3,
            file: 7,
            name: "/Users/someone/private/clip.mov".into(),
            position_frames: 0,
            length_frames: 0,
            fps: 25,
            status: SourceStatus::Failed {
                failure: SourceFailure::MissingFile,
            },
            playing: false,
        }]);
        let framed = body_of(&status);

        assert!(
            !framed.body.windows(5).any(|window| window == b"Users"),
            "a path must never reach a console's display"
        );
        let flags = *framed
            .body
            .last_chunk::<4>()
            .expect("the flags end the layer");
        assert_eq!(u32::from_le_bytes(flags), flags::MEDIA_FAILED);
    }

    #[test]
    fn every_lifecycle_state_is_distinguishable_on_the_wire() {
        // A console has to tell these apart; if two produced the same flags it could not.
        let loading = status_flags(SourceStatus::Loading, false);
        let ready = status_flags(SourceStatus::Ready, true);
        let failed = status_flags(
            SourceStatus::Failed {
                failure: SourceFailure::DecodeFailed,
            },
            false,
        );
        let completed = status_flags(SourceStatus::Completed, false);
        let unselected = status_flags(SourceStatus::Unselected, false);

        let all = [loading, ready, failed, completed, unselected];
        for (index, value) in all.iter().enumerate() {
            for other in &all[index + 1..] {
                assert_ne!(value, other, "two lifecycle states publish the same flags");
            }
        }
        assert_eq!(ready & flags::PLAYING, flags::PLAYING);
        assert_eq!(unselected, 0);
    }

    #[test]
    fn a_recovered_layer_stops_reporting_a_failure() {
        // Recovery is not a state of its own: it is the failure flag going away, which is what a
        // console redraws on.
        let failed = status_flags(
            SourceStatus::Failed {
                failure: SourceFailure::MissingFile,
            },
            false,
        );
        let recovered = status_flags(SourceStatus::Ready, true);
        assert_eq!(failed & flags::MEDIA_FAILED, flags::MEDIA_FAILED);
        assert_eq!(recovered & flags::MEDIA_FAILED, 0);
    }

    #[test]
    fn library_information_uses_the_shape_the_negotiated_version_can_read() {
        let folders = [LibraryFolder {
            number: 4,
            name: "Looks".into(),
            element_count: 9,
        }];

        let old = body_of(&element_library_information((1, 0), &folders));
        assert_eq!(old.version, (1, 0));
        assert_eq!(Reader::new(&old.body).u8(2), 4, "a bare folder number");

        let new = body_of(&element_library_information((1, 1), &folders));
        assert_eq!(new.version, (1, 1));
        let reader = Reader::new(&new.body);
        assert_eq!(
            reader.u8(2),
            1,
            "a library identifier starts with its level"
        );
        assert_eq!(reader.u8(3), 4);
    }

    #[test]
    fn an_oversized_thumbnail_is_not_sent_at_all() {
        let huge = Thumbnail {
            width: 64,
            height: 64,
            jpeg: vec![0; usize::from(u16::MAX) + 1],
        };
        assert!(!huge.fits());
        assert_eq!(element_thumbnail((1, 1), 1, 1, &huge), None);
        assert_eq!(stream_frame(1, &huge), None);
    }

    #[test]
    fn a_thumbnail_carries_its_real_size_and_its_own_length() {
        let thumbnail = Thumbnail {
            width: 128,
            height: 72,
            jpeg: vec![7; 40],
        };
        let framed = body_of(&element_thumbnail((1, 1), 3, 5, &thumbnail).expect("it fits"));
        let reader = Reader::new(&framed.body);

        assert_eq!(reader.u8(0), LIBRARY_TYPE_MEDIA);
        assert_eq!(reader.u8(2), 3, "the folder");
        assert_eq!(reader.u8(5), 5, "the element");
        assert_eq!(reader.four_cc(6), FORMAT_JPEG);
        assert_eq!(reader.u16(10), 128);
        assert_eq!(reader.u16(12), 72);
        assert_eq!(reader.u16(14), 40);
        assert_eq!(framed.body.len(), 16 + 40);
    }

    #[test]
    fn a_stream_request_says_how_long_the_console_wants_it_for() {
        let mut request = Body::new();
        request
            .u16(1)
            .four_cc(FORMAT_JPEG)
            .u16(320)
            .u16(180)
            .u8(0)
            .u8(0);
        let single = read_stream_request(request.as_slice()).expect("well formed");
        assert!(single.single_frame(), "a zero timeout is one frame");
        assert_eq!(single.fps, 1, "and never a zero frame rate");

        let mut request = Body::new();
        request
            .u16(2)
            .four_cc(FORMAT_JPEG)
            .u16(320)
            .u16(180)
            .u8(10)
            .u8(30);
        let continuous = read_stream_request(request.as_slice()).expect("well formed");
        assert!(!continuous.single_frame());
        assert_eq!(continuous.fps, 10);
        assert_eq!(continuous.timeout_seconds, 30);

        assert_eq!(read_stream_request(&[1, 2, 3]), None, "a truncated request");
    }

    #[test]
    fn a_request_naming_no_folders_means_every_folder() {
        let mut request = Body::new();
        request.u8(LIBRARY_TYPE_MEDIA).u8(0).u8(0).u8(0).u8(0).u8(0);
        let all = read_library_request((1, 1), request.as_slice()).expect("media library");
        assert!(all.folders.is_empty());

        let mut request = Body::new();
        request
            .u8(LIBRARY_TYPE_MEDIA)
            .u8(0)
            .u8(0)
            .u8(0)
            .u8(0)
            .u8(2)
            .u8(3)
            .u8(9);
        let some = read_library_request((1, 1), request.as_slice()).expect("media library");
        assert_eq!(some.folders, vec![3, 9]);
    }

    #[test]
    fn a_request_for_a_library_type_this_server_does_not_publish_is_declined() {
        let mut request = Body::new();
        request.u8(2).u8(0).u8(0).u8(0).u8(0).u8(0);
        assert_eq!(read_library_request((1, 1), request.as_slice()), None);
        assert_eq!(read_element_request((1, 1), request.as_slice()), None);
    }

    #[test]
    fn a_thumbnail_request_states_the_size_the_console_wants() {
        let mut request = Body::new();
        request
            .four_cc(FORMAT_JPEG)
            .u16(96)
            .u16(54)
            .u8(1)
            .u8(LIBRARY_TYPE_MEDIA)
            // The library identifier starts immediately after the type, and the folder is its
            // first level.
            .library_id(1, 6)
            .u8(2)
            .u8(1)
            .u8(4);
        let asked = read_element_thumbnail_request((1, 1), request.as_slice()).expect("media");

        assert_eq!(asked.width, 96);
        assert_eq!(asked.height, 54);
        assert!(asked.preserve_aspect);
        assert_eq!(asked.folder, 6);
        assert_eq!(asked.elements, vec![1, 4]);
    }

    #[test]
    fn a_v1_2_thumbnail_request_reads_its_sixteen_bit_element_count() {
        let mut request = Body::new();
        request
            .four_cc(FORMAT_JPEG)
            .u16(128)
            .u16(72)
            .u8(1)
            .u8(LIBRARY_TYPE_MEDIA)
            .library_id(1, 1)
            .u16(2)
            .u8(1)
            .u8(4);
        let asked = read_element_thumbnail_request((1, 2), request.as_slice()).expect("media");
        assert_eq!(asked.folder, 1);
        assert_eq!(asked.elements, vec![1, 4]);
    }
}

#[cfg(test)]
mod interoperability {
    //! What Light's own CITP client reads, asserted as byte offsets.
    //!
    //! Light and Media are separate products and must not depend on each other, so the agreement
    //! between them cannot be a shared type — it has to be checked. These offsets are exactly what
    //! `crates/light/adapters/media/src/protocol.rs` reads at MSEX 1.1; changing an encoder here
    //! without changing that parser would break a desk this repository ships.

    use super::*;
    use crate::packet::parse;

    #[test]
    fn light_can_read_an_element_thumbnail_this_server_sends() {
        let thumbnail = Thumbnail {
            width: 96,
            height: 54,
            jpeg: vec![0xAB; 12],
        };
        let framed = parse(&element_thumbnail((1, 1), 6, 9, &thumbnail).expect("it fits"))
            .expect("it frames");
        let payload = &framed.body;

        assert_eq!(payload[1], 1, "the library level Light reads at byte 1");
        assert_eq!(
            payload[2..5],
            [6, 0, 0],
            "the identifier Light reads at 2..5"
        );
        assert_eq!(payload[5], 9, "the element Light reads at byte 5");
        assert_eq!(&payload[6..10], b"JPEG", "the format Light reads at byte 6");
        assert_eq!(u16::from_le_bytes([payload[10], payload[11]]), 96);
        assert_eq!(u16::from_le_bytes([payload[12], payload[13]]), 54);
        assert_eq!(
            u16::from_le_bytes([payload[14], payload[15]]) as usize,
            payload.len() - 16,
            "Light rejects a buffer whose stated length does not match what arrived"
        );
    }

    #[test]
    fn light_can_read_a_stream_frame_this_server_sends() {
        let frame = Thumbnail {
            width: 320,
            height: 180,
            jpeg: vec![0xCD; 8],
        };
        let framed = parse(&stream_frame(2, &frame).expect("it fits")).expect("it frames");
        let payload = &framed.body;

        assert_eq!(
            framed.version,
            (1, 1),
            "Light reads a 1.2 frame from a different offset; this server states 1.1"
        );
        assert_eq!(
            u16::from_le_bytes([payload[0], payload[1]]),
            2,
            "the source"
        );
        assert_eq!(&payload[2..6], b"JPEG");
        assert_eq!(
            u16::from_le_bytes([payload[10], payload[11]]) as usize,
            payload.len() - 12
        );
    }
}
