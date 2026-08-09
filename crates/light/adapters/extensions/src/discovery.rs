use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::configuration::ExtensionsConfiguration;
use crate::manifest::{ExtensionManifest, validate_manifest};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformTarget {
    pub os: String,
    pub architecture: String,
}

impl PlatformTarget {
    pub fn current() -> Self {
        Self {
            os: normalized_os(std::env::consts::OS).into(),
            architecture: normalized_arch(std::env::consts::ARCH).into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveryOptions {
    pub target: PlatformTarget,
    pub host_api_version: u16,
    pub protocol_version: u16,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            target: PlatformTarget::current(),
            host_api_version: 1,
            protocol_version: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageReadiness {
    Ready,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct DiscoveredPackage {
    pub directory: PathBuf,
    pub manifest: Option<ExtensionManifest>,
    pub package_digest: Option<String>,
    pub executable: Option<PathBuf>,
    pub readiness: PackageReadiness,
    pub diagnostics: Vec<DiscoveryDiagnostic>,
    pub locally_approved_unsigned: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryDiagnostic {
    pub code: DiscoveryDiagnosticCode,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryDiagnosticCode {
    InvalidManifest,
    DuplicateId,
    UnsupportedPlatform,
    UnsupportedHostApi,
    UnsupportedProtocol,
    MissingFile,
    ExtraFile,
    SymlinkRejected,
    DigestMismatch,
    Unapproved,
    ChangedDigest,
    UnsignedPackage,
}

#[derive(Clone, Debug)]
pub struct ExtensionsCatalog {
    pub extensions_directory: PathBuf,
    pub packages: Vec<DiscoveredPackage>,
}

pub fn discover_packages(
    root: &Path,
    configuration: &ExtensionsConfiguration,
    options: &DiscoveryOptions,
) -> ExtensionsCatalog {
    let mut directories = fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    directories.sort();
    let mut packages = directories
        .into_iter()
        .map(|directory| inspect_package(directory, configuration, options))
        .collect::<Vec<_>>();

    let mut by_id: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, package) in packages.iter().enumerate() {
        if let Some(manifest) = &package.manifest {
            by_id.entry(manifest.id.clone()).or_default().push(index);
        }
    }
    for (id, indexes) in by_id.into_iter().filter(|(_, indexes)| indexes.len() > 1) {
        for index in indexes {
            packages[index].readiness = PackageReadiness::Disabled;
            packages[index].diagnostics.push(DiscoveryDiagnostic {
                code: DiscoveryDiagnosticCode::DuplicateId,
                detail: format!("duplicate manifest id `{id}`; every duplicate is disabled"),
            });
        }
    }
    ExtensionsCatalog {
        extensions_directory: root.to_path_buf(),
        packages,
    }
}

fn inspect_package(
    directory: PathBuf,
    configuration: &ExtensionsConfiguration,
    options: &DiscoveryOptions,
) -> DiscoveredPackage {
    let manifest_path = directory.join("extension.json");
    let bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return invalid(
                directory,
                DiscoveryDiagnosticCode::InvalidManifest,
                format!("cannot read extension.json: {error}"),
            );
        }
    };
    let manifest: ExtensionManifest = match serde_json::from_slice(&bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            return invalid(
                directory,
                DiscoveryDiagnosticCode::InvalidManifest,
                format!("invalid extension.json: {error}"),
            );
        }
    };
    if let Err(error) = validate_manifest(&manifest) {
        return invalid_with_manifest(
            directory,
            manifest,
            DiscoveryDiagnosticCode::InvalidManifest,
            error.to_string(),
        );
    }
    let package_digest = package_digest(&bytes, &manifest);
    let mut diagnostics = Vec::new();

    if !(manifest.host_api.minimum..=manifest.host_api.maximum).contains(&options.host_api_version)
    {
        diagnostics.push(diag(
            DiscoveryDiagnosticCode::UnsupportedHostApi,
            "host API version is outside the manifest range",
        ));
    }
    if !(manifest.protocol.minimum..=manifest.protocol.maximum).contains(&options.protocol_version)
    {
        diagnostics.push(diag(
            DiscoveryDiagnosticCode::UnsupportedProtocol,
            "extension protocol version is outside the manifest range",
        ));
    }
    let executable = manifest
        .artifacts
        .iter()
        .find(|artifact| {
            artifact.os == options.target.os && artifact.architecture == options.target.architecture
        })
        .map(|artifact| directory.join(&artifact.executable));
    if executable.is_none() {
        diagnostics.push(diag(
            DiscoveryDiagnosticCode::UnsupportedPlatform,
            "package has no executable for this OS and architecture",
        ));
    }

    verify_package_files(&directory, &manifest, &mut diagnostics);
    match configuration.approved_digest(&manifest.id) {
        None => diagnostics.push(diag(
            DiscoveryDiagnosticCode::Unapproved,
            "package digest is not approved by installation configuration",
        )),
        Some(approved) if approved != package_digest => diagnostics.push(diag(
            DiscoveryDiagnosticCode::ChangedDigest,
            "package content changed and its new digest is not approved",
        )),
        Some(_) => {}
    }
    if manifest.signature.is_none()
        && configuration.approved_digest(&manifest.id) == Some(package_digest.as_str())
    {
        diagnostics.push(diag(
            DiscoveryDiagnosticCode::UnsignedPackage,
            "locally approved unsigned package",
        ));
    }
    let blocking = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code != DiscoveryDiagnosticCode::UnsignedPackage);
    DiscoveredPackage {
        directory,
        manifest: Some(manifest),
        package_digest: Some(package_digest),
        executable,
        readiness: if blocking {
            PackageReadiness::Disabled
        } else {
            PackageReadiness::Ready
        },
        diagnostics,
        locally_approved_unsigned: !blocking,
    }
}

fn verify_package_files(
    directory: &Path,
    manifest: &ExtensionManifest,
    diagnostics: &mut Vec<DiscoveryDiagnostic>,
) {
    let declared = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    for file in &manifest.files {
        let path = directory.join(&file.path);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => {
                diagnostics.push(diag(
                    DiscoveryDiagnosticCode::MissingFile,
                    format!("declared file `{}` is missing", file.path),
                ));
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            diagnostics.push(diag(
                DiscoveryDiagnosticCode::SymlinkRejected,
                format!("declared file `{}` is a symlink", file.path),
            ));
            continue;
        }
        if !metadata.is_file() {
            diagnostics.push(diag(
                DiscoveryDiagnosticCode::MissingFile,
                format!("declared path `{}` is not a regular file", file.path),
            ));
            continue;
        }
        match fs::read(&path) {
            Ok(bytes) if hex_sha256(&bytes) == file.sha256 => {}
            Ok(_) => diagnostics.push(diag(
                DiscoveryDiagnosticCode::DigestMismatch,
                format!("digest mismatch for `{}`", file.path),
            )),
            Err(error) => diagnostics.push(diag(
                DiscoveryDiagnosticCode::MissingFile,
                format!("cannot read `{}`: {error}", file.path),
            )),
        }
    }
    let mut pending = vec![directory.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(directory)
                .expect("package child")
                .to_string_lossy()
                .replace('\\', "/");
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(path);
            } else if relative != "extension.json" && !declared.contains(relative.as_str()) {
                diagnostics.push(diag(
                    DiscoveryDiagnosticCode::ExtraFile,
                    format!("undeclared package file `{relative}`"),
                ));
            }
        }
    }
}

