import type {
	EventClientMessage,
	PatchFixtureInput,
	PatchFixturesRequest,
} from "./generated/light-wire";
import type {
	PatchDirectControlEndpoint,
	PatchFixtureWrite,
	PatchMutation,
} from "../features/patch/contracts";
import {
	type PatchEventObserver,
	type PatchEventStream,
	type PatchTransport,
	PatchTransportError,
} from "../features/patch/transport";
import {
	decodePatchErrorResponse,
	decodePatchEventServerMessage,
	decodePatchFixturesOutcome,
	decodePatchSnapshot,
} from "./patchWire";

export interface HttpPatchTransportOptions {
	baseUrl: string;
	sessionToken: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

/** HTTP/WebSocket adapter for the feature-owned Patch transport port. */
export class HttpPatchTransport implements PatchTransport {
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;
	private readonly WebSocketImplementation: typeof globalThis.WebSocket;

	constructor(private readonly options: HttpPatchTransportOptions) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation = options.fetch ?? globalThis.fetch;
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async snapshot(showId: string) {
		const response = await this.fetchImplementation(this.patchPath(showId), {
			headers: this.headers(),
		});
		return decodePatchSnapshot(await this.responseValue(response));
	}

	async patchFixtures(
		showId: string,
		expectedPatchRevision: number,
		mutation: PatchMutation,
	) {
		const headers = this.headers();
		headers.set("content-type", "application/json");
		headers.set("if-match", String(expectedPatchRevision));
		const request: PatchFixturesRequest = {
			request_id: mutation.requestId,
			fixtures: mutation.fixtures.map(toWireFixture),
			remove_fixture_ids: [...mutation.removeFixtureIds],
		};
		const response = await this.fetchImplementation(
			this.patchPath(showId) + "/fixtures",
			{
				method: "POST",
				headers,
				body: JSON.stringify(request),
			},
		);
		return decodePatchFixturesOutcome(await this.responseValue(response));
	}

	subscribe(
		showId: string,
		afterSequence: number,
		observer: PatchEventObserver,
	): PatchEventStream {
		const url = new URL("/api/v2/events", this.baseUrl);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		const protocols = [
			"light.events.v2",
			"light.token." + this.options.sessionToken,
		];
		if (this.options.deskBoundaryToken)
			protocols.push(
				"light.desk.b64." + base64Url(this.options.deskBoundaryToken),
			);
		const socket = new this.WebSocketImplementation(url, protocols);
		let explicitlyClosed = false;
		socket.addEventListener("open", () => {
			const request: EventClientMessage = {
				type: "subscribe",
				filter: {
					capabilities: ["show"],
					classes: ["projection"],
					objects: [{ capability: "show", id: "patch:" + showId }],
				},
				after_sequence: afterSequence,
				capacity: 128,
				rate_limits: [],
			};
			socket.send(JSON.stringify(request));
		});
		socket.addEventListener("message", (event) => {
			try {
				observer.message(
					decodePatchEventServerMessage(JSON.parse(String(event.data))),
				);
			} catch (reason) {
				observer.error(asError(reason));
			}
		});
		socket.addEventListener("error", () => {
			observer.error(new Error("Patch event connection failed"));
		});
		socket.addEventListener("close", () => {
			if (!explicitlyClosed) observer.closed();
		});
		return {
			repair: (cursor) => {
				if (socket.readyState !== this.WebSocketImplementation.OPEN) return;
				const request: EventClientMessage = {
					type: "repair",
					cursor: { sequence: cursor },
				};
				socket.send(JSON.stringify(request));
			},
			close: () => {
				explicitlyClosed = true;
				socket.close();
			},
		};
	}

	private patchPath(showId: string) {
		return (
			this.baseUrl +
			"/api/v2/shows/" +
			encodeURIComponent(showId) +
			"/patch"
		);
	}

	private headers() {
		const headers = new Headers({
			authorization: "Bearer " + this.options.sessionToken,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}

	private async responseValue(response: Response): Promise<unknown> {
		const text = await response.text();
		let value: unknown;
		try {
			value = text ? JSON.parse(text) : null;
		} catch {
			throw new Error(
				"Patch transport returned invalid JSON (" + response.status + ")",
			);
		}
		if (response.ok) return value;
		try {
			const error = decodePatchErrorResponse(value);
			throw new PatchTransportError(
				error.error,
				response.status,
				error.currentRevision,
				error.retryable,
			);
		} catch (reason) {
			if (reason instanceof PatchTransportError) throw reason;
			throw new PatchTransportError(
				text || response.status + " " + response.statusText,
				response.status,
				null,
				response.status >= 500,
			);
		}
	}
}

export function browserDeskBoundaryToken(): string {
	const testStorage = globalThis.sessionStorage;
	const storage =
		testStorage?.getItem("light.test-server-url") != null
			? testStorage
			: globalThis.localStorage;
	return storage?.getItem("light.desk-token")?.trim() ?? "";
}

function toWireFixture(fixture: PatchFixtureWrite): PatchFixtureInput {
	return {
		fixture_id: fixture.fixtureId,
		fixture_number: fixture.fixtureNumber,
		virtual_fixture_number: fixture.virtualFixtureNumber,
		name: fixture.name,
		profile_id: fixture.profileId,
		profile_revision: fixture.profileRevision,
		mode_id: fixture.modeId,
		split_patches: fixture.splitPatches.map((split) => ({ ...split })),
		layer_id: fixture.layerId,
		direct_control: toWireDirectControl(fixture.directControl),
		location: { ...fixture.location },
		rotation: { ...fixture.rotation },
		multipatch: fixture.multipatch.map((instance) => ({
			id: instance.id,
			name: instance.name,
			split_patches: instance.splitPatches.map((split) => ({ ...split })),
			location: { ...instance.location },
			rotation: { ...instance.rotation },
		})),
		move_in_black_enabled: fixture.moveInBlackEnabled,
		move_in_black_delay_millis: fixture.moveInBlackDelayMillis,
		highlight_overrides: fixture.highlightOverrides.map((override) => ({
			channel_id: override.channelId,
			raw_value: override.rawValue,
		})),
	};
}

function toWireDirectControl(
	endpoint: PatchDirectControlEndpoint | null,
): PatchFixtureInput["direct_control"] {
	return endpoint
		? {
				protocol: endpoint.protocol,
				ip_address: endpoint.ipAddress,
				port: endpoint.port,
			}
		: null;
}

function base64Url(value: string): string {
	const bytes = new TextEncoder().encode(value);
	let binary = "";
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary)
		.replaceAll("+", "-")
		.replaceAll("/", "_")
		.replace(/=+$/, "");
}

function asError(reason: unknown): Error {
	return reason instanceof Error ? reason : new Error(String(reason));
}
