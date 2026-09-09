//! Native desktop entry point for converting a local group of media files.
//!
//! The picker accepts files and directories in one pass. Directory contents are traversed in a
//! stable order without following directory symlinks, and only formats the Pixel clip importer
//! can currently represent are queued. The existing importer owns conversion, retry, progress,
//! destination reservations, and publication; this module owns only native operator interaction.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use media_domain::MediaAddress;

#[derive(Clone)]
pub struct BulkImport {
    importer: media_library::Importer,
    administration_endpoint: String,
    available: bool,
}

impl BulkImport {
    pub fn new(importer: media_library::Importer, administration_endpoint: String) -> Self {
        Self {
            importer,
            administration_endpoint,
            available: media_codec::import::ffmpeg_available(),
        }
    }

    /// Starts the native workflow away from the event callback so output presentation remains
    /// responsive while the selected directory tree is scanned and queued. The pickers themselves
    /// stay on the calling UI thread so macOS and Windows keep them owned by Pixel while open.
    pub fn prompt(&self) {
        let selected = pick_sources();
        if selected.is_empty() {
            return;
        }
        let workflow = self.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("media-bulk-import-dialog".to_owned())
            .spawn(move || workflow.run(selected))
        {
            tracing::error!(%error, "the multi-file conversion dialog could not start");
        }
    }

    fn run(self, selected: Vec<PathBuf>) {
        if !self.available {
            alert(
                "This machine cannot convert media because FFmpeg is not installed or not on PATH.",
            );
            return;
        }
        let sources = collect_sources(&selected);
        if sources.is_empty() {
            alert("The selection contains no supported video or image files.");
            return;
        }
        let Some(folder) = prompt_number("Start folder (1–199)", 1, 199) else {
            return;
        };
        let Some(file) = prompt_number("Start file (1–254)", 1, 254) else {
            return;
        };
        let available = remaining_addresses(folder, file);
        if sources.len() > available {
            alert(&format!(
                "The selection contains {} supported files, but only {available} addresses remain from {folder:03}/{file:03}.",
                sources.len()
            ));
            return;
        }

        let batch = media_library::BatchId::new();
        for (index, source) in sources.into_iter().enumerate() {
            let Some(destination) = address_at(folder, file, index) else {
                break;
            };
            let name = source
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("Media")
                .to_owned();
            self.importer
                .submit_in_batch(source, destination, &name, Some(batch));
        }
        open_library(&self.administration_endpoint);
    }
}

#[cfg(target_os = "macos")]
fn pick_sources() -> Vec<PathBuf> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::NSString;

    let Some(main_thread) = MainThreadMarker::new() else {
        tracing::error!("the macOS multi-file picker was requested away from the main thread");
        return Vec::new();
    };
    let panel = NSOpenPanel::openPanel(main_thread);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(true);
    panel.setAllowsMultipleSelection(true);
    panel.setResolvesAliases(true);
    panel.setTitle(Some(&NSString::from_str(
        "Select videos, images, and folders",
    )));
    if panel.runModal() != NSModalResponseOK {
        return Vec::new();
    }
    let urls = panel.URLs();
    (0..urls.count())
        .filter_map(|index| urls.objectAtIndex(index).to_file_path())
        .collect()
}

#[cfg(target_os = "windows")]
fn pick_sources() -> Vec<PathBuf> {
    let mut selected = rfd::FileDialog::new()
        .set_title("Select videos and images (optional)")
        .add_filter(
            "Videos and images",
            &[
                "mp4", "mov", "m4v", "mkv", "avi", "webm", "mpg", "mpeg", "mxf", "wmv", "png",
                "jpg", "jpeg", "tif", "tiff", "bmp", "webp", "gif", "heic", "heif",
            ],
        )
        .pick_files()
        .unwrap_or_default();
    selected.extend(
        rfd::FileDialog::new()
            .set_title("Select folders to include (optional)")
            .pick_folders()
            .unwrap_or_default(),
    );
    selected
}

#[cfg(target_os = "macos")]
const MACOS_NUMBER_SCRIPT: &str = r#"
on run argv
  set promptText to item 1 of argv
  set defaultValue to item 2 of argv
  set answer to display dialog promptText default answer defaultValue buttons {"Cancel", "Continue"} default button "Continue" with title "Convert multiple files"
  return text returned of answer
