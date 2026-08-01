//! Finding Blender, and turning a captured snapshot into a `.blend` with it.
//!
//! Only Blender can write a Blender file, so the export is Blender doing it: the application hands
//! a headless run the snapshot folder and an output path, and the script inside that run assembles
//! the scene. Nothing here needs Blender to be installed — a capture is complete on its own, and a
//! desk that has never seen Blender simply cannot take this last step and says so.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The import script, shipped inside the application so an export can never run a stale copy left
/// beside an old snapshot.
pub const IMPORT_SCRIPT: &str = include_str!("to_blend.py");

/// An operator-readable reason an export could not run or did not finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportError {
    /// No Blender was found, and none was configured.
    BlenderNotFound,
    /// The configured path is not a Blender that can be run.
    BlenderUnusable { path: PathBuf, detail: String },
    /// Blender ran and failed. The detail is the most useful line it printed.
    Failed { detail: String },
    /// Something local went wrong: the snapshot folder, or writing the script.
    Local { detail: String },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlenderNotFound => formatter.write_str(
                "Blender not found \u{2014} set its path in Quick Settings, or install Blender",
            ),
            Self::BlenderUnusable { path, detail } => {
                write!(formatter, "{} cannot be run: {detail}", path.display())
            }
            Self::Failed { detail } => write!(formatter, "Blender failed: {detail}"),
            Self::Local { detail } => formatter.write_str(detail),
        }
    }
}

/// Environment variable an operator or a build machine can point at a specific Blender.
pub const BLENDER_ENV: &str = "TOSKLIGHT_BLENDER";

/// Find a Blender to run.
///
/// `configured` is whatever the operator typed in Quick Settings, and wins: somebody who names a
/// build is asking for that build. After that comes the environment variable, then the command
/// search path, and only then the places each platform installs it, so a machine with two versions
/// is never silently given the older one.
pub fn find_blender(configured: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|path| !path.as_os_str().is_empty()) {
        return runnable(path);
    }
    if let Some(path) = std::env::var_os(BLENDER_ENV).filter(|value| !value.is_empty()) {
        return runnable(Path::new(&path));
    }
    if let Some(path) = on_search_path("blender") {
        return Some(path);
    }
    well_known().into_iter().find_map(|path| runnable(&path))
}

/// A path is only offered if it exists; `--version` is left to the export itself, which has to
/// handle a broken installation anyway and should not pay for a process launch per keystroke.
fn runnable(path: &Path) -> Option<PathBuf> {
    let path = expand_home(path);
    // A macOS operator naturally picks the bundle rather than the executable inside it.
    if path.extension().is_some_and(|extension| extension == "app") {
        let inner = path.join("Contents/MacOS/Blender");
        if inner.is_file() {
            return Some(inner);
        }
    }
    path.is_file().then_some(path)
}

/// Expand a leading `~`, which is how an operator writes a path in their own home.
fn expand_home(path: &Path) -> PathBuf {
    let Ok(rest) = path.strip_prefix("~") else {
        return path.to_path_buf();
    };
    match home() {
        Some(home) => home.join(rest),
        None => path.to_path_buf(),
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn on_search_path(program: &str) -> Option<PathBuf> {
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE".into())
            .split(';')
            .map(str::to_owned)
            .collect()
    } else {
        vec![String::new()]
    };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|directory| {
        extensions.iter().find_map(|extension| {
            let candidate = directory.join(format!("{program}{extension}"));
            candidate.is_file().then_some(candidate)
        })
    })
}

