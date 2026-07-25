use crate::{DMX_SLOTS, DmxFrame, OutputRoute, Protocol, artdmx_packet, sacn_data_packet};
use light_core::Universe;
use std::{collections::HashMap, io, net::SocketAddr};

#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub protocol: Protocol,
    pub universe: Universe,
    pub destination: SocketAddr,
    pub bytes: Vec<u8>,
}

pub fn encode_routes(
    routes: &[OutputRoute],
    frames: &HashMap<Universe, DmxFrame>,
    patched_slots: &HashMap<Universe, u16>,
    sequences: &mut HashMap<(Protocol, Universe), u8>,
    cid: [u8; 16],
    source_name: &str,
    sacn_priority: u8,
) -> io::Result<Vec<EncodedPacket>> {
    routes
        .iter()
        .filter(|route| route.enabled)
        .map(|route| {
            encode_route(
                route,
                frames,
                patched_slots,
                sequences,
                cid,
                source_name,
                sacn_priority,
            )
        })
        .collect()
}

pub fn next_sequence(
    sequences: &mut HashMap<(Protocol, Universe), u8>,
    key: (Protocol, Universe),
) -> u8 {
    let sequence = sequences.entry(key).or_insert(0);
    *sequence = sequence.wrapping_add(1);
    if *sequence == 0 {
        *sequence = 1;
    }
    *sequence
}

#[allow(clippy::too_many_arguments)]
fn encode_route(
    route: &OutputRoute,
    frames: &HashMap<Universe, DmxFrame>,
    patched_slots: &HashMap<Universe, u16>,
    sequences: &mut HashMap<(Protocol, Universe), u8>,
    cid: [u8; 16],
    source_name: &str,
    sacn_priority: u8,
) -> io::Result<EncodedPacket> {
    let empty = [0; DMX_SLOTS];
    let frame = frames.get(&route.logical_universe).unwrap_or(&empty);
    let slot_count = payload_slot_count(route, patched_slots);
    let sequence = next_sequence(sequences, sequence_key(route));
    let destination = route.resolved_destination().map_err(io::Error::other)?;
    let bytes = encode_payload(
        route,
        sequence,
        &frame[..slot_count],
        cid,
        source_name,
        sacn_priority,
    );
    Ok(EncodedPacket {
        protocol: route.protocol,
        universe: route.destination_universe,
        destination,
        bytes,
    })
}

fn payload_slot_count(route: &OutputRoute, patched_slots: &HashMap<Universe, u16>) -> usize {
    let minimum = usize::from(route.minimum_slots.clamp(1, DMX_SLOTS as u16));
    let patched = usize::from(
        patched_slots
            .get(&route.logical_universe)
            .copied()
            .unwrap_or_default(),
    );
    let slot_count = minimum.max(patched).min(DMX_SLOTS);
    match (route.protocol, slot_count % 2) {
        (Protocol::ArtNet, 1) => (slot_count + 1).min(DMX_SLOTS),
        _ => slot_count,
    }
}

fn sequence_key(route: &OutputRoute) -> (Protocol, Universe) {
    (route.protocol, route.destination_universe)
}

fn encode_payload(
    route: &OutputRoute,
    sequence: u8,
    payload: &[u8],
    cid: [u8; 16],
    source_name: &str,
    sacn_priority: u8,
) -> Vec<u8> {
    match route.protocol {
        Protocol::ArtNet => artdmx_packet(route.destination_universe, sequence, payload),
        Protocol::Sacn => sacn_data_packet(
            route.destination_universe,
            sequence,
            payload,
            cid,
            source_name,
            sacn_priority,
            false,
        ),
    }
}
