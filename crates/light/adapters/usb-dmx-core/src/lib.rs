#![forbid(unsafe_code)]
//! Transport-neutral protocol core for the two documented USB-DMX families supported by
//! ToskLight. OS serial discovery and I/O adapters deliberately live outside this crate.

mod open_dmx;
mod usb_pro;

pub use open_dmx::{
    Clock, DmxFrame, OPEN_DMX_BREAK_US, OPEN_DMX_FRAME_INTERVAL_US, OPEN_DMX_MAB_US, OpenDmxDriver,
    OpenDmxSerial, Parity, SerialConfiguration,
};
pub use usb_pro::{
    ProDriver, ProFrame, ProFrameParser, ProLabel, ProProtocolError, ProSerial, RequestError,
    WidgetParameterReply, WidgetParameters, decode_widget_serial, encode_pro_frame,
};

/// One complete DMX universe. Core USB output always transmits start code zero and all 512 slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UniverseFrame([u8; 512]);

impl UniverseFrame {
    pub const BLACKOUT: Self = Self([0; 512]);

    pub const fn new(slots: [u8; 512]) -> Self {
        Self(slots)
    }

    pub const fn slots(&self) -> &[u8; 512] {
        &self.0
    }

    fn wire_payload(&self) -> [u8; 513] {
        let mut payload = [0; 513];
        payload[1..].copy_from_slice(&self.0);
        payload
    }
}

/// A transport failure carries no OS-specific error type across the core boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportError(pub String);

impl From<&str> for TransportError {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
