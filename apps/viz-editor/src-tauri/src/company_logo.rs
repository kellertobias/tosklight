//! The lighting designer's company logo, and what this computer remembers of the designer.
//!
//! A logo is read once from the operator's file and kept in the show as one small JPEG, so it
//! travels with the show and prints on every page with no file beside it. Whatever was chosen —
//! PNG, JPEG or WebP, transparent or not — is flattened onto white, as it prints, and fitted inside
//! 800 × 400 pixels, which is sharper than a title block prints and small enough to carry.
//!
//! **Make Default** keeps the designer's name, phone, email and logo on this computer rather than
//! in any show, and a show created here afterwards starts with them.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ImageFormat, Rgb, RgbImage, Rgba};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A logo file larger than this is refused before it is decoded.
const MAX_LOGO_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_WIDTH: u32 = 800;
const MAX_HEIGHT: u32 = 400;
const JPEG_QUALITY: u8 = 90;

/// The file in the application data folder that holds this computer's lighting designer.
const DEFAULT_FILE: &str = "lighting-designer-default.json";

type Answer<T> = Result<T, String>;

/// A logo as the show keeps it: always a JPEG, with its size so a page can fit it without decoding.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLogo {
    pub media_type: String,
    pub width: u32,
    pub height: u32,
    /// The JPEG bytes, base64-encoded.
    pub data: String,
}

/// The lighting designer this computer fills into a show it creates.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LightingDesignerDefault {
    #[serde(default)]
    pub lighting_designer: String,
    #[serde(default)]
    pub contact_phone: String,
    #[serde(default)]
    pub contact_email: String,
    /// The logo as the show stores it (the JSON of a [`CompanyLogo`]), or empty for none.
    #[serde(default)]
    pub company_logo: String,
}

/// Turn an image file's bytes into the logo the show keeps, or say why it cannot be one.
pub fn normalize_company_logo(name: &str, bytes: &[u8]) -> Answer<CompanyLogo> {
    let format = image::guess_format(bytes)
        .map_err(|_| format!("{name} is not a PNG, JPEG or WebP image."))?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(format!("{name} is not a PNG, JPEG or WebP image."));
    }
    let decoded = image::load_from_memory_with_format(bytes, format)
        .map_err(|error| format!("{name} could not be read: {error}"))?;
    if decoded.width() == 0 || decoded.height() == 0 {
        return Err(format!("{name} has no pixels."));
    }
    let fitted = if decoded.width() > MAX_WIDTH || decoded.height() > MAX_HEIGHT {
        decoded.resize(MAX_WIDTH, MAX_HEIGHT, FilterType::Lanczos3)
    } else {
        decoded
    };
    let rgba = fitted.to_rgba8();
    let mut flat = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let Rgba([red, green, blue, alpha]) = *pixel;
        let over_white = |channel: u8| {
            let alpha = u16::from(alpha);
            ((u16::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8
        };
        flat.put_pixel(
            x,
            y,
            Rgb([over_white(red), over_white(green), over_white(blue)]),
        );
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&flat)
        .map_err(|error| format!("{name} could not be kept as a logo: {error}"))?;
    Ok(CompanyLogo {
        media_type: "image/jpeg".to_owned(),
        width: flat.width(),
        height: flat.height(),
        data: STANDARD.encode(jpeg),
    })
}

/// Refuse a stored logo that is not one this module wrote.
pub fn validate_stored_logo(stored: &str) -> Answer<()> {
    if stored.trim().is_empty() {
        return Ok(());
    }
    let logo: CompanyLogo = serde_json::from_str(stored)
        .map_err(|error| format!("The company logo is not readable: {error}"))?;
    if logo.media_type != "image/jpeg" || logo.width == 0 || logo.height == 0 {
        return Err("The company logo is not a JPEG with a size.".to_owned());
    }
    STANDARD
        .decode(&logo.data)
        .map(|_| ())
        .map_err(|error| format!("The company logo's image data is damaged: {error}"))
}

fn default_path(dir: &Path) -> PathBuf {
    dir.join(DEFAULT_FILE)
}

/// This computer's lighting designer, or none when **Make Default** was never pressed.
pub fn read_default(dir: &Path) -> Answer<Option<LightingDesignerDefault>> {
    match std::fs::read(default_path(dir)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| {
            format!("The lighting designer saved on this computer could not be read: {error}")
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "The lighting designer saved on this computer could not be read: {error}"
        )),
    }
}

