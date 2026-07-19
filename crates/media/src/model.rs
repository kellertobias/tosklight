use serde::{Deserialize, Serialize};
use std::{io, time::SystemTime};
use thiserror::Error;

pub(crate) const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("CITP I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("CITP operation timed out")]
    Timeout,
    #[error("invalid CITP packet: {0}")]
    Invalid(String),
    #[error("media server rejected {0}")]
    Rejected(String),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct LibraryId {
    pub level: u8,
    pub ids: [u8; 3],
}

impl LibraryId {
    pub const ROOT: Self = Self {
        level: 0,
        ids: [0; 3],
    };

    pub(crate) fn encode(self, output: &mut Vec<u8>) {
        output.push(self.level.min(3));
        output.extend_from_slice(&self.ids);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Jpeg,
    Png,
    Rgb8,
}

impl ImageFormat {
    pub(crate) fn cookie(self) -> [u8; 4] {
        match self {
            Self::Jpeg => *b"JPEG",
            Self::Png => *b"PNG ",
            Self::Rgb8 => *b"RGB8",
        }
    }

    pub(crate) fn parse(value: [u8; 4]) -> Result<Self, MediaError> {
        match &value {
            b"JPEG" => Ok(Self::Jpeg),
            b"PNG " => Ok(Self::Png),
            b"RGB8" => Ok(Self::Rgb8),
            _ => Err(MediaError::Invalid(format!(
                "unsupported image format {:?}",
                String::from_utf8_lossy(&value)
            ))),
        }
    }

    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Rgb8 => "application/x-citp-rgb8",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaImage {
    pub format: ImageFormat,
    pub width: u16,
    pub height: u16,
    pub bytes: Vec<u8>,
}

impl MediaImage {
    pub(crate) fn validate(&self) -> Result<(), MediaError> {
        validate_dimensions(self.width, self.height)?;
        if self.bytes.is_empty() || self.bytes.len() > MAX_IMAGE_BYTES {
            return Err(MediaError::Invalid(
                "image payload is empty or exceeds the cache limit".into(),
            ));
        }
        if self.format == ImageFormat::Rgb8
            && self.bytes.len() != usize::from(self.width) * usize::from(self.height) * 3
        {
            return Err(MediaError::Invalid(
                "RGB8 payload size does not match dimensions".into(),
            ));
        }
        Ok(())
    }
}

fn validate_dimensions(width: u16, height: u16) -> Result<(), MediaError> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(MediaError::Invalid(
            "image dimensions are outside 1-4096".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ThumbnailKey {
    pub fixture: String,
    pub library_type: u8,
    pub library: LibraryId,
    pub element: u8,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PreviewKey {
    pub fixture: String,
    pub source: u16,
}

#[derive(Clone, Debug)]
pub struct CachedImage {
    pub image: MediaImage,
    pub received_at: SystemTime,
}
