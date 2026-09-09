fn main() {
    use std::io::Write;

    println!("cargo:rerun-if-changed=../../assets/branding/ToskLight Pixel.png");
    println!("cargo:rerun-if-env-changed=LIGHT_RELEASE_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let source = std::fs::read("../../assets/branding/ToskLight Pixel.png")
        .expect("the ToskLight Pixel icon exists");
    let decoder = png::Decoder::new(std::io::Cursor::new(source));
    let mut reader = decoder.read_info().expect("the Pixel icon is a PNG");
    let mut pixels = vec![0; reader.output_buffer_size().unwrap_or_default()];
    let info = reader
        .next_frame(&mut pixels)
        .expect("the Pixel icon has one readable frame");
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    let images: Vec<(u32, Vec<u8>)> = [16, 32, 48, 256]
        .into_iter()
        .map(|edge| {
            (
                edge,
                encode_png(&pixels[..info.buffer_size()], info.width, info.height, edge),
            )
        })
        .collect();

    let icon_path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"))
        .join("tosklight-pixel.ico");
    let mut icon = std::fs::File::create(&icon_path).expect("the Pixel icon can be staged");
    icon.write_all(&[0, 0, 1, 0])
        .expect("the icon header can be written");
    icon.write_all(&(images.len() as u16).to_le_bytes())
        .expect("the icon count can be written");
    let mut offset = 6 + images.len() as u32 * 16;
    for (edge, image) in &images {
        icon.write_all(&[
            if *edge == 256 { 0 } else { *edge as u8 },
            if *edge == 256 { 0 } else { *edge as u8 },
            0,
            0,
            1,
            0,
            32,
            0,
        ])
        .expect("the icon directory can be written");
        icon.write_all(&(image.len() as u32).to_le_bytes())
            .expect("the icon length can be written");
        icon.write_all(&offset.to_le_bytes())
            .expect("the icon offset can be written");
        offset += image.len() as u32;
    }
    for (_, image) in images {
        icon.write_all(&image)
            .expect("the icon image can be written");
    }

    let version = std::env::var("LIGHT_RELEASE_VERSION")
        .unwrap_or_else(|_| std::env::var("CARGO_PKG_VERSION").expect("package version"));
    let numeric_version = numeric_version(&version);
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(icon_path.to_str().expect("the icon path is Unicode"))
        .set("ProductName", "ToskLight Pixel")
        .set("FileDescription", "ToskLight Pixel launcher")
        .set("InternalName", "ToskLight Pixel")
        .set("OriginalFilename", "ToskLight Pixel.exe")
        .set("FileVersion", &version)
        .set("ProductVersion", &version)
        .set_version_info(winresource::VersionInfo::FILEVERSION, numeric_version)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric_version)
        .compile()
        .expect("the ToskLight Pixel Windows resources compile");
}

fn scale_rgba(source: &[u8], width: u32, height: u32, edge: u32) -> Vec<u8> {
    let mut destination = vec![0u8; (edge * edge * 4) as usize];
    for y in 0..edge {
        for x in 0..edge {
            let from_x = x * width / edge;
            let to_x = ((x + 1) * width / edge).max(from_x + 1).min(width);
            let from_y = y * height / edge;
            let to_y = ((y + 1) * height / edge).max(from_y + 1).min(height);
            let mut totals = [0u32; 4];
            let mut count = 0u32;
            for source_y in from_y..to_y {
                for source_x in from_x..to_x {
                    let at = ((source_y * width + source_x) * 4) as usize;
                    for channel in 0..4 {
                        totals[channel] += u32::from(source[at + channel]);
                    }
                    count += 1;
                }
            }
            let at = ((y * edge + x) * 4) as usize;
            for channel in 0..4 {
                destination[at + channel] = (totals[channel] / count.max(1)) as u8;
            }
        }
    }
    destination
}

fn encode_png(source: &[u8], width: u32, height: u32, edge: u32) -> Vec<u8> {
    let pixels = scale_rgba(source, width, height, edge);
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, edge, edge);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .expect("the Pixel icon can be encoded");
        writer
            .write_image_data(&pixels)
            .expect("the Pixel icon pixels can be encoded");
    }
    encoded
}

fn numeric_version(version: &str) -> u64 {
    let mut parts = version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map(|part| part.parse::<u16>().unwrap_or(0));
    (u64::from(parts.next().unwrap_or(0)) << 48)
        | (u64::from(parts.next().unwrap_or(0)) << 32)
        | (u64::from(parts.next().unwrap_or(0)) << 16)
        | u64::from(parts.next().unwrap_or(0))
}
