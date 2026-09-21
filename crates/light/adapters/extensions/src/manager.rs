use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use uuid::Uuid;

use crate::claims::{InstanceDiagnostic, resolve_instances};
use crate::configuration::load_extensions_configuration;
use crate::discovery::{DiscoveredPackage, DiscoveryOptions, ExtensionsCatalog, discover_packages};
use crate::ports::ExtensionApplicationPorts;
use crate::supervisor::{
    ExtensionHost, ExtensionLimits, ExtensionSpec, HostHealth, RunningExtension,
};

#[derive(Clone, Debug, Serialize)]
pub struct ManagedInstanceSnapshot {
    pub instance_id: String,
    pub extension_id: String,
    pub package_digest: String,
    pub executable: PathBuf,
    pub capabilities: BTreeSet<light_extensions_contract::ExtensionCapability>,
    #[serde(skip)]
    pub health: Option<HostHealth>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExtensionManagerSnapshot {
    pub extensions_directory: PathBuf,
    pub configuration_path: PathBuf,
    pub configuration_diagnostic: Option<String>,
    #[serde(skip)]
    pub packages: Vec<DiscoveredPackage>,
    #[serde(skip)]
    pub instance_diagnostics: Vec<InstanceDiagnostic>,
    pub instances: Vec<ManagedInstanceSnapshot>,
}

struct ManagedProcess {
    fingerprint: String,
    extension_id: String,
    executable: PathBuf,
    capabilities: BTreeSet<light_extensions_contract::ExtensionCapability>,
    process: RunningExtension,
}

pub struct ExtensionManager<P: ExtensionApplicationPorts> {
    extensions_directory: PathBuf,
    configuration_path: PathBuf,
    options: DiscoveryOptions,
    limits: ExtensionLimits,
    ports: Arc<P>,
    running: BTreeMap<String, ManagedProcess>,
    snapshot: ExtensionManagerSnapshot,
}

impl<P: ExtensionApplicationPorts> ExtensionManager<P> {
    pub fn new(extensions_directory: PathBuf, configuration_path: PathBuf, ports: Arc<P>) -> Self {
        Self {
            snapshot: ExtensionManagerSnapshot {
                extensions_directory: extensions_directory.clone(),
                configuration_path: configuration_path.clone(),
                configuration_diagnostic: None,
                packages: Vec::new(),
                instance_diagnostics: Vec::new(),
                instances: Vec::new(),
            },
            extensions_directory,
            configuration_path,
            options: DiscoveryOptions::default(),
            limits: ExtensionLimits::default(),
            ports,
            running: BTreeMap::new(),
        }
    }

