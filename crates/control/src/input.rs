use crate::{ControlEvent, parse_art_timecode, parse_osc_message};
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[async_trait]
pub trait ControlInput: Send {
    async fn next_event(&mut self) -> Option<ControlEvent>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UdpInputProtocol {
    Osc,
    ArtTimeCode,
}

pub struct UdpControlInput {
    socket: UdpSocket,
    protocol: UdpInputProtocol,
    buffer: Vec<u8>,
}

impl UdpControlInput {
    pub async fn bind(address: SocketAddr, protocol: UdpInputProtocol) -> std::io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(address).await?,
            protocol,
            buffer: vec![0; 65_535],
        })
    }
}

#[async_trait]
impl ControlInput for UdpControlInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        loop {
            let (length, source) = self.socket.recv_from(&mut self.buffer).await.ok()?;
            let result = match self.protocol {
                UdpInputProtocol::Osc => {
                    parse_osc_message(&self.buffer[..length]).map(|event| match event {
                        ControlEvent::Osc {
                            address, arguments, ..
                        } => ControlEvent::Osc {
                            address,
                            arguments,
                            source: Some(source.to_string()),
                        },
                        event => event,
                    })
                }
                UdpInputProtocol::ArtTimeCode => {
                    parse_art_timecode(&self.buffer[..length], &source.to_string())
                        .map(ControlEvent::Timecode)
                }
            };
            if let Ok(event) = result {
                return Some(event);
            }
        }
    }
}
