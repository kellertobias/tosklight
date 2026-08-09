//! Platform-neutral USB endpoint configuration and routed-frame fanout.
//!
//! Drivers must own their worker/queue and make `enqueue_latest` non-blocking. This module never
//! performs serial I/O and can therefore be called after the renderer releases its output lock.

use crate::{DmxFrame, OutputRoute, OutputRouteTarget};
use light_core::Universe;
use light_usb_dmx_core::UniverseFrame;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbEndpointDriverKind {
    OpenDmx,
    EnttecUsbProV144,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct UsbDeviceIdentity {
    pub vendor_id: u16,
    pub product_id: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usb_serial: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_serial: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_topology_hint: Option<String>,
}

impl UsbDeviceIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.usb_serial.as_deref() == Some("") || self.widget_serial.as_deref() == Some("") {
            return Err("USB serial identities must not be empty".into());
        }
        if self.usb_serial.is_none()
            && self.widget_serial.is_none()
            && self.port_topology_hint.as_deref().is_none_or(str::is_empty)
        {
            return Err(
                "USB endpoint identity needs a USB serial, widget serial, or port-topology hint"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsbEndpointConfiguration {
    pub endpoint_id: String,
    pub driver: UsbEndpointDriverKind,
    pub identity: UsbDeviceIdentity,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UsbEndpointDocument {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub endpoints: Vec<UsbEndpointConfiguration>,
}

impl UsbEndpointDocument {
    pub fn validate(&self) -> Result<(), String> {
        let mut endpoint_ids = HashSet::new();
        let mut identities = HashSet::new();
        for endpoint in &self.endpoints {
            if endpoint.endpoint_id.trim().is_empty()
                || endpoint.endpoint_id.len() > 128
                || endpoint.endpoint_id.chars().any(char::is_control)
            {
                return Err("USB endpoint ID must contain 1 to 128 printable characters".into());
            }
            endpoint.identity.validate()?;
            if !endpoint_ids.insert(endpoint.endpoint_id.clone()) {
                return Err(format!(
                    "duplicate USB endpoint ID `{}`",
                    endpoint.endpoint_id
                ));
            }
            if endpoint.enabled && !identities.insert(endpoint.identity.clone()) {
                return Err(
                    "one physical USB identity cannot satisfy two enabled endpoints".into(),
                );
            }
        }
        Ok(())
    }
}

pub trait UsbEndpointDriver: Send + Sync {
    /// Must replace any obsolete pending frame and return without doing serial I/O.
    fn enqueue_latest(&self, frame: UniverseFrame) -> Result<(), String>;
    fn health(&self) -> UsbEndpointDriverHealth;
    fn shutdown(&self) -> UsbEndpointShutdown;
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UsbEndpointDriverHealth {
    pub online: bool,
    pub reconnecting: bool,
    pub accepted_frames: u64,
    pub reconnect_attempts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

pub trait UsbPlatformDriverFactory: Send + Sync {
    fn open(
        &self,
        endpoint: &UsbEndpointConfiguration,
    ) -> Result<Arc<dyn UsbEndpointDriver>, String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbEndpointShutdown {
    FinalFrameConfirmed,
    DeviceRetainsLastFrame,
    FinalOutputUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbEndpointDiagnosticCode {
    Disabled,
    PlatformAdapterUnavailable,
    Offline,
    Ready,
    SendFailed,
    DuplicateRouteClaim,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UsbEndpointDiagnostic {
    pub endpoint_id: String,
    pub code: UsbEndpointDiagnosticCode,
    pub message: String,
    pub dropped_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_health: Option<UsbEndpointDriverHealth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shutdown: Option<UsbEndpointShutdown>,
}

struct ManagedEndpoint {
    configuration: UsbEndpointConfiguration,
    driver: Option<Arc<dyn UsbEndpointDriver>>,
    diagnostic: UsbEndpointDiagnostic,
}

#[derive(Default)]
struct FanoutState {
    revision: u64,
    endpoints: HashMap<String, ManagedEndpoint>,
}

pub struct UsbOutputFanout {
    factory: Arc<dyn UsbPlatformDriverFactory>,
    state: Mutex<FanoutState>,
}

impl UsbOutputFanout {
    pub fn new(factory: Arc<dyn UsbPlatformDriverFactory>) -> Self {
        Self {
            factory,
            state: Mutex::new(FanoutState::default()),
        }
    }

    pub fn configure(&self, document: &UsbEndpointDocument) -> Result<(), String> {
        document.validate()?;
        let mut next = HashMap::new();
        let mut state = self.state.lock().expect("USB fanout mutex poisoned");
        for configuration in &document.endpoints {
            let existing = state.endpoints.remove(&configuration.endpoint_id);
            let managed = match existing {
                Some(existing)
                    if configuration.enabled && existing.configuration == *configuration =>
                {
                    existing
                }
                existing => {
                    if let Some(driver) = existing.and_then(|managed| managed.driver) {
                        let _ = driver.shutdown();
                    }
                    if !configuration.enabled {
                        managed_without_driver(
                            configuration.clone(),
                            UsbEndpointDiagnosticCode::Disabled,
                            "USB endpoint is disabled",
                        )
                    } else {
                        match self.factory.open(configuration) {
                            Ok(driver) => ManagedEndpoint {
                                configuration: configuration.clone(),
                                driver: Some(driver),
                                diagnostic: diagnostic(
                                    &configuration.endpoint_id,
                                    UsbEndpointDiagnosticCode::Ready,
                                    "USB endpoint worker is ready",
                                ),
                            },
                            Err(message) => managed_without_driver(
                                configuration.clone(),
                                if message.contains("not installed in this build") {
                                    UsbEndpointDiagnosticCode::PlatformAdapterUnavailable
                                } else {
                                    UsbEndpointDiagnosticCode::Offline
                                },
                                message,
                            ),
                        }
                    }
                }
            };
            next.insert(configuration.endpoint_id.clone(), managed);
        }
        for (_, removed) in state.endpoints.drain() {
            if let Some(driver) = removed.driver {
                let _ = driver.shutdown();
            }
        }
        state.revision = document.revision;
        state.endpoints = next;
        Ok(())
    }

    pub fn enqueue_routes(
        &self,
        routes: &[OutputRoute],
        frames: &HashMap<Universe, DmxFrame>,
    ) -> u64 {
        let (deliveries, duplicates): (Vec<_>, Vec<_>) = {
            let state = self.state.lock().expect("USB fanout mutex poisoned");
            let mut deliveries = HashMap::new();
            let mut duplicates = HashSet::new();
            for route in routes.iter().filter(|route| route.enabled) {
                let OutputRouteTarget::UsbEndpoint { endpoint_id } = &route.target else {
                    continue;
                };
                let Some(managed) = state.endpoints.get(endpoint_id) else {
                    continue;
                };
                let Some(driver) = managed.driver.as_ref().cloned() else {
                    continue;
                };
                let frame = frames
                    .get(&route.logical_universe)
                    .copied()
                    .unwrap_or([0; 512]);
                if deliveries
                    .insert(endpoint_id.clone(), (driver, frame))
                    .is_some()
                {
                    duplicates.insert(endpoint_id.clone());
                }
            }
            for endpoint_id in &duplicates {
                deliveries.remove(endpoint_id);
            }
            (
                deliveries
                    .into_iter()
                    .map(|(endpoint_id, (driver, frame))| (endpoint_id, driver, frame))
                    .collect(),
                duplicates.into_iter().collect(),
            )
        };
        if !duplicates.is_empty() {
            let mut state = self.state.lock().expect("USB fanout mutex poisoned");
            for endpoint_id in duplicates {
                if let Some(managed) = state.endpoints.get_mut(&endpoint_id) {
                    managed.diagnostic.code = UsbEndpointDiagnosticCode::DuplicateRouteClaim;
                    managed.diagnostic.message =
                        "multiple enabled show routes claim this one-universe USB endpoint".into();
                    managed.diagnostic.dropped_frames += 1;
                }
            }
        }
        let mut accepted = 0;
        for (endpoint_id, driver, frame) in deliveries {
            match driver.enqueue_latest(UniverseFrame::new(frame)) {
                Ok(()) => accepted += 1,
                Err(message) => {
                    let mut state = self.state.lock().expect("USB fanout mutex poisoned");
                    if let Some(managed) = state.endpoints.get_mut(&endpoint_id) {
                        managed.diagnostic.code = UsbEndpointDiagnosticCode::SendFailed;
                        managed.diagnostic.message = message;
                        managed.diagnostic.dropped_frames += 1;
                    }
                }
            }
        }
        accepted
    }

    pub fn diagnostics(&self) -> Vec<UsbEndpointDiagnostic> {
        let state = self.state.lock().expect("USB fanout mutex poisoned");
        let mut diagnostics: Vec<_> = state
            .endpoints
            .values()
            .map(|managed| {
                let mut diagnostic = managed.diagnostic.clone();
                if let Some(driver) = &managed.driver {
                    let health = driver.health();
                    if matches!(
                        diagnostic.code,
                        UsbEndpointDiagnosticCode::Ready | UsbEndpointDiagnosticCode::Offline
                    ) {
                        diagnostic.code = if health.online {
                            UsbEndpointDiagnosticCode::Ready
                        } else {
                            UsbEndpointDiagnosticCode::Offline
                        };
                        diagnostic.message = health.last_error.clone().unwrap_or_else(|| {
                            if health.reconnecting {
                                "USB endpoint worker is reconnecting".into()
                            } else if health.online {
                                "USB endpoint worker is online".into()
                            } else {
                                "USB endpoint worker is offline".into()
                            }
                        });
                    }
                    diagnostic.driver_health = Some(health);
                }
                diagnostic
            })
            .collect();
        diagnostics.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
        diagnostics
    }

    /// Requests a safe zero frame for USB routes which are being disabled or removed. The driver
    /// remains honest about whether that queued frame reaches the physical line during shutdown.
    pub fn terminate_routes(&self, routes: &[OutputRoute]) -> u64 {
        let zeros = HashMap::new();
        let mut terminating = routes.to_vec();
        for route in &mut terminating {
            route.logical_universe = 0;
        }
        self.enqueue_routes(&terminating, &zeros)
    }

    pub fn shutdown(&self) -> Vec<UsbEndpointDiagnostic> {
        let mut state = self.state.lock().expect("USB fanout mutex poisoned");
        for managed in state.endpoints.values_mut() {
            if let Some(driver) = managed.driver.take() {
                managed.diagnostic.shutdown = Some(driver.shutdown());
            }
        }
        state
            .endpoints
            .values()
            .map(|managed| managed.diagnostic.clone())
            .collect()
    }
}

fn managed_without_driver(
    configuration: UsbEndpointConfiguration,
    code: UsbEndpointDiagnosticCode,
    message: impl Into<String>,
) -> ManagedEndpoint {
    let endpoint_id = configuration.endpoint_id.clone();
    ManagedEndpoint {
        configuration,
        driver: None,
        diagnostic: diagnostic(&endpoint_id, code, message),
    }
}

fn diagnostic(
    endpoint_id: &str,
    code: UsbEndpointDiagnosticCode,
    message: impl Into<String>,
) -> UsbEndpointDiagnostic {
    UsbEndpointDiagnostic {
        endpoint_id: endpoint_id.into(),
        code,
        message: message.into(),
        dropped_frames: 0,
        driver_health: None,
        shutdown: None,
    }
}

pub struct UnavailableUsbDriverFactory;

impl UsbPlatformDriverFactory for UnavailableUsbDriverFactory {
    fn open(
        &self,
        _endpoint: &UsbEndpointConfiguration,
    ) -> Result<Arc<dyn UsbEndpointDriver>, String> {
        Err("USB serial platform adapter is not installed in this build".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[derive(Default)]
    struct FakeDriver {
        latest: Mutex<Option<UniverseFrame>>,
        enqueue_count: AtomicU64,
        shutdown_count: AtomicU64,
        health_override: Mutex<Option<UsbEndpointDriverHealth>>,
    }

    impl UsbEndpointDriver for FakeDriver {
        fn enqueue_latest(&self, frame: UniverseFrame) -> Result<(), String> {
            *self.latest.lock().unwrap() = Some(frame);
            self.enqueue_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn health(&self) -> UsbEndpointDriverHealth {
            if let Some(health) = self.health_override.lock().unwrap().clone() {
                return health;
            }
            UsbEndpointDriverHealth {
                online: true,
                accepted_frames: self.enqueue_count.load(Ordering::Relaxed),
                ..UsbEndpointDriverHealth::default()
            }
        }

        fn shutdown(&self) -> UsbEndpointShutdown {
            self.shutdown_count.fetch_add(1, Ordering::Relaxed);
            UsbEndpointShutdown::FinalOutputUnknown
        }
    }

    #[derive(Default)]
    struct FakeFactory {
        drivers: Mutex<HashMap<String, Arc<FakeDriver>>>,
    }

    impl UsbPlatformDriverFactory for FakeFactory {
        fn open(
            &self,
            endpoint: &UsbEndpointConfiguration,
        ) -> Result<Arc<dyn UsbEndpointDriver>, String> {
            let driver = Arc::new(FakeDriver::default());
            self.drivers
                .lock()
                .unwrap()
                .insert(endpoint.endpoint_id.clone(), driver.clone());
            Ok(driver)
        }
    }

    fn endpoint(id: &str, serial: &str) -> UsbEndpointConfiguration {
        UsbEndpointConfiguration {
            endpoint_id: id.into(),
            driver: UsbEndpointDriverKind::EnttecUsbProV144,
            identity: UsbDeviceIdentity {
                vendor_id: 0x0403,
                product_id: 0x6001,
                manufacturer: Some("ENTTEC".into()),
                product: Some("DMX USB PRO".into()),
                usb_serial: Some(serial.into()),
                widget_serial: None,
                port_topology_hint: None,
            },
            enabled: true,
        }
    }

    fn usb_route(endpoint_id: &str, universe: Universe) -> OutputRoute {
        OutputRoute {
            target: OutputRouteTarget::UsbEndpoint {
                endpoint_id: endpoint_id.into(),
            },
            protocol: crate::Protocol::ArtNet,
            logical_universe: universe,
            destination_universe: 1,
            delivery_mode: None,
            destination: None,
            enabled: true,
            minimum_slots: 512,
        }
    }

    #[test]
    fn fanout_routes_latest_frames_and_reconfiguration_stops_removed_worker() {
        let factory = Arc::new(FakeFactory::default());
        let fanout = UsbOutputFanout::new(factory.clone());
        fanout
            .configure(&UsbEndpointDocument {
                revision: 1,
                endpoints: vec![endpoint("front", "A")],
            })
            .unwrap();
        assert_eq!(
            fanout.enqueue_routes(&[usb_route("front", 2)], &HashMap::from([(2, [7; 512])])),
            1
        );
        let driver = factory.drivers.lock().unwrap()["front"].clone();
        assert_eq!(
            driver.latest.lock().unwrap().as_ref().unwrap().slots()[0],
            7
        );
        assert_eq!(
            fanout.diagnostics()[0]
                .driver_health
                .as_ref()
                .unwrap()
                .accepted_frames,
            1
        );

        *driver.health_override.lock().unwrap() = Some(UsbEndpointDriverHealth {
            reconnecting: true,
            reconnect_attempts: 2,
            last_error: Some("device unplugged".into()),
            ..UsbEndpointDriverHealth::default()
        });
        let diagnostic = &fanout.diagnostics()[0];
        assert_eq!(diagnostic.code, UsbEndpointDiagnosticCode::Offline);
        assert_eq!(diagnostic.message, "device unplugged");
        assert!(diagnostic.driver_health.as_ref().unwrap().reconnecting);

        fanout
            .configure(&UsbEndpointDocument {
                revision: 2,
                endpoints: vec![],
            })
            .unwrap();
        assert_eq!(driver.shutdown_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn duplicate_physical_claims_are_rejected() {
        let mut duplicate = endpoint("rear", "A");
        duplicate.endpoint_id = "rear".into();
        let document = UsbEndpointDocument {
            revision: 1,
            endpoints: vec![endpoint("front", "A"), duplicate],
        };
        assert!(document.validate().unwrap_err().contains("one physical"));
    }

    #[test]
    fn duplicate_route_claim_sends_nothing_instead_of_last_route_wins() {
        let factory = Arc::new(FakeFactory::default());
        let fanout = UsbOutputFanout::new(factory);
        fanout
            .configure(&UsbEndpointDocument {
                revision: 1,
                endpoints: vec![endpoint("front", "A")],
            })
            .unwrap();
        assert_eq!(
            fanout.enqueue_routes(
                &[usb_route("front", 1), usb_route("front", 2)],
                &HashMap::from([(1, [1; 512]), (2, [2; 512])]),
            ),
            0
        );
        assert_eq!(
            fanout.diagnostics()[0].code,
            UsbEndpointDiagnosticCode::DuplicateRouteClaim
        );
    }
}
