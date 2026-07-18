use crate::{DMX_SLOTS, DmxFrame, artdmx_packet, sacn_data_packet, sacn_multicast_destination};
use async_trait::async_trait;
use light_core::Universe;
use std::{
    io,
    net::{IpAddr, SocketAddr},
};
use tokio::net::UdpSocket;

#[async_trait]
pub trait OutputDriver: Send + Sync {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()>;

    async fn terminate(&self, universe: Universe, sequence: u8) -> io::Result<()> {
        self.send(universe, sequence, &[0; DMX_SLOTS]).await
    }
}

pub struct ArtNetDriver {
    socket: UdpSocket,
    destination: SocketAddr,
}

impl ArtNetDriver {
    pub async fn bind(bind: SocketAddr, destination: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind).await?;
        if matches!(destination.ip(), IpAddr::V4(address) if address.is_broadcast()) {
            socket.set_broadcast(true)?;
        }
        Ok(Self {
            socket,
            destination,
        })
    }
}

#[async_trait]
impl OutputDriver for ArtNetDriver {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()> {
        let packet = artdmx_packet(universe, sequence, frame);
        self.socket.send_to(&packet, self.destination).await?;
        Ok(())
    }
}

pub struct SacnDriver {
    socket: UdpSocket,
    cid: [u8; 16],
    source_name: String,
    priority: u8,
    destination: Option<SocketAddr>,
}

impl SacnDriver {
    pub async fn bind(
        bind: SocketAddr,
        cid: [u8; 16],
        source_name: impl Into<String>,
        priority: u8,
        destination: Option<SocketAddr>,
    ) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(bind).await?,
            cid,
            source_name: source_name.into(),
            priority,
            destination,
        })
    }

    fn destination_for(&self, universe: Universe) -> SocketAddr {
        self.destination
            .unwrap_or_else(|| sacn_multicast_destination(universe))
    }

    fn packet(&self, universe: Universe, sequence: u8, terminated: bool) -> Vec<u8> {
        sacn_data_packet(
            universe,
            sequence,
            &[0; DMX_SLOTS],
            self.cid,
            &self.source_name,
            self.priority,
            terminated,
        )
    }
}

#[async_trait]
impl OutputDriver for SacnDriver {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()> {
        let packet = sacn_data_packet(
            universe,
            sequence,
            frame,
            self.cid,
            &self.source_name,
            self.priority,
            false,
        );
        self.socket
            .send_to(&packet, self.destination_for(universe))
            .await?;
        Ok(())
    }

    async fn terminate(&self, universe: Universe, sequence: u8) -> io::Result<()> {
        let packet = self.packet(universe, sequence, true);
        for _ in 0..3 {
            self.socket
                .send_to(&packet, self.destination_for(universe))
                .await?;
        }
        Ok(())
    }
}
