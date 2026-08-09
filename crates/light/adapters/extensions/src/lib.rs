#![forbid(unsafe_code)]
//! Phase-0 proof of the supervised native-extension boundary.
//!
//! The host is deliberately independent of the headless runtime. It proves process isolation,
//! authenticated protocol startup, bounded communication, snapshot repair, and typed application
//! ports before the production runtime adopts the boundary.

mod bounded;
mod claims;
mod configuration;
mod discovery;
mod manager;
mod manifest;
mod package;
mod ports;
mod session;
mod supervisor;

pub use claims::{
    InstanceDiagnostic, InstanceDiagnosticCode, ResolvedExtensionInstance, resolve_instances,
};
pub use configuration::{
    DeviceBinding, ExtensionInstanceConfiguration, ExtensionsConfiguration,
    ExtensionsConfigurationError, LoadedExtensionsConfiguration, load_extensions_configuration,
    write_extensions_configuration,
};
pub use discovery::{
    DiscoveredPackage, DiscoveryDiagnostic, DiscoveryDiagnosticCode, DiscoveryOptions,
    ExtensionsCatalog, ExtensionsDirectoryMode, PackageReadiness, PlatformTarget,
    discover_packages, effective_extensions_directory,
};
pub use manager::{ExtensionManager, ExtensionManagerSnapshot, ManagedInstanceSnapshot};
pub use manifest::{
    ArtifactManifest, ControlDeclaration, ControlKind, DeviceMatchManifest, ExtensionManifest,
    FeedbackFeature, HostApiRange, LicenseManifest, ManifestError, Multiplicity,
    PackageFileManifest, PackageLimits, ProtocolRange, VendorManifest, validate_manifest,
};
pub use package::{InstalledPackage, PackageInstallError, install_staged_package};
pub use ports::{
    BoundControlInput, ExtensionApplicationPorts, HostControlContext, PortError, TelemetryEnvelope,
    TimecodeEnvelope,
};
pub use supervisor::{
    DeviceActionEnqueueError, ExtensionHost, ExtensionLimits, ExtensionSpec, ExtensionState,
    FeedbackEnqueueError, HostHealth, RunningExtension, TelemetryChannelHealth,
};

/// The private pipe credential is supplied through the child environment, never command-line
/// arguments, and never crosses the pipe itself. This draft proof hashes unambiguous
/// length-prefixed credential and challenge bytes; its shape is not a frozen authentication API.
pub fn channel_response(credential: &str, challenge: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut digest = Sha256::new();
    digest.update((credential.len() as u64).to_be_bytes());
    digest.update(credential.as_bytes());
    digest.update((challenge.len() as u64).to_be_bytes());
    digest.update(challenge.as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_response_is_a_digest_and_binds_both_inputs() {
        let response = channel_response("credential", "challenge");
        assert_eq!(response.len(), 64);
        assert!(!response.contains("credential"));
        assert_ne!(response, channel_response("wrong", "challenge"));
        assert_ne!(response, channel_response("credential", "wrong"));
    }
}
