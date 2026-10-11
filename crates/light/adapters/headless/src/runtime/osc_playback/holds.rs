//! Ingress receipts only: the Playback engine remains the authority for held output.
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(in crate::runtime) struct HoldKey {
    pub desk_id: Uuid,
    pub surface_id: String,
    pub control_id: u16,
    pub route: Option<String>,
}
#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) struct HeldPlayback {
    pub address: light_playback::PlaybackAddress,
    pub action: &'static str,
    pub button: Option<u8>,
}
#[derive(Default)]
pub(in crate::runtime) struct OscPlaybackHolds {
    epoch: u64,
    pub receipts: HashMap<HoldKey, HeldPlayback>,
}
impl OscPlaybackHolds {
    pub fn synchronize(&mut self, epoch: u64) {
        if self.epoch != epoch {
            self.receipts.clear();
            self.epoch = epoch;
        }
    }
    pub fn remove_surface(&mut self, desk_id: Uuid, surface_id: &str) {
        self.receipts
            .retain(|key, _| key.desk_id != desk_id || key.surface_id != surface_id);
    }
}

pub(super) fn held_action(
    state: &super::AppState,
    address: light_playback::PlaybackAddress,
    action: &str,
    input: &super::PoolPlaybackInput,
) -> Option<&'static str> {
    let snapshot = state.output.snapshot();
    let definition = snapshot
        .playback_pages
        .iter()
        .find(|page| page.number == address.page())?
        .assignments
        .get(&address.position())?;
    match super::super::requested_playback_button_action(definition, action, input)
        .ok()
        .flatten()?
    {
        light_playback::PlaybackButtonAction::Flash => Some("flash"),
        light_playback::PlaybackButtonAction::Swap => Some("swap"),
        _ => None,
    }
}

pub(super) struct CapturedInput<'a> {
    pub held: parking_lot::MutexGuard<'a, OscPlaybackHolds>,
    pub key: HoldKey,
    pub epoch: u64,
    pub receipt: Option<HeldPlayback>,
    pub source: light_playback::PlaybackControlSource,
}
impl<'a> CapturedInput<'a> {
    pub fn begin(
        state: &'a super::AppState,
        address: &mut super::PlaybackAddress,
        input: &super::PoolPlaybackInput,
        arguments: &[super::OscArgument],
        desk: &super::ControlDesk,
        origin: (Option<std::net::SocketAddr>, Option<&str>, &str),
    ) -> Option<Self> {
        let control_id = super::osc_playback_control_id(address, input, arguments, origin.2)?;
        let source = light_playback::PlaybackControlSource {
            surface_id: origin
                .0
                .and_then(|socket| state.integrations.osc_control_source_for_source(socket))
                .unwrap_or_else(|| format!("osc:{}", origin.1.unwrap_or("local"))),
            control_id,
        };
        let route = if arguments.get(1).is_some() {
            None
        } else {
            let selector = match address {
                super::PlaybackAddress::Playback(address) => {
                    format!("explicit:{}.{}", address.page(), address.position())
                }
                super::PlaybackAddress::CurrentPage { position } => format!("current:{position}"),
                _ => return None,
            };
            Some(format!(
                "{selector}/{}/{}",
                origin.2,
                input.button.unwrap_or(0)
            ))
        };
        let key = HoldKey {
            route,
            desk_id: desk.id,
            surface_id: source.surface_id.clone(),
            control_id,
        };
        let held = state.integrations.osc_playback_holds();
        let epoch = state.integrations.osc_playback_epoch();
        let receipt = held.receipts.get(&key).copied();
        if receipt.is_none()
            && let super::PlaybackAddress::CurrentPage { position } = address
        {
            let page = state.installation.command_page(desk.id).ok()?;
            *address = super::PlaybackAddress::Playback(
                light_playback::PlaybackAddress::new(page, *position).ok()?,
            );
        }
        Some(Self {
            held,
            key,
            epoch,
            receipt,
            source,
        })
    }
}
