import type {
	NetworkEndpoint,
	NetworkEndpointsSnapshot,
} from "../features/dmxDiagnostics/networkEndpoints";
import type {
	NetworkEndpoint as NetworkEndpointWire,
	NetworkEndpointsSnapshot as NetworkEndpointsSnapshotWire,
} from "./generated/light-wire";

function decodeNetworkEndpoint(endpoint: NetworkEndpointWire): NetworkEndpoint {
	return {
		id: endpoint.id,
		protocol: endpoint.protocol,
		direction: endpoint.direction,
		origin: endpoint.origin,
		role: endpoint.role,
		endpoint: endpoint.endpoint,
		name: endpoint.name ?? null,
		deliveryMode: endpoint.delivery_mode ?? null,
		logicalUniverse: endpoint.logical_universe ?? null,
		universes: endpoint.universes ?? [],
		status: endpoint.status,
		detail: endpoint.detail,
		errors: endpoint.errors,
		lastActivityMillisAgo: endpoint.last_activity_millis_ago ?? null,
	};
}

/** Maps the network-endpoints wire snapshot into the DMX diagnostics model. */
export function decodeNetworkEndpointsSnapshot(
	snapshot: NetworkEndpointsSnapshotWire,
): NetworkEndpointsSnapshot {
	return {
		outputBindIp: snapshot.output_bind_ip,
		networkOutputAvailable: snapshot.network_output_available,
		endpoints: (snapshot.endpoints ?? []).map(decodeNetworkEndpoint),
	};
}
