use crate::{TransportError, UniverseFrame};

pub type DmxFrame = UniverseFrame;

pub const OPEN_DMX_BREAK_US: u64 = 92;
pub const OPEN_DMX_MAB_US: u64 = 12;
pub const OPEN_DMX_FRAME_INTERVAL_US: u64 = 25_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SerialConfiguration {
    pub baud: u32,
    pub data_bits: u8,
    pub parity: Parity,
    pub stop_bits: u8,
    pub flow_control: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Parity {
    None,
}

impl SerialConfiguration {
    pub const OPEN_DMX: Self = Self {
        baud: 250_000,
        data_bits: 8,
        parity: Parity::None,
        stop_bits: 2,
        flow_control: false,
    };
}

pub trait Clock {
    fn now_micros(&self) -> u64;
    fn sleep_until_micros(&mut self, deadline_micros: u64);
}

pub trait OpenDmxSerial {
    fn configure(&mut self, configuration: SerialConfiguration) -> Result<(), TransportError>;
    fn set_break(&mut self, enabled: bool) -> Result<(), TransportError>;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Wait until the operating-system transmit queue is empty before the next BREAK.
    fn wait_transmitted(&mut self) -> Result<(), TransportError>;
}

/// Host-timed Open DMX sender with a single replaceable pending slot.
#[derive(Clone, Debug, Default)]
pub struct OpenDmxDriver {
    latest: Option<UniverseFrame>,
    next_frame_at: Option<u64>,
}

impl OpenDmxDriver {
    pub fn enqueue(&mut self, frame: UniverseFrame) {
        self.latest = Some(frame);
    }

    /// Open DMX has no device-side buffer, so an authoritative frame remains pending forever and
    /// is emitted on every paced worker iteration until replaced.
    pub fn has_pending_frame(&self) -> bool {
        self.latest.is_some()
    }

    pub fn connect(&mut self, serial: &mut impl OpenDmxSerial) -> Result<(), TransportError> {
        serial.configure(SerialConfiguration::OPEN_DMX)?;
        // Reconnect must resend the newest authoritative frame.
        self.next_frame_at = None;
        Ok(())
    }

    pub fn transmit_pending(
        &mut self,
        serial: &mut impl OpenDmxSerial,
        clock: &mut impl Clock,
    ) -> Result<bool, TransportError> {
        let Some(frame) = self.latest.as_ref() else {
            return Ok(false);
        };
        let now = clock.now_micros();
        // A delayed worker starts a fresh correctly timed frame; it never tries to catch up by
        // compressing BREAK or MAB against deadlines which are already in the past.
        let frame_start = self.next_frame_at.unwrap_or(now).max(now);
        clock.sleep_until_micros(frame_start);

        // Any failure leaves the latest frame retained, so reconnect retries the newest state.
        serial.set_break(true)?;
        let break_started_at = clock.now_micros();
        clock.sleep_until_micros(break_started_at.saturating_add(OPEN_DMX_BREAK_US));
        serial.set_break(false)?;
        let mab_started_at = clock.now_micros();
        clock.sleep_until_micros(mab_started_at.saturating_add(OPEN_DMX_MAB_US));
        serial.write_all(&frame.wire_payload())?;
        serial.wait_transmitted()?;

        self.next_frame_at = Some(frame_start.saturating_add(OPEN_DMX_FRAME_INTERVAL_US));
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    #[derive(Default)]
    struct FakeClock {
        now: u64,
        sleeps: Vec<u64>,
    }

    impl Clock for FakeClock {
        fn now_micros(&self) -> u64 {
            self.now
        }

        fn sleep_until_micros(&mut self, deadline_micros: u64) {
            self.now = self.now.max(deadline_micros);
            self.sleeps.push(deadline_micros);
        }
    }

    #[derive(Default)]
    struct FakeSerial {
        configuration: Option<SerialConfiguration>,
        breaks: Vec<bool>,
        writes: Vec<Vec<u8>>,
        fail_operation: Option<usize>,
        operation: usize,
    }

    impl FakeSerial {
        fn checkpoint(&mut self) -> Result<(), TransportError> {
            self.operation += 1;
            if self.fail_operation == Some(self.operation) {
                Err(TransportError("unplugged".into()))
            } else {
                Ok(())
            }
        }
    }

    impl OpenDmxSerial for FakeSerial {
        fn configure(&mut self, configuration: SerialConfiguration) -> Result<(), TransportError> {
            self.checkpoint()?;
            self.configuration = Some(configuration);
            Ok(())
        }

        fn set_break(&mut self, enabled: bool) -> Result<(), TransportError> {
            self.checkpoint()?;
            self.breaks.push(enabled);
            Ok(())
        }

        fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            self.checkpoint()?;
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn wait_transmitted(&mut self) -> Result<(), TransportError> {
            self.checkpoint()
        }
    }

    fn frame(value: u8) -> UniverseFrame {
        UniverseFrame::new([value; 512])
    }

    #[test]
    fn configures_250k_8n2_without_flow_control_and_emits_complete_frame() {
        let mut driver = OpenDmxDriver::default();
        let mut serial = FakeSerial::default();
        let mut clock = FakeClock::default();
        driver.connect(&mut serial).unwrap();
        driver.enqueue(frame(0x5a));

        assert!(driver.transmit_pending(&mut serial, &mut clock).unwrap());
        assert_eq!(serial.configuration, Some(SerialConfiguration::OPEN_DMX));
        assert_eq!(serial.operation, 5, "frame write must be drained");
        assert_eq!(clock.sleeps, vec![0, 92, 104]);
        assert_eq!(serial.breaks, vec![true, false]);
        assert_eq!(serial.writes[0].len(), 513);
        assert_eq!(serial.writes[0][0], 0);
        assert!(serial.writes[0][1..].iter().all(|byte| *byte == 0x5a));
    }

    #[test]
    fn paces_at_40_hz_and_coalesces_obsolete_frames() {
        let mut driver = OpenDmxDriver::default();
        let mut serial = FakeSerial::default();
        let mut clock = FakeClock::default();
        driver.connect(&mut serial).unwrap();
        driver.enqueue(frame(1));
        driver.enqueue(frame(2));
        driver.enqueue(frame(3));
        driver.transmit_pending(&mut serial, &mut clock).unwrap();
        driver.transmit_pending(&mut serial, &mut clock).unwrap();
        driver.enqueue(frame(4));
        driver.transmit_pending(&mut serial, &mut clock).unwrap();

        assert_eq!(serial.writes.len(), 3);
        assert_eq!(serial.writes[0][1], 3);
        assert_eq!(serial.writes[1][1], 3);
        assert_eq!(serial.writes[2][1], 4);
        assert_eq!(clock.sleeps[3], 25_000);
        assert_eq!(clock.sleeps[6], 50_000);
    }

    #[test]
    fn delayed_worker_never_compresses_break_or_mab_to_catch_up() {
        let mut driver = OpenDmxDriver::default();
        let mut serial = FakeSerial::default();
        let mut clock = FakeClock::default();
        driver.connect(&mut serial).unwrap();
        driver.enqueue(frame(1));
        driver.transmit_pending(&mut serial, &mut clock).unwrap();
        clock.now = 100_000;
        driver.transmit_pending(&mut serial, &mut clock).unwrap();
        assert_eq!(&clock.sleeps[3..], &[100_000, 100_092, 100_104]);
    }

    #[test]
    fn serial_ioctl_latency_cannot_shorten_break_or_mark_after_break() {
        struct LagClock {
            now: Arc<AtomicU64>,
            sleeps: Vec<u64>,
        }
        impl Clock for LagClock {
            fn now_micros(&self) -> u64 {
                self.now.load(Ordering::Acquire)
            }
            fn sleep_until_micros(&mut self, deadline_micros: u64) {
                self.now.fetch_max(deadline_micros, Ordering::AcqRel);
                self.sleeps.push(deadline_micros);
            }
        }
        struct LagSerial(Arc<AtomicU64>);
        impl OpenDmxSerial for LagSerial {
            fn configure(
                &mut self,
                _configuration: SerialConfiguration,
            ) -> Result<(), TransportError> {
                Ok(())
            }
            fn set_break(&mut self, enabled: bool) -> Result<(), TransportError> {
                self.0
                    .fetch_add(if enabled { 50 } else { 30 }, Ordering::AcqRel);
                Ok(())
            }
            fn write_all(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
                Ok(())
            }
            fn wait_transmitted(&mut self) -> Result<(), TransportError> {
                Ok(())
            }
        }

        let now = Arc::new(AtomicU64::new(0));
        let mut clock = LagClock {
            now: Arc::clone(&now),
            sleeps: Vec::new(),
        };
        let mut serial = LagSerial(now);
        let mut driver = OpenDmxDriver::default();
        driver.connect(&mut serial).unwrap();
        driver.enqueue(UniverseFrame::new([0; 512]));
        driver.transmit_pending(&mut serial, &mut clock).unwrap();

        assert_eq!(clock.sleeps, vec![0, 142, 184]);
    }

    #[test]
    fn unplug_at_each_break_or_write_boundary_keeps_latest_pending_for_reconnect() {
        // connect is operation 1; transmit operations are BREAK on/off, write, and drain.
        for failing_operation in 2..=5 {
            let mut driver = OpenDmxDriver::default();
            let mut serial = FakeSerial::default();
            let mut clock = FakeClock::default();
            driver.connect(&mut serial).unwrap();
            driver.enqueue(frame(failing_operation as u8));
            serial.fail_operation = Some(failing_operation);
            assert!(driver.transmit_pending(&mut serial, &mut clock).is_err());
            assert!(driver.has_pending_frame());

            let mut reconnected = FakeSerial::default();
            driver.connect(&mut reconnected).unwrap();
            assert!(
                driver
                    .transmit_pending(&mut reconnected, &mut clock)
                    .unwrap()
            );
            assert_eq!(reconnected.writes[0][1], failing_operation as u8);
        }
    }
}
