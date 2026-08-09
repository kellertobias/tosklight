//! Cross-platform VCP adapter for the built-in Open DMX and USB Pro protocol cores.

use light_output::{
    UsbDeviceIdentity, UsbEndpointConfiguration, UsbEndpointDriver, UsbEndpointDriverHealth,
    UsbEndpointDriverKind, UsbEndpointShutdown, UsbPlatformDriverFactory,
};
use light_usb_dmx_core::{
    Clock, OpenDmxDriver, OpenDmxSerial, ProDriver, ProFrameParser, ProLabel, ProSerial,
    SerialConfiguration, TransportError, UniverseFrame, decode_widget_serial, encode_pro_frame,
};
use serde::Serialize;
use serialport::{DataBits, FlowControl, Parity, SerialPort, SerialPortType, StopBits};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PRO_BAUD: u32 = 57_600;
const IO_TIMEOUT: Duration = Duration::from_millis(100);
const RECONNECT_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiscoveredUsbSerialDevice {
    pub port_name: String,
    pub identity: UsbDeviceIdentity,
}

pub fn discover_usb_serial_devices() -> Result<Vec<DiscoveredUsbSerialDevice>, String> {
    let mut devices = serialport::available_ports()
        .map_err(|error| format!("cannot enumerate serial devices: {error}"))?
        .into_iter()
        .filter_map(|port| match port.port_type {
            SerialPortType::UsbPort(usb) => Some(DiscoveredUsbSerialDevice {
                port_name: port.port_name.clone(),
                identity: UsbDeviceIdentity {
                    vendor_id: usb.vid,
                    product_id: usb.pid,
                    manufacturer: usb.manufacturer,
                    product: usb.product,
                    usb_serial: usb.serial_number,
                    widget_serial: None,
                    port_topology_hint: Some(port.port_name),
                },
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    devices.sort_by(|left, right| left.port_name.cmp(&right.port_name));
    Ok(devices)
}

#[derive(Default)]
pub struct SerialUsbDriverFactory;

impl UsbPlatformDriverFactory for SerialUsbDriverFactory {
    fn open(
        &self,
        endpoint: &UsbEndpointConfiguration,
    ) -> Result<Arc<dyn UsbEndpointDriver>, String> {
        let matched = resolve_device(endpoint, true)?;
        let mut stable_endpoint = endpoint.clone();
        if stable_endpoint.identity.usb_serial.is_none() {
            stable_endpoint.identity.usb_serial = matched.identity.usb_serial;
        }
        if stable_endpoint.identity.usb_serial.is_none() {
            stable_endpoint.identity.port_topology_hint = Some(matched.port_name);
        } else {
            stable_endpoint.identity.port_topology_hint = None;
        }
        // The widget identity was proven before output started. Reconnect uses the captured USB
        // serial or topology and must never send label 10 while a widget may retain DMX output.
        stable_endpoint.identity.widget_serial = None;
        Ok(Arc::new(SerialUsbEndpoint::start(stable_endpoint)?))
    }
}

struct WorkerState {
    latest: Option<UniverseFrame>,
    generation: u64,
    shutdown: bool,
}

struct SerialUsbEndpoint {
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
    join: Mutex<Option<JoinHandle<()>>>,
    done: Mutex<Receiver<()>>,
    successful_output: Arc<AtomicBool>,
    health: Arc<Mutex<UsbEndpointDriverHealth>>,
    driver: UsbEndpointDriverKind,
}

impl SerialUsbEndpoint {
    fn start(endpoint: UsbEndpointConfiguration) -> Result<Self, String> {
        let shared = Arc::new((
            Mutex::new(WorkerState {
                latest: None,
                generation: 0,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let worker_shared = Arc::clone(&shared);
        let successful_output = Arc::new(AtomicBool::new(false));
        let worker_success = Arc::clone(&successful_output);
        let health = Arc::new(Mutex::new(UsbEndpointDriverHealth::default()));
        let worker_health = Arc::clone(&health);
        let driver = endpoint.driver;
        let (done_tx, done) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name(format!("usb-dmx-{}", endpoint.endpoint_id))
            .spawn(move || {
                match endpoint.driver {
                    UsbEndpointDriverKind::OpenDmx => {
                        run_open_dmx(endpoint, worker_shared, worker_success, worker_health)
                    }
                    UsbEndpointDriverKind::EnttecUsbProV144 => {
                        run_usb_pro(endpoint, worker_shared, worker_success, worker_health)
                    }
                }
                let _ = done_tx.try_send(());
            })
            .map_err(|error| format!("cannot start USB DMX endpoint worker: {error}"))?;
        Ok(Self {
            shared,
            join: Mutex::new(Some(join)),
            done: Mutex::new(done),
            successful_output,
            health,
            driver,
        })
    }
}

impl UsbEndpointDriver for SerialUsbEndpoint {
    fn enqueue_latest(&self, frame: UniverseFrame) -> Result<(), String> {
        let (state, changed) = &*self.shared;
        let mut state = state.lock().map_err(|_| "USB worker mutex poisoned")?;
        if state.shutdown {
            return Err("USB endpoint is shutting down".into());
        }
        state.latest = Some(frame);
        state.generation = state.generation.wrapping_add(1);
        changed.notify_one();
        Ok(())
    }

    fn health(&self) -> UsbEndpointDriverHealth {
        self.health.lock().map_or_else(
            |_| UsbEndpointDriverHealth::default(),
            |health| health.clone(),
        )
    }

    fn shutdown(&self) -> UsbEndpointShutdown {
        let (state, changed) = &*self.shared;
        if let Ok(mut state) = state.lock() {
            state.shutdown = true;
            changed.notify_all();
        }
        let stopped = self
            .done
            .lock()
            .ok()
            .is_some_and(|done| done.recv_timeout(Duration::from_millis(500)).is_ok());
        if let Ok(mut join) = self.join.lock()
            && let Some(join) = join.take()
            && stopped
        {
            let _ = join.join();
        }
        if stopped
            && self.successful_output.load(Ordering::Acquire)
            && self.driver == UsbEndpointDriverKind::EnttecUsbProV144
        {
            UsbEndpointShutdown::DeviceRetainsLastFrame
        } else {
            UsbEndpointShutdown::FinalOutputUnknown
        }
    }
}

impl Drop for SerialUsbEndpoint {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_open_dmx(
    endpoint: UsbEndpointConfiguration,
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
    successful_output: Arc<AtomicBool>,
    health: Arc<Mutex<UsbEndpointDriverHealth>>,
) {
    let mut driver = OpenDmxDriver::default();
    let mut port: Option<RealSerial> = None;
    let mut applied_generation = 0;
    let mut clock = SystemClock::new();
    loop {
        let (latest, generation, shutdown) =
            snapshot(&shared, !driver.has_pending_frame(), applied_generation);
        if shutdown {
            return;
        }
        if let Some(frame) = latest
            && generation != applied_generation
        {
            driver.enqueue(frame);
            applied_generation = generation;
        }
        if port.is_none() {
            mark_reconnecting(&health, None);
            match open_matching_port(&endpoint, 250_000) {
                Ok(mut serial) => match driver.connect(&mut serial) {
                    Ok(()) => {
                        mark_online(&health);
                        port = Some(serial);
                    }
                    Err(error) => mark_reconnecting(&health, Some(error.0)),
                },
                Err(error) => mark_reconnecting(&health, Some(error)),
            }
        }
        if let Some(serial) = port.as_mut() {
            match driver.transmit_pending(serial, &mut clock) {
                Ok(true) => {
                    successful_output.store(true, Ordering::Release);
                    mark_accepted(&health);
                }
                Ok(false) => {}
                Err(error) => {
                    mark_reconnecting(&health, Some(error.0));
                    port = None;
                }
            }
        }
        if port.is_none() && wait_for_shutdown(&shared, RECONNECT_DELAY) {
            return;
        }
    }
}

fn run_usb_pro(
    endpoint: UsbEndpointConfiguration,
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
    successful_output: Arc<AtomicBool>,
    health: Arc<Mutex<UsbEndpointDriverHealth>>,
) {
    let mut driver = ProDriver::default();
    let mut port: Option<RealSerial> = None;
    let mut applied_generation = 0;
    loop {
        let (latest, generation, shutdown) = snapshot(&shared, true, applied_generation);
        if shutdown {
            return;
        }
        if let Some(frame) = latest
            && generation != applied_generation
        {
            driver.enqueue(frame);
            applied_generation = generation;
        }
        if port.is_none() {
            mark_reconnecting(&health, None);
            match open_matching_port(&endpoint, PRO_BAUD) {
                Ok(mut serial) => match driver.connect(&mut serial) {
                    Ok(()) => {
                        mark_online(&health);
                        port = Some(serial);
                    }
                    Err(error) => mark_reconnecting(&health, Some(error.0)),
                },
                Err(error) => mark_reconnecting(&health, Some(error)),
            }
        }
        if let Some(serial) = port.as_mut() {
            match driver.transmit_pending(serial) {
                Ok(true) => {
                    successful_output.store(true, Ordering::Release);
                    mark_accepted(&health);
                }
                Ok(false) => {}
                Err(error) => {
                    mark_reconnecting(&health, Some(error.0));
                    port = None;
                }
            }
        }
        if port.is_none() && wait_for_shutdown(&shared, RECONNECT_DELAY) {
            return;
        }
    }
}

fn mark_reconnecting(health: &Mutex<UsbEndpointDriverHealth>, error: Option<String>) {
    if let Ok(mut health) = health.lock() {
        health.online = false;
        health.reconnecting = true;
        health.reconnect_attempts = health.reconnect_attempts.saturating_add(1);
        if error.is_some() {
            health.last_error = error;
        }
    }
}

fn mark_online(health: &Mutex<UsbEndpointDriverHealth>) {
    if let Ok(mut health) = health.lock() {
        health.online = true;
        health.reconnecting = false;
        health.last_error = None;
    }
}

fn mark_accepted(health: &Mutex<UsbEndpointDriverHealth>) {
    if let Ok(mut health) = health.lock() {
        health.accepted_frames = health.accepted_frames.saturating_add(1);
    }
}

fn wait_for_shutdown(shared: &Arc<(Mutex<WorkerState>, Condvar)>, timeout: Duration) -> bool {
    let (state, changed) = &**shared;
    let mut state = state.lock().expect("USB worker mutex poisoned");
    if !state.shutdown {
        state = changed
            .wait_timeout(state, timeout)
            .expect("USB worker mutex poisoned")
            .0;
    }
    state.shutdown
}

fn snapshot(
    shared: &Arc<(Mutex<WorkerState>, Condvar)>,
    wait_for_frame: bool,
    applied_generation: u64,
) -> (Option<UniverseFrame>, u64, bool) {
    let (state, changed) = &**shared;
    let mut state = state.lock().expect("USB worker mutex poisoned");
    if wait_for_frame && state.generation == applied_generation && !state.shutdown {
        state = changed
            .wait_timeout(state, RECONNECT_DELAY)
            .expect("USB worker mutex poisoned")
            .0;
    }
    (state.latest.clone(), state.generation, state.shutdown)
}

fn open_matching_port(
    endpoint: &UsbEndpointConfiguration,
    baud: u32,
) -> Result<RealSerial, String> {
    let port_name = resolve_device(endpoint, false)?.port_name;
    let port = serialport::new(&port_name, baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(if endpoint.driver == UsbEndpointDriverKind::OpenDmx {
            StopBits::Two
        } else {
            StopBits::One
        })
        .flow_control(FlowControl::None)
        .timeout(IO_TIMEOUT)
        .open()
        .map_err(|error| format!("cannot open `{port_name}`: {error}"))?;
    Ok(RealSerial(port))
}

fn resolve_device(
    endpoint: &UsbEndpointConfiguration,
    allow_widget_probe: bool,
) -> Result<DiscoveredUsbSerialDevice, String> {
    if endpoint.driver == UsbEndpointDriverKind::OpenDmx && endpoint.identity.vendor_id != 0x0403 {
        return Err("Open DMX core support is limited to FTDI VCP identities".into());
    }
    let mut candidates = discover_usb_serial_devices()?
        .into_iter()
        .filter(|candidate| identity_matches(&endpoint.identity, candidate))
        .collect::<Vec<_>>();
    if allow_widget_probe && endpoint.driver == UsbEndpointDriverKind::EnttecUsbProV144 {
        let expected_widget_serial = endpoint.identity.widget_serial.as_deref();
        candidates = candidates
            .into_iter()
            .filter_map(|mut candidate| {
                let probed = probe_usb_pro(&candidate.port_name, expected_widget_serial.is_some())?;
                if let Some(expected) = expected_widget_serial
                    && probed.as_deref() != Some(expected)
                {
                    return None;
                }
                candidate.identity.widget_serial = probed;
                Some(candidate)
            })
            .collect();
    } else if endpoint.identity.widget_serial.is_some()
        && endpoint.driver != UsbEndpointDriverKind::EnttecUsbProV144
    {
        return Err("widget serial matching is only valid for USB Pro endpoints".into());
    }
    deduplicate_macos_pairs(&mut candidates);
    match candidates.as_slice() {
        [candidate] => Ok(candidate.clone()),
        [] => Err(format!(
            "no serial device matches USB endpoint `{}`",
            endpoint.endpoint_id
        )),
        _ => Err(format!(
            "USB endpoint `{}` is ambiguous across {} matching serial devices",
            endpoint.endpoint_id,
            candidates.len()
        )),
    }
}

fn identity_matches(expected: &UsbDeviceIdentity, candidate: &DiscoveredUsbSerialDevice) -> bool {
    let actual = &candidate.identity;
    expected.vendor_id == actual.vendor_id
        && expected.product_id == actual.product_id
        && expected
            .usb_serial
            .as_ref()
            .is_none_or(|value| actual.usb_serial.as_ref() == Some(value))
        && (expected.usb_serial.is_some()
            || expected.widget_serial.is_some()
            || expected
                .port_topology_hint
                .as_ref()
                .is_some_and(|value| &candidate.port_name == value))
}

fn deduplicate_macos_pairs(candidates: &mut Vec<DiscoveredUsbSerialDevice>) {
    let callouts = candidates
        .iter()
        .filter(|candidate| candidate.port_name.starts_with("/dev/cu."))
        .cloned()
        .collect::<Vec<_>>();
    candidates.retain(|candidate| {
        !candidate.port_name.starts_with("/dev/tty.")
            || !callouts
                .iter()
                .any(|callout| same_usb_identity(callout, candidate))
    });
}

fn same_usb_identity(left: &DiscoveredUsbSerialDevice, right: &DiscoveredUsbSerialDevice) -> bool {
    (left.identity.usb_serial.is_some() || left.identity.widget_serial.is_some())
        && left.identity.vendor_id == right.identity.vendor_id
        && left.identity.product_id == right.identity.product_id
        && left.identity.usb_serial == right.identity.usb_serial
        && left.identity.widget_serial == right.identity.widget_serial
        && left.identity.manufacturer == right.identity.manufacturer
        && left.identity.product == right.identity.product
}

fn probe_usb_pro(port_name: &str, read_serial: bool) -> Option<Option<String>> {
    let mut port = serialport::new(port_name, PRO_BAUD)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .timeout(IO_TIMEOUT)
        .open()
        .ok()?;
    port.write_all(&encode_pro_frame(ProLabel::GetParameters, &[]).ok()?)
        .ok()?;
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut parser = ProFrameParser::default();
    let mut buffer = [0_u8; 64];
    while Instant::now() < deadline {
        match port.read(&mut buffer) {
            Ok(count) => {
                for frame in parser.push(&buffer[..count]) {
                    if frame.label == ProLabel::GetParameters
                        && light_usb_dmx_core::WidgetParameters::decode_reply(&frame.payload)
                            .is_ok()
                    {
                        if !read_serial {
                            return Some(None);
                        }
                        port.write_all(&encode_pro_frame(ProLabel::GetSerial, &[]).ok()?)
                            .ok()?;
                        let serial_deadline = Instant::now() + IO_TIMEOUT;
                        while Instant::now() < serial_deadline {
                            match port.read(&mut buffer) {
                                Ok(serial_count) => {
                                    for serial_frame in parser.push(&buffer[..serial_count]) {
                                        if serial_frame.label == ProLabel::GetSerial {
                                            return Some(decode_widget_serial(
                                                &serial_frame.payload,
                                            ));
                                        }
                                    }
                                }
                                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
                                Err(_) => return None,
                            }
                        }
                        return None;
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => return None,
        }
    }
    None
}

struct RealSerial(Box<dyn SerialPort>);

impl OpenDmxSerial for RealSerial {
    fn configure(&mut self, configuration: SerialConfiguration) -> Result<(), TransportError> {
        self.0
            .set_baud_rate(configuration.baud)
            .and_then(|()| self.0.set_data_bits(DataBits::Eight))
            .and_then(|()| self.0.set_parity(Parity::None))
            .and_then(|()| self.0.set_stop_bits(StopBits::Two))
            .and_then(|()| self.0.set_flow_control(FlowControl::None))
            .map_err(serial_error)
    }

    fn set_break(&mut self, enabled: bool) -> Result<(), TransportError> {
        if enabled {
            self.0.set_break()
        } else {
            self.0.clear_break()
        }
        .map_err(serial_error)
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        Write::write_all(&mut self.0, bytes).map_err(io_error)
    }

    fn wait_transmitted(&mut self) -> Result<(), TransportError> {
        Write::flush(&mut self.0).map_err(io_error)?;
        let deadline = Instant::now() + IO_TIMEOUT;
        loop {
            if self.0.bytes_to_write().map_err(serial_error)? == 0 {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(TransportError(
                    "serial transmit queue did not drain before the next Open DMX frame".into(),
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

impl ProSerial for RealSerial {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        Write::write_all(&mut self.0, bytes).map_err(io_error)
    }
}

fn serial_error(error: serialport::Error) -> TransportError {
    TransportError(error.to_string())
}

fn io_error(error: std::io::Error) -> TransportError {
    TransportError(error.to_string())
}

struct SystemClock {
    epoch: Instant,
}

impl SystemClock {
    fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now_micros(&self) -> u64 {
        self.epoch
            .elapsed()
            .as_micros()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn sleep_until_micros(&mut self, deadline_micros: u64) {
        let now = self.now_micros();
        if deadline_micros > now {
            thread::sleep(Duration::from_micros(deadline_micros - now));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(port: &str, serial: Option<&str>) -> DiscoveredUsbSerialDevice {
        DiscoveredUsbSerialDevice {
            port_name: port.into(),
            identity: UsbDeviceIdentity {
                vendor_id: 0x0403,
                product_id: 0x6001,
                manufacturer: Some("FTDI".into()),
                product: Some("DMX".into()),
                usb_serial: serial.map(str::to_owned),
                widget_serial: None,
                port_topology_hint: Some(port.into()),
            },
        }
    }

    #[test]
    fn exact_identity_never_falls_back_to_another_serial() {
        let expected = UsbDeviceIdentity {
            usb_serial: Some("A".into()),
            port_topology_hint: None,
            ..candidate("COM4", Some("A")).identity
        };
        assert!(identity_matches(&expected, &candidate("COM4", Some("A"))));
        assert!(!identity_matches(&expected, &candidate("COM5", Some("B"))));
    }

    #[test]
    fn strong_usb_serial_survives_a_port_name_change() {
        let expected = UsbDeviceIdentity {
            usb_serial: Some("A".into()),
            port_topology_hint: Some("COM4".into()),
            ..candidate("COM4", Some("A")).identity
        };
        assert!(identity_matches(&expected, &candidate("COM19", Some("A"))));
        assert!(!identity_matches(&expected, &candidate("COM4", Some("B"))));
    }

    #[test]
    fn topology_only_identity_never_follows_a_reassigned_port_name() {
        let expected = UsbDeviceIdentity {
            usb_serial: None,
            widget_serial: None,
            port_topology_hint: Some("/dev/cu.usb-a".into()),
            ..candidate("/dev/cu.usb-a", None).identity
        };
        assert!(identity_matches(
            &expected,
            &candidate("/dev/cu.usb-a", None)
        ));
        assert!(!identity_matches(
            &expected,
            &candidate("/dev/cu.usb-b", None)
        ));
    }

    #[test]
    fn macos_callout_and_dial_in_names_for_one_usb_device_are_not_ambiguous() {
        let mut candidates = vec![
            candidate("/dev/tty.usbserial-A", Some("A")),
            candidate("/dev/cu.usbserial-A", Some("A")),
        ];
        deduplicate_macos_pairs(&mut candidates);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].port_name, "/dev/cu.usbserial-A");
    }

    #[test]
    fn macos_deduplicates_each_of_multiple_physical_pairs() {
        let mut candidates = vec![
            candidate("/dev/tty.usbserial-A", Some("A")),
            candidate("/dev/cu.usbserial-A", Some("A")),
            candidate("/dev/tty.usbserial-B", Some("B")),
            candidate("/dev/cu.usbserial-B", Some("B")),
        ];
        deduplicate_macos_pairs(&mut candidates);
        assert_eq!(candidates.len(), 2);
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.port_name.starts_with("/dev/cu."))
        );
    }

    #[test]
    fn worker_wait_is_edge_woken_and_open_dmx_can_repeat_without_a_quarter_second_delay() {
        let shared = Arc::new((
            Mutex::new(WorkerState {
                latest: Some(UniverseFrame::new([1; 512])),
                generation: 1,
                shutdown: false,
            }),
            Condvar::new(),
        ));
        let started = Instant::now();
        let (_, generation, _) = snapshot(&shared, false, 1);
        assert_eq!(generation, 1);
        assert!(started.elapsed() < Duration::from_millis(25));

        let changed = Arc::clone(&shared);
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            let (state, wake) = &*changed;
            let mut state = state.lock().unwrap();
            state.generation = 2;
            wake.notify_one();
        });
        let started = Instant::now();
        let (_, generation, _) = snapshot(&shared, true, 1);
        writer.join().unwrap();
        assert_eq!(generation, 2);
        assert!(started.elapsed() < RECONNECT_DELAY);
    }
}
