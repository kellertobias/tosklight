use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::configuration::ExtensionsConfiguration;
use crate::discovery::{DiscoveryOptions, PackageReadiness, discover_packages};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledPackage {
    pub extension_id: String,
    pub package_digest: String,
    pub directory: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageInstallError {
    StagingOutsideExtensionsFolder,
    StagingNameRequired,
    InvalidPackage(Vec<String>),
    UnexpectedIdentity { expected: String, actual: String },
    Io(String),
}

impl std::fmt::Display for PackageInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PackageInstallError {}

/// Validates a fully copied hidden staging directory, then swaps it into the discovered namespace.
/// Callers serialize install/update with catalog rescans; hidden staging and backup folders are
/// deliberately ignored by discovery, so a partial copy can never become executable.
pub fn install_staged_package(
    extensions_directory: &Path,
    staged_directory: &Path,
    expected_extension_id: &str,
    options: &DiscoveryOptions,
) -> Result<InstalledPackage, PackageInstallError> {
    if staged_directory.parent() != Some(extensions_directory) {
        return Err(PackageInstallError::StagingOutsideExtensionsFolder);
    }
    if !staged_directory
        .file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with(".staging-"))
    {
        return Err(PackageInstallError::StagingNameRequired);
    }

    let validation_root = extensions_directory.join(format!(".validation-{}", Uuid::new_v4()));
    fs::create_dir(&validation_root).map_err(io_error)?;
    let validation_package = validation_root.join("package");
    fs::rename(staged_directory, &validation_package).map_err(io_error)?;
    let catalog = discover_packages(
        &validation_root,
        &ExtensionsConfiguration::default(),
        options,
    );
    let mut package = catalog.packages.into_iter().next().ok_or_else(|| {
        PackageInstallError::InvalidPackage(vec!["staged package was not discoverable".into()])
    })?;
    let validation_errors = package
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            !matches!(
                diagnostic.code,
                crate::discovery::DiscoveryDiagnosticCode::Unapproved
                    | crate::discovery::DiscoveryDiagnosticCode::UnsignedPackage
            )
        })
        .map(|diagnostic| diagnostic.detail.clone())
        .collect::<Vec<_>>();
    if package.readiness == PackageReadiness::Disabled && !validation_errors.is_empty() {
        let _ = fs::rename(&validation_package, staged_directory);
        let _ = fs::remove_dir(&validation_root);
        return Err(PackageInstallError::InvalidPackage(validation_errors));
    }
    let manifest = package.manifest.take().expect("validated package manifest");
    if manifest.id != expected_extension_id {
        let _ = fs::rename(&validation_package, staged_directory);
        let _ = fs::remove_dir(&validation_root);
        return Err(PackageInstallError::UnexpectedIdentity {
            expected: expected_extension_id.into(),
            actual: manifest.id,
        });
    }
    let digest = package.package_digest.expect("validated package digest");
    let target = extensions_directory.join(expected_extension_id);
    let backup = extensions_directory.join(format!(".backup-{}", Uuid::new_v4()));
    if target.exists() {
        fs::rename(&target, &backup).map_err(io_error)?;
    }
    if let Err(error) = fs::rename(&validation_package, &target) {
        if backup.exists() {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_dir(&validation_root);
        return Err(io_error(error));
    }
    let _ = fs::remove_dir(&validation_root);
    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(io_error)?;
    }
    Ok(InstalledPackage {
        extension_id: expected_extension_id.into(),
        package_digest: digest,
        directory: target,
    })
}

fn io_error(error: std::io::Error) -> PackageInstallError {
    PackageInstallError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::PlatformTarget;
    use crate::manifest::test_manifest;
    use sha2::{Digest, Sha256};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let id = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                PathBuf::from(".artifacts/tmp/extensions-install-tests").join(id.to_string());
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn stage(root: &Path, contents: &[u8]) -> PathBuf {
        let path = root.join(".staging-package");
        fs::create_dir_all(path.join("bin")).unwrap();
        fs::write(path.join("bin/extension"), contents).unwrap();
        let digest: String = Sha256::digest(contents)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let target = PlatformTarget::current();
        let manifest = test_manifest(&target.os, &target.architecture, "bin/extension", &digest);
        fs::write(
            path.join("extension.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        path
    }

    #[test]
    fn complete_staging_atomically_replaces_package_and_invalid_update_preserves_old() {
        let root = TestDirectory::new();
        let options = DiscoveryOptions::default();
        let first = stage(&root.0, b"first");
        let installed =
            install_staged_package(&root.0, &first, "de.tosklight.example", &options).unwrap();
        assert_eq!(
            fs::read(installed.directory.join("bin/extension")).unwrap(),
            b"first"
        );
        let invalid = stage(&root.0, b"second");
        fs::write(invalid.join("bin/extension"), b"tampered").unwrap();
        assert!(
            install_staged_package(&root.0, &invalid, "de.tosklight.example", &options).is_err()
        );
        assert_eq!(
            fs::read(root.0.join("de.tosklight.example/bin/extension")).unwrap(),
            b"first"
        );
    }
}
