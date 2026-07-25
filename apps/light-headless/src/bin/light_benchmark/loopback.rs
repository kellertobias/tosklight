use light_output::EncodedPacket;
use serde::Serialize;
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct LoopbackSummary {
    pub datagrams_received: u64,
    pub bytes_received: u64,
}

pub struct LoopbackDelivery {
    sender: UdpSocket,
    destination: SocketAddr,
    stop: Arc<AtomicBool>,
    datagrams_received: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
    receiver: Option<JoinHandle<()>>,
}

impl LoopbackDelivery {
    pub fn start() -> io::Result<Self> {
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
        receiver.set_read_timeout(Some(Duration::from_millis(50)))?;
        let destination = receiver.local_addr()?;
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
        let stop = Arc::new(AtomicBool::new(false));
        let datagrams_received = Arc::new(AtomicU64::new(0));
        let bytes_received = Arc::new(AtomicU64::new(0));
        let receiver_task = spawn_receiver(
            receiver,
            Arc::clone(&stop),
            Arc::clone(&datagrams_received),
            Arc::clone(&bytes_received),
        );
        Ok(Self {
            sender,
            destination,
            stop,
            datagrams_received,
            bytes_received,
            receiver: Some(receiver_task),
        })
    }

    pub fn destination(&self) -> SocketAddr {
        self.destination
    }

    pub fn send(&self, packets: &[EncodedPacket]) -> io::Result<()> {
        for packet in packets {
            self.sender.send_to(&packet.bytes, packet.destination)?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> LoopbackSummary {
        self.stop_and_join();
        LoopbackSummary {
            datagrams_received: self.datagrams_received.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
        }
    }

    fn stop_and_join(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.sender.send_to(&[], self.destination);
        if let Some(receiver) = self.receiver.take() {
            let _ = receiver.join();
        }
    }
}

impl Drop for LoopbackDelivery {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

fn spawn_receiver(
    receiver: UdpSocket,
    stop: Arc<AtomicBool>,
    datagrams_received: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 65_535];
        loop {
            match receiver.recv_from(&mut buffer) {
                Ok((0, _)) if stop.load(Ordering::Relaxed) => break,
                Ok((0, _)) => {}
                Ok((bytes, _)) => {
                    datagrams_received.fetch_add(1, Ordering::Relaxed);
                    bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    })
}
