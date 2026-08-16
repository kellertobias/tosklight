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
use media_domain::OutputId;
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
    /// Browser snapshot requests renew this short lease instead of holding a streaming socket.
    requested_until_unix_millis: AtomicU64,
    /// The size subscribers asked for, packed as two 16-bit halves so it is one atomic read.
    requested: AtomicU64,
    /// Increments with each captured frame, so a connection can tell a new one from the last.
    sequence: AtomicU64,
    frame: arc_swap::ArcSwapOption<Thumbnail>,
    web_frame: arc_swap::ArcSwapOption<WebPreview>,
}

/// The browser representation of the same captured compositor frame.
///
/// Program is JPEG, byte-for-byte the image handed to CITP. Layers are PNG so transparent pixels
/// survive and the operator UI can reveal them over its checkerboard instead of baking that
/// checkerboard into the source.
#[derive(Debug, Clone)]
pub struct WebPreview {
    pub width: u16,
    pub height: u16,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// Shared between the outputs, which capture, and the connections, which send.
pub type SharedPreview = Arc<Preview>;

/// Stable one-to-one mapping between configured logical outputs, advertised CITP sources and
/// their independently demand-driven latest-frame slots.
#[derive(Clone, Default)]
pub struct SharedPreviews {
    by_output: Arc<std::collections::BTreeMap<OutputId, OutputPreviews>>,
}

#[derive(Clone)]
struct OutputPreviews {
    program: (u16, SharedPreview),
    layers: Vec<(u16, SharedPreview)>,
}

impl SharedPreviews {
    pub fn configured(configuration: &media_application::MediaConfiguration) -> Self {
        let mut used = std::collections::BTreeSet::new();
        let by_output = configuration
            .outputs
            .iter()
            .filter(|output| output.enabled)
            .map(|output| {
                let bytes = output.id.as_uuid().into_bytes();
                let mut source = u16::from_le_bytes([bytes[0], bytes[1]]).max(1);
                while !used.insert(source) {
                    source = source.wrapping_add(1).max(1);
                }
                let preview_size = default_preview_size(Size::new(
                    output.resolution.width,
                    output.resolution.height,
                ));
                let program_preview = Arc::new(Preview::new());
                program_preview.publish_pixels(
                    &vec![0; preview_size.width as usize * preview_size.height as usize * 4],
                    preview_size,
                    preview_size,
                    false,
                );
                let program = (source, program_preview);
                let layers = (0..output.personality.layer_count())
                    .map(|_| {
                        source = source.wrapping_add(1).max(1);
                        while !used.insert(source) {
                            source = source.wrapping_add(1).max(1);
                        }
                        let preview = Arc::new(Preview::new());
                        preview.publish_pixels(
                            &vec![
                                0;
                                preview_size.width as usize * preview_size.height as usize * 4
                            ],
                            preview_size,
                            preview_size,
                            true,
                        );
                        (source, preview)
                    })
                    .collect();
                (output.id, OutputPreviews { program, layers })
            })
            .collect();
        Self {
            by_output: Arc::new(by_output),
        }
    }

    pub fn for_output(&self, output: OutputId) -> Option<&SharedPreview> {
        self.by_output
            .get(&output)
            .map(|previews| &previews.program.1)
    }

    pub fn for_layer(&self, output: OutputId, layer: usize) -> Option<&SharedPreview> {
        self.by_output
            .get(&output)?
            .layers
            .get(layer)
            .map(|(_, preview)| preview)
    }

    pub fn for_source(&self, source: u16) -> Option<&SharedPreview> {
        self.by_output.values().find_map(|previews| {
            (previews.program.0 == source)
                .then_some(&previews.program.1)
                .or_else(|| {
                    previews
                        .layers
                        .iter()
                        .find_map(|(id, preview)| (*id == source).then_some(preview))
                })
        })
    }

    pub fn source_for_output(&self, output: OutputId) -> Option<u16> {
        self.by_output
            .get(&output)
            .map(|previews| previews.program.0)
    }

