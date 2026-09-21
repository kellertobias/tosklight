/** Art-Net and sACN endpoints the desk sends to or hears from, as the DMX window shows them. */
export type NetworkEndpointProtocol = "art_net" | "sacn";
export type NetworkEndpointDirection = "send" | "receive";
export type NetworkEndpointOrigin = "configured" | "observed";
export type NetworkEndpointDeliveryMode = "broadcast" | "multicast" | "unicast";
export type NetworkEndpointStatus =
	| "active"
	| "listening"
	| "idle"
	| "disabled"
	| "conflict"
	| "error"
	| "unavailable";

export interface NetworkEndpoint {
	/** Stable within one snapshot and between snapshots while the endpoint exists. */
	id: string;
	protocol: NetworkEndpointProtocol;
	direction: NetworkEndpointDirection;
	origin: NetworkEndpointOrigin;
	/** What the endpoint is for, e.g. "DMX output" or "Universe discovery". */
	role: string;
	/** The address and port packets go to or arrive at. */
	endpoint: string;
	/** The peer's name, when it announces one. */
	name: string | null;
	/**
	 * Which ToskLight application this endpoint is, when it is one of the desk's own — "Media
	 * Server", "Visualizer" or "Desk". `null` for third-party hardware.
	 */
	software: string | null;
	deliveryMode: NetworkEndpointDeliveryMode | null;
	/** The show's logical universe a send route carries. */
	logicalUniverse: number | null;
	/** Protocol universes on the wire, ascending. */
	universes: readonly number[];
	status: NetworkEndpointStatus;
	/** What the status means and what to do about it. */
	detail: string;
	errors: number;
	lastActivityMillisAgo: number | null;
}

export interface NetworkEndpointsSnapshot {
	outputBindIp: string;
	/** False when the network output could not start; every send endpoint is then unavailable. */
	networkOutputAvailable: boolean;
	endpoints: readonly NetworkEndpoint[];
}
