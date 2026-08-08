//! What a media server answers, decided without a socket in sight.
//!
//! Every CITP exchange is a pure function of the message, what the library holds, and what the
//! layers are doing — so it is one here, and the socket loop above only carries bytes. That is
//! what makes version negotiation, subscription expiry, and thumbnail bounds testable.

use crate::message::{
    LayerStatus, LibraryElement, LibraryFolder, Presence, Thumbnail, ThumbnailRequest,
    element_library_information, element_library_thumbnail, element_thumbnail, layer_status,
    media_element_information, peer_location, read_element_request, read_element_thumbnail_request,
    read_library_request, read_library_thumbnail_request, read_stream_request, server_information,
    stream_frame, video_sources,
};
use crate::packet::{Message, content};

/// The MSEX versions this server speaks, newest first.
///
/// A console states what it supports; the reply is the newest both sides know. A console that
/// asked for nothing gets 1.0, which every console can read.
pub const SUPPORTED: [(u8, u8); 2] = [(1, 1), (1, 0)];

/// Chooses the version to answer a request at.
pub fn negotiate(requested: (u8, u8)) -> (u8, u8) {
    SUPPORTED
        .into_iter()
        .find(|supported| *supported <= requested)
        .unwrap_or((1, 0))
}

/// What the server can publish. Implemented by the runtime over the real catalog.
pub trait Library {
    fn folders(&self) -> Vec<LibraryFolder>;
    fn elements(&self, folder: u8) -> Vec<LibraryElement>;
    /// A thumbnail at the size the console asked for, or nothing when there is none to send.
    fn thumbnail(
        &self,
        folder: u8,
        element: Option<u8>,
        request: &ThumbnailRequest,
    ) -> Option<Thumbnail>;
    /// A timestamp for element information. Seconds since the epoch.
    fn timestamp(&self) -> u64;
}

/// One console's subscription to the live preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub source: u16,
    pub width: u16,
    pub height: u16,
    pub fps: u8,
    pub expires_at_millis: u64,
    pub single_frame: bool,
    /// The last preview this subscription was sent, so one frame is never sent twice.
    pub last_sequence: u64,
}

/// Everything the server carries between messages.
#[derive(Debug, Default, Clone)]
pub struct Sessions {
    subscriptions: Vec<Subscription>,
}

impl Sessions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anything wants preview frames right now.
    ///
    /// The renderer reads pixels back only when this is true: a preview nobody asked for must
    /// never cost the program output a frame.
    pub fn anyone_subscribed(&self, now_millis: u64) -> bool {
        self.subscriptions
            .iter()
            .any(|subscription| subscription.expires_at_millis > now_millis)
    }

    /// The largest preview any subscriber asked for, or nothing when none did.
    pub fn requested_size(&self, now_millis: u64) -> Option<(u16, u16)> {
        self.subscriptions
            .iter()
            .filter(|subscription| subscription.expires_at_millis > now_millis)
            .map(|subscription| (subscription.width.max(1), subscription.height.max(1)))
            .reduce(|first, second| (first.0.max(second.0), first.1.max(second.1)))
    }

    fn expire(&mut self, now_millis: u64) {
        self.subscriptions
            .retain(|subscription| subscription.expires_at_millis > now_millis);
    }

    /// Records a console's request, replacing any earlier one for the same source.
    pub fn subscribe(&mut self, request: &crate::message::StreamRequest, now_millis: u64) {
        self.expire(now_millis);
        let seconds = if request.single_frame() {
            1
        } else {
            u64::from(request.timeout_seconds)
        };
        let last_sequence = self
            .subscriptions
            .iter()
            .find(|existing| existing.source == request.source)
            .map_or(0, |existing| existing.last_sequence);

        let subscription = Subscription {
            source: request.source,
            width: request.width,
            height: request.height,
            fps: request.fps,
            expires_at_millis: now_millis + seconds * 1_000,
            single_frame: request.single_frame(),
            last_sequence,
        };
        self.subscriptions
            .retain(|existing| existing.source != request.source);
        self.subscriptions.push(subscription);
    }

    /// The frames to send for one newly captured preview.
    ///
    /// A subscription that has already seen this sequence is skipped, and a single-frame request
    /// ends as soon as it has been served.
    pub fn frames_for(
        &mut self,
        preview: &Thumbnail,
        sequence: u64,
        now_millis: u64,
    ) -> Vec<Vec<u8>> {
        self.expire(now_millis);
        let mut messages = Vec::new();
        for subscription in &mut self.subscriptions {
            if subscription.last_sequence == sequence {
                continue;
            }
            subscription.last_sequence = sequence;
            if let Some(message) = stream_frame(subscription.source, preview) {
                messages.push(message);
            }
            if subscription.single_frame {
                subscription.expires_at_millis = now_millis;
            }
        }
        self.expire(now_millis);
        messages
    }
}

