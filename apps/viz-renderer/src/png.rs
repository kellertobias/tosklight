//! PNG writing for captured frames, and reading the application icon.
//!
//! Golden images and benchmark evidence need a real image file, but the renderer must not pull an
//! image-processing dependency into the desk workspace for it. Stored deflate blocks keep the
//! encoder to a few dozen lines at the cost of file size, which does not matter for evidence.

/// The ToskLight Viz application icon, decoded to `RGBA8` at the overlay's icon size.
///
/// The operator surface shows the real application mark rather than something drawn to look like
/// it, so the Viz application's own icon is the source: the ToskLight mark badged "3D", shared
/// with the editor that owns this session. A packaging problem must not stop the visualizer from
/// opening, so every failure here simply leaves the corner empty.
pub fn application_icon() -> Option<Vec<u8>> {
    const ENCODED: &[u8] = include_bytes!("../../viz-editor/src-tauri/icons/128x128.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(ENCODED));
    let mut reader = decoder.read_info().ok()?;
    let mut raw = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut raw).ok()?;
    let edge = viz_render::overlay::ICON_SIZE;
    if info.width as usize != edge || info.height as usize != edge {
        return None;
    }
    let rgba = match info.color_type {
        png::ColorType::Rgba => raw[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => raw[..info.buffer_size()]
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 0xff])
            .collect(),
        _ => return None,
    };
    (rgba.len() == edge * edge * 4).then_some(rgba)
}

pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0); // filter: none
        let start = row * width as usize * 4;
        raw.extend_from_slice(&rgba[start..start + width as usize * 4]);
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    chunk(&mut png, b"IEND", &[]);
    png
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut stream = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < data.len() {
        let length = (data.len() - offset).min(65_535);
        let final_block = offset + length >= data.len();
        stream.push(u8::from(final_block));
        stream.extend_from_slice(&(length as u16).to_le_bytes());
        stream.extend_from_slice(&(!(length as u16)).to_le_bytes());
        stream.extend_from_slice(&data[offset..offset + length]);
        offset += length;
    }
    stream.extend_from_slice(&adler32(data).to_be_bytes());
    stream
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1_u32;
    let mut b = 0_u32;
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_application_icon_decodes_at_the_atlas_size() {
        let icon = super::application_icon().expect("the packaged icon decodes");
        assert_eq!(
            icon.len(),
            viz_render::overlay::ICON_SIZE * viz_render::overlay::ICON_SIZE * 4
        );
        assert!(
            icon.chunks_exact(4).any(|texel| texel[3] > 0),
            "a fully transparent icon would leave the corner empty"
        );
    }
    use super::*;

    #[test]
    fn the_encoder_emits_a_valid_signature_and_terminator() {
        let png = encode_rgba(2, 2, &[255; 16]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert!(png.ends_with(&crc32(b"IEND").to_be_bytes()));
        assert!(png.windows(4).any(|window| window == b"IHDR"));
        assert!(png.windows(4).any(|window| window == b"IDAT"));
    }

    #[test]
    fn crc32_matches_the_known_png_test_vector() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
    }
}
