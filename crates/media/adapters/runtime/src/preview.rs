//! The output preview a console can subscribe to.
//!
//! Two rules shape this. Reading pixels back off the GPU costs the program output, so it happens
//! only while a console is actually subscribed — never speculatively, never "just in case a
//! console connects". And the preview is a *preview*: it is captured at a fraction of the output's
//! rate and scaled down, because a desk drawing a thumbnail does not need sixty frames a second
//! and the program output does.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use media_citp::Thumbnail;
use media_domain::geometry::Size;

/// The most preview frames captured per second, however fast the output runs.
const MAX_PREVIEW_FPS: u64 = 10;
/// What a preview is scaled to when no console has asked for a size.
const DEFAULT_PREVIEW: Size = Size::new(320, 180);

/// The latest preview, and whether anyone wants one.
#[derive(Debug, Default)]
pub struct Preview {
    /// How many connected consoles are currently subscribed. Zero means capture nothing.
    subscribers: AtomicUsize,
    /// The size subscribers asked for, packed as two 16-bit halves so it is one atomic read.
    requested: AtomicU64,
    /// Increments with each captured frame, so a connection can tell a new one from the last.
    sequence: AtomicU64,
    frame: arc_swap::ArcSwapOption<Thumbnail>,
}

/// Shared between the outputs, which capture, and the connections, which send.
pub type SharedPreview = Arc<Preview>;

impl Preview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that one connection has started or stopped wanting frames.
    pub fn subscribed(&self, wanted: bool, size: Option<(u16, u16)>) {
        if wanted {
            self.subscribers.fetch_add(1, Ordering::SeqCst);
            if let Some((width, height)) = size {
                self.requested
                    .store(u64::from(width) << 16 | u64::from(height), Ordering::SeqCst);
            }
        } else {
            // Never below zero: a connection that closes while unsubscribed must not make the
            // count wrap and leave the GPU reading back forever.
            let _ = self
                .subscribers
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                    count.checked_sub(1)
                });
        }
    }

    /// Whether an output should read its pixels back at all.
    pub fn wanted(&self) -> bool {
        self.subscribers.load(Ordering::SeqCst) > 0
    }

    /// The size to scale a capture to.
    pub fn requested_size(&self) -> Size {
        let packed = self.requested.load(Ordering::SeqCst);
        let (width, height) = ((packed >> 16) as u32, (packed & 0xFFFF) as u32);
        if width == 0 || height == 0 {
            DEFAULT_PREVIEW
        } else {
            Size::new(width, height)
        }
    }

    /// The newest frame and its sequence, or nothing when none has been captured.
    pub fn latest(&self) -> Option<(u64, Arc<Thumbnail>)> {
        let frame = self.frame.load_full()?;
        Some((self.sequence.load(Ordering::SeqCst), frame))
    }

    /// Publishes a captured frame.
    pub fn publish(&self, frame: Thumbnail) {
        self.frame.store(Some(Arc::new(frame)));
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
}

/// Decides whether this instant should be captured, given when the last one was.
pub const fn due(last_capture_millis: Option<u64>, now_millis: u64) -> bool {
    match last_capture_millis {
        None => true,
        Some(last) => now_millis.saturating_sub(last) >= 1_000 / MAX_PREVIEW_FPS,
    }
}