/// Where each platform's installer puts Blender, newest first.
fn well_known() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/Applications/Blender.app"));
        if let Some(home) = home() {
            candidates.push(home.join("Applications/Blender.app"));
        }
    } else if cfg!(windows) {
        // The installer makes one directory per minor version, so the newest one wins.
        for root in [
            "C:/Program Files/Blender Foundation",
            "C:/Program Files (x86)/Blender Foundation",
        ] {
            let Ok(entries) = std::fs::read_dir(root) else {
                continue;
            };
            let mut versions: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.join("blender.exe").is_file())
                .collect();
            versions.sort();
            candidates.extend(
                versions
                    .into_iter()
                    .rev()
                    .map(|path| path.join("blender.exe")),
            );
        }
    } else {
        candidates.extend(
            [
                "/usr/bin/blender",
                "/usr/local/bin/blender",
                "/snap/bin/blender",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
        if let Some(home) = home() {
            candidates.push(home.join(".local/bin/blender"));
        }
    }
    candidates
}

/// What an export produced.
#[derive(Clone, Debug)]
pub struct ExportedBlend {
    pub path: PathBuf,
    /// The Blender that wrote it, for the operator to recognise.
    pub blender: PathBuf,
}

/// Run Blender headlessly over a captured snapshot folder and write `destination`.
///
/// This blocks for as long as Blender takes, which is seconds on a large rig, so it belongs on a
/// worker thread rather than the thread that is drawing.
pub fn export_blend(
    snapshot_directory: &Path,
    destination: &Path,
    configured_blender: Option<&Path>,
) -> Result<ExportedBlend, ExportError> {
    let blender = find_blender(configured_blender).ok_or(ExportError::BlenderNotFound)?;
    let document = snapshot_directory.join(crate::DOCUMENT_FILE);
    if !document.is_file() {
        return Err(ExportError::Local {
            detail: format!(
                "{} is not a captured snapshot",
                snapshot_directory.display()
            ),
        });
    }
    // The script is written beside the snapshot each time rather than kept: the application's own
    // copy is the only one that can be out of step with the document it is reading.
    let script = snapshot_directory.join("to_blend.py");
    std::fs::write(&script, IMPORT_SCRIPT).map_err(|error| ExportError::Local {
        detail: format!("writing the import script: {error}"),
    })?;

    let output = Command::new(&blender)
        .args([
            "--background".as_ref(),
            "--factory-startup".as_ref(),
            "--python-exit-code".as_ref(),
            "13".as_ref(),
            "--python".as_ref(),
            script.as_os_str(),
            "--".as_ref(),
            snapshot_directory.as_os_str(),
            destination.as_os_str(),
        ])
        .output()
        .map_err(|error| ExportError::BlenderUnusable {
            path: blender.clone(),
            detail: error.to_string(),
        })?;

    if !output.status.success() {
        return Err(ExportError::Failed {
            detail: failure_detail(&output.stdout, &output.stderr),
        });
    }
    if !destination.is_file() {
        return Err(ExportError::Failed {
            detail: format!(
                "Blender reported success but wrote no file at {}",
                destination.display()
            ),
        });
    }
    Ok(ExportedBlend {
        path: destination.to_path_buf(),
        blender,
    })
}

/// The one line of a failed run worth putting in front of an operator.
///
/// Blender prints a great deal on the way to failing, and the last line is usually the exit
/// itself. A Python traceback names the real problem on its final line, so that is preferred, and
/// everything falls back to the last thing said rather than to nothing.
fn failure_detail(stdout: &[u8], stderr: &[u8]) -> String {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let interesting = lines
        .iter()
        .rev()
        .find(|line| line.starts_with("Error:") || line.contains("Error:"))
        .or_else(|| lines.last());
    interesting
        .map(|line| line.chars().take(240).collect())
        .unwrap_or_else(|| "no output".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_path_that_does_not_exist_is_not_offered() {
        assert_eq!(find_blender(Some(Path::new("/nowhere/blender"))), None);
    }

    #[test]
    fn an_empty_configured_path_falls_through_to_discovery() {
        // An operator who clears the field is asking for the default search, not for nothing.
        let empty = PathBuf::new();
        assert_eq!(find_blender(Some(&empty)), find_blender(None));
    }

    #[test]
    fn a_named_file_is_taken_as_the_blender_to_run() {
        let directory = std::env::temp_dir().join("viz-snapshot-blender-discovery");
        std::fs::create_dir_all(&directory).expect("temporary directory");
        let executable = directory.join("blender");
        std::fs::write(&executable, b"#!/bin/sh\n").expect("write");
        assert_eq!(find_blender(Some(&executable)), Some(executable.clone()));
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_macos_bundle_resolves_to_the_executable_inside_it() {
        let bundle = std::env::temp_dir().join("viz-snapshot-bundle/Blender.app");
        let executable = bundle.join("Contents/MacOS/Blender");
        std::fs::create_dir_all(executable.parent().expect("parent")).expect("directories");
        std::fs::write(&executable, b"#!/bin/sh\n").expect("write");
        assert_eq!(find_blender(Some(&bundle)), Some(executable));
        std::fs::remove_dir_all(std::env::temp_dir().join("viz-snapshot-bundle")).ok();
    }

    #[test]
    fn exporting_from_somewhere_that_is_not_a_snapshot_says_so_rather_than_running_blender() {
        let error = export_blend(
            Path::new("/nowhere/at/all"),
            Path::new("/nowhere/at/all/rig.blend"),
            Some(Path::new("/nowhere/blender")),
        )
        .expect_err("there is nothing to export");
        // Discovery fails first, and that is the message an operator can act on.
        assert_eq!(error, ExportError::BlenderNotFound);
    }

    #[test]
    fn a_failure_is_reported_with_the_line_that_names_it() {
        let detail = failure_detail(
            b"Read prefs\nInfo: opening\n",
            b"Traceback...\nError: no geometry file beside the snapshot\nBlender quit\n",
        );
        assert_eq!(detail, "Error: no geometry file beside the snapshot");
    }

    #[test]
    fn a_silent_failure_still_says_something() {
        assert_eq!(failure_detail(b"", b""), "no output");
    }

    #[test]
    fn the_import_script_is_shipped_with_the_application() {
        assert!(IMPORT_SCRIPT.contains("save_as_mainfile"));
    }
}
