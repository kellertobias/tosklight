//! Import: any source format into a `.toskclip`.
//!
//! FFmpeg decodes, out of process, into raw RGBA — something every build can do for every format,
//! and a process boundary that keeps its licensing entirely separate from ToskLight's. This crate
//! compresses those frames to HAP Alpha and writes the container.
//!
//! Import is long-running by nature, so it reports progress rather than going quiet, and it
//! publishes atomically: the destination only appears once the whole clip is written. A failed or
//! cancelled import never leaves a truncated file that looks playable.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::container::{ClipHeader, ClipWriter};

/// What an import is doing, for the operator and the job model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    /// The source has been probed and the work is known.
    Started {
        width: u32,
        height: u32,
        frames: Option<u32>,
    },
    /// A frame has been encoded. `total` is absent when the source did not report a frame count.
    Encoded {
        frame: u32,
        total: Option<u32>,
    },
    Finished {
        frames: u32,
        bytes: u64,
    },
}

impl Progress {
    /// How far along, when the total is known. Nothing invents a percentage it cannot compute.
    pub fn fraction(&self) -> Option<f32> {
        match self {
            Self::Encoded {
                frame,
                total: Some(total),
            } if *total > 0 => Some(*frame as f32 / *total as f32),
            Self::Finished { .. } => Some(1.0),
            _ => None,
        }
    }
}

/// Why an import could not finish.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("FFmpeg is not installed or not on PATH; import needs it to read source media")]
    FfmpegMissing,
    #[error("FFmpeg could not read {}: {detail}", path.display())]
    Unreadable { path: PathBuf, detail: String },
    #[error("{} has no video stream", path.display())]
    NoVideoStream { path: PathBuf },
    #[error("the source decoded no frames")]
    NoFrames,
    #[error(transparent)]
    Frame(#[from] crate::hap::FrameError),
    #[error(transparent)]
    Container(#[from] crate::container::ContainerError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What a source turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: (u32, u32),
    /// Absent when the container does not carry a reliable count.
    pub frames: Option<u32>,
}

/// Asks FFprobe what a source is.
pub fn probe(source: &Path) -> Result<SourceInfo, ImportError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate,nb_frames",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(source)
        .output()
        .map_err(|_| ImportError::FfmpegMissing)?;

    if !output.status.success() {
        return Err(ImportError::Unreadable {
            path: source.to_path_buf(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('=').map(str::trim))
            .filter(|value| *value != "N/A")
            .map(str::to_owned)
    };

    let (Some(width), Some(height)) = (field("width"), field("height")) else {
        return Err(ImportError::NoVideoStream {
            path: source.to_path_buf(),
        });
    };

    let rate = field("r_frame_rate").unwrap_or_else(|| "25/1".to_owned());
    let (numerator, denominator) = rate.split_once('/').unwrap_or((rate.as_str(), "1"));

    Ok(SourceInfo {
        width: width.parse().unwrap_or(0),
        height: height.parse().unwrap_or(0),
        frame_rate: (
            numerator.parse().unwrap_or(25),
            denominator.parse().unwrap_or(1),
        ),
        frames: field("nb_frames").and_then(|value| value.parse().ok()),
    })
}

/// Converts a source into a `.toskclip` at `destination`.
///
/// The authored tempo comes from the source's filename if it carries the token; it is stored as
/// metadata the operator can correct, and runtime never re-derives it.
///
/// `report` is called as the work proceeds. Returning `false` from it cancels the import, which
/// leaves nothing behind at the destination.
pub fn import(
    source: &Path,
    destination: &Path,
    report: &mut dyn FnMut(Progress) -> bool,
) -> Result<u32, ImportError> {
    let info = probe(source)?;
    if info.width == 0 || info.height == 0 {
        return Err(ImportError::NoVideoStream {
            path: source.to_path_buf(),
        });
    }
    if !report(Progress::Started {
        width: info.width,
        height: info.height,
        frames: info.frames,
    }) {
        return Ok(0);
    }

    let intrinsic_bpm = source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(media_domain::authored_tempo::from_filename);

    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(source)
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| ImportError::FfmpegMissing)?;
    let mut decoded = ffmpeg.stdout.take().expect("stdout was piped");

    // Written beside the destination and renamed at the end, so a reader never sees a partial
    // clip and an interrupted import leaves the library exactly as it was.
    let staging = staging_path(destination);
    let mut writer = ClipWriter::new(
        std::fs::File::create(&staging)?,
        ClipHeader {
            width: info.width,
            height: info.height,
            frame_count: 0,
            frame_rate: info.frame_rate,
            intrinsic_bpm,
        },
    )?;

    let frame_bytes = info.width as usize * info.height as usize * 4;
    let interval_micros = frame_interval_micros(info.frame_rate);
    let mut raw = vec![0u8; frame_bytes];
    let mut frames = 0u32;
    let mut cancelled = false;

    loop {
        match decoded.read_exact(&mut raw) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => {
                abandon(ffmpeg, &staging);
                return Err(ImportError::Io(error));
            }
        }

        let payload = match crate::hap::encode(info.width, info.height, &raw) {
            Ok(payload) => payload,
            Err(error) => {
                abandon(ffmpeg, &staging);
                return Err(error.into());
            }
        };
        writer.write_frame(&payload, u64::from(frames) * interval_micros)?;
        frames += 1;

        if !report(Progress::Encoded {
            frame: frames,
            total: info.frames,
        }) {
            cancelled = true;
            break;
        }
    }

    let _ = ffmpeg.kill();
    let status = ffmpeg.wait();

    if cancelled {
        drop(writer);
        let _ = std::fs::remove_file(&staging);
        return Ok(frames);
    }
    if frames == 0 {
        drop(writer);
        let _ = std::fs::remove_file(&staging);
        // A source that decoded nothing is a failure with FFmpeg's own reason attached.
        let detail = status
            .ok()
            .filter(|status| !status.success())
            .map(|status| format!("ffmpeg exited with {status}"))
            .unwrap_or_else(|| "no frames were produced".to_owned());
        return Err(ImportError::Unreadable {
            path: source.to_path_buf(),
            detail,
        });
    }

    let file = writer.finish()?;
    let bytes = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    drop(file);
    std::fs::rename(&staging, destination)?;

    report(Progress::Finished { frames, bytes });
    Ok(frames)
}

