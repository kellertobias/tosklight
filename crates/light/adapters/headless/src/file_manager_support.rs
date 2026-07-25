use image::{DynamicImage, ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[path = "file_manager_support/platform.rs"]
mod platform;

pub(crate) use platform::{
    capabilities, discover_removable_paths, is_hidden, native_notes_supported, read_native_note,
    trash_path, trash_supported, write_native_note,
};
#[cfg(test)]
use platform::{
    discover_directories_under, linux_removable_mount_paths, windows_removable_drive_paths,
};

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
#[path = "file_manager_support/tests.rs"]
mod tests;
