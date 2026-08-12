//! Periodic CITP identity sent to every configured Media Server.

use super::AppState;
use light_fixture::DirectControlProtocol;
use std::{collections::HashSet, net::SocketAddr, time::Duration};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(1);

pub(super) async fn run(state: AppState, cancellation: CancellationToken) -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|error| {
        anyhow::anyhow!("Light Desk could not open its CITP identity socket: {error}")
    })?;
    let mut ticker = tokio::time::interval(ANNOUNCE_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => return Ok(()),
            _ = ticker.tick() => announce(&socket, &state).await,
        }
    }
}

async fn announce(socket: &UdpSocket, state: &AppState) {
    let Some(show) = state.active_show.current() else {
        return;
    };
    let packet = light_media::console_announcement(&show.name);
    let endpoints = state
        .output
        .snapshot()
        .fixtures
        .iter()
        .filter_map(|fixture| fixture.direct_control.as_ref())
        .filter(|endpoint| endpoint.protocol == DirectControlProtocol::Citp)
        .map(|endpoint| SocketAddr::new(endpoint.ip_address, endpoint.port))
        .collect::<HashSet<_>>();
    for endpoint in endpoints {
        if let Err(error) = socket.send_to(&packet, endpoint).await {
            tracing::debug!(%endpoint, %error, "Light Desk CITP identity could not be sent");
        }
    }
}