fn staging_path(destination: &Path) -> PathBuf {
    let mut name = destination.file_name().unwrap_or_default().to_os_string();
    name.push(".importing");
    destination.with_file_name(name)
}

fn abandon(mut ffmpeg: std::process::Child, staging: &Path) {
    let _ = ffmpeg.kill();
    let _ = ffmpeg.wait();
    let _ = std::fs::remove_file(staging);
}

const fn frame_interval_micros(rate: (u32, u32)) -> u64 {
    let (numerator, denominator) = rate;
    if numerator == 0 {
        return 40_000;
    }
    (1_000_000u64 * denominator as u64) / numerator as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_only_reports_a_fraction_it_can_actually_compute() {
        assert_eq!(
            Progress::Started {
                width: 1920,
                height: 1080,
                frames: Some(100)
            }
            .fraction(),
            None
        );
        assert_eq!(
            Progress::Encoded {
                frame: 25,
                total: Some(100)
            }
            .fraction(),
            Some(0.25)
        );
        assert_eq!(
            Progress::Encoded {
                frame: 25,
                total: None
            }
            .fraction(),
            None,
            "a source with no frame count gets no invented percentage"
        );
        assert_eq!(
            Progress::Encoded {
                frame: 1,
                total: Some(0)
            }
            .fraction(),
            None
        );
        assert_eq!(
            Progress::Finished {
                frames: 100,
                bytes: 1
            }
            .fraction(),
            Some(1.0)
        );
    }

    #[test]
    fn frame_intervals_follow_the_rational_rate() {
        assert_eq!(frame_interval_micros((25, 1)), 40_000);
        assert_eq!(frame_interval_micros((60, 1)), 16_666);
        assert_eq!(frame_interval_micros((30_000, 1_001)), 33_366);
        assert_eq!(
            frame_interval_micros((0, 1)),
            40_000,
            "a nonsense rate does not divide by zero"
        );
    }

    #[test]
    fn staging_sits_beside_the_destination_so_the_rename_is_atomic() {
        let staging = staging_path(Path::new("/library/001/007-Clip.toskclip"));
        assert_eq!(
            staging.parent(),
            Path::new("/library/001/007-Clip.toskclip").parent()
        );
        assert_eq!(
            staging.file_name().unwrap().to_str().unwrap(),
            "007-Clip.toskclip.importing"
        );
    }

    #[test]
    fn probing_something_that_is_not_media_reports_rather_than_panicking() {
        let directory = std::env::temp_dir().join("media-import-probe-test");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("not-media.txt");
        std::fs::write(&path, b"this is not a video").unwrap();

        match probe(&path) {
            Err(ImportError::Unreadable { .. } | ImportError::NoVideoStream { .. }) => {}
            // A machine without FFmpeg cannot import at all, which this says plainly.
            Err(ImportError::FfmpegMissing) => {}
            other => panic!("expected a readable failure, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }
}