    pub fn source_for_layer(&self, output: OutputId, layer: usize) -> Option<u16> {
        self.by_output
            .get(&output)?
            .layers
            .get(layer)
            .map(|(source, _)| *source)
    }
}

impl Preview {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that one connection has started or stopped wanting frames.
    pub fn subscribed(&self, wanted: bool, size: Option<(u16, u16)>) {
        if wanted {
            self.subscribers.fetch_add(1, Ordering::SeqCst);
            self.requested_size_is(size);
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

    pub fn requested_size_is(&self, size: Option<(u16, u16)>) {
        if let Some((width, height)) = size {
            self.requested
                .store(u64::from(width) << 16 | u64::from(height), Ordering::SeqCst);
        }
    }

    /// Whether an output should read its pixels back at all.
    pub fn wanted(&self) -> bool {
        self.subscribers.load(Ordering::SeqCst) > 0
            || self.requested_until_unix_millis.load(Ordering::SeqCst) > unix_millis()
    }

    /// Requests demand-driven frames for a browser that polls the snapshot endpoint.
    pub fn requested_for_browser(&self, size: Option<(u16, u16)>) {
        self.requested_size_is(size);
        self.requested_until_unix_millis
            .store(unix_millis().saturating_add(2_000), Ordering::SeqCst);
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

    /// The newest browser-safe representation of the same compositor frame.
    pub fn latest_web(&self) -> Option<(u64, Arc<WebPreview>)> {
        let frame = self.web_frame.load_full()?;
        Some((self.sequence.load(Ordering::SeqCst), frame))
    }

    /// Publishes a captured frame.
    pub fn publish(&self, frame: Thumbnail) {
        let web = WebPreview {
            width: frame.width,
            height: frame.height,
            content_type: "image/jpeg",
            bytes: frame.jpeg.clone(),
        };
        self.frame.store(Some(Arc::new(frame)));
        self.web_frame.store(Some(Arc::new(web)));
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }

    /// Encodes and publishes one captured compositor frame for both CITP and the browser.
    pub fn publish_pixels(&self, pixels: &[u8], from: Size, to: Size, layer: bool) {
        match encode_publication(pixels, from, to, layer) {
            Ok((citp, web)) => {
                self.frame.store(Some(Arc::new(citp)));
                self.web_frame.store(Some(Arc::new(web)));
                self.sequence.fetch_add(1, Ordering::SeqCst);
            }
            Err(error) => tracing::warn!(%error, "a live preview could not be encoded"),
        }
    }
}

fn default_preview_size(output: Size) -> Size {
    let width = DEFAULT_PREVIEW.width.min(output.width.max(1));
    let height = ((u64::from(width) * u64::from(output.height.max(1)))
        / u64::from(output.width.max(1)))
    .max(1) as u32;
    Size::new(width, height.min(output.height.max(1)))
}

fn unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
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

/// Encodes one compositor frame into CITP JPEG and browser-safe bytes.
fn encode_publication(
    pixels: &[u8],
    from: Size,
    to: Size,
    layer: bool,
) -> anyhow::Result<(Thumbnail, WebPreview)> {
    if !layer {
        let citp = encode(pixels, from, to)?;
        let web = WebPreview {
            width: citp.width,
            height: citp.height,
            content_type: "image/jpeg",
            bytes: citp.jpeg.clone(),
        };
        return Ok((citp, web));
    }

    let width = to.width.clamp(1, from.width.max(1));
    let height = to.height.clamp(1, from.height.max(1));
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height {
        let source_row = (row as u64 * u64::from(from.height) / u64::from(height)) as u32;
        for column in 0..width {
            let source_column = (column as u64 * u64::from(from.width) / u64::from(width)) as u32;
            let at = (source_row as usize * from.width as usize + source_column as usize) * 4;
            rgba.extend_from_slice(pixels.get(at..at + 4).unwrap_or(&[0, 0, 0, 0]));
        }
    }

    // The compositor target stores premultiplied RGB, as every correct source-over blend target
    // does. PNG/browser pixels are straight alpha; writing the premultiplied bytes directly makes
    // semitransparent edges dark and displaying them over a checkerboard multiplies alpha twice.
    for pixel in rgba.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        if alpha == 0 {
            pixel[..3].fill(0);
            continue;
        }
        for channel in 0..3 {
            pixel[channel] = ((u16::from(pixel[channel]) * 255 + alpha / 2) / alpha).min(255) as u8;
        }
    }

    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
    }

    // CITP StFr is JPEG and cannot carry alpha. Composite its representation over the same
    // checkerboard the web UI exposes behind the transparent PNG, so both operator surfaces show
    // transparency rather than silently turning it black.
    let mut checker = rgba.clone();
    for (index, pixel) in checker.chunks_exact_mut(4).enumerate() {
        let x = index as u32 % width;
        let y = index as u32 / width;
        let background = if (x / 8 + y / 8) % 2 == 0 {
            [22_u8, 27, 32]
        } else {
            [41_u8, 49, 57]
        };
        let alpha = u16::from(pixel[3]);
        for channel in 0..3 {
            pixel[channel] = ((u16::from(pixel[channel]) * alpha
                + u16::from(background[channel]) * (255 - alpha))
                / 255) as u8;
        }
        pixel[3] = 255;
    }
    let citp = encode(&checker, Size::new(width, height), Size::new(width, height))?;
    Ok((
        citp,
        WebPreview {
            width: width as u16,
            height: height as u16,
            content_type: "image/png",
            bytes: png_bytes,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(size: Size, colour: [u8; 4]) -> Vec<u8> {
        colour.repeat(size.width as usize * size.height as usize)
    }

    #[test]
    fn configured_outputs_have_stable_isolated_preview_slots() {
        let first = media_application::configuration::OutputConfiguration::new("Main");
        let second = media_application::configuration::OutputConfiguration::new("Aux");
        let configuration = media_application::MediaConfiguration {
            outputs: vec![first.clone(), second.clone()],
            ..Default::default()
        };
        let previews = SharedPreviews::configured(&configuration);
        let first_source = previews.source_for_output(first.id).unwrap();
        let first_layer = previews.source_for_layer(first.id, 0).unwrap();
        let second_source = previews.source_for_output(second.id).unwrap();
        assert_ne!(first_source, second_source);
        assert_ne!(first_source, first_layer);
        assert!(Arc::ptr_eq(
            previews.for_output(first.id).unwrap(),
            previews.for_source(first_source).unwrap()
        ));
        assert!(!previews.for_output(second.id).unwrap().wanted());
        assert!(Arc::ptr_eq(
            previews.for_layer(first.id, 0).unwrap(),
            previews.for_source(first_layer).unwrap()
        ));
        previews
            .for_source(first_source)
            .unwrap()
            .subscribed(true, Some((160, 90)));
        assert!(previews.for_output(first.id).unwrap().wanted());
        assert!(!previews.for_output(second.id).unwrap().wanted());
    }

    #[test]
    fn configured_previews_start_with_valid_black_program_and_transparent_layer_frames() {
        let output = media_application::configuration::OutputConfiguration::new("Main");
        let configuration = media_application::MediaConfiguration {
            outputs: vec![output.clone()],
            ..Default::default()
        };
        let previews = SharedPreviews::configured(&configuration);
        let (_, program_citp) = previews
            .for_output(output.id)
            .unwrap()
            .latest()
            .expect("Program has a black CITP fallback");
        let (_, program_web) = previews
            .for_output(output.id)
            .unwrap()
            .latest_web()
            .expect("Program has a black browser fallback");
        assert_eq!(program_web.content_type, "image/jpeg");
        assert_eq!(program_web.bytes, program_citp.jpeg);

        let (_, layer_web) = previews
            .for_layer(output.id, 0)
            .unwrap()
            .latest_web()
            .expect("Layer has a transparent browser fallback");
        assert_eq!(layer_web.content_type, "image/png");
        let decoder = png::Decoder::new(std::io::Cursor::new(&layer_web.bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert!(
            pixels[..info.buffer_size()]
                .chunks_exact(4)
                .all(|pixel| pixel[3] == 0),
            "empty layer pixels stay transparent"
        );
    }

    #[test]
    fn every_live_program_publication_is_byte_identical_for_web_and_citp() {
        let preview = Preview::new();
        let size = Size::new(32, 18);
        preview.publish_pixels(&image(size, [220, 10, 40, 255]), size, size, false);
        let (first_sequence, first_citp) = preview.latest().unwrap();
        let (_, first_web) = preview.latest_web().unwrap();
        assert_eq!(first_web.content_type, "image/jpeg");
        assert_eq!(first_web.bytes, first_citp.jpeg);

        preview.publish_pixels(&image(size, [10, 40, 220, 255]), size, size, false);
        let (second_sequence, second_citp) = preview.latest().unwrap();
        let (_, second_web) = preview.latest_web().unwrap();
        assert!(second_sequence > first_sequence);
        assert_ne!(
            second_citp.jpeg, first_citp.jpeg,
            "live video frames advance"
        );
        assert_eq!(second_web.bytes, second_citp.jpeg);
    }

    #[test]
    fn a_layer_preview_converts_premultiplied_compositor_pixels_to_straight_png_alpha() {
        let size = Size::new(1, 1);
        let (_, web) = encode_publication(&[64, 32, 16, 128], size, size, true).unwrap();
        assert_eq!(web.content_type, "image/png");
        let decoder = png::Decoder::new(std::io::Cursor::new(&web.bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(&pixels[..info.buffer_size()], &[128, 64, 32, 128]);
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
    fn a_browser_snapshot_renews_a_bounded_capture_lease() {
        let preview = Preview::new();
        preview.requested_for_browser(Some((480, 270)));
        assert!(preview.wanted());
        assert_eq!(preview.requested_size(), Size::new(480, 270));
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
