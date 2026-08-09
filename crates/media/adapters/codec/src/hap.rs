//! HAP Alpha frames.
//!
//! A HAP Alpha frame is BC3 (DXT5) blocks, Snappy-compressed. Both halves are cheap: a GPU that
//! samples BC uploads the blocks untouched, and Snappy decompresses at gigabytes per second on one
//! core. That is what makes two 1080p layers affordable on modest hardware.
//!
//! Every frame is independently decodable, which is the property the whole playback contract rests
//! on: reverse, bounce, speed changes, seeking, and a frame-exact Once all become ordinary random
//! access rather than the hardest part of a decoder.

use texpresso::Format;

/// Bytes one BC3 block occupies. Each block covers a 4×4 pixel tile.
pub const BC3_BLOCK_BYTES: usize = 16;

/// The compressed-texture format a HAP Alpha frame stores.
pub const TEXTURE_FORMAT: Format = Format::Bc3;

/// Why a frame could not be encoded or decoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("a frame cannot have a zero width or height")]
    Empty,
    #[error("the pixel buffer holds {found} bytes but {width}x{height} RGBA needs {expected}")]
    WrongPixelLength {
        width: u32,
        height: u32,
        expected: usize,
        found: usize,
    },
    #[error("the frame did not decompress: {detail}")]
    Corrupt { detail: String },
    #[error("the frame decompressed to {found} bytes but {width}x{height} BC3 needs {expected}")]
    WrongBlockLength {
        width: u32,
        height: u32,
        expected: usize,
        found: usize,
    },
}

/// The number of BC3 bytes a frame of this size occupies, uncompressed.
///
/// Block-compressed formats cover whole 4×4 tiles, so an image whose dimensions are not multiples
/// of four still pays for the partial tiles at its edges.
pub const fn block_bytes(width: u32, height: u32) -> usize {
    let blocks_across = width.div_ceil(4) as usize;
    let blocks_down = height.div_ceil(4) as usize;
    blocks_across * blocks_down * BC3_BLOCK_BYTES
}

/// The compression settings import uses.
///
/// Range fit rather than cluster fit: the default iterative search costs roughly two orders of
/// magnitude more time for a quality difference that does not survive being composited, tinted,
/// and dimmed. Import has to keep up with whole clips, so a frame that takes over a second is not
/// a trade worth making.
fn params() -> texpresso::Params {
    texpresso::Params {
        algorithm: texpresso::Algorithm::RangeFit,
        ..Default::default()
    }
}

/// Compresses one straight 8-bit RGBA frame into a HAP Alpha payload.
///
/// Alpha survives: BC3 stores it in its own interpolated channel, which is why this is the format
/// the product needs — transparency was the requirement that eliminated H.264 and MJPEG.
pub fn encode(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, FrameError> {
    if width == 0 || height == 0 {
        return Err(FrameError::Empty);
    }
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(FrameError::WrongPixelLength {
            width,
            height,
            expected,
            found: rgba.len(),
        });
    }

    let mut blocks = vec![0u8; block_bytes(width, height)];
    TEXTURE_FORMAT.compress(rgba, width as usize, height as usize, params(), &mut blocks);

    snap::raw::Encoder::new()
        .compress_vec(&blocks)
        .map_err(|error| FrameError::Corrupt {
            detail: error.to_string(),
        })
}

/// Decompresses a HAP Alpha payload back to BC3 blocks.
///
/// The blocks are what the GPU wants, so this is the whole decode path on any adapter that
/// samples BC. Expanding them to RGBA is only needed where it does not.
pub fn decode_blocks(width: u32, height: u32, payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    if width == 0 || height == 0 {
        return Err(FrameError::Empty);
    }
    let blocks = snap::raw::Decoder::new()
        .decompress_vec(payload)
        .map_err(|error| FrameError::Corrupt {
            detail: error.to_string(),
        })?;

    let expected = block_bytes(width, height);
    if blocks.len() != expected {
        return Err(FrameError::WrongBlockLength {
            width,
            height,
            expected,
            found: blocks.len(),
        });
    }
    Ok(blocks)
}

