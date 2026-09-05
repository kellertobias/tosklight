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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Checked independently of blocking decoder reads so shutdown can interrupt a hung subprocess.
pub type Cancellation = Arc<dyn Fn() -> bool + Send + Sync>;

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
    #[error("the import was cancelled")]
    Cancelled,
    #[error("the media decoder produced no output for 30 seconds")]
    DecoderStalled,
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
    probe_cancellable(source, Arc::new(|| false))
}

fn probe_cancellable(source: &Path, cancelled: Cancellation) -> Result<SourceInfo, ImportError> {
    if cancelled() {
        return Err(ImportError::Cancelled);
    }
    let mut command = Command::new("ffprobe");
    command
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command_output_cancellable(&mut command, cancelled)?;
    let status = output.status;
    let stdout = output.stdout;
    let stderr = output.stderr;

    if !status.success() {
        return Err(ImportError::Unreadable {
            path: source.to_path_buf(),
            detail: String::from_utf8_lossy(&stderr).trim().to_owned(),
        });
    }

    let text = String::from_utf8_lossy(&stdout);
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

/// Runs a bounded metadata/thumbnail command with cancellation and concurrent pipe draining.
/// These commands must finish within 30 seconds; clip conversion uses per-read inactivity instead.
pub fn command_output_cancellable(
    command: &mut Command,
    cancelled: Cancellation,
) -> Result<std::process::Output, ImportError> {
    if cancelled() {
        return Err(ImportError::Cancelled);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let process = SupervisedChild::spawn(command, cancelled)?;
    let (mut stdout, mut stderr) = {
        let mut child = process.child.lock().expect("the decoder process");
        (
            child.stdout.take().expect("piped stdout"),
            child.stderr.take().expect("piped stderr"),
        )
    };
    let (stdout, stderr) = std::thread::scope(|scope| {
        let errors = scope.spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let mut bytes = Vec::new();
        let output = stdout.read_to_end(&mut bytes).map(|_| bytes);
        (output, errors.join().expect("the stderr reader"))
    });
    Ok(std::process::Output {
        status: process.wait()?,
        stdout: stdout?,
        stderr: stderr?,
    })
}

/// Whether this machine can transcode at all.
///
/// Import shells out to FFmpeg, so a machine without it can do everything else a Media Server does
/// and none of this. Asked once and reported, rather than discovered one failed clip at a time.
pub fn ffmpeg_available() -> bool {
    command_output_cancellable(Command::new("ffmpeg").arg("-version"), Arc::new(|| false))
        .is_ok_and(|output| output.status.success())
}

/// Converts a source into a `.toskclip` at `destination`.
///
/// The authored tempo comes from the source's filename if it carries the token; it is stored as
/// metadata the operator can correct, and runtime never re-derives it.
///
/// `report` is called as the work proceeds. Returning `false` from it cancels the import, which
/// leaves nothing behind at the destination and returns zero. A positive frame count means
/// the complete clip was published. Cancellation after `Finished` cannot undo publication.
pub fn import(
    source: &Path,
    destination: &Path,
    report: &mut dyn FnMut(Progress) -> bool,
) -> Result<u32, ImportError> {
    import_cancellable(source, destination, report, Arc::new(|| false))
}

/// Like `import`, with cancellation supervised even while probing or waiting for a raw frame.
pub fn import_cancellable(
    source: &Path,
    destination: &Path,
    report: &mut dyn FnMut(Progress) -> bool,
    cancelled: Cancellation,
) -> Result<u32, ImportError> {
    match import_inner(source, destination, report, cancelled) {
        Err(ImportError::Cancelled) => Ok(0),
        result => result,
    }
}

fn import_inner(
    source: &Path,
    destination: &Path,
    report: &mut dyn FnMut(Progress) -> bool,
    cancelled: Cancellation,
) -> Result<u32, ImportError> {
    let info = probe_cancellable(source, Arc::clone(&cancelled))?;
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

    let mut command = Command::new("ffmpeg");
    command
        .args(["-v", "error", "-i"])
        .arg(source)
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stdout(Stdio::piped())
        // Never leave an unread stderr pipe: corrupt sources can fill it and deadlock decoding.
        .stderr(Stdio::null());
    let work = ImportWork {
        ffmpeg: SupervisedChild::spawn(&mut command, cancelled)?,
        staging: staging_path(destination),
    };
    let mut decoded = work
        .ffmpeg
        .child
        .lock()
        .expect("the decoder process")
        .stdout
        .take()
        .expect("stdout was piped");

    // Written beside the destination and renamed at the end, so a reader never sees a partial
    // clip and an interrupted import leaves the library exactly as it was.
    let staging = &work.staging;
    let mut writer = ClipWriter::new(
        std::fs::File::create(staging)?,
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
        if !work.ffmpeg.read_frame(&mut decoded, &mut raw)? {
            break;
        }

        let payload = match crate::hap::encode(info.width, info.height, &raw) {
            Ok(payload) => payload,
            Err(error) => {
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

    if cancelled {
        drop(writer);
        return Ok(0);
    }
    let status = work.ffmpeg.wait()?;
    if !status.success() || frames == 0 {
        return Err(ImportError::Unreadable {
            path: source.to_path_buf(),
            detail: if status.success() {
                "no frames were produced".to_owned()
            } else {
                format!("ffmpeg exited with {status}")
            },
        });
    }

    let file = writer.finish()?;
    let bytes = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    drop(file);
    std::fs::rename(staging, destination)?;

    report(Progress::Finished { frames, bytes });
    Ok(frames)
}

fn staging_path(destination: &Path) -> PathBuf {
    let mut name = destination.file_name().unwrap_or_default().to_os_string();
    name.push(".importing");
    destination.with_file_name(name)
}

/// Every exit path reaps the decoder and removes unpublished output, including write failures.
struct ImportWork {
    ffmpeg: SupervisedChild,
    staging: PathBuf,
}

impl Drop for ImportWork {
    fn drop(&mut self) {
        self.ffmpeg.stop();
        let _ = std::fs::remove_file(&self.staging);
    }
}

/// Owns and reaps a decoder; the watcher never holds the child mutex across a blocking wait.
struct SupervisedChild {
    child: Arc<Mutex<std::process::Child>>,
    done: Arc<AtomicBool>,
    interrupted: Arc<AtomicBool>,
    stalled: Arc<AtomicBool>,
    waiting_since: Arc<Mutex<Option<Instant>>>,
    watcher: Option<std::thread::JoinHandle<()>>,
}

impl SupervisedChild {
    fn spawn(command: &mut Command, cancelled: Cancellation) -> Result<Self, ImportError> {
        let child = command.spawn().map_err(|_| ImportError::FfmpegMissing)?;
        let mut process = Self {
            child: Arc::new(Mutex::new(child)),
            done: Arc::new(AtomicBool::new(false)),
            interrupted: Arc::new(AtomicBool::new(false)),
            stalled: Arc::new(AtomicBool::new(false)),
            waiting_since: Arc::new(Mutex::new(Some(Instant::now()))),
            watcher: None,
        };
        let child = Arc::clone(&process.child);
        let done = Arc::clone(&process.done);
        let interrupted = Arc::clone(&process.interrupted);
        let stalled = Arc::clone(&process.stalled);
        let waiting_since = Arc::clone(&process.waiting_since);
        process.watcher = Some(
            std::thread::Builder::new()
                .name("media-decoder-watch".to_owned())
                .spawn(move || {
                    while !done.load(Ordering::SeqCst) {
                        let should_cancel = cancelled();
                        let timed_out = waiting_since
                            .lock()
                            .expect("decoder wait time")
                            .is_some_and(|start| start.elapsed() >= Duration::from_secs(30));
                        if should_cancel || timed_out {
                            interrupted.store(should_cancel, Ordering::SeqCst);
                            stalled.store(timed_out, Ordering::SeqCst);
                            let _ = child.lock().expect("the decoder process").kill();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                })?,
        );
        Ok(process)
    }

    fn begin_wait(&self) {
        *self.waiting_since.lock().expect("decoder wait time") = Some(Instant::now());
    }

    fn end_wait(&self) {
        *self.waiting_since.lock().expect("decoder wait time") = None;
    }

    fn read_frame(
        &self,
        source: &mut impl std::io::Read,
        frame: &mut [u8],
    ) -> Result<bool, ImportError> {
        let mut filled = 0;
        while filled < frame.len() {
            self.begin_wait();
            let read = source.read(&mut frame[filled..]);
            self.end_wait();
            self.check_interrupted()?;
            match read {
                Ok(0) if filled == 0 => return Ok(false),
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "the decoder stopped within a raw frame",
                    )
                    .into());
                }
                Ok(bytes) => filled += bytes,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(true)
    }

    fn check_interrupted(&self) -> Result<(), ImportError> {
        if self.interrupted.load(Ordering::SeqCst) {
            return Err(ImportError::Cancelled);
        }
        if self.stalled.load(Ordering::SeqCst) {
            return Err(ImportError::DecoderStalled);
        }
        Ok(())
    }

    fn wait(&self) -> Result<std::process::ExitStatus, ImportError> {
        self.begin_wait();
        loop {
            let status = self.child.lock().expect("the decoder process").try_wait()?;
            if let Some(status) = status {
                self.check_interrupted()?;
                self.end_wait();
                return Ok(status);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        {
            let mut child = self.child.lock().expect("the decoder process");
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}

impl Drop for SupervisedChild {
    fn drop(&mut self) {
        self.stop();
    }
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

    #[cfg(unix)]
    #[test]
    fn cancellation_interrupts_blocked_child_output_and_reaps_the_child() {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let mut command = Command::new("sh");
        command.args(["-c", "exec sleep 60"]).stdout(Stdio::piped());
        let mut process =
            SupervisedChild::spawn(&mut command, Arc::new(move || flag.load(Ordering::SeqCst)))
                .unwrap();
        let mut stdout = process.child.lock().unwrap().stdout.take().unwrap();
        cancel.store(true, Ordering::SeqCst);
        let started = Instant::now();
        assert_eq!(stdout.read(&mut [0u8; 1]).unwrap(), 0);
        assert!(matches!(process.wait(), Err(ImportError::Cancelled)));
        process.stop();
        assert!(process.child.lock().unwrap().try_wait().unwrap().is_some());
        assert!(process.watcher.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[test]
    fn a_stalled_decoder_is_killed_without_limiting_normal_clip_duration() {
        let mut command = Command::new("sh");
        command.args(["-c", "exec sleep 60"]).stdout(Stdio::piped());
        let process = SupervisedChild::spawn(&mut command, Arc::new(|| false)).unwrap();
        *process.waiting_since.lock().unwrap() = Some(Instant::now() - Duration::from_secs(31));
        let mut stdout = process.child.lock().unwrap().stdout.take().unwrap();
        assert_eq!(stdout.read(&mut [0u8; 1]).unwrap(), 0);
        assert!(matches!(process.wait(), Err(ImportError::DecoderStalled)));
    }

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
    fn cancelling_after_a_frame_returns_zero_and_preserves_the_previous_clip() {
        if !ffmpeg_available() {
            return;
        }
        let directory =
            std::env::temp_dir().join(format!("media-import-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("source.mp4");
        assert!(
            Command::new("ffmpeg")
                .args([
                    "-y",
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=64x64:rate=10:duration=1"
                ])
                .arg(&source)
                .status()
                .unwrap()
                .success()
        );
        let destination = directory.join("clip.toskclip");
        std::fs::write(&destination, b"previous playable clip").unwrap();
        let mut encoded = false;
        let frames = import(&source, &destination, &mut |progress| {
            if matches!(progress, Progress::Encoded { .. }) {
                encoded = true;
                return false;
            }
            assert!(!matches!(progress, Progress::Finished { .. }));
            true
        })
        .unwrap();
        assert!(encoded);
        assert_eq!(frames, 0, "cancelled work is never reported as published");
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"previous playable clip"
        );
        assert!(!staging_path(&destination).exists());
        std::fs::remove_dir_all(directory).unwrap();
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
