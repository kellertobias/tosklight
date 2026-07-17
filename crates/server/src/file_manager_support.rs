use image::{DynamicImage, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

#[cfg(target_os = "macos")]
const NOTE_ATTRIBUTE: &str = "com.tosklight.note";
#[cfg(all(unix, not(target_os = "macos")))]
const NOTE_ATTRIBUTE: &str = "user.tosklight.note";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConflictChoice {
    Error,
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TransferOutcome {
    Completed(PathBuf),
    Skipped(PathBuf),
}

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
        // Alternate data streams are the native metadata mechanism on NTFS.
        // Actual reads and writes still report a clear error on FAT/exFAT.
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
    // ENOATTR on macOS and ENODATA on Linux. Their numeric values are stable
    // platform ABI values, and avoiding libc keeps this crate unsafe-free.
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
fn linux_removable_mount_paths(mounts: &str) -> Vec<PathBuf> {
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
fn windows_removable_drive_paths(output: &[u8]) -> Vec<PathBuf> {
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

pub(crate) fn keep_both_path(target: &Path) -> io::Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let name = target
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target name is invalid"))?;
    let (stem, suffix) = match name.rsplit_once('.') {
        Some((stem, suffix)) if !stem.is_empty() => (stem, format!(".{suffix}")),
        _ => (name, String::new()),
    };
    for sequence in 1..=10_000 {
        let marker = if sequence == 1 {
            " copy".to_owned()
        } else {
            format!(" copy {sequence}")
        };
        let candidate = parent.join(format!("{stem}{marker}{suffix}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not find a Keep Both name",
    ))
}

pub(crate) fn copy_or_move(
    source: &Path,
    requested_target: &Path,
    move_source: bool,
    cross_root: bool,
    conflict: ConflictChoice,
) -> io::Result<TransferOutcome> {
    if source == requested_target {
        return match conflict {
            ConflictChoice::KeepBoth if !move_source => copy_or_move(
                source,
                &keep_both_path(requested_target)?,
                false,
                cross_root,
                ConflictChoice::Error,
            ),
            ConflictChoice::Skip => Ok(TransferOutcome::Skipped(requested_target.to_owned())),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source and destination are the same item",
            )),
        };
    }
    if source.is_dir() && requested_target.starts_with(source) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a folder cannot be copied or moved into itself",
        ));
    }
    let target = if requested_target.exists() {
        match conflict {
            ConflictChoice::Error => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "an item with that name already exists",
                ));
            }
            ConflictChoice::Skip => {
                return Ok(TransferOutcome::Skipped(requested_target.to_owned()));
            }
            ConflictChoice::KeepBoth => keep_both_path(requested_target)?,
            ConflictChoice::Replace => requested_target.to_owned(),
        }
    } else {
        requested_target.to_owned()
    };

    if move_source && !cross_root {
        commit_rename(source, &target)?;
        return Ok(TransferOutcome::Completed(target));
    }

    let staged = staging_path(&target, "copy");
    let result = (|| {
        copy_recursive(source, &staged)?;
        verify_tree(source, &staged)?;
        commit_staged(&staged, &target)?;
        verify_tree(source, &target)?;
        if move_source {
            remove_permanent(source)?;
        }
        Ok(TransferOutcome::Completed(target.clone()))
    })();
    if staged.exists() {
        let _ = remove_permanent(&staged);
    }
    result
}

fn staging_path(target: &Path, marker: &str) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target.file_name().and_then(OsStr::to_str).unwrap_or("item");
    parent.join(format!(".{name}.light-{marker}-{}", Uuid::new_v4()))
}

fn commit_rename(source: &Path, target: &Path) -> io::Result<()> {
    if !target.exists() {
        return fs::rename(source, target);
    }
    let backup = staging_path(target, "replaced");
    fs::rename(target, &backup)?;
    match fs::rename(source, target) {
        Ok(()) => remove_permanent(&backup),
        Err(error) => {
            let _ = fs::rename(&backup, target);
            Err(error)
        }
    }
}

fn commit_staged(staged: &Path, target: &Path) -> io::Result<()> {
    if !target.exists() {
        return fs::rename(staged, target);
    }
    let backup = staging_path(target, "replaced");
    fs::rename(target, &backup)?;
    match fs::rename(staged, target) {
        Ok(()) => remove_permanent(&backup),
        Err(error) => {
            let _ = fs::rename(&backup, target);
            Err(error)
        }
    }
}