    pub fn with_limits(mut self, limits: ExtensionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn rescan(&mut self) -> &ExtensionManagerSnapshot {
        let loaded = load_extensions_configuration(&self.configuration_path);
        let catalog = discover_packages(
            &self.extensions_directory,
            &loaded.configuration,
            &self.options,
        );
        let (resolved, instance_diagnostics) = resolve_instances(&loaded.configuration, &catalog);
        let wanted = resolved
            .iter()
            .map(|instance| {
                (
                    instance.instance_id.clone(),
                    format!(
                        "{}:{}:{}",
                        instance.package_digest,
                        instance.desk_id.as_deref().unwrap_or("system"),
                        instance.device_identity.as_deref().unwrap_or("unbound")
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let obsolete = self
            .running
            .iter()
            .filter(|(id, running)| wanted.get(*id) != Some(&running.fingerprint))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in obsolete {
            if let Some(mut process) = self.running.remove(&id) {
                process.process.stop();
            }
        }
        for instance in resolved {
            if self.running.contains_key(&instance.instance_id) {
                continue;
            }
            let fingerprint = wanted[&instance.instance_id].clone();
            let capabilities = instance.capabilities.clone();
            let mut environment = BTreeMap::new();
            if let Some(identity) = &instance.device_identity {
                environment.insert(
                    "TOSKLIGHT_EXTENSION_DEVICE_IDENTITY".into(),
                    identity.clone(),
                );
            }
            let spec = ExtensionSpec {
                program: instance.executable.clone(),
                extension_id: instance.extension_id.clone(),
                extension_instance_id: instance.instance_id.clone(),
                extension_version: instance.extension_version,
                approved_package_digest: instance.package_digest.clone(),
                desk_id: instance.desk_id.unwrap_or_else(|| "system".into()),
                channel_credential: Uuid::new_v4().to_string(),
                requested_capabilities: instance.capabilities,
                feedback_features: instance.feedback_features,
                telemetry_channels: instance.telemetry_channels,
                maximum_telemetry_rate_hz: instance.maximum_telemetry_rate_hz,
                device_actions: instance.device_actions,
                settings: instance.settings,
                control_bindings: instance.control_bindings,
                environment,
            };
            let process =
                ExtensionHost::new(spec, self.limits.clone(), Arc::clone(&self.ports)).start();
            self.running.insert(
                instance.instance_id,
                ManagedProcess {
                    fingerprint,
                    extension_id: instance.extension_id,
                    executable: instance.executable,
                    capabilities,
                    process,
                },
            );
        }
        self.snapshot = snapshot(
            &self.extensions_directory,
            &self.configuration_path,
            loaded.diagnostic,
            catalog,
            instance_diagnostics,
            &self.running,
        );
        &self.snapshot
    }

    pub fn snapshot(&mut self) -> &ExtensionManagerSnapshot {
        for instance in &mut self.snapshot.instances {
            instance.health = self
                .running
                .get(&instance.instance_id)
                .map(|process| process.process.health());
        }
        &self.snapshot
    }

    pub fn restart(&self, instance_id: &str) -> Result<(), String> {
        self.running
            .get(instance_id)
            .ok_or_else(|| format!("extension instance `{instance_id}` is not running"))?
            .process
            .restart()
            .map_err(|error| format!("restart rejected: {error:?}"))
    }

    pub fn refresh_feedback_snapshots(&self) {
        for process in self.running.values() {
            let _ = process.process.refresh_snapshot();
        }
    }

    /// Whether an attached control-surface process has completed its authenticated handshake.
    /// Telemetry- and timecode-only extensions never make the desk enter hardware-connected mode.
    pub fn control_surface_connected(&self) -> bool {
        self.running.values().any(|process| {
            connected_control_surface(&process.capabilities, &process.process.health().state)
        })
    }

    pub fn stop(&mut self) {
        for (_, mut process) in std::mem::take(&mut self.running) {
            process.process.stop();
        }
        self.snapshot.instances.clear();
    }
}

fn connected_control_surface(
    capabilities: &BTreeSet<light_extensions_contract::ExtensionCapability>,
    state: &crate::supervisor::ExtensionState,
) -> bool {
    capabilities.contains(&light_extensions_contract::ExtensionCapability::ControlSurface)
        && matches!(state, crate::supervisor::ExtensionState::Running)
}

impl<P: ExtensionApplicationPorts> Drop for ExtensionManager<P> {
    fn drop(&mut self) {
        self.stop();
    }
}

fn snapshot(
    extensions_directory: &std::path::Path,
    configuration_path: &std::path::Path,
    configuration_diagnostic: Option<String>,
    catalog: ExtensionsCatalog,
    instance_diagnostics: Vec<InstanceDiagnostic>,
    running: &BTreeMap<String, ManagedProcess>,
) -> ExtensionManagerSnapshot {
    ExtensionManagerSnapshot {
        extensions_directory: extensions_directory.to_path_buf(),
        configuration_path: configuration_path.to_path_buf(),
        configuration_diagnostic,
        packages: catalog.packages,
        instance_diagnostics,
        instances: running
            .iter()
            .map(|(id, process)| ManagedInstanceSnapshot {
                instance_id: id.clone(),
                extension_id: process.extension_id.clone(),
                package_digest: process
                    .fingerprint
                    .split(':')
                    .next()
                    .unwrap_or_default()
                    .into(),
                executable: process.executable.clone(),
                capabilities: process.capabilities.clone(),
                health: Some(process.process.health()),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_extensions_contract::ExtensionCapability;

    #[test]
    fn only_a_running_control_surface_counts_as_connected_hardware() {
        let control_surface = BTreeSet::from([ExtensionCapability::ControlSurface]);
        let telemetry = BTreeSet::from([ExtensionCapability::TelemetrySource]);
        let timecode = BTreeSet::from([ExtensionCapability::TimecodeSource]);

        assert!(connected_control_surface(
            &control_surface,
            &crate::supervisor::ExtensionState::Running,
        ));
        assert!(!connected_control_surface(
            &control_surface,
            &crate::supervisor::ExtensionState::Handshaking,
        ));
        assert!(!connected_control_surface(
            &control_surface,
            &crate::supervisor::ExtensionState::Restarting {
                failures: 1,
                delay: std::time::Duration::from_millis(250),
            },
        ));
        assert!(!connected_control_surface(
            &telemetry,
            &crate::supervisor::ExtensionState::Running,
        ));
        assert!(!connected_control_surface(
            &timecode,
            &crate::supervisor::ExtensionState::Running,
        ));
    }
}
