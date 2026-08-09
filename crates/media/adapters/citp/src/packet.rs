//! CITP framing.
//!
//! Every CITP message is a 20-byte header followed by a content layer. The two layers this server
//! speaks are PINF, which carries discovery, and MSEX, which carries everything a media server
//! does. Nothing above this module writes a byte by hand.
//!
//! Field order and widths are ported from the C++ application this rebuild replaces, which is the
//! settled behaviour two desks have already been proven against. Everything is little-endian.

/// The CITP header, before any content layer.
pub const CITP_HEADER: usize = 20;
/// A CITP header plus an MSEX layer header.
pub const MSEX_HEADER: usize = 26;
/// The largest message this server will accept. A console asking for more is a fault, not a
/// request, and reading it would let a peer allocate this process out of memory.
pub const MAX_MESSAGE: usize = 1024 * 1024;

pub const COOKIE: [u8; 4] = *b"CITP";
pub const PINF: [u8; 4] = *b"PINF";
pub const MSEX: [u8; 4] = *b"MSEX";

/// Content types this server sends or receives.
pub mod content {
    pub const PLOC: [u8; 4] = *b"PLoc";
    pub const CINF: [u8; 4] = *b"CInf";
    pub const SINF: [u8; 4] = *b"SInf";
    pub const LSTA: [u8; 4] = *b"LSta";
    pub const GELI: [u8; 4] = *b"GELI";
    pub const ELIN: [u8; 4] = *b"ELIn";
    pub const GEIN: [u8; 4] = *b"GEIn";
    pub const MEIN: [u8; 4] = *b"MEIn";
    pub const GELT: [u8; 4] = *b"GELT";
    pub const ELTH: [u8; 4] = *b"ELTh";
    pub const GETH: [u8; 4] = *b"GETh";
    pub const ETHN: [u8; 4] = *b"EThn";
    pub const GVSR: [u8; 4] = *b"GVSr";
    pub const VSRC: [u8; 4] = *b"VSrc";
    pub const RQST: [u8; 4] = *b"RqSt";
    pub const STFR: [u8; 4] = *b"StFr";
}

/// The image format this server produces. JPEG is what every desk in the interoperability target
/// accepts, and it is what the application this replaces sent.
pub const FORMAT_JPEG: [u8; 4] = *b"JPEG";

/// The only library type a media server publishes.
pub const LIBRARY_TYPE_MEDIA: u8 = 1;

/// Builds a message body byte by byte, in CITP's little-endian order.
#[derive(Debug, Default, Clone)]
pub struct Body(Vec<u8>);

impl Body {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.0.push(value);
        self
    }

    pub fn u16(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn four_cc(&mut self, value: [u8; 4]) -> &mut Self {
        self.0.extend_from_slice(&value);
        self
    }

    /// A null-terminated single-byte string, as CITP's PINF layer uses.
    pub fn ucs1(&mut self, value: &str) -> &mut Self {
        for byte in value.bytes() {
            self.0.push(if byte < 128 { byte } else { b'?' });
        }
        self.0.push(0);
        self
    }

    /// A null-terminated UCS-2 string, as MSEX uses.
    ///
    /// Non-ASCII becomes `?` rather than a wrong code unit: a console showing an operator a
    /// question mark is honest, and a mangled surrogate is not.
    pub fn ucs2(&mut self, value: &str) -> &mut Self {
        for character in value.chars() {
            let unit = if character.is_ascii() {
                character as u16
            } else {
                u16::from(b'?')
            };
            self.u16(unit);
        }
        self.u16(0)
    }

    /// A four-byte MSEX 1.1 library identifier.
    pub fn library_id(&mut self, level: u8, first: u8) -> &mut Self {
        self.u8(level).u8(first).u8(0).u8(0)
    }

    pub fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.0.extend_from_slice(value);
        self
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn finish(self) -> Vec<u8> {
        self.0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

/// Wraps a content layer in a CITP header.
fn citp_message(content_type: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(CITP_HEADER + body.len());
    message.extend_from_slice(&COOKIE);
    message.push(1); // version major
    message.push(0); // version minor
    message.extend_from_slice(&0u16.to_le_bytes()); // request index
    message.extend_from_slice(&((CITP_HEADER + body.len()) as u32).to_le_bytes());
    message.extend_from_slice(&1u16.to_le_bytes()); // part count
    message.extend_from_slice(&0u16.to_le_bytes()); // part
    message.extend_from_slice(&content_type);
    message.extend_from_slice(body);
    message
}

/// A PINF message — discovery, which travels over UDP.
pub fn pinf_message(content_type: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut layer = Vec::with_capacity(4 + body.len());
    layer.extend_from_slice(&content_type);
    layer.extend_from_slice(body);
    citp_message(PINF, &layer)
}

/// An MSEX message at a stated version.
pub fn msex_message(content_type: [u8; 4], version: (u8, u8), body: &[u8]) -> Vec<u8> {
    let mut layer = Vec::with_capacity(6 + body.len());
    layer.push(version.0);
    layer.push(version.1);
    layer.extend_from_slice(&content_type);
    layer.extend_from_slice(body);
    citp_message(MSEX, &layer)
}

/// One received message, already framed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub layer: [u8; 4],
    /// The MSEX version the peer stated. `(0, 0)` for a PINF message.
    pub version: (u8, u8),
    pub content_type: [u8; 4],
    /// Everything after the content-layer header.
    pub body: Vec<u8>,
}

/// Why a byte stream could not be framed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FramingError {
    #[error("a CITP message claims {claimed} bytes, which is outside the {MAX_MESSAGE}-byte bound")]
    ImplausibleLength { claimed: usize },
}

