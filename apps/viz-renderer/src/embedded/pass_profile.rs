//! Explicit benchmark diagnostic only; samples come from existing successful draw statistics.
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};
use viz_render::FrameStats;
use viz_scene::ViewConfiguration;

const MAX_ROWS: u64 = 8192;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
const ROW_BYTES: usize = 1024;
const LABELS: [&str; 8] = [
    "cull",
    "shadow",
    "opaque",
    "beams",
    "lasers",
    "bloom",
    "composite",
    "overlay",
];

pub(super) struct PassProfile {
    file: Option<File>,
    rows: u64,
    bytes: u64,
    last_sample: Option<u64>,
}
#[derive(serde::Serialize)]
struct Row<'a> {
    pid: u32,
    view: &'a str,
    quality: &'a str,
    width: u32,
    height: u32,
    pixels: u64,
    follow_preload: bool,
    scene_revision: u64,
    gpu_sample_id: u64,
    gpu_provenance_valid: bool,
    timing_coverage: Option<viz_scene::gpu_timing::TimingCoverage>,
    pass_labels: [&'static str; 8],
    pass_micros: [Option<u64>; 8],
}
impl PassProfile {
    /// Called only when benchmark reporting is enabled, once per pane/helper lifetime.
    pub(super) fn from_environment() -> Option<Self> {
        let path = std::env::var_os("LIGHT_STAGE_PASS_PROFILE_PATH")?;
        match Self::open(Path::new(&path)) {
            Ok(profile) => Some(profile),
            Err(error) => {
                eprintln!("Stage pass profile disabled: {error}");
                None
            }
        }
    }
    fn open(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::other(
                "LIGHT_STAGE_PASS_PROFILE_PATH must be an absolute caller-owned artifact path",
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("pass profile requires a regular file"));
        }
        Ok(Self {
            file: Some(file),
            rows: 0,
            bytes: 0,
            last_sample: None,
        })
    }
    pub(super) fn observe(
        &mut self,
        stats: &FrameStats,
        view: &ViewConfiguration,
        size: (u32, u32),
        scene_revision: u64,
        follow_preload: bool,
        timing_coverage: Option<viz_scene::gpu_timing::TimingCoverage>,
    ) {
        if self.file.is_none() || !eligible(stats, self.last_sample) {
            return;
        }
        let sample = stats.gpu_sample_id.unwrap();
        let row = Row {
            pid: std::process::id(),
            view: view.mode.wire(),
            quality: view.quality.wire(),
            width: size.0,
            height: size.1,
            pixels: u64::from(size.0) * u64::from(size.1),
            follow_preload,
            scene_revision,
            gpu_sample_id: sample,
            gpu_provenance_valid: true,
            timing_coverage,
            pass_labels: LABELS,
            pass_micros: stats.gpu_passes.as_array(),
        };
        let mut encoded = RowBuffer::default();
        let result = serde_json::to_writer(&mut encoded, &row)
            .map_err(io::Error::other)
            .and_then(|()| encoded.write_all(b"\n"))
            .and_then(|()| self.append(&encoded.bytes[..encoded.len]));
        match result {
            Ok(Append::Written) => self.last_sample = Some(sample),
            Ok(Append::Contended) => {}
            Ok(Append::Capped) => self.file = None,
            Err(error) => {
                self.file = None;
                eprintln!("Stage pass profile disabled after I/O failure: {error}");
            }
        }
    }
    fn append(&mut self, row: &[u8]) -> io::Result<Append> {
        let Some(file) = self.file.as_mut() else {
            return Ok(Append::Capped);
        };
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Ok(Append::Contended),
            Err(error) => return Err(io::Error::other(format!("profile lock: {error}"))),
        }
        let result = (|| {
            count_appended(file, &mut self.bytes, &mut self.rows)?;
            if self.rows >= MAX_ROWS || self.bytes.saturating_add(row.len() as u64) > MAX_BYTES {
                return Ok(Append::Capped);
            }
            // One O_APPEND write under the shared nonblocking lock: no buffered queue or retry.
            if file.write(row)? != row.len() {
                return Err(io::Error::other("partial profile row write"));
            }
            self.rows += 1;
            self.bytes += row.len() as u64;
            Ok(Append::Written)
        })();
        let unlock = file.unlock();
        result.and_then(|result| unlock.map(|()| result))
    }
}
#[derive(PartialEq, Debug)]
enum Append {
    Written,
    Contended,
    Capped,
}
fn eligible(stats: &FrameStats, previous: Option<u64>) -> bool {
    stats.gpu_sample_matches_frame
        && stats.gpu_micros.is_some()
        && stats.gpu_passes.as_array().iter().any(Option::is_some)
        && stats
            .gpu_sample_id
            .is_some_and(|id| previous.is_none_or(|previous| id > previous))
}
fn count_appended(file: &mut File, bytes: &mut u64, rows: &mut u64) -> io::Result<()> {
    let length = file.metadata()?.len();
    if length < *bytes {
        return Err(io::Error::other(
            "profile file was truncated during observation",
        ));
    }
    if length >= MAX_BYTES {
        *bytes = length;
        *rows = MAX_ROWS;
        return Ok(());
    }
    file.seek(SeekFrom::Start(*bytes))?;
    let mut buffer = [0u8; 4096];
    let mut last = None;
    while *bytes < length {
        let count = file.read(&mut buffer[..(length - *bytes).min(4096) as usize])?;
        if count == 0 {
            return Err(io::Error::other("profile file changed during observation"));
        }
        last = Some(buffer[count - 1]);
        *bytes += count as u64;
        *rows += buffer[..count].iter().filter(|b| **b == b'\n').count() as u64;
    }
    if last.is_some_and(|byte| byte != b'\n') {
        return Err(io::Error::other("profile file ends with a partial row"));
    }
    Ok(())
}
struct RowBuffer {
    bytes: [u8; ROW_BYTES],
    len: usize,
}
impl Default for RowBuffer {
    fn default() -> Self {
        Self {
            bytes: [0; ROW_BYTES],
            len: 0,
        }
    }
}
impl Write for RowBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.len + bytes.len() > ROW_BYTES {
            return Err(io::Error::other("profile row exceeds1KiB"));
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[cfg(test)]
mod tests;