pub(crate) fn copy_recursive(source: &Path, target: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "symbolic links are not copied by File Manager",
        ));
    }
    if metadata.is_dir() {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &target.join(entry.file_name()))?;
        }
        fs::set_permissions(target, metadata.permissions())?;
    } else if metadata.is_file() {
        fs::copy(source, target)?;
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported filesystem item",
        ));
    }
    copy_native_note(source, target)?;
    Ok(())
}

fn copy_native_note(source: &Path, target: &Path) -> io::Result<()> {
    if !native_notes_supported(source) {
        return Ok(());
    }
    if let Some(note) = read_native_note(source)? {
        if !native_notes_supported(target) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "destination filesystem cannot preserve the source native note",
            ));
        }
        write_native_note(target, &note)?;
    }
    Ok(())
}

pub(crate) fn verify_tree(source: &Path, target: &Path) -> io::Result<()> {
    let source_metadata = fs::metadata(source)?;
    let target_metadata = fs::metadata(target)?;
    if source_metadata.is_dir() != target_metadata.is_dir()
        || source_metadata.is_file() != target_metadata.is_file()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "copied item type does not match source",
        ));
    }
    if source_metadata.is_file() {
        if source_metadata.len() != target_metadata.len()
            || hash_file(source)? != hash_file(target)?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "copied file verification failed",
            ));
        }
    } else {
        let mut source_names = fs::read_dir(source)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        let mut target_names = fs::read_dir(target)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        source_names.sort();
        target_names.sort();
        if source_names != target_names {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "copied folder contents do not match source",
            ));
        }
        for name in source_names {
            verify_tree(&source.join(&name), &target.join(name))?;
        }
    }
    if native_notes_supported(source) {
        let source_note = read_native_note(source)?;
        if source_note.is_some() && !native_notes_supported(target) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "destination filesystem cannot preserve the source native note",
            ));
        }
        if native_notes_supported(target) && source_note != read_native_note(target)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "copied native note verification failed",
            ));
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().into())
}

pub(crate) fn remove_permanent(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn thumbnail_png(source: &Path, max_size: u32) -> io::Result<Vec<u8>> {
    let max_size = max_size.clamp(32, 1_024);
    let extension = source
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "this file type does not support raster thumbnails",
        ));
    }
    let metadata = fs::metadata(source)?;
    if !metadata.is_file() || metadata.len() > 256 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image is unavailable or exceeds the 256 MiB thumbnail limit",
        ));
    }
    let dimensions = ImageReader::open(source)
        .and_then(|reader| reader.with_guessed_format())?
        .into_dimensions()
        .map_err(io::Error::other)?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > 100_000_000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "image exceeds the 100 megapixel thumbnail limit",
        ));
    }
    let image = ImageReader::open(source)
        .and_then(|reader| reader.with_guessed_format())?
        .decode()
        .map_err(io::Error::other)?;
    encode_thumbnail(image, max_size)
}