/// Takes whole messages off the front of a stream buffer.
///
/// Returns what it could frame and leaves any partial message in `pending`. A byte that is not the
/// start of a cookie is dropped one at a time rather than resynchronising by guesswork, which is
/// how the application this replaces recovered from a peer that sent something unexpected.
pub fn take_messages(pending: &mut Vec<u8>) -> Result<Vec<Message>, FramingError> {
    let mut taken = Vec::new();
    loop {
        if pending.len() < CITP_HEADER {
            return Ok(taken);
        }
        if pending[..4] != COOKIE {
            pending.remove(0);
            continue;
        }
        let claimed = u32::from_le_bytes(pending[8..12].try_into().expect("four bytes")) as usize;
        if !(CITP_HEADER..=MAX_MESSAGE).contains(&claimed) {
            return Err(FramingError::ImplausibleLength { claimed });
        }
        if pending.len() < claimed {
            return Ok(taken);
        }

        let message: Vec<u8> = pending.drain(..claimed).collect();
        if let Some(framed) = parse(&message) {
            taken.push(framed);
        }
    }
}

/// Reads one already-complete message.
pub fn parse(message: &[u8]) -> Option<Message> {
    if message.len() < CITP_HEADER || message[..4] != COOKIE {
        return None;
    }
    let layer: [u8; 4] = message[16..20].try_into().expect("four bytes");

    if layer == PINF {
        if message.len() < CITP_HEADER + 4 {
            return None;
        }
        return Some(Message {
            layer,
            version: (0, 0),
            content_type: message[CITP_HEADER..CITP_HEADER + 4]
                .try_into()
                .expect("four bytes"),
            body: message[CITP_HEADER + 4..].to_vec(),
        });
    }

    if layer == MSEX {
        if message.len() < MSEX_HEADER {
            return None;
        }
        return Some(Message {
            layer,
            version: (message[CITP_HEADER], message[CITP_HEADER + 1]),
            content_type: message[CITP_HEADER + 2..CITP_HEADER + 6]
                .try_into()
                .expect("four bytes"),
            body: message[MSEX_HEADER..].to_vec(),
        });
    }

    // A layer this server does not speak is not a fault; it is simply not for us.
    None
}

