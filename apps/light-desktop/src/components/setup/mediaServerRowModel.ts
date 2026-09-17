import type { MediaServerDiscovery } from "../../api/client/mediaOutput";
import type { MediaServerFixture, PatchedFixture } from "../../api/types";

/** The shipped ToskLight Media Server profile; the desk also speaks its native API. */
export const TOSKLIGHT_MEDIA_SERVER_PROFILE_ID =
	"0a14fb60-280d-5ef1-aa4a-2ff11bd06943";

export type MediaServerType = "tosklight" | "citp" | "none";
export type MediaServerProtocol = "off" | "citp";

export const MEDIA_SERVER_TYPE_LABELS: Record<MediaServerType, string> = {
	tosklight: "ToskLight Media",
	citp: "CITP media server",
	none: "No network control",
};

const PROTOCOL_LABELS: Record<MediaServerProtocol, string> = {
	off: "Off",
	citp: "CITP",
};

export const DEFAULT_CITP_PORT = 4809;

/** One row's editable endpoint, as typed. */
export type MediaServerDraft = {
	protocol: MediaServerProtocol;
	ip: string;
	port: number;
};

export type DraftProblems = { ip?: string; port?: string };

export function isMediaFixture(fixture: PatchedFixture): boolean {
	return (
		Boolean(fixture.direct_control) ||
		Boolean(fixture.definition.direct_control_protocols?.length) ||
		(fixture.definition.heads ?? []).some((head) =>
			head.parameters.some((parameter) =>
				parameter.attribute.startsWith("media."),
			),
		)
	);
}

export function supportedProtocols(
	fixture: PatchedFixture,
): MediaServerProtocol[] {
	const advertised = fixture.definition.direct_control_protocols ?? [];
	// A stored endpoint stays editable even if an older profile does not list its protocol.
	if (!advertised.length && fixture.direct_control) return ["citp"];
	return advertised.filter((protocol) => protocol === "citp");
}

export function protocolOptions(fixture: PatchedFixture) {
	return (["off", ...supportedProtocols(fixture)] as MediaServerProtocol[]).map(
		(value) => ({ value, label: PROTOCOL_LABELS[value] }),
	);
}

export function mediaServerType(
	fixture: PatchedFixture,
	status?: MediaServerFixture,
): MediaServerType {
	if (!supportedProtocols(fixture).length) return "none";
	const definition = fixture.definition;
	const native =
		Boolean(status?.native_action) ||
		definition.profile_id === TOSKLIGHT_MEDIA_SERVER_PROFILE_ID ||
		(definition.manufacturer === "ToskLight" &&
			definition.name === "Media Server");
	return native ? "tosklight" : "citp";
}

export function fixtureDraft(fixture: PatchedFixture): MediaServerDraft {
	return {
		protocol: fixture.direct_control ? "citp" : "off",
		ip: fixture.direct_control?.ip_address ?? "",
		port: fixture.direct_control?.port ?? DEFAULT_CITP_PORT,
	};
}

export function sameDraft(a: MediaServerDraft, b: MediaServerDraft): boolean {
	if (a.protocol === "off" && b.protocol === "off") return true;
	return (
		a.protocol === b.protocol &&
		a.ip.trim() === b.ip.trim() &&
		a.port === b.port
	);
}

const IPV4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/u;

/** Whether the desk will accept this as a literal IPv4 or IPv6 address (no host names). */
export function isIpAddress(value: string): boolean {
	const ipv4 = IPV4.exec(value);
	if (ipv4)
		return ipv4
			.slice(1)
			.every(
				(octet) => Number(octet) <= 255 && String(Number(octet)) === octet,
			);
	if (!value.includes(":") || /[^0-9a-f:.]/iu.test(value)) return false;
	try {
		return new URL(`http://[${value}]/`).hostname.length > 2;
	} catch {
		return false;
	}
}

/** Operator-worded problems with a draft; an Off row has nothing to validate. */
export function draftProblems(draft: MediaServerDraft): DraftProblems {
	if (draft.protocol === "off") return {};
	const problems: DraftProblems = {};
	const ip = draft.ip.trim();
	if (!ip)
		problems.ip = "Enter the server's IP address, or set Protocol to Off.";
	else if (!isIpAddress(ip))
		problems.ip = "Enter an IPv4 or IPv6 address, for example 192.168.1.50.";
	if (!Number.isInteger(draft.port) || draft.port < 1 || draft.port > 65535)
		problems.port = "Choose a port from 1 to 65535.";
	return problems;
}

/** The server's status for this fixture, only while it describes the endpoint now patched. */
export function matchingStatus(
	statuses: readonly MediaServerFixture[],
	fixture: PatchedFixture,
): MediaServerFixture | undefined {
	const endpoint = fixture.direct_control;
	if (!endpoint) return undefined;
	return statuses.find(
		(status) =>
			status.fixture_id === fixture.fixture_id &&
			status.endpoint?.protocol === endpoint.protocol &&
			status.endpoint.ip_address === endpoint.ip_address &&
			status.endpoint.port === endpoint.port,
	);
}

export type ConnectionState =
	| "off"
	| "unsupported"
	| "checking"
	| "connected"
	| "offline"
	| "unchecked";

export const CONNECTION_LABELS: Record<ConnectionState, string> = {
	off: "Off",
	unsupported: "No network control",
	checking: "Checking…",
	connected: "Connected",
	offline: "Offline",
	unchecked: "Not checked",
};

export function connectionState(
	fixture: PatchedFixture,
	status: MediaServerFixture | undefined,
	checking: boolean,
): ConnectionState {
	if (!fixture.direct_control)
		return supportedProtocols(fixture).length ? "off" : "unsupported";
	if (checking) return "checking";
	if (status?.status.online) return "connected";
	if (status?.status.last_error) return "offline";
	return "unchecked";
}

/** Whether the desk has anything authoritative to say about this endpoint yet. */
export function needsFirstCheck(
	fixture: PatchedFixture,
	status: MediaServerFixture | undefined,
): boolean {
	return Boolean(
		fixture.direct_control &&
			!status?.status.online &&
			!status?.status.last_error &&
			!status?.status.last_success,
	);
}

/** What the last discovery says about the address this row uses. */
export function networkState(
	fixture: PatchedFixture,
	discovery: MediaServerDiscovery | null,
	discoveryError: string | null,
): string | null {
	const ip = fixture.direct_control?.ip_address;
	if (!ip) return null;
	if (!discovery) return discoveryError ? "Discovery unavailable" : null;
	const found = discovery.servers.find((server) => server.host === ip);
	if (!found) return "Not found by discovery";
	return found.error ? "Found, needs attention" : "Found on network";
}

export function offlineHint(error: string): string {
	return `${error} Check the IP address, port, and that the server is running, then Check connection to retry.`;
}
