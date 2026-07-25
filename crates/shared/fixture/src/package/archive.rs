use super::invalid;
use super::manifest::FixturePackageError;
use std::io::Cursor;
use std::path::Component;
use zip::CompressionMethod;

pub(super) fn validate_zip_entry(
    entry: &zip::read::ZipFile<'_, Cursor<&[u8]>>,
) -> Result<(), FixturePackageError> {
    let name = entry.name();
    if name.is_empty()
        || name.contains('\0')
        || name.contains('\\')
        || name.contains("//")
        || name.starts_with('/')
    {
        return Err(invalid(format!("archive contains unsafe path {name}")));
    }
    let Some(enclosed) = entry.enclosed_name() else {
        return Err(invalid(format!("archive contains unsafe path {name}")));
    };
    if enclosed
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid(format!("archive contains unsafe path {name}")));
    }
    if let Some(mode) = entry.unix_mode()
        && mode & 0o170000 == 0o120000
    {
        return Err(invalid(format!("archive entry {name} is a symbolic link")));
    }
    if entry.encrypted() {
        return Err(invalid(format!("archive entry {name} is encrypted")));
    }
    if !matches!(
        entry.compression(),
        CompressionMethod::Stored | CompressionMethod::Deflated
    ) {
        return Err(invalid(format!(
            "archive entry {name} uses unsupported compression"
        )));
    }
    Ok(())
}
