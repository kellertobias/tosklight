import type {
	DiscoveredMediaOutput,
	DiscoveredMediaServer,
} from "../../api/client/mediaOutput";
import type {
	MediaServerFixture,
	OutputRoute,
	PatchedFixture,
	VersionedObject,
} from "../../api/types";
import { patchedModeMismatch } from "./mediaDiscoveryModel";
import {
	CONNECTION_LABELS,
	connectionState,
	matchingStatus,
} from "./mediaServerRowModel";

/** A Media Server DMX input, numbered as its protocol numbers universes on the wire. */
export type MediaDmxInput = { protocol: "art-net" | "sacn"; universe: number };

type Routes = readonly VersionedObject<OutputRoute>[];

const WIRE_PROTOCOL: Record<
	OutputRoute["protocol"],
	MediaDmxInput["protocol"]
> = { art_net: "art-net", sacn: "sacn" };

export const DMX_PROTOCOL_LABELS: Record<string, string> = {
	"art-net": "Art-Net",
	sacn: "sACN",
};

export function dmxInputLabel(input: MediaDmxInput): string {
	return `${DMX_PROTOCOL_LABELS[input.protocol]} ${input.universe}`;
}

/** Enabled routes that put a universe on the network, where a Media Server can receive it. */
function networkRoutes(routes: Routes): OutputRoute[] {
	return routes
		.map((route) => route.body)
		.filter(
			(route) =>
				route.enabled && (!route.target || route.target.kind === "network"),
		);
}

/**
 * How the desk sends one of its universes onto the network. A universe sent on several routes
 * prefers the protocol the server already listens with, so reconciling never switches protocol
 * without need.
 */
export function deskDmxInputFor(
	routes: Routes,
	logicalUniverse: number,
	preferred?: string,
): MediaDmxInput | null {
	const sending = networkRoutes(routes)
		.filter((route) => route.logical_universe === logicalUniverse)
		.map((route) => ({
			protocol: WIRE_PROTOCOL[route.protocol],
			universe: route.destination_universe,
		}));
	return (
		sending.find((input) => input.protocol === preferred) ?? sending[0] ?? null
	);
}

/** The desk universe whose route reaches what this output listens to, if any. */
export function deskUniverseReaching(
	routes: Routes,
	output: Pick<DiscoveredMediaOutput, "protocol" | "universe">,
): number | null {
	const route = networkRoutes(routes).find(
		(candidate) =>
			WIRE_PROTOCOL[candidate.protocol] === output.protocol &&
			candidate.destination_universe === output.universe,
	);
	return route?.logical_universe ?? null;
}

/** The desk fixture already patched for this exact output (by bound output, then address). */
export function matchingDiscoveredFixture(
	fixtures: readonly PatchedFixture[],
	server: DiscoveredMediaServer,
	output: DiscoveredMediaOutput,
	routes: Routes,
): PatchedFixture | undefined {
	const universe = deskUniverseReaching(routes, output) ?? output.universe;
	const bound = fixtures.filter(
		(fixture) => fixture.internal_bindings?.output === output.id,
	);
	return (
		// A copied Media configuration can repeat an output id on another host.
		bound.find(
			(fixture) => fixture.direct_control?.ip_address === server.host,
		) ??
		bound[0] ??
		fixtures.find(
			(fixture) =>
				!fixture.internal_bindings?.output &&
				fixture.direct_control?.ip_address === server.host &&
				fixture.universe === universe &&
				fixture.address === output.startAddress,
		)
	);
}

export type DiscoveredPatchKind =
	| "needs-update"
	| "not-patched"
	| "patched"
	| "mode-differs"
	| "endpoint-differs"
	| "address-differs"
	| "not-received";

export const DISCOVERED_PATCH_LABELS: Record<DiscoveredPatchKind, string> = {
	"needs-update": "Needs update",
	"not-patched": "Not patched",
	patched: "Patched",
	"mode-differs": "Mode differs",
	"endpoint-differs": "Endpoint differs",
	"address-differs": "Address differs",
	"not-received": "Not received",
};

export type DiscoveredPatchState = {
	kind: DiscoveredPatchKind;
	/** What differs and which action resolves it; absent when the two sides agree. */
	problem?: string;
};

/**
 * Whether the desk patch and the server agree. Only a patch whose mode, endpoint, address, and
 * delivered universe all match is Patched, so a mismatch is never reported as patched.
 */
export function discoveredPatchState(
	fixture: PatchedFixture | undefined,
	server: DiscoveredMediaServer,
	output: DiscoveredMediaOutput,
	routes: Routes,
): DiscoveredPatchState {
	if (!output.mode) return { kind: "needs-update" };
	if (!fixture) return { kind: "not-patched" };
	const mode = patchedModeMismatch(fixture, output);
	if (mode) return { kind: "mode-differs", problem: mode };
	const endpoint = fixture.direct_control;
	if (endpoint?.ip_address !== server.host || endpoint.port !== server.citpPort)
		return {
			kind: "endpoint-differs",
			problem: `The desk controls this server at ${endpoint ? `${endpoint.ip_address}:${endpoint.port}` : "no network address"}, but discovery found it at ${server.host}:${server.citpPort}. Patch suggested updates the desk endpoint.`,
		};
	const listens: MediaDmxInput = {
		protocol: output.protocol === "sacn" ? "sacn" : "art-net",
		universe: output.universe,
	};
	if (fixture.universe == null || fixture.address == null)
		return {
			kind: "not-received",
			problem:
				"The desk fixture for this output is unpatched, so the server receives nothing. Patch suggested or Patch address gives it a DMX address.",
		};
	const sent = deskDmxInputFor(routes, fixture.universe, output.protocol);
	if (!sent)
		return {
			kind: "not-received",
			problem: `The desk sends no network output for universe ${fixture.universe}, so this server receives nothing. Add an output route for universe ${fixture.universe} under Setup › Outputs, or choose Patch address.`,
		};
	if (
		sent.protocol !== listens.protocol ||
		sent.universe !== listens.universe ||
		fixture.address !== output.startAddress
	)
		return {
			kind: "address-differs",
			problem: `The desk sends DMX ${fixture.universe}.${fixture.address} as ${dmxInputLabel(sent)}, but this output listens to ${dmxInputLabel(listens)} at address ${output.startAddress}. Patch suggested moves the desk patch to the server; Patch address sets both.`,
		};
	return { kind: "patched" };
}

/** The desk's connection to a discovered output, as the patched table reports it. */
export function discoveredConnection(
	fixture: PatchedFixture | undefined,
	statuses: readonly MediaServerFixture[],
): string {
	if (!fixture) return "Not connected (not patched)";
	const status = matchingStatus(statuses, fixture);
	return CONNECTION_LABELS[connectionState(fixture, status, false)];
}