/// Reads fields out of a received body without ever running off the end.
///
/// A short or malformed request reads as zeros rather than panicking or being rejected, which is
/// what keeps one confused console from dropping a connection the rest of a show depends on.
#[derive(Debug, Clone, Copy)]
pub struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub const fn new(body: &'a [u8]) -> Self {
        Self(body)
    }

    pub fn u8(&self, offset: usize) -> u8 {
        self.0.get(offset).copied().unwrap_or(0)
    }

    pub fn u16(&self, offset: usize) -> u16 {
        u16::from_le_bytes([self.u8(offset), self.u8(offset + 1)])
    }

    pub fn u32(&self, offset: usize) -> u32 {
        u32::from_le_bytes([
            self.u8(offset),
            self.u8(offset + 1),
            self.u8(offset + 2),
            self.u8(offset + 3),
        ])
    }

    pub fn four_cc(&self, offset: usize) -> [u8; 4] {
        [
            self.u8(offset),
            self.u8(offset + 1),
            self.u8(offset + 2),
            self.u8(offset + 3),
        ]
    }

    /// A run of single-byte values, stopping at the end of what actually arrived.
    pub fn list(&self, offset: usize, count: u8) -> Vec<u8> {
        (0..usize::from(count))
            .filter(|index| offset + index < self.0.len())
            .map(|index| self.u8(offset + index))
            .collect()
    }

    /// The second byte of each four-byte library identifier in a run.
    pub fn library_ids(&self, offset: usize, count: u8) -> Vec<u8> {
        (0..usize::from(count))
            .map(|index| offset + index * 4 + 1)
            .filter(|position| *position < self.0.len())
            .map(|position| self.u8(position))
            .collect()
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_states_its_own_length_including_the_header() {
        let message = msex_message(content::SINF, (1, 1), &[9, 9, 9]);
        assert_eq!(&message[..4], b"CITP");
        assert_eq!(message.len(), MSEX_HEADER + 3);
        assert_eq!(
            u32::from_le_bytes(message[8..12].try_into().unwrap()) as usize,
            message.len(),
            "a peer reads the length before the body, so it must count the header"
        );
        assert_eq!(&message[16..20], b"MSEX");
        assert_eq!(message[CITP_HEADER], 1);
        assert_eq!(message[CITP_HEADER + 1], 1);
        assert_eq!(&message[CITP_HEADER + 2..CITP_HEADER + 6], b"SInf");
    }

    #[test]
    fn a_message_round_trips_through_the_framer() {
        let mut body = Body::new();
        body.u16(4809).ucs1("MediaServer");
        let message = pinf_message(content::PLOC, body.as_slice());

        let framed = parse(&message).expect("a PINF message frames");
        assert_eq!(framed.layer, PINF);
        assert_eq!(framed.content_type, content::PLOC);
        assert_eq!(Reader::new(&framed.body).u16(0), 4809);
    }

    #[test]
    fn a_stream_yields_whole_messages_and_keeps_a_partial_one() {
        let first = msex_message(content::CINF, (1, 2), &[1]);
        let second = msex_message(content::GETH, (1, 1), &[2, 3]);

        let mut pending = first.clone();
        pending.extend_from_slice(&second[..4]);

        let taken = take_messages(&mut pending).expect("framed");
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].content_type, content::CINF);
        assert_eq!(taken[0].version, (1, 2));
        assert_eq!(pending.len(), 4, "the partial second message is kept");

        pending.extend_from_slice(&second[4..]);
        let taken = take_messages(&mut pending).expect("framed");
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].content_type, content::GETH);
        assert!(pending.is_empty());
    }

    #[test]
    fn rubbish_before_a_message_is_skipped_a_byte_at_a_time() {
        let mut pending = vec![0xFF; 7];
        pending.extend_from_slice(&msex_message(content::CINF, (1, 0), &[]));

        let taken = take_messages(&mut pending).expect("framed");
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].content_type, content::CINF);
    }

    #[test]
    fn an_implausible_length_is_refused_rather_than_allocated() {
        let mut message = msex_message(content::CINF, (1, 0), &[]);
        message[8..12].copy_from_slice(&(MAX_MESSAGE as u32 + 1).to_le_bytes());
        let mut pending = message;

        assert!(matches!(
            take_messages(&mut pending),
            Err(FramingError::ImplausibleLength { .. })
        ));
    }

    #[test]
    fn a_reader_past_the_end_of_a_short_request_reads_zero() {
        let reader = Reader::new(&[7, 8]);
        assert_eq!(reader.u8(0), 7);
        assert_eq!(reader.u16(0), 0x0807);
        assert_eq!(reader.u8(99), 0, "a truncated request must not panic");
        assert_eq!(reader.u32(1), 8, "and reads what is there, zero-filled");
        assert_eq!(reader.list(0, 8), vec![7, 8]);
    }

    #[test]
    fn strings_are_null_terminated_in_both_widths() {
        let mut body = Body::new();
        body.ucs1("ab");
        assert_eq!(body.as_slice(), &[b'a', b'b', 0]);

        let mut body = Body::new();
        body.ucs2("ab");
        assert_eq!(body.as_slice(), &[b'a', 0, b'b', 0, 0, 0]);

        let mut body = Body::new();
        body.ucs2("é");
        assert_eq!(
            body.as_slice(),
            &[b'?', 0, 0, 0],
            "a character that does not fit becomes a question mark, not a wrong one"
        );
    }
}