fn package_digest(manifest_bytes: &[u8], manifest: &ExtensionManifest) -> String {
    let mut digest = Sha256::new();
    digest.update((manifest_bytes.len() as u64).to_be_bytes());
    digest.update(manifest_bytes);
    let mut files = manifest.files.clone();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    for file in files {
        digest.update((file.path.len() as u64).to_be_bytes());
        digest.update(file.path.as_bytes());
        digest.update(file.sha256.as_bytes());
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn diag(code: DiscoveryDiagnosticCode, detail: impl Into<String>) -> DiscoveryDiagnostic {
    DiscoveryDiagnostic {
        code,
        detail: detail.into(),
    }
}
fn invalid(directory: PathBuf, code: DiscoveryDiagnosticCode, detail: String) -> DiscoveredPackage {
    DiscoveredPackage {
        directory,
        manifest: None,
        package_digest: None,
        executable: None,
        readiness: PackageReadiness::Disabled,
        diagnostics: vec![diag(code, detail)],
        locally_approved_unsigned: false,
    }
}
fn invalid_with_manifest(
    directory: PathBuf,
    manifest: ExtensionManifest,
    code: DiscoveryDiagnosticCode,
    detail: String,
) -> DiscoveredPackage {
    DiscoveredPackage {
        directory,
        manifest: Some(manifest),
        package_digest: None,
        executable: None,
        readiness: PackageReadiness::Disabled,
        diagnostics: vec![diag(code, detail)],
        locally_approved_unsigned: false,
    }
}
fn normalized_os(value: &str) -> &str {
    match value {
        "macos" => "macos",
        "windows" => "windows",
        other => other,
    }
}
fn normalized_arch(value: &str) -> &str {
    match value {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => other,
    }
}

pub enum ExtensionsDirectoryMode<'a> {
    Explicit(&'a Path),
    Development(&'a Path),
    Portable(&'a Path),
    ApplicationData(&'a Path),
}

pub fn effective_extensions_directory(mode: ExtensionsDirectoryMode<'_>) -> PathBuf {
    match mode {
        ExtensionsDirectoryMode::Explicit(path) => path.to_path_buf(),
        ExtensionsDirectoryMode::Development(repository) => {
            repository.join(".artifacts/runtime/extensions")
        }
        ExtensionsDirectoryMode::Portable(executable) => executable
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("extensions"),
        ExtensionsDirectoryMode::ApplicationData(data) => data.join("extensions"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::test_manifest;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let id = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = PathBuf::from(".artifacts/tmp/extensions-host-tests")
                .join(format!("{}-{id}-{sequence}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_package(root: &Path, folder: &str, id: &str) -> String {
        let package = root.join(folder);
        fs::create_dir_all(package.join("bin")).unwrap();
        let executable = b"extension";
        fs::write(package.join("bin/extension"), executable).unwrap();
        let mut manifest = test_manifest(
            &PlatformTarget::current().os,
            &PlatformTarget::current().architecture,
            "bin/extension",
            &hex_sha256(executable),
        );
        manifest.id = id.into();
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        fs::write(package.join("extension.json"), &bytes).unwrap();
        package_digest(&bytes, &manifest)
    }

    #[test]
    fn discovery_requires_exact_approval_and_ignores_nested_or_staging_packages() {
        let temporary = TestDirectory::new();
        let digest = write_package(&temporary.0, "package", "de.tosklight.example");
        fs::create_dir_all(temporary.0.join(".staging-copy")).unwrap();
        write_package(
            &temporary.0.join("package"),
            "nested",
            "de.tosklight.nested",
        );
        let empty = ExtensionsConfiguration::default();
        let catalog = discover_packages(&temporary.0, &empty, &DiscoveryOptions::default());
        assert_eq!(catalog.packages.len(), 1);
        assert_eq!(catalog.packages[0].readiness, PackageReadiness::Disabled);
        fs::remove_dir_all(temporary.0.join("package/nested")).unwrap();
        let mut approved = empty;
        approved
            .approved_packages
            .insert("de.tosklight.example".into(), digest);
        let catalog = discover_packages(&temporary.0, &approved, &DiscoveryOptions::default());
        assert_eq!(catalog.packages[0].readiness, PackageReadiness::Ready);
        assert!(
            catalog.packages[0]
                .diagnostics
                .iter()
                .any(|item| item.code == DiscoveryDiagnosticCode::UnsignedPackage)
        );
    }

    #[test]
    fn duplicate_manifest_ids_disable_every_copy() {
        let temporary = TestDirectory::new();
        write_package(&temporary.0, "one", "de.tosklight.same");
        write_package(&temporary.0, "two", "de.tosklight.same");
        let catalog = discover_packages(
            &temporary.0,
            &ExtensionsConfiguration::default(),
            &DiscoveryOptions::default(),
        );
        assert_eq!(catalog.packages.len(), 2);
        assert!(catalog.packages.iter().all(|package| {
            package
                .diagnostics
                .iter()
                .any(|item| item.code == DiscoveryDiagnosticCode::DuplicateId)
        }));
    }
}
