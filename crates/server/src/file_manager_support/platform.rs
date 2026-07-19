use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "macos")]
const NOTE_ATTRIBUTE: &str = "com.tosklight.note";
#[cfg(all(unix, not(target_os = "macos")))]
const NOTE_ATTRIBUTE: &str = "user.tosklight.note";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeCapabilities {
    pub created_time: bool,
    pub hidden_attributes: bool,
    pub native_notes: bool,
    pub trash: bool,
}

pub(crate) fn capabilities(path: &Path) -> NativeCapabilities {
    let metadata = fs::metadata(path);
    NativeCapabilities {
        created_time: metadata.as_ref().is_ok_and(|value| value.created().is_ok()),
        hidden_attributes: cfg!(target_os = "windows"),
        native_notes: native_notes_supported(path),
        trash: trash_supported(),
    }
}

pub(crate) fn is_hidden(name: &OsStr, metadata: &fs::Metadata) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = metadata;
        name.to_string_lossy().starts_with('.')
    }
}

pub(crate) fn native_notes_supported(path: &Path) -> bool {
    #[cfg(unix)]
    {
        xattr::list(path).is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        path.exists()
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = path;
        false
    }
}

pub(crate) fn read_native_note(path: &Path) -> io::Result<Option<String>> {
    #[cfg(unix)]
    {
        let bytes = xattr::get_deref(path, NOTE_ATTRIBUTE)?;
        bytes
            .map(String::from_utf8)
            .transpose()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "native note is not UTF-8"))
    }
    #[cfg(target_os = "windows")]
    {
        let stream = alternate_data_stream(path);
        match fs::read(stream) {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "native note is not UTF-8")
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native notes are unavailable",
        ))
    }
}

pub(crate) fn write_native_note(path: &Path, note: &str) -> io::Result<()> {
    #[cfg(unix)]
    {
        if note.is_empty() {
            match xattr::remove_deref(path, NOTE_ATTRIBUTE) {
                Ok(()) => Ok(()),
                Err(error) if error.raw_os_error().is_some_and(is_missing_xattr_error) => Ok(()),
                Err(error) => Err(error),
            }
        } else {
            xattr::set_deref(path, NOTE_ATTRIBUTE, note.as_bytes())
        }
    }
    #[cfg(target_os = "windows")]
    {
        let stream = alternate_data_stream(path);
        if note.is_empty() {
            match fs::remove_file(stream) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        } else {
            fs::write(stream, note)
        }
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = (path, note);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native notes are unavailable",
        ))
    }
}

#[cfg(unix)]
fn is_missing_xattr_error(code: i32) -> bool {
    #[cfg(target_os = "macos")]
    {
        code == 93
    }
    #[cfg(not(target_os = "macos"))]
    {
        code == 61
    }
}

#[cfg(target_os = "windows")]
fn alternate_data_stream(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}:ToskLight.Note", path.display()))
}

pub(crate) fn trash_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        Path::new("/usr/bin/trash").is_file()
    }
    #[cfg(target_os = "linux")]
    {
        executable_in_path("gio").is_some() || executable_in_path("trash-put").is_some()
    }
    #[cfg(target_os = "windows")]
    {
        executable_in_path("powershell.exe").is_some() || executable_in_path("pwsh.exe").is_some()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        false
    }
}

pub(crate) fn trash_path(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let status = Command::new("/usr/bin/trash").arg(path).status()?;
    #[cfg(target_os = "linux")]
    let status = if let Some(gio) = executable_in_path("gio") {
        Command::new(gio).arg("trash").arg(path).status()?
    } else if let Some(command) = executable_in_path("trash-put") {
        Command::new(command).arg(path).status()?
    } else {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "platform trash is unavailable",
        ));
    };
    #[cfg(target_os = "windows")]
    let status = {
        let shell = executable_in_path("powershell.exe")
            .or_else(|| executable_in_path("pwsh.exe"))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "platform trash is unavailable")
            })?;
        let escaped = path.to_string_lossy().replace(char::from(39), "''");
        let script = format!(
            "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile('{escaped}','OnlyErrorDialogs','SendToRecycleBin')"
        );
        Command::new(shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()?
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "platform trash is unavailable",
    ));

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "platform trash failed with {status}"
        )))
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn executable_in_path(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 && candidate.is_file() {
        return Some(candidate.to_owned());
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
}

pub(crate) fn discover_removable_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        discover_directories_under(Path::new("/Volumes"), false)
            .into_iter()
            .filter(|path| fs::canonicalize(path).ok().as_deref() != Some(Path::new("/")))
            .collect()
    }
    #[cfg(target_os = "linux")]
    {
        let mut paths = linux_removable_mounts();
        if paths.is_empty() {
            paths = discover_directories_under(Path::new("/media"), true);
            paths.extend(discover_directories_under(Path::new("/run/media"), true));
        }
        paths.sort();
        paths.dedup();
        return paths;
    }
    #[cfg(target_os = "windows")]
    {
        let Some(shell) =
            executable_in_path("powershell.exe").or_else(|| executable_in_path("pwsh.exe"))
        else {
            return Vec::new();
        };
        let Ok(output) = Command::new(shell).args(["-NoProfile", "-NonInteractive", "-Command", "Get-CimInstance Win32_LogicalDisk | Where-Object DriveType -eq 2 | ForEach-Object DeviceID"]).output() else { return Vec::new(); };
        if !output.status.success() {
            return Vec::new();
        }
        return windows_removable_drive_paths(&output.stdout)
            .into_iter()
            .filter(|path| path.is_dir())
            .collect();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    Vec::new()
}

#[cfg(target_os = "linux")]
fn linux_removable_mounts() -> Vec<PathBuf> {
    let Ok(mounts) = fs::read_to_string("/proc/self/mountinfo") else {
        return Vec::new();
    };
    linux_removable_mount_paths(&mounts)
        .into_iter()
        .filter(|path| path.is_dir())
        .collect()
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(super) fn linux_removable_mount_paths(mounts: &str) -> Vec<PathBuf> {
    mounts
        .lines()
        .filter_map(|line| {
            let mount = line
                .split_whitespace()
                .nth(4)?
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\134", "\\");
            let path = PathBuf::from(mount);
            (path.starts_with("/media") || path.starts_with("/run/media")).then_some(path)
        })
        .collect()
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(super) fn windows_removable_drive_paths(output: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(output)
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.len() == 2 && line.ends_with(':') && line.as_bytes()[0].is_ascii_alphabetic()
        })
        .map(|drive| PathBuf::from(format!("{drive}\\")))
        .collect()
}

pub(crate) fn discover_directories_under(
    parent: &Path,
    include_second_level: bool,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let Ok(entries) = fs::read_dir(parent) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if include_second_level {
            let nested = fs::read_dir(&path)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            if nested.is_empty() {
                result.push(path);
            } else {
                result.extend(nested);
            }
        } else {
            result.push(path);
        }
    }
    result
}