/// What the server is, for the messages that describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity<'a> {
    pub name: &'a str,
    pub layers: u8,
    pub preview_width: u16,
    pub preview_height: u16,
}

/// The greeting sent the moment a console connects, before it has asked anything.
pub fn greeting(identity: &Identity<'_>) -> Vec<u8> {
    server_information(identity.name, (1, 1), identity.layers)
}

/// The discovery announcement.
pub fn announcement(name: &str, listening_port: u16) -> Vec<u8> {
    peer_location(&Presence {
        listening_port,
        kind: "MediaServer",
        name,
        state: "Running",
    })
}

/// Answers one message.
///
/// Returns every reply it produced, which may be several — a console asking for six thumbnails
/// gets six messages, because CITP has no batched thumbnail response.
pub fn respond(
    message: &Message,
    identity: &Identity<'_>,
    library: &dyn Library,
    sessions: &mut Sessions,
    now_millis: u64,
) -> Vec<Vec<u8>> {
    if message.content_type == content::PLOC {
        return vec![announcement(identity.name, crate::MSEX_PORT)];
    }

    let version = negotiate(message.version);
    match message.content_type {
        content::CINF => vec![server_information(identity.name, version, identity.layers)],
        content::GELI => library_information(message, version, library),
        content::GEIN => element_information(message, version, library),
        content::GELT => library_thumbnails(message, version, library),
        content::GETH => element_thumbnails(message, version, library),
        content::GVSR => vec![video_sources(
            version,
            "Program",
            identity.preview_width,
            identity.preview_height,
        )],
        content::RQST => {
            if let Some(request) = read_stream_request(&message.body) {
                sessions.subscribe(&request, now_millis);
            }
            // A subscription is answered by frames, when there are frames — not by an
            // acknowledgement a console would have to correlate.
            Vec::new()
        }
        // Anything else is a message this server does not implement. Staying silent is correct:
        // MSEX has no general error reply, and inventing one confuses a console.
        _ => Vec::new(),
    }
}

/// The periodic status every connected console receives.
pub fn status(layers: &[LayerStatus]) -> Vec<u8> {
    layer_status(layers)
}

fn library_information(
    message: &Message,
    version: (u8, u8),
    library: &dyn Library,
) -> Vec<Vec<u8>> {
    let Some(request) = read_library_request(version, &message.body) else {
        return Vec::new();
    };
    let folders: Vec<LibraryFolder> = library
        .folders()
        .into_iter()
        .filter(|folder| wanted(folder.number, &request.folders))
        .collect();
    vec![element_library_information(version, &folders)]
}

fn element_information(
    message: &Message,
    version: (u8, u8),
    library: &dyn Library,
) -> Vec<Vec<u8>> {
    let Some(request) = read_element_request(version, &message.body) else {
        return Vec::new();
    };
    let elements: Vec<LibraryElement> = library
        .elements(request.folder)
        .into_iter()
        .filter(|element| wanted(element.number, &request.elements))
        .collect();
    vec![media_element_information(
        version,
        request.folder,
        &elements,
        library.timestamp(),
    )]
}

fn library_thumbnails(message: &Message, version: (u8, u8), library: &dyn Library) -> Vec<Vec<u8>> {
    let Some(request) = read_library_thumbnail_request(version, &message.body) else {
        return Vec::new();
    };
    library
        .folders()
        .into_iter()
        .filter(|folder| wanted(folder.number, &request.folders))
        .filter_map(|folder| {
            let thumbnail = library.thumbnail(folder.number, None, &request)?;
            element_library_thumbnail(version, folder.number, &thumbnail)
        })
        .collect()
}

fn element_thumbnails(message: &Message, version: (u8, u8), library: &dyn Library) -> Vec<Vec<u8>> {
    let Some(request) = read_element_thumbnail_request(version, &message.body) else {
        return Vec::new();
    };
    library
        .elements(request.folder)
        .into_iter()
        .filter(|element| wanted(element.number, &request.elements))
        .filter_map(|element| {
            let thumbnail = library.thumbnail(request.folder, Some(element.number), &request)?;
            element_thumbnail(version, request.folder, element.number, &thumbnail)
        })
        .collect()
}