/// Scales an 8-bit RGBA image down and encodes it as JPEG.
///
/// Nearest-neighbour, deliberately: a preview is looked at as a thumbnail, and a box filter over a
/// 1080p readback would cost more than the readback itself on the thread that also presents.
pub fn encode(
    pixels: &[u8],
    from: Size,
    to: Size,
) -> Result<Thumbnail, jpeg_encoder::EncodingError> {
    let width = to.width.clamp(1, from.width.max(1));
    let height = to.height.clamp(1, from.height.max(1));
    let mut scaled = Vec::with_capacity(width as usize * height as usize * 3);

    for row in 0..height {
        let source_row = (row as u64 * u64::from(from.height) / u64::from(height)) as u32;
        for column in 0..width {
            let source_column = (column as u64 * u64::from(from.width) / u64::from(width)) as u32;
            let at = (source_row as usize * from.width as usize + source_column as usize) * 4;
            scaled.extend_from_slice(pixels.get(at..at + 3).unwrap_or(&[0, 0, 0]));
        }
    }

    let mut encoded = Vec::new();
    // Quality chosen so a 1080p preview stays inside CITP's 16-bit length at a thumbnail size.
    jpeg_encoder::Encoder::new(&mut encoded, 70).encode(
        &scaled,
        width as u16,
        height as u16,
        jpeg_encoder::ColorType::Rgb,
    )?;

    Ok(Thumbnail {
        width: width as u16,
        height: height as u16,
        jpeg: encoded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(size: Size, colour: [u8; 4]) -> Vec<u8> {
        colour.repeat(size.width as usize * size.height as usize)
    }

    #[test]
    fn nothing_is_captured_until_a_console_subscribes() {
        let preview = Preview::new();
        assert!(
            !preview.wanted(),
            "a preview nobody asked for costs nothing"
        );

        preview.subscribed(true, Some((160, 90)));
        assert!(preview.wanted());
        assert_eq!(preview.requested_size(), Size::new(160, 90));

        preview.subscribed(false, None);
        assert!(!preview.wanted());
    }

    #[test]
    fn two_consoles_both_have_to_leave_before_capture_stops() {
        let preview = Preview::new();
        preview.subscribed(true, Some((160, 90)));
        preview.subscribed(true, Some((320, 180)));

        preview.subscribed(false, None);
        assert!(
            preview.wanted(),
            "one console left; the other is still watching"
        );
        preview.subscribed(false, None);
        assert!(!preview.wanted());
    }

    #[test]
    fn a_connection_that_never_subscribed_cannot_make_the_count_wrap() {
        // Otherwise a closing connection would leave the GPU reading back for the rest of the show.
        let preview = Preview::new();
        preview.subscribed(false, None);
        assert!(!preview.wanted());
    }

    #[test]
    fn a_frame_carries_a_sequence_so_one_is_never_sent_twice() {
        let preview = Preview::new();
        assert!(preview.latest().is_none());

        preview.publish(Thumbnail {
            width: 4,
            height: 4,
            jpeg: vec![1],
        });
        let (first, _) = preview.latest().expect("a frame was published");
        preview.publish(Thumbnail {
            width: 4,
            height: 4,
            jpeg: vec![2],
        });
        let (second, frame) = preview.latest().expect("and another");

        assert!(second > first);
        assert_eq!(frame.jpeg, vec![2], "the newest frame, not the first");
    }

    #[test]
    fn captures_are_rate_limited_however_fast_the_output_runs() {
        assert!(due(None, 0), "the first capture is always due");
        assert!(!due(Some(1_000), 1_050), "not sixty times a second");
        assert!(due(Some(1_000), 1_100), "ten times a second");
    }

    #[test]
    fn a_capture_is_scaled_down_and_encoded_within_citp_s_length() {
        let from = Size::new(1920, 1080);
        let encoded = encode(&image(from, [10, 200, 30, 255]), from, Size::new(320, 180))
            .expect("it encodes");

        assert_eq!(encoded.width, 320);
        assert_eq!(encoded.height, 180);
        assert_eq!(&encoded.jpeg[..2], &[0xFF, 0xD8], "a JPEG starts with SOI");
        assert!(
            encoded.fits(),
            "a preview must fit the 16-bit length CITP gives it: {} bytes",
            encoded.jpeg.len()
        );
    }

    #[test]
    fn a_preview_is_never_scaled_up_past_the_output_it_came_from() {
        let from = Size::new(64, 64);
        let encoded =
            encode(&image(from, [0, 0, 0, 255]), from, Size::new(1920, 1080)).expect("it encodes");

        assert_eq!(
            (encoded.width, encoded.height),
            (64, 64),
            "inventing detail the output never had would be a lie about the program"
        );
    }
}
