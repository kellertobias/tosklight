use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use light_extensions_contract::ExtensionCapability;

use crate::configuration::ExtensionsConfiguration;
use crate::discovery::{ExtensionsCatalog, PackageReadiness};
use crate::manifest::Multiplicity;

#[derive(Clone, Debug)]
pub struct ResolvedExtensionInstance {
    pub instance_id: String,
    pub extension_id: String,
    pub extension_version: String,
    pub package_digest: String,
    pub executable: PathBuf,
    pub desk_id: Option<String>,
    pub device_identity: Option<String>,
    pub capabilities: BTreeSet<ExtensionCapability>,
    pub feedback_features: BTreeSet<crate::manifest::FeedbackFeature>,
    pub telemetry_channels: Vec<light_extensions_contract::TelemetryChannelDeclaration>,
    pub maximum_telemetry_rate_hz: u32,
    pub device_actions: Vec<light_extensions_contract::DeviceActionDeclaration>,
    pub settings: BTreeMap<String, serde_json::Value>,
    pub control_bindings: BTreeMap<String, light_extensions_contract::CanonicalControlIntent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceDiagnostic {
    pub instance_id: String,
    pub code: InstanceDiagnosticCode,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceDiagnosticCode {
    PackageUnavailable,
    MissingDeskBinding,
    DeviceConflict,
    MultiplicityExceeded,
    InvalidControlBinding,
}

pub fn resolve_instances(
    configuration: &ExtensionsConfiguration,
    catalog: &ExtensionsCatalog,
) -> (Vec<ResolvedExtensionInstance>, Vec<InstanceDiagnostic>) {
    let packages = catalog
        .packages
        .iter()
        .filter_map(|package| {
            package
                .manifest
                .as_ref()
                .map(|manifest| (&manifest.id, (package, manifest)))
        })
        .collect::<BTreeMap<_, _>>();
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    let mut devices = BTreeMap::<String, String>::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for instance in configuration
        .instances
        .iter()
        .filter(|instance| instance.enabled)
    {
        let Some((package, manifest)) = packages
            .get(&instance.extension_id)
            .map(|(package, manifest)| (*package, *manifest))
        else {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::PackageUnavailable,
                "configured package is not installed",
            ));
            continue;
        };
        let (Some(executable), Some(digest)) = (&package.executable, &package.package_digest)
        else {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::PackageUnavailable,
                "package has no executable or verified digest",
            ));
            continue;
        };
        if package.readiness != PackageReadiness::Ready {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::PackageUnavailable,
                "package is disabled; inspect package diagnostics",
            ));
            continue;
        }
        if manifest
            .capabilities
            .contains(&ExtensionCapability::ControlSurface)
            && instance
                .desk_id
                .as_ref()
                .is_none_or(|desk| desk.trim().is_empty())
        {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::MissingDeskBinding,
                "control-surface instance must bind a logical desk",
            ));
            continue;
        }
        let declared_controls = manifest
            .controls
            .iter()
            .map(|control| control.id.as_str())
            .collect::<BTreeSet<_>>();
        if instance
            .control_bindings
            .keys()
            .any(|id| !declared_controls.contains(id.as_str()))
        {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::InvalidControlBinding,
                "configuration binds a control not declared by the package",
            ));
            continue;
        }
        if let Some(device) = &instance.device
            && let Some(owner) = devices.insert(device.identity.clone(), instance.id.clone())
        {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::DeviceConflict,
                format!("device is already claimed by instance `{owner}`"),
            ));
            continue;
        }
        let count = counts.entry(instance.extension_id.clone()).or_default();
        *count += 1;
        if manifest.multiplicity == Multiplicity::Single && *count > 1 {
            diagnostics.push(problem(
                instance,
                InstanceDiagnosticCode::MultiplicityExceeded,
                "manifest permits only one enabled instance",
            ));
            continue;
        }
        resolved.push(ResolvedExtensionInstance {
            instance_id: instance.id.clone(),
            extension_id: instance.extension_id.clone(),
            extension_version: manifest.version.clone(),
            package_digest: digest.clone(),
            executable: executable.clone(),
            desk_id: instance.desk_id.clone(),
            device_identity: instance
                .device
                .as_ref()
                .map(|device| device.identity.clone()),
            capabilities: manifest.capabilities.clone(),
            feedback_features: manifest.feedback_features.iter().copied().collect(),
            telemetry_channels: manifest.telemetry_channels.clone(),
            maximum_telemetry_rate_hz: manifest.limits.maximum_telemetry_rate_hz,
            device_actions: manifest
                .device_actions
                .iter()
                .filter(|action| {
                    instance
                        .device_action_permissions
                        .contains(&action.required_permission)
                })
                .cloned()
                .collect(),
            settings: instance.settings.clone(),
            control_bindings: instance.control_bindings.clone(),
        });
    }
    (resolved, diagnostics)
}

fn problem(
    instance: &crate::configuration::ExtensionInstanceConfiguration,
    code: InstanceDiagnosticCode,
    detail: impl Into<String>,
) -> InstanceDiagnostic {
    InstanceDiagnostic {
        instance_id: instance.id.clone(),
        code,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{DeviceBinding, ExtensionInstanceConfiguration};
    use crate::discovery::{DiscoveredPackage, ExtensionsCatalog};
    use crate::manifest::test_manifest;
    use std::path::PathBuf;

    #[test]
    fn instances_require_desk_and_unique_device_claims() {
        let target = crate::discovery::PlatformTarget::current();
        let manifest = test_manifest(
            &target.os,
            &target.architecture,
            "bin/extension",
            &"a".repeat(64),
        );
        let catalog = ExtensionsCatalog {
            extensions_directory: PathBuf::from("extensions"),
            packages: vec![DiscoveredPackage {
                directory: PathBuf::from("extensions/package"),
                manifest: Some(manifest),
                package_digest: Some("b".repeat(64)),
                executable: Some(PathBuf::from("extensions/package/bin/extension")),
                readiness: PackageReadiness::Ready,
                diagnostics: vec![],
                locally_approved_unsigned: true,
            }],
        };
        let instance = |id: &str, desk: Option<&str>| ExtensionInstanceConfiguration {
            id: id.into(),
            extension_id: "de.tosklight.example".into(),
            enabled: true,
            desk_id: desk.map(str::to_owned),
            device: Some(DeviceBinding {
                identity: "usb:serial".into(),
                transport: Some("hid".into()),
            }),
            settings: BTreeMap::new(),
            control_bindings: BTreeMap::new(),
            device_action_permissions: BTreeSet::new(),
        };
        let configuration = ExtensionsConfiguration {
            version: 1,
            approved_packages: BTreeMap::new(),
            instances: vec![
                instance("one", Some("main")),
                instance("two", Some("aux")),
                instance("three", None),
            ],
        };
        let (resolved, diagnostics) = resolve_instances(&configuration, &catalog);
        assert_eq!(resolved.len(), 1);
        assert!(
            diagnostics
                .iter()
                .any(|item| item.code == InstanceDiagnosticCode::DeviceConflict)
        );
        assert!(
            diagnostics
                .iter()
                .any(|item| item.code == InstanceDiagnosticCode::MissingDeskBinding)
        );
    }
}
