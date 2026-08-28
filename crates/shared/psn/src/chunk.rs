//! The one structure the whole protocol is made of.
//!
//! Every part of a PSN packet — the packet itself, its header, a tracker, that tracker's position —
//! is a chunk: a 32-bit little-endian word, then that many bytes. The word says what the chunk is,
//! how long it is, and whether its bytes are more chunks.
//!
//! Reading is therefore the same three lines everywhere, and a chunk whose id means nothing to this
//! decoder costs exactly one skip. That is what the protocol asks for: an application must ignore
//! chunks it does not know, which is how a desk survives meeting a newer sender.
//!
//! One caveat worth knowing before trusting this against real hardware. The bit layout below comes
//! from the figure in the v2.02 specification, and the tests that check it — including the
//! round trip through the encoder — are all built from that same reading. If the reading is wrong,
//! everything here agrees with itself about it. The independent check is a datagram captured from a
//! real sender; there has not been one yet, and the first time this meets an actual tracking system
//! is the first time that assumption is tested.

use crate::PsnError;

pub(crate) struct Chunk<'a> {
    pub(crate) id: u16,
    pub(crate) data: &'a [u8],
    /// Whether the sender says `data` is itself a sequence of chunks. This decoder reads a chunk's
    /// body according to what its id means, so the flag is not consulted — it is kept because a
    /// reader of the protocol expects to find it here.
    #[allow(dead_code)]
    pub(crate) has_subchunks: bool,
}

pub(crate) struct Chunks<'a> {
    rest: &'a [u8],
}

impl<'a> Chunks<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    /// The id of the next chunk, without consuming it or trusting its length.
    ///
    /// Reading the id first is how a foreign datagram is told apart from a damaged PSN one.
    pub(crate) fn peek_id(&self) -> Option<u16> {
        let header = self.rest.get(0..4)?;
        Some((u32::from_le_bytes(header.try_into().ok()?) & 0xFFFF) as u16)
    }

    /// The next chunk, or `None` at the end.
    ///
    /// A body that ends in fewer than four spare bytes is the end, not an error: that is padding.
    /// A chunk that claims more data than remains is an error, because the alternative is handing
    /// back a position built from bytes that were never sent.
    pub(crate) fn next_chunk(&mut self) -> Result<Option<Chunk<'a>>, PsnError> {
        if self.rest.len() < 4 {
            return Ok(None);
        }
        let header = u32::from_le_bytes(self.rest[0..4].try_into().unwrap_or([0; 4]));
        let id = (header & 0xFFFF) as u16;
        let data_len = ((header >> 16) & 0x7FFF) as usize;
        let has_subchunks = (header >> 31) & 1 == 1;
        let body = self.rest.get(4..4 + data_len).ok_or(PsnError::Truncated)?;
        self.rest = &self.rest[4 + data_len..];
        Ok(Some(Chunk {
            id,
            data: body,
            has_subchunks,
        }))
    }
}
