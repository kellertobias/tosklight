import type {
	EventClientMessage,
	PatchFixtureInput,
	PatchFixturesRequest,
	PatchPlacementIntent,
} from "./generated/light-wire";
import type {
	PatchDirectControlEndpoint,
	PatchFixturePolicyAction,
	PatchFixtureWrite,
	PatchMutation,
	PatchPlacement,
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
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
		this.WebSocketImplementation = options.webSocket ?? globalThis.WebSocket;
	}

	async snapshot(showId: string) {
		const response = await this.fetchImplementation(this.patchPath(), {
			headers: this.headers(showId),
		});
		return decodePatchSnapshot(await this.responseValue(response));
	}

	async patchFixtures(
		showId: string,
		expectedPatchRevision: number,
		mutation: PatchMutation,
	) {
		const headers = this.headers(showId);
		headers.set("content-type", "application/json");
		headers.set("if-match", String(expectedPatchRevision));
		const request: PatchFixturesRequest = {
			request_id: mutation.requestId,
			fixtures: mutation.fixtures.map(toWireFixture),
			remove_fixture_ids: [...mutation.removeFixtureIds],
			placements: (mutation.placements ?? []).map(toWirePlacement),
		};
		const response = await this.fetchImplementation(
			this.patchPath() + "/fixtures",
			{
				method: "POST",
				headers,
				body: JSON.stringify(request),
			},
		);
		return decodePatchFixturesOutcome(await this.responseValue(response));
	}

	async patchFixturePolicy(
		showId: string,
		fixtureId: string,
		expectedPatchRevision: number,
		requestId: string,
		action: PatchFixturePolicyAction,
	) {
		const headers = this.headers(showId);
		headers.set("content-type", "application/json");
		headers.set("if-match", String(expectedPatchRevision));
		const body =
			action.type === "group_masters"
				? {
						request_id: requestId,
						action: "set_group_masters",
						controlled: action.controlled,
					}
				: action.type === "grand_master"
					? {
							request_id: requestId,
							action: "set_grand_master",
							controlled: action.controlled,
						}
					: {
							request_id: requestId,
							action: "set_axis_inversion",
							axis: action.axis,
							inverted: action.inverted,
							multipatch_instance_id: action.multipatchInstanceId,
						};
		const response = await this.fetchImplementation(
			`${this.patchPath()}/fixtures/${fixtureId}/policy`,
			{
				method: "POST",
				headers,
				body: JSON.stringify(body),
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

	private patchPath() {
		return this.baseUrl + "/api/v2/patch";
	}

	private headers(showId: string) {
		const headers = new Headers({
			authorization: "Bearer " + this.options.sessionToken,
			"x-tosk-show": showId,
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

function toWirePlacement(placement: PatchPlacement): PatchPlacementIntent {
	return {
		fixture_ids: [...placement.fixtureIds],
		splits: placement.splits.map((split) => ({
			split: split.split,
			universe: split.universe,
			address: split.address,
			mode:
				split.mode.type === "consecutive"
					? { type: "consecutive" }
					: {
							type: "operator_overrides",
							overrides: split.mode.overrides.map((override) => ({
								fixture_id: override.fixtureId,
								universe: override.universe,
								address: override.address,
							})),
						},
		})),
	};
}

export function browserDeskBoundaryToken(): string {
	const testStorage = globalThis.sessionStorage;
	const storage =
		testStorage?.getItem("light.test-server-url") != null
			? testStorage
			: globalThis.localStorage;
	return storage?.getItem("light.desk-token")?.trim() ?? "";
}

export function toWireFixture(fixture: PatchFixtureWrite): PatchFixtureInput {
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
			invert_pan: instance.invertPan ?? false,
			invert_tilt: instance.invertTilt ?? false,
		})),
		group_masters_enabled: fixture.groupMastersEnabled ?? true,
		grand_master_enabled: fixture.grandMasterEnabled ?? true,
		invert_pan: fixture.invertPan ?? false,
		invert_tilt: fixture.invertTilt ?? false,
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