/// Expands BC3 blocks to straight 8-bit RGBA.
///
/// Only for adapters that cannot sample BC — some ARM GPUs expose only ETC2 and ASTC. On
/// everything else the blocks go to the GPU untouched and this is never called.
pub fn expand_to_rgba(width: u32, height: u32, blocks: &[u8]) -> Result<Vec<u8>, FrameError> {
    if width == 0 || height == 0 {
        return Err(FrameError::Empty);
    }
    let expected = block_bytes(width, height);
    if blocks.len() != expected {
        return Err(FrameError::WrongBlockLength {
            width,
            height,
            expected,
            found: blocks.len(),
        });
    }
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    TEXTURE_FORMAT.decompress(blocks, width as usize, height as usize, &mut rgba);
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, colour: [u8; 4]) -> Vec<u8> {
        colour.repeat(width as usize * height as usize)
    }

    #[test]
    fn block_sizes_cover_partial_tiles() {
        assert_eq!(block_bytes(4, 4), 16, "one block");
        assert_eq!(block_bytes(8, 8), 64, "four blocks");
        assert_eq!(
            block_bytes(5, 5),
            64,
            "a partial tile still costs a whole block"
        );
        assert_eq!(block_bytes(1920, 1080), 1920 / 4 * (1080 / 4) * 16);
    }

    #[test]
    fn a_frame_round_trips_through_the_codec() {
        let (width, height) = (64, 64);
        let payload = encode(width, height, &solid(width, height, [200, 40, 90, 255])).unwrap();
        let blocks = decode_blocks(width, height, &payload).unwrap();
        assert_eq!(blocks.len(), block_bytes(width, height));

        let rgba = expand_to_rgba(width, height, &blocks).unwrap();
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        // BC3 is lossy in colour, so a flat fill comes back close rather than exact.
        for pixel in rgba.chunks_exact(4) {
            assert!(
                (pixel[0] as i32 - 200).abs() <= 8,
                "red drifted to {}",
                pixel[0]
            );
            assert!(
                (pixel[1] as i32 - 40).abs() <= 8,
                "green drifted to {}",
                pixel[1]
            );
            assert!(
                (pixel[2] as i32 - 90).abs() <= 8,
                "blue drifted to {}",
                pixel[2]
            );
        }
    }

    #[test]
    fn transparency_survives_the_round_trip() {
        let (width, height) = (16, 16);
        // The requirement that chose this codec: alpha has to come back.
        for alpha in [0u8, 64, 128, 255] {
            let payload =
                encode(width, height, &solid(width, height, [255, 255, 255, alpha])).unwrap();
            let blocks = decode_blocks(width, height, &payload).unwrap();
            let rgba = expand_to_rgba(width, height, &blocks).unwrap();
            for pixel in rgba.chunks_exact(4) {
                assert!(
                    (pixel[3] as i32 - alpha as i32).abs() <= 8,
                    "alpha {alpha} came back as {}",
                    pixel[3]
                );
            }
        }
    }

    #[test]
    fn a_frame_with_mixed_alpha_keeps_its_shape() {
        let (width, height) = (16, 16);
        let mut rgba = Vec::new();
        for y in 0..height {
            for _ in 0..width {
                // Top half opaque, bottom half fully transparent.
                let alpha = if y < height / 2 { 255 } else { 0 };
                rgba.extend_from_slice(&[10, 200, 30, alpha]);
            }
        }
        let payload = encode(width, height, &rgba).unwrap();
        let decoded = expand_to_rgba(
            width,
            height,
            &decode_blocks(width, height, &payload).unwrap(),
        )
        .unwrap();

        let row = |y: u32| decoded[(y * width * 4) as usize + 3];
        assert!(row(0) > 240, "the opaque half stayed opaque");
        assert!(
            row(height - 1) < 16,
            "the transparent half stayed transparent"
        );
    }

    #[test]
    fn compression_actually_saves_space_on_real_content() {
        let (width, height) = (256, 256);
        let raw = width as usize * height as usize * 4;
        let payload = encode(width, height, &solid(width, height, [12, 34, 56, 255])).unwrap();
        assert!(
            payload.len() < raw / 8,
            "a flat frame compressed to {} of {raw} bytes",
            payload.len()
        );
    }

    #[test]
    fn a_wrongly_sized_pixel_buffer_is_refused() {
        let error = encode(8, 8, &[0u8; 16]).unwrap_err();
        assert_eq!(
            error,
            FrameError::WrongPixelLength {
                width: 8,
                height: 8,
                expected: 256,
                found: 16
            }
        );
    }

    #[test]
    fn an_empty_frame_is_refused_rather_than_producing_nothing() {
        assert_eq!(encode(0, 8, &[]).unwrap_err(), FrameError::Empty);
        assert_eq!(decode_blocks(8, 0, &[]).unwrap_err(), FrameError::Empty);
    }

    #[test]
    fn a_corrupt_payload_reports_rather_than_panicking() {
        let error = decode_blocks(8, 8, &[0xFF, 0xFF, 0xFF, 0xFF]).unwrap_err();
        assert!(matches!(error, FrameError::Corrupt { .. }), "{error}");
    }

    #[test]
    fn a_payload_for_the_wrong_size_is_refused() {
        let payload = encode(16, 16, &solid(16, 16, [1, 2, 3, 4])).unwrap();
        let error = decode_blocks(32, 32, &payload).unwrap_err();
        assert!(
            matches!(error, FrameError::WrongBlockLength { .. }),
            "{error}"
        );
    }
}