/// An empty request names everything, which is how CITP asks for a whole library.
fn wanted(number: u8, requested: &[u8]) -> bool {
    requested.is_empty() || requested.contains(&number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::StreamRequest;
    use crate::packet::{Body, FORMAT_JPEG, LIBRARY_TYPE_MEDIA, msex_message, parse};

    struct Shelf;

    impl Library for Shelf {
        fn folders(&self) -> Vec<LibraryFolder> {
            vec![
                LibraryFolder {
                    number: 1,
                    name: "Looks".into(),
                    element_count: 2,
                },
                LibraryFolder {
                    number: 4,
                    name: "Textures".into(),
                    element_count: 1,
                },
            ]
        }

        fn elements(&self, folder: u8) -> Vec<LibraryElement> {
            if folder != 1 {
                return Vec::new();
            }
            vec![
                LibraryElement {
                    number: 1,
                    name: "Blue haze".into(),
                    width: 1920,
                    height: 1080,
                    length_frames: 600,
                    fps: 25,
                },
                LibraryElement {
                    number: 2,
                    name: "Static grid".into(),
                    width: 1920,
                    height: 1080,
                    length_frames: 0,
                    fps: 25,
                },
            ]
        }

        fn thumbnail(
            &self,
            _folder: u8,
            _element: Option<u8>,
            request: &ThumbnailRequest,
        ) -> Option<Thumbnail> {
            Some(Thumbnail {
                width: request.width.max(1),
                height: request.height.max(1),
                jpeg: vec![1, 2, 3],
            })
        }

        fn timestamp(&self) -> u64 {
            1_700_000_000
        }
    }

    fn identity() -> Identity<'static> {
        Identity {
            name: "ToskLight Media",
            layers: 8,
            preview_width: 320,
            preview_height: 180,
        }
    }

    fn request(content_type: [u8; 4], version: (u8, u8), body: &[u8]) -> Message {
        parse(&msex_message(content_type, version, body)).expect("it frames")
    }

    fn reply(message: &Message) -> Vec<Message> {
        let mut sessions = Sessions::new();
        respond(message, &identity(), &Shelf, &mut sessions, 0)
            .iter()
            .map(|bytes| parse(bytes).expect("a reply frames"))
            .collect()
    }

    #[test]
    fn a_console_gets_the_newest_version_both_sides_speak() {
        assert_eq!(negotiate((1, 2)), (1, 1), "never above what we implement");
        assert_eq!(negotiate((1, 1)), (1, 1));
        assert_eq!(negotiate((1, 0)), (1, 0), "and never above what it asked");
        assert_eq!(
            negotiate((0, 9)),
            (1, 0),
            "a console that asked for nothing usable"
        );
    }

    #[test]
    fn a_version_request_is_answered_at_the_negotiated_version() {
        let replies = reply(&request(content::CINF, (1, 0), &[]));
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].content_type, content::SINF);
        assert_eq!(replies[0].version, (1, 0), "a 1.0 console gets 1.0 back");

        let replies = reply(&request(content::CINF, (1, 1), &[]));
        assert_eq!(replies[0].version, (1, 1));
    }

    #[test]
    fn a_library_request_publishes_only_the_folders_it_named() {
        let mut body = Body::new();
        body.u8(LIBRARY_TYPE_MEDIA)
            .u8(0)
            .u8(0)
            .u8(0)
            .u8(0)
            .u8(1)
            .u8(4);
        let replies = reply(&request(content::GELI, (1, 1), body.as_slice()));

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].content_type, content::ELIN);
        assert_eq!(
            replies[0].body[1], 1,
            "one folder, because one was asked for"
        );
        assert_eq!(replies[0].body[3], 4, "and it is the one named");
    }

    #[test]
    fn asking_for_no_folder_in_particular_publishes_them_all() {
        let mut body = Body::new();
        body.u8(LIBRARY_TYPE_MEDIA).u8(0).u8(0).u8(0).u8(0).u8(0);
        let replies = reply(&request(content::GELI, (1, 1), body.as_slice()));
        assert_eq!(replies[0].body[1], 2);
    }

    #[test]
    fn element_thumbnails_come_back_one_message_each() {
        let mut body = Body::new();
        body.four_cc(FORMAT_JPEG)
            .u16(64)
            .u16(36)
            .u8(1)
            .u8(LIBRARY_TYPE_MEDIA)
            .library_id(1, 1)
            .u8(0); // no elements named: every element of the folder
        let replies = reply(&request(content::GETH, (1, 1), body.as_slice()));

        assert_eq!(replies.len(), 2, "CITP has no batched thumbnail response");
        for message in &replies {
            assert_eq!(message.content_type, content::ETHN);
        }
        assert_eq!(replies[0].body[5], 1, "the first element");
        assert_eq!(replies[1].body[5], 2);
    }

    #[test]
    fn a_request_for_a_folder_that_is_not_there_answers_nothing_rather_than_guessing() {
        let mut body = Body::new();
        body.u8(LIBRARY_TYPE_MEDIA)
            .library_id(1, 9)
            .u8(0)
            .u8(0)
            .u8(0);
        let replies = reply(&request(content::GEIN, (1, 1), body.as_slice()));
        assert_eq!(replies.len(), 1, "an empty list, not silence");
        assert_eq!(replies[0].content_type, content::MEIN);
        assert_eq!(replies[0].body[4], 0, "with no elements in it");
    }

    #[test]
    fn a_message_this_server_does_not_implement_is_answered_with_silence() {
        let replies = reply(&request(*b"ZZZZ", (1, 1), &[]));
        assert!(replies.is_empty());
    }

    #[test]
    fn discovery_is_answered_wherever_it_arrives() {
        let mut sessions = Sessions::new();
        let ploc = Message {
            layer: crate::packet::PINF,
            version: (0, 0),
            content_type: content::PLOC,
            body: Vec::new(),
        };
        let replies = respond(&ploc, &identity(), &Shelf, &mut sessions, 0);
        assert_eq!(replies.len(), 1);
        assert_eq!(
            parse(&replies[0]).expect("frames").content_type,
            content::PLOC
        );
    }

    #[test]
    fn nothing_is_read_back_from_the_gpu_until_a_console_subscribes() {
        let mut sessions = Sessions::new();
        assert!(!sessions.anyone_subscribed(0));
        assert_eq!(sessions.requested_size(0), None);

        sessions.subscribe(
            &StreamRequest {
                source: 1,
                format: FORMAT_JPEG,
                width: 320,
                height: 180,
                fps: 10,
                timeout_seconds: 5,
            },
            1_000,
        );
        assert!(sessions.anyone_subscribed(1_000));
        assert_eq!(sessions.requested_size(1_000), Some((320, 180)));

        assert!(
            !sessions.anyone_subscribed(6_001),
            "a subscription that ran out stops costing the output anything"
        );
    }

    #[test]
    fn one_preview_is_sent_once_and_a_single_frame_request_ends_itself() {
        let mut sessions = Sessions::new();
        sessions.subscribe(
            &StreamRequest {
                source: 1,
                format: FORMAT_JPEG,
                width: 160,
                height: 90,
                fps: 1,
                timeout_seconds: 0,
            },
            0,
        );
        let preview = Thumbnail {
            width: 160,
            height: 90,
            jpeg: vec![9; 10],
        };

        assert_eq!(sessions.frames_for(&preview, 1, 0).len(), 1);
        assert_eq!(
            sessions.frames_for(&preview, 1, 0).len(),
            0,
            "a console asked for one frame and got it"
        );
        assert!(!sessions.anyone_subscribed(0));
    }

    #[test]
    fn a_continuing_subscription_is_not_sent_the_same_frame_twice() {
        let mut sessions = Sessions::new();
        sessions.subscribe(
            &StreamRequest {
                source: 2,
                format: FORMAT_JPEG,
                width: 160,
                height: 90,
                fps: 10,
                timeout_seconds: 30,
            },
            0,
        );
        let preview = Thumbnail {
            width: 160,
            height: 90,
            jpeg: vec![9; 10],
        };

        assert_eq!(sessions.frames_for(&preview, 1, 0).len(), 1);
        assert_eq!(sessions.frames_for(&preview, 1, 100).len(), 0);
        assert_eq!(
            sessions.frames_for(&preview, 2, 200).len(),
            1,
            "a new frame is sent"
        );
    }

    #[test]
    fn resubscribing_replaces_the_earlier_request_and_keeps_its_place() {
        let mut sessions = Sessions::new();
        let mut request = StreamRequest {
            source: 1,
            format: FORMAT_JPEG,
            width: 160,
            height: 90,
            fps: 10,
            timeout_seconds: 5,
        };
        sessions.subscribe(&request, 0);
        let preview = Thumbnail {
            width: 160,
            height: 90,
            jpeg: vec![9; 10],
        };
        sessions.frames_for(&preview, 7, 0);

        request.width = 640;
        sessions.subscribe(&request, 1_000);
        assert_eq!(sessions.requested_size(1_000), Some((640, 90)));
        assert_eq!(
            sessions.frames_for(&preview, 7, 1_000).len(),
            0,
            "the frame it already had is not resent because it asked again"
        );
    }
}