fn encode_thumbnail(image: DynamicImage, max_size: u32) -> io::Result<Vec<u8>> {
    let thumbnail = image.thumbnail(max_size, max_size);
    let mut bytes = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(io::Error::other)?;
    Ok(bytes.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("light-file-support-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn discovery_adapters_cover_macos_linux_and_windows_mount_layouts() {
        let root = temporary_root("mounts");
        fs::create_dir_all(root.join("direct")).unwrap();
        assert_eq!(
            discover_directories_under(&root, false),
            vec![root.join("direct")]
        );
        fs::create_dir_all(root.join("operator/usb")).unwrap();
        let nested = discover_directories_under(&root, true);
        assert!(nested.contains(&root.join("operator/usb")));

        let linux = linux_removable_mount_paths(
            "24 1 8:1 / /media/operator/TOUR\\040USB rw - vfat /dev/sdb1 rw\n25 1 8:2 / /run/media/operator/SECOND rw - exfat /dev/sdc1 rw\n26 1 8:3 / /mnt/internal rw - ext4 /dev/sda1 rw\n",
        );
        assert_eq!(
            linux,
            vec![
                PathBuf::from("/media/operator/TOUR USB"),
                PathBuf::from("/run/media/operator/SECOND"),
            ]
        );

        assert_eq!(
            windows_removable_drive_paths(b"E:\r\nnot-a-drive\r\nF:\r\n"),
            vec![PathBuf::from("E:\\"), PathBuf::from("F:\\")],
        );
        remove_permanent(&root).unwrap();
    }

    #[test]
    fn keep_both_copy_and_replace_are_safe() {
        let root = temporary_root("conflicts");
        let source = root.join("source.txt");
        let target = root.join("target.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&target, b"old").unwrap();
        let copied =
            copy_or_move(&source, &target, false, false, ConflictChoice::KeepBoth).unwrap();
        let TransferOutcome::Completed(copied) = copied else {
            panic!("copy unexpectedly skipped")
        };
        assert_eq!(copied.file_name().unwrap(), "target copy.txt");
        assert_eq!(fs::read(&target).unwrap(), b"old");
        copy_or_move(&source, &target, false, false, ConflictChoice::Replace).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        remove_permanent(&root).unwrap();
    }

    #[test]
    fn cross_root_move_verifies_nested_content_before_deleting_source() {
        let source_root = temporary_root("source");
        let target_root = temporary_root("target");
        let source = source_root.join("folder");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("show.txt"), b"verified payload").unwrap();
        let target = target_root.join("folder");
        copy_or_move(&source, &target, true, true, ConflictChoice::Error).unwrap();
        assert!(!source.exists());
        assert_eq!(
            fs::read(target.join("show.txt")).unwrap(),
            b"verified payload"
        );
        remove_permanent(&source_root).unwrap();
        remove_permanent(&target_root).unwrap();
    }

    #[test]
    fn failed_cross_root_copy_never_deletes_the_source() {
        let source_root = temporary_root("failure-source");
        let target_root = temporary_root("failure-target");
        let source = source_root.join("show.txt");
        fs::write(&source, b"must survive").unwrap();
        let impossible = target_root.join("missing-parent/show.txt");
        assert!(copy_or_move(&source, &impossible, true, true, ConflictChoice::Error).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"must survive");
        remove_permanent(&source_root).unwrap();
        remove_permanent(&target_root).unwrap();
    }

    #[test]
    fn skip_conflicts_leave_both_items_unchanged() {
        let root = temporary_root("skip");
        let source = root.join("source.txt");
        let target = root.join("target.txt");
        fs::write(&source, b"source").unwrap();
        fs::write(&target, b"target").unwrap();
        assert_eq!(
            copy_or_move(&source, &target, true, false, ConflictChoice::Skip).unwrap(),
            TransferOutcome::Skipped(target.clone())
        );
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&target).unwrap(), b"target");
        remove_permanent(&root).unwrap();
    }

    #[test]
    fn raster_thumbnail_is_bounded_png() {
        let root = temporary_root("thumbnail");
        let source = root.join("image.png");
        DynamicImage::new_rgb8(640, 320).save(&source).unwrap();
        let bytes = thumbnail_png(&source, 64).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert!(decoded.width() <= 64 && decoded.height() <= 64);
        remove_permanent(&root).unwrap();
    }

    #[test]
    fn unix_hidden_adapter_uses_dotfile_convention() {
        let root = temporary_root("hidden");
        let visible = root.join("visible");
        let hidden = root.join(".hidden");
        fs::write(&visible, []).unwrap();
        fs::write(&hidden, []).unwrap();
        #[cfg(not(target_os = "windows"))]
        {
            assert!(!is_hidden(
                visible.file_name().unwrap(),
                &fs::metadata(&visible).unwrap()
            ));
            assert!(is_hidden(
                hidden.file_name().unwrap(),
                &fs::metadata(&hidden).unwrap()
            ));
        }
        remove_permanent(&root).unwrap();
    }

    #[test]
    fn native_notes_never_create_sidecar_files() {
        let root = temporary_root("notes");
        let file = root.join("item.txt");
        fs::write(&file, b"item").unwrap();
        if native_notes_supported(&file) {
            write_native_note(&file, "operator note").unwrap();
            assert_eq!(
                read_native_note(&file).unwrap().as_deref(),
                Some("operator note")
            );
            assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
            write_native_note(&file, "").unwrap();
            assert_eq!(read_native_note(&file).unwrap(), None);
        }
        remove_permanent(&root).unwrap();
    }
}