/// Keep `default` as this computer's lighting designer, replacing the file in one step.
pub fn write_default(dir: &Path, default: &LightingDesignerDefault) -> Answer<()> {
    validate_stored_logo(&default.company_logo)?;
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let body = serde_json::to_vec_pretty(default).map_err(|error| error.to_string())?;
    let target = default_path(dir);
    let partial = target.with_extension("json.partial");
    std::fs::write(&partial, body).map_err(|error| error.to_string())?;
    std::fs::rename(&partial, &target).map_err(|error| error.to_string())
}

/// Read an image file the operator chose and return the logo the show would keep.
#[tauri::command]
pub fn read_company_logo(path: String) -> Answer<CompanyLogo> {
    let path = Path::new(&path);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("The logo")
        .to_owned();
    let size = std::fs::metadata(path)
        .map_err(|error| format!("{name} could not be read: {error}"))?
        .len();
    if size > MAX_LOGO_FILE_BYTES {
        return Err(format!(
            "{name} is {} MB; a logo may be at most {} MB.",
            size.div_ceil(1024 * 1024),
            MAX_LOGO_FILE_BYTES / (1024 * 1024)
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|error| format!("{name} could not be read: {error}"))?;
    normalize_company_logo(&name, &bytes)
}

/// This computer's lighting designer, if one was made the default.
#[tauri::command]
pub fn lighting_designer_default(app: tauri::AppHandle) -> Answer<Option<LightingDesignerDefault>> {
    let dir = crate::portable::app_data_dir(&app).map_err(|error| error.to_string())?;
    read_default(&dir)
}

/// Make these the lighting designer every show created on this computer starts with.
#[tauri::command]
pub fn save_lighting_designer_default(
    app: tauri::AppHandle,
    default: LightingDesignerDefault,
) -> Answer<LightingDesignerDefault> {
    let dir = crate::portable::app_data_dir(&app).map_err(|error| error.to_string())?;
    let trimmed = LightingDesignerDefault {
        lighting_designer: default.lighting_designer.trim().to_owned(),
        contact_phone: default.contact_phone.trim().to_owned(),
        contact_email: default.contact_email.trim().to_owned(),
        company_logo: default.company_logo.trim().to_owned(),
    };
    write_default(&dir, &trimmed)?;
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, RgbaImage};
    use std::io::Cursor;

    fn png(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        let image: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba(pixel));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .expect("png");
        bytes
    }

    #[test]
    fn keeps_a_transparent_png_as_a_jpeg_flattened_onto_white_and_fitted_to_the_title_block() {
        let logo = normalize_company_logo("logo.png", &png(1600, 400, [0, 0, 0, 0])).unwrap();
        assert_eq!(logo.media_type, "image/jpeg");
        assert_eq!((logo.width, logo.height), (800, 200));
        let jpeg = STANDARD.decode(&logo.data).unwrap();
        let decoded = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg)
            .unwrap()
            .to_rgb8();
        let Rgb([red, green, blue]) = *decoded.get_pixel(10, 10);
        assert!(
            red > 240 && green > 240 && blue > 240,
            "transparent reads as white"
        );
        assert!(validate_stored_logo(&serde_json::to_string(&logo).unwrap()).is_ok());
    }

    #[test]
    fn leaves_a_small_logo_at_its_own_size() {
        let logo = normalize_company_logo("mark.png", &png(120, 60, [200, 0, 0, 255])).unwrap();
        assert_eq!((logo.width, logo.height), (120, 60));
    }

    #[test]
    fn refuses_a_file_that_is_not_an_image_by_name() {
        let error = normalize_company_logo("notes.txt", b"not an image").unwrap_err();
        assert!(error.contains("notes.txt"), "{error}");
        assert!(validate_stored_logo("{\"mediaType\":\"image/gif\"}").is_err());
        assert!(validate_stored_logo("").is_ok());
    }

    #[test]
    fn remembers_the_lighting_designer_on_this_computer() {
        let dir =
            std::env::temp_dir().join(format!("tosklight-ld-default-{}", uuid::Uuid::new_v4()));
        assert_eq!(read_default(&dir).unwrap(), None);
        let logo = normalize_company_logo("mark.png", &png(40, 20, [0, 0, 255, 255])).unwrap();
        let default = LightingDesignerDefault {
            lighting_designer: "Tobias Keller".to_owned(),
            contact_phone: "+49 30 1234".to_owned(),
            contact_email: "ld@example.com".to_owned(),
            company_logo: serde_json::to_string(&logo).unwrap(),
        };
        write_default(&dir, &default).unwrap();
        assert_eq!(read_default(&dir).unwrap(), Some(default));
        assert!(
            write_default(
                &dir,
                &LightingDesignerDefault {
                    company_logo: "not json".to_owned(),
                    ..LightingDesignerDefault::default()
                }
            )
            .is_err()
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
