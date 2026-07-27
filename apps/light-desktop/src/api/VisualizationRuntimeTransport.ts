import { frontendPerformanceDiagnostics } from "../features/frontendWarmup/diagnostics";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "../features/visualizationRuntime/contracts";
import type {
	VisualizationRuntimeStream,
	VisualizationRuntimeStreamObserver,
	VisualizationRuntimeTransport,
} from "../features/visualizationRuntime/transport";
import {
	VisualizationRuntimeHttpError,
	VisualizationRuntimeProtocolError,
} from "../features/visualizationRuntime/transport";
import {
	arrayAt,
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	nullable,
	numberAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { decodeAttributeValue } from "./programmerValuesWireProjection";
import { programmingUuidAt } from "./programmingWireProjection";
import type { AttributeValue, VisualizationSnapshot } from "./types";
import { WireValidationError } from "./wireValidation";

export interface HttpVisualizationRuntimeTransportOptions {
	baseUrl: string;
	sessionToken: string;
	showId: string;
	sessionId: string;
	authorityKey: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
	webSocket?: typeof globalThis.WebSocket;
}

/** Narrow, authenticated adapter for the transitional v1 Visualization snapshot. */
export class HttpVisualizationRuntimeTransport
	implements VisualizationRuntimeTransport
{
	private readonly baseUrl: string;
	private readonly fetchImplementation: typeof globalThis.fetch;

	constructor(
		private readonly options: HttpVisualizationRuntimeTransportOptions,
	) {
		this.baseUrl = options.baseUrl.replace(/\/$/, "");
		this.fetchImplementation =
			options.fetch ?? globalThis.fetch.bind(globalThis);
	}

	async loadSnapshot(
		scope: VisualizationRuntimeScope,
		lane: VisualizationRuntimeLane,
	): Promise<VisualizationSnapshot> {
		this.validateScope(scope);
		const finishDiagnostic =
			frontendPerformanceDiagnostics.beginStageVisualizationRequest(lane);
		const query = lane === "preload" ? "?preload=true" : "";
		let response: Response;
		try {
			response = await this.fetchImplementation(
				`${this.baseUrl}/api/v2/output/visualization${query}`,
				{ headers: this.headers() },
			);
		} catch (reason) {
			finishDiagnostic(undefined, reason);
			throw new VisualizationRuntimeHttpError(asError(reason).message, 0);
		}
		try {
			const value = await responseValue(response);
			const decoded = decodeVisualizationRuntimeSnapshot(value, lane);
			finishDiagnostic(decoded);
			return decoded;
		} catch (reason) {
			finishDiagnostic(undefined, reason);
			if (reason instanceof VisualizationRuntimeProtocolError) throw reason;
			throw new VisualizationRuntimeProtocolError(asError(reason).message);
		}
	}

	openStream(
		scope: VisualizationRuntimeScope,
		observer: VisualizationRuntimeStreamObserver,
	): VisualizationRuntimeStream {
		this.validateScope(scope);
		return new WebSocketVisualizationRuntimeStream(
			this.baseUrl,
			this.options.sessionToken,
			observer,
			this.options.webSocket ?? globalThis.WebSocket,
		);
	}

	private validateScope(scope: VisualizationRuntimeScope) {
		const showId = programmingUuidAt(scope.showId, "$.scope.showId");
		const sessionId = programmingUuidAt(scope.sessionId, "$.scope.sessionId");
		if (!sameUuid(showId, this.options.showId))
			throw new VisualizationRuntimeProtocolError(
				"Visualization scope does not match the configured Show",
			);
		if (!sameUuid(sessionId, this.options.sessionId))
			throw new VisualizationRuntimeProtocolError(
				"Visualization scope does not match the authenticated session",
			);
		if (scope.authorityKey !== this.options.authorityKey)
			throw new VisualizationRuntimeProtocolError(
				"Visualization scope does not match the configured server authority",
			);
	}

	private headers() {
		const headers = new Headers({
			authorization: `Bearer ${this.options.sessionToken}`,
			"x-tosk-show": this.options.showId,
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}
}

class WebSocketVisualizationRuntimeStream
	implements VisualizationRuntimeStream
{
	private socket: WebSocket | null = null;
	private claims = new Set<VisualizationRuntimeLane>();
	private maxRateHz = 10;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;
	private reconnectDelay = 250;
	private stopped = false;
	private lastSequence = 0;
	private snapshots: Record<
		VisualizationRuntimeLane,
		VisualizationSnapshot | null
	> = { normal: null, preload: null };

	constructor(
		private readonly baseUrl: string,
		private readonly sessionToken: string,
		private readonly observer: VisualizationRuntimeStreamObserver,
		private readonly WebSocketImplementation: typeof globalThis.WebSocket,
	) {}

	updateClaims(lanes: readonly VisualizationRuntimeLane[], maxRateHz: number) {
		const removed = [...this.claims].filter((lane) => !lanes.includes(lane));
		this.claims = new Set(lanes);
		this.maxRateHz = Math.max(1, Math.min(10, Math.floor(maxRateHz)));
		if (!this.claims.size) {
			this.closeSocket();
			return;
		}
		if (!this.socket) this.connect();
		else {
			if (removed.length) this.send({ type: "unsubscribe", lanes: removed });
			this.sendSubscription();
		}
	}

	close() {
		this.stopped = true;
		this.claims.clear();
		if (this.reconnectTimer !== null)
			globalThis.clearTimeout(this.reconnectTimer);
		this.reconnectTimer = null;
		this.closeSocket();
	}

	private connect() {
		if (this.stopped || this.socket || !this.claims.size) return;
		const url = new URL("/api/v2/visualization/stream", this.baseUrl);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		const socket = new this.WebSocketImplementation(url, [
			"light.visualization.v1",
			`light.token.${this.sessionToken}`,
		]);
		this.socket = socket;
		socket.addEventListener("open", () => {
			this.reconnectDelay = 250;
			this.lastSequence = 0;
			this.snapshots = { normal: null, preload: null };
			this.sendSubscription();
		});
		socket.addEventListener("message", (event) => this.receive(event.data));
		socket.addEventListener("error", () =>
			this.observer.error(
				new VisualizationRuntimeProtocolError(
					"Visualization stream connection failed",
				),
			),
		);
		socket.addEventListener("close", () => {
			if (this.socket === socket) this.socket = null;
			this.scheduleReconnect();
		});
	}

	private receive(raw: unknown) {
		try {
			if (typeof raw !== "string")
				throw new VisualizationRuntimeProtocolError(
					"Visualization stream message was not text",
				);
			const message = recordAt(JSON.parse(raw) as unknown, "$");
			const type = stringAt(message.type, "$.type");
			if (type === "hello") {
				if (integerAt(message.protocol_version, "$.protocol_version") !== 1)
					throw new VisualizationRuntimeProtocolError(
						"Visualization stream protocol version is unsupported",
					);
				return;
			}
			if (type === "snapshot") {
				const lane = enumAt(message.lane, "$.lane", ["normal", "preload"]);
				const sequence = integerAt(message.sequence, "$.sequence");
				const sourceFrame = integerAt(message.source_frame, "$.source_frame");
				const sourceGeneratedAt = timestampAt(
					message.source_timestamp,
					"$.source_timestamp",
				);
				const publishedAt = timestampAt(message.published_at, "$.published_at");
				if (this.lastSequence && sequence !== this.lastSequence + 1)
					this.send({ type: "resynchronize", lane });
				this.lastSequence = sequence;
				const snapshot = decodeVisualizationRuntimeSnapshot(
					message.snapshot,
					lane,
				);
				this.snapshots[lane] = snapshot;
				frontendPerformanceDiagnostics.recordStageFrameReceived({
					lane,
					sourceFrame,
					sourceGeneratedAt,
					publishedAt,
				});
				this.observer.snapshot(lane, snapshot);
				return;
			}
			if (type === "delta") {
				const lane = enumAt(message.lane, "$.lane", ["normal", "preload"]);
				const sequence = integerAt(message.sequence, "$.sequence");
				const sourceFrame = integerAt(message.source_frame, "$.source_frame");
				const sourceGeneratedAt = timestampAt(
					message.source_timestamp,
					"$.source_timestamp",
				);
				const publishedAt = timestampAt(message.published_at, "$.published_at");
				if (this.lastSequence && sequence !== this.lastSequence + 1) {
					this.lastSequence = sequence;
					this.send({ type: "resynchronize", lane });
					return;
				}
				this.lastSequence = sequence;
				const current = this.snapshots[lane];
				if (!current) {
					this.send({ type: "resynchronize", lane });
					return;
				}
				const snapshot = applyVisualizationDelta(current, message.delta, lane);
				this.snapshots[lane] = snapshot;
				frontendPerformanceDiagnostics.recordStageFrameReceived({
					lane,
					sourceFrame,
					sourceGeneratedAt,
					publishedAt,
				});
				this.observer.snapshot(lane, snapshot);
				return;
			}
			if (type === "structural_invalidation") {
				for (const lane of this.claims) {
					this.snapshots[lane] = null;
					this.send({ type: "resynchronize", lane });
				}
				return;
			}
			if (type === "heartbeat") {
				const sequence = integerAt(message.sequence, "$.sequence");
				if (this.lastSequence && sequence !== this.lastSequence + 1)
					for (const lane of this.claims)
						this.send({ type: "resynchronize", lane });
				this.lastSequence = sequence;
				return;
			}
			if (type === "error")
				throw new VisualizationRuntimeProtocolError(
					stringAt(message.message, "$.message"),
				);
		} catch (reason) {
			this.observer.error(asError(reason));
		}
	}

	private sendSubscription() {
		if (!this.claims.size) return;
		this.send({
			type: "subscribe",
			lanes: [...this.claims],
			max_rate_hz: this.maxRateHz,
		});
	}

	private send(message: unknown) {
		if (this.socket?.readyState === this.WebSocketImplementation.OPEN)
			this.socket.send(JSON.stringify(message));
	}

	private scheduleReconnect() {
		if (this.stopped || !this.claims.size || this.reconnectTimer !== null)
			return;
		const delay = this.reconnectDelay;
		this.reconnectDelay = Math.min(this.reconnectDelay * 2, 2_000);
		this.reconnectTimer = globalThis.setTimeout(() => {
			this.reconnectTimer = null;
			this.connect();
		}, delay);
	}

	private closeSocket() {
		const socket = this.socket;
		this.socket = null;
		socket?.close();
	}
}

function applyVisualizationDelta(
	current: VisualizationSnapshot,
	value: unknown,
	lane: VisualizationRuntimeLane,
): VisualizationSnapshot {
	const delta = recordAt(value, "$.delta");
	const preload = booleanAt(delta.preload, "$.delta.preload");
	if (preload !== (lane === "preload"))
		throw new VisualizationRuntimeProtocolError(
			"Visualization delta belongs to a different lane",
		);
	return {
		revision: integerAt(delta.revision, "$.delta.revision"),
		generated_at: timestampAt(delta.generated_at, "$.delta.generated_at"),
		grand_master: normalizedAt(delta.grand_master, "$.delta.grand_master"),
		blackout: booleanAt(delta.blackout, "$.delta.blackout"),
		preload,
		values: mergeVisualizationValues(
			current.values,
			decodeValues(delta.values, "$.delta.values"),
			decodeRemovedValueKeys(delta.removed_values, "$.delta.removed_values"),
		),
		dynamic_stack: decodeDynamicStack(
			delta.dynamic_stack,
			"$.delta.dynamic_stack",
		),
		profile_output_values: mergeVisualizationValues(
			current.profile_output_values ?? [],
			decodeValues(
				delta.profile_output_values,
				"$.delta.profile_output_values",
			),
			decodeRemovedValueKeys(
				delta.removed_profile_output_values,
				"$.delta.removed_profile_output_values",
			),
		),
	};
}

function decodeRemovedValueKeys(value: unknown, path: string) {
	return arrayAt(value, path).map((entry, index) => {
		const key = exactRecordAt(entry, `${path}[${index}]`, [
			"fixture_id",
			"attribute",
		]);
		return `${stringAt(key.fixture_id, `${path}[${index}].fixture_id`)}\u0000${stringAt(
			key.attribute,
			`${path}[${index}].attribute`,
		)}`;
	});
}

function mergeVisualizationValues(
	current: VisualizationSnapshot["values"],
	upserts: VisualizationSnapshot["values"],
	removed: readonly string[],
) {
	const values = new Map(
		current.map((value) => [
			`${value.fixture_id}\u0000${value.attribute}`,
			value,
		]),
	);
	for (const key of removed) values.delete(key);
	for (const value of upserts)
		values.set(`${value.fixture_id}\u0000${value.attribute}`, value);
	return [...values.values()].sort(
		(left, right) =>
			left.fixture_id.localeCompare(right.fixture_id) ||
			left.attribute.localeCompare(right.attribute),
	);
}

export function decodeVisualizationRuntimeSnapshot(
	value: unknown,
	expectedLane: VisualizationRuntimeLane,
): VisualizationSnapshot {
	const snapshot = recordAt(value, "$");
	const preload = booleanAt(snapshot.preload, "$.preload");
	if (preload !== (expectedLane === "preload"))
		throw new VisualizationRuntimeProtocolError(
			`Visualization response belongs to the ${preload ? "preload" : "normal"} lane`,
		);
	const generatedAt = timestampAt(snapshot.generated_at, "$.generated_at");
	return {
		revision: integerAt(snapshot.revision, "$.revision"),
		generated_at: generatedAt,
		grand_master: normalizedAt(snapshot.grand_master, "$.grand_master"),
		blackout: booleanAt(snapshot.blackout, "$.blackout"),
		preload,
		values: decodeValues(snapshot.values, "$.values"),
		dynamic_stack:
			snapshot.dynamic_stack === undefined
				? undefined
				: decodeDynamicStack(snapshot.dynamic_stack, "$.dynamic_stack"),
		profile_output_values: decodeValues(
			snapshot.profile_output_values,
			"$.profile_output_values",
		),
	};
}

function decodeDynamicStack(value: unknown, path: string) {
	return arrayAt(value, path).map((entry, index) =>
		decodeDynamicStackEntry(entry, `${path}[${index}]`),
	);
}

function decodeDynamicStackEntry(value: unknown, path: string) {
	const entry = recordAt(value, path);
	return {
		fixture_id: stringAt(entry.fixture_id, `${path}.fixture_id`),
		attribute: stringAt(entry.attribute, `${path}.attribute`),
		entry_type: enumAt(entry.entry_type, `${path}.entry_type`, [
			"ordinary_static",
			"dynamic",
			"fix_at",
			"dynamic_off",
			"static",
		]),
		priority: signedIntegerAt(entry.priority, `${path}.priority`),
		changed_at_millis: integerAt(
			entry.changed_at_millis,
			`${path}.changed_at_millis`,
		),
		source: stringAt(entry.source, `${path}.source`),
		dynamic_id: nullable(entry.dynamic_id, `${path}.dynamic_id`, stringAt),
		pool_number: nullable(entry.pool_number, `${path}.pool_number`, integerAt),
		name: stringAt(entry.name, `${path}.name`),
		runtime_instance_id: nullable(
			entry.runtime_instance_id,
			`${path}.runtime_instance_id`,
			stringAt,
		),
		controller_id: nullable(
			entry.controller_id,
			`${path}.controller_id`,
			stringAt,
		),
		lane_id: nullable(entry.lane_id, `${path}.lane_id`, stringAt),
		size: nullable(entry.size, `${path}.size`, numberAt),
		activation_mix: nullable(
			entry.activation_mix,
			`${path}.activation_mix`,
			normalizedAt,
		),
		paused: booleanAt(entry.paused, `${path}.paused`),
		hidden: booleanAt(entry.hidden, `${path}.hidden`),
		pending: booleanAt(entry.pending, `${path}.pending`),
		winning: booleanAt(entry.winning, `${path}.winning`),
		value: nullable(entry.value, `${path}.value`, decodeAttributeValue),
		resolved_value: nullable(
			entry.resolved_value,
			`${path}.resolved_value`,
			decodeAttributeValue,
		),
	};
}

function decodeValues(value: unknown, path: string) {
	return arrayAt(value, path).map((entry, index) =>
		decodeVisualizationValue(entry, `${path}[${index}]`),
	);
}

function signedIntegerAt(value: unknown, path: string) {
	const integer = numberAt(value, path);
	if (!Number.isSafeInteger(integer))
		throw new WireValidationError(path, "integer", value);
	return integer;
}

function decodeVisualizationValue(
	value: unknown,
	path: string,
): {
	fixture_id: string;
	attribute: string;
	value: AttributeValue;
} {
	const entry = exactRecordAt(value, path, [
		"fixture_id",
		"attribute",
		"value",
	]);
	return {
		fixture_id: stringAt(entry.fixture_id, `${path}.fixture_id`),
		attribute: stringAt(entry.attribute, `${path}.attribute`),
		value: decodeAttributeValue(entry.value, `${path}.value`),
	};
}

function timestampAt(value: unknown, path: string) {
	const timestamp = stringAt(value, path);
	if (
		!/^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:[0-9]{2})$/.test(
			timestamp,
		) ||
		!Number.isFinite(Date.parse(timestamp))
	)
		throw new WireValidationError(path, "ISO-8601 timestamp", value);
	return timestamp;
}

function normalizedAt(value: unknown, path: string) {
	const normalized = numberAt(value, path);
	if (normalized < 0 || normalized > 1)
		throw new WireValidationError(path, "number between 0 and 1", value);
	return normalized;
}

async function responseValue(response: Response) {
	const text = await response.text();
	if (!response.ok)
		throw new VisualizationRuntimeHttpError(
			text || `${response.status} ${response.statusText}`,
			response.status,
		);
	try {
		return text ? (JSON.parse(text) as unknown) : null;
	} catch {
		throw new VisualizationRuntimeProtocolError(
			"Visualization response was not valid JSON",
		);
	}
}

function sameUuid(left: string, right: string) {
	return left.toLowerCase() === right.toLowerCase();
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