end run
"#;

#[cfg(target_os = "windows")]
const WINDOWS_NUMBER_SCRIPT: &str = r#"
Add-Type -AssemblyName Microsoft.VisualBasic
$value = [Microsoft.VisualBasic.Interaction]::InputBox(
  $env:TOSKLIGHT_BULK_PROMPT,
  'Convert multiple files',
  $env:TOSKLIGHT_BULK_DEFAULT
)
[Console]::Out.Write($value)
"#;

fn prompt_number(label: &str, minimum: u8, maximum: u8) -> Option<u8> {
    loop {
        let value = platform_prompt_number(label, minimum)?;
        let value = value.trim().parse().ok();
        if value.is_some_and(|value| (minimum..=maximum).contains(&value)) {
            return value;
        }
        alert(&format!("Enter a number from {minimum} through {maximum}."));
    }
}

#[cfg(target_os = "macos")]
fn platform_prompt_number(label: &str, default_value: u8) -> Option<String> {
    let output = Command::new("/usr/bin/osascript")
        .args([
            "-e",
            MACOS_NUMBER_SCRIPT,
            "--",
            label,
            &default_value.to_string(),
        ])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "windows")]
fn platform_prompt_number(label: &str, default_value: u8) -> Option<String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-Command", WINDOWS_NUMBER_SCRIPT])
        .env("TOSKLIGHT_BULK_PROMPT", label)
        .env("TOSKLIGHT_BULK_DEFAULT", default_value.to_string())
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn alert(message: &str) {
    let _ = rfd::MessageDialog::new()
        .set_title("Convert multiple files")
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(target_os = "macos")]
fn open_library(endpoint: &str) {
    let _ = Command::new("/usr/bin/open")
        .arg(format!("http://{endpoint}/library"))
        .status();
}

#[cfg(target_os = "windows")]
fn open_library(endpoint: &str) {
    let _ = Command::new("rundll32.exe")
        .args([
            "url.dll,FileProtocolHandler",
            &format!("http://{endpoint}/library"),
        ])
        .status();
}

fn collect_sources(selected: &[PathBuf]) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    for path in selected {
        collect(path, &mut seen, &mut sources);
    }
    sources
}

fn collect(path: &Path, seen: &mut HashSet<PathBuf>, sources: &mut Vec<PathBuf>) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    if metadata.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        children.sort();
        for child in children {
            collect(&child, seen, sources);
        }
        return;
    }
    let Some(filename) = path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    if !metadata.is_file() || media_library::naming::importable_extension(filename).is_none() {
        return;
    }
    let identity = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if seen.insert(identity) {
        sources.push(path.to_path_buf());
    }
}

fn remaining_addresses(folder: u8, file: u8) -> usize {
    (199usize - usize::from(folder)) * 254 + (255usize - usize::from(file))
}

fn address_at(folder: u8, file: u8, offset: usize) -> Option<MediaAddress> {
    let linear = (usize::from(folder) - 1) * 254 + (usize::from(file) - 1) + offset;
    let destination_folder = linear / 254 + 1;
    (destination_folder <= 199)
        .then(|| MediaAddress::new(destination_folder as u8, (linear % 254 + 1) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_begin_exactly_where_the_operator_asked_and_wrap() {
        assert_eq!(address_at(7, 253, 0), Some(MediaAddress::new(7, 253)));
        assert_eq!(address_at(7, 253, 1), Some(MediaAddress::new(7, 254)));
        assert_eq!(address_at(7, 253, 2), Some(MediaAddress::new(8, 1)));
        assert_eq!(address_at(199, 254, 1), None);
        assert_eq!(remaining_addresses(199, 254), 1);
    }

    #[test]
    fn nested_sources_are_stable_deduplicated_and_ignore_audio() {
        let root = std::env::temp_dir().join("pixel-bulk-source-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("b.mov"), b"video").unwrap();
        std::fs::write(root.join("nested/a.png"), b"image").unwrap();
        std::fs::write(root.join("nested/music.wav"), b"audio").unwrap();

        let sources = collect_sources(&[root.clone(), root.join("b.mov")]);

        assert_eq!(sources, vec![root.join("b.mov"), root.join("nested/a.png")]);
        let _ = std::fs::remove_dir_all(root);
    }
}
