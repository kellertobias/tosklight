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

let nextVisualizationDiagnosticScopeActivation = 0;

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
	private readonly diagnosticScopeActivation =
		++nextVisualizationDiagnosticScopeActivation;

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
		options?: { dynamicStackOnly?: boolean; fixtureIds?: readonly string[] },
	): Promise<VisualizationSnapshot> {
		this.validateScope(scope);
		const finishDiagnostic =
			frontendPerformanceDiagnostics.beginStageVisualizationRequest(lane);
		const query = new URLSearchParams();
		if (lane === "preload") query.set("preload", "true");
		if (options?.dynamicStackOnly) query.set("dynamic_stack_only", "true");
		if (options?.fixtureIds?.length)
			query.set("fixture_ids", options.fixtureIds.join(","));
		const suffix = query.size ? `?${query}` : "";
		let response: Response;
		try {
			response = await this.fetchImplementation(
				`${this.baseUrl}/api/v2/output/visualization${suffix}`,
				{ headers: this.headers() },
			);
		} catch (reason) {
			finishDiagnostic(undefined, reason);
			throw new VisualizationRuntimeHttpError(asError(reason).message, 0);
		}
		try {
			const payload = await responseValue(response);
			const source = decodeVisualizationHttpSource(payload.value);
			const decoded = decodeVisualizationRuntimeSnapshot(payload.value, lane);
			finishDiagnostic(
				{ generated_at: source.timestamp ?? decoded.generated_at },
				undefined,
				payload.bytes,
			);
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
			scope.showId,
			this.diagnosticScopeActivation,
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
	private includeDynamicStack = false;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;
	private reconnectDelay = 250;
	private stopped = false;
	private lastSequence = 0;
	// WebKit acknowledgement round trips can throttle a one-frame publication
	// window below the renderer's useful cadence. The server's overwrite-old
	// outgoing queue remains authoritative; the desktop consumes at a bounded rate.
	private readonly acknowledgementBackpressure = false;
	private pendingAcknowledgement = 0;
	private acknowledgementScheduled = false;
	private snapshots: Record<
		VisualizationRuntimeLane,
		VisualizationSnapshot | null
	> = { normal: null, preload: null };

	constructor(
		private readonly baseUrl: string,
		private readonly sessionToken: string,
		private readonly expectedShowId: string,
		private readonly diagnosticScopeActivation: number,
		private readonly observer: VisualizationRuntimeStreamObserver,
		private readonly WebSocketImplementation: typeof globalThis.WebSocket,
	) {}

	updateClaims(
		lanes: readonly VisualizationRuntimeLane[],
		maxRateHz: number,
		includeDynamicStack = false,
	) {
		const removed = [...this.claims].filter((lane) => !lanes.includes(lane));
		this.claims = new Set(lanes);
		this.maxRateHz = Math.max(1, Math.min(10, Math.floor(maxRateHz)));
		this.includeDynamicStack = includeDynamicStack;
		if (!this.claims.size) {
			if (removed.length) this.send({ type: "unsubscribe", lanes: removed });
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
		socket.addEventListener("message", (event) =>
			this.receive(event.data, Date.now()),
		);
		socket.addEventListener("error", () =>
			this.observer.error(
				new VisualizationRuntimeProtocolError(
					"Visualization stream connection failed",
				),
			),
		);
		socket.addEventListener("close", () => {
			if (this.socket === socket) this.socket = null;
			if (!this.stopped && this.claims.size)
				this.observer.error(
					new VisualizationRuntimeProtocolError(
						"Visualization stream closed; reconnecting",
					),
				);
			this.scheduleReconnect();
		});
	}

	private receive(raw: unknown, rawReceivedAt: number) {
		try {
			if (typeof raw !== "string")
				throw new VisualizationRuntimeProtocolError(
					"Visualization stream message was not text",
				);
			const decoded = JSON.parse(raw) as unknown;
			// A batch is handed on already decoded. Re-serialising each element only to parse
			// it again doubled the JSON work on exactly the payload batching exists to cheapen.
			if (Array.isArray(decoded)) {
				for (const message of decoded) this.receiveDecoded(message, rawReceivedAt);
				return;
			}
			this.receiveDecoded(decoded, rawReceivedAt);
		} catch (reason) {
			this.observer.error(asError(reason));
			this.closeSocket();
		}
	}

	private receiveDecoded(decoded: unknown, rawReceivedAt: number) {
		try {
			const message = recordAt(decoded, "$");
			const type = stringAt(message.type, "$.type");
			if (type === "hello") {
				if (integerAt(message.protocol_version, "$.protocol_version") !== 1)
					throw new VisualizationRuntimeProtocolError(
						"Visualization stream protocol version is unsupported",
					);
				this.assertExpectedScope(
					decodeVisualizationScope(message.scope, "$.scope"),
					"$.scope",
				);
				return;
			}
			if (type === "snapshot") {
				const scope = decodeVisualizationScope(message.scope, "$.scope");
				this.assertExpectedScope(scope, "$.scope");
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
				assertVisualizationScope(scope, snapshot.scope, "$.snapshot.scope");
				this.snapshots[lane] = snapshot;
				frontendPerformanceDiagnostics.recordStageFrameReceived({
					lane,
					showId: this.expectedShowId,
					scopeActivation: this.diagnosticScopeActivation,
					sourceFrame,
					sourceGeneratedAt,
					publishedAt,
					rawReceivedAt,
				});
				// A complete reconnect snapshot is authoritative even when the output
				// source has been settled for longer than the changing-frame latency
				// budget. Its age means "unchanged", not "unsafe to install".
				this.observer.snapshot(lane, snapshot);
				this.acknowledge(sequence);
				return;
			}
			if (type === "delta") {
				const scope = decodeVisualizationScope(message.scope, "$.scope");
				this.assertExpectedScope(scope, "$.scope");
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
					this.acknowledge(sequence);
					return;
				}
				this.lastSequence = sequence;
				const current = this.snapshots[lane];
				if (!current) {
					this.send({ type: "resynchronize", lane });
					this.acknowledge(sequence);
					return;
				}
				assertVisualizationScope(scope, current.scope, "$.scope");
				const snapshot = applyVisualizationDelta(current, message.delta, lane);
				this.snapshots[lane] = snapshot;
				frontendPerformanceDiagnostics.recordStageFrameReceived({
					lane,
					showId: this.expectedShowId,
					scopeActivation: this.diagnosticScopeActivation,
					sourceFrame,
					sourceGeneratedAt,
					publishedAt,
					rawReceivedAt,
				});
				if (staleVisualizationSource(sourceGeneratedAt)) {
					frontendPerformanceDiagnostics.recordStageFrameApplied(
						sourceGeneratedAt,
						true,
						lane,
						false,
					);
					this.acknowledge(sequence);
					return;
				}
				this.observer.snapshot(lane, snapshot);
				this.acknowledge(sequence);
				return;
			}
			if (type === "structural_invalidation") {
				this.assertExpectedScope(
					decodeVisualizationScope(message.scope, "$.scope"),
					"$.scope",
				);
				for (const lane of this.claims) {
					this.snapshots[lane] = null;
					this.send({ type: "resynchronize", lane });
				}
				return;
			}
			if (type === "heartbeat") {
				this.assertExpectedScope(
					decodeVisualizationScope(message.scope, "$.scope"),
					"$.scope",
				);
				const sequence = integerAt(message.sequence, "$.sequence");
				if (this.lastSequence && sequence !== this.lastSequence + 1)
					for (const lane of this.claims)
						this.send({ type: "resynchronize", lane });
				this.lastSequence = sequence;
				this.acknowledge(sequence);
				return;
			}
			if (type === "error")
				throw new VisualizationRuntimeProtocolError(
					stringAt(message.message, "$.message"),
				);
			throw new VisualizationRuntimeProtocolError(
				`Visualization stream message type ${type} is unsupported`,
			);
		} catch (reason) {
			this.observer.error(asError(reason));
			// An acknowledged stream cannot safely continue after a message that the
			// client could not process: withholding its acknowledgement would leave
			// the server waiting forever. Reconnect to negotiate a fresh snapshot.
			this.closeSocket();
		}
	}

	private sendSubscription() {
		if (!this.claims.size) return;
		this.send({
			type: "subscribe",
			lanes: [...this.claims],
			max_rate_hz: this.maxRateHz,
			include_dynamic_stack: this.includeDynamicStack,
			sparse_dynamic_stack: true,
			batched_messages: true,
			acknowledgements: this.acknowledgementBackpressure,
		});
	}

	private acknowledge(sequence: number) {
		if (!this.acknowledgementBackpressure) return;
		this.pendingAcknowledgement = Math.max(
			this.pendingAcknowledgement,
			sequence,
		);
		if (this.acknowledgementScheduled) return;
		// The observer installs each lane synchronously. A microtask coalesces
		// same-task protocol work while returning credit promptly; the server-side
		// one-publication window still replaces superseded visualization state.
		this.acknowledgementScheduled = true;
		globalThis.queueMicrotask(() => {
			if (!this.acknowledgementScheduled) return;
			this.acknowledgementScheduled = false;
			const acknowledged = this.pendingAcknowledgement;
			this.pendingAcknowledgement = 0;
			if (acknowledged)
				this.send({ type: "acknowledge", sequence: acknowledged });
		});
	}

	private assertExpectedScope(
		scope: NonNullable<VisualizationSnapshot["scope"]>,
		path: string,
	) {
		if (!sameUuid(scope.show_id ?? "", this.expectedShowId))
			throw new WireValidationError(
				path,
				"configured visualization Show scope",
				scope,
			);
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
		this.acknowledgementScheduled = false;
		this.pendingAcknowledgement = 0;
		const socket = this.socket;
		this.socket = null;
		socket?.close();
	}
}

function staleVisualizationSource(timestamp: string) {
	const generatedAt = Date.parse(timestamp);
	return Number.isFinite(generatedAt) && Date.now() - generatedAt > 200;
}

function applyVisualizationDelta(
	current: VisualizationSnapshot,
	value: unknown,
	lane: VisualizationRuntimeLane,
): VisualizationSnapshot {
	const delta = recordAt(value, "$.delta");
	const scope = decodeVisualizationScope(delta.scope, "$.delta.scope");
	assertVisualizationScope(scope, current.scope, "$.delta.scope");
	const preload = booleanAt(delta.preload, "$.delta.preload");
	if (preload !== (lane === "preload"))
		throw new VisualizationRuntimeProtocolError(
			"Visualization delta belongs to a different lane",
		);
	return {
		scope,
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
		dynamic_stack:
			delta.dynamic_stack === undefined
				? current.dynamic_stack
				: decodeDynamicStack(delta.dynamic_stack, "$.delta.dynamic_stack"),
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
		const key = recordAt(entry, `${path}[${index}]`);
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
	if (!upserts.length && !removed.length) return current;
	const removedKeys = new Set(removed);
	const upsertsByKey = new Map(
		upserts.map((value) => [visualizationValueKey(value), value]),
	);
	const retained = current.filter((value) => {
		const key = visualizationValueKey(value);
		return !removedKeys.has(key) && !upsertsByKey.has(key);
	});
	const sortedUpserts = [...upsertsByKey.values()].sort(
		compareVisualizationValues,
	);
	const merged: VisualizationSnapshot["values"] = [];
	let retainedIndex = 0;
	let upsertIndex = 0;
	while (
		retainedIndex < retained.length ||
		upsertIndex < sortedUpserts.length
	) {
		const retainedValue = retained[retainedIndex];
		const upsertValue = sortedUpserts[upsertIndex];
		if (
			upsertValue === undefined ||
			(retainedValue !== undefined &&
				compareVisualizationValues(retainedValue, upsertValue) < 0)
		) {
			merged.push(retainedValue);
			retainedIndex++;
		} else {
			merged.push(upsertValue);
			upsertIndex++;
		}
	}
	return merged;
}

function visualizationValueKey(value: VisualizationSnapshot["values"][number]) {
	return `${value.fixture_id}\u0000${value.attribute}`;
}

function compareVisualizationValues(
	left: VisualizationSnapshot["values"][number],
	right: VisualizationSnapshot["values"][number],
) {
	const leftKey = visualizationValueKey(left);
	const rightKey = visualizationValueKey(right);
	return leftKey < rightKey ? -1 : leftKey > rightKey ? 1 : 0;
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
		scope: decodeVisualizationScope(snapshot.scope, "$.scope"),
		revision: integerAt(snapshot.revision, "$.revision"),
		generated_at: generatedAt,
		grand_master: normalizedAt(snapshot.grand_master, "$.grand_master"),
		blackout: booleanAt(snapshot.blackout, "$.blackout"),
		preload,
		values: decodeValues(snapshot.values, "$.values").sort(
			compareVisualizationValues,
		),
		dynamic_stack:
			snapshot.dynamic_stack === undefined
				? undefined
				: decodeDynamicStack(snapshot.dynamic_stack, "$.dynamic_stack"),
		profile_output_values: decodeValues(
			snapshot.profile_output_values,
			"$.profile_output_values",
		).sort(compareVisualizationValues),
	};
}

function decodeVisualizationScope(value: unknown, path: string) {
	const scope = recordAt(value, path);
	return {
		show_id:
			scope.show_id === null
				? null
				: stringAt(scope.show_id, `${path}.show_id`),
	};
}

function assertVisualizationScope(
	expected: NonNullable<VisualizationSnapshot["scope"]>,
	actual: VisualizationSnapshot["scope"],
	path: string,
) {
	if (expected.show_id !== actual?.show_id)
		throw new WireValidationError(
			path,
			"matching visualization Show scope",
			actual,
		);
}

function decodeVisualizationHttpSource(value: unknown) {
	const snapshot = recordAt(value, "$");
	return {
		frame:
			snapshot.source_frame === undefined
				? null
				: integerAt(snapshot.source_frame, "$.source_frame"),
		timestamp:
			snapshot.source_timestamp === undefined
				? null
				: timestampAt(snapshot.source_timestamp, "$.source_timestamp"),
	};
}

function decodeDynamicStack(value: unknown, path: string) {
	return arrayAt(value, path).map((entry, index) =>
		decodeDynamicStackEntry(entry, `${path}[${index}]`),
	);
}

function decodeDynamicStackEntry(value: unknown, path: string) {
	const entry = recordAt(value, path);
	const optionalBoolean = (field: string) =>
		entry[field] === undefined ? false : booleanAt(entry[field], `${path}.${field}`);
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
		priority:
			entry.priority === undefined
				? 0
				: signedIntegerAt(entry.priority, `${path}.priority`),
		changed_at_millis:
			entry.changed_at_millis === undefined
				? 0
				: integerAt(entry.changed_at_millis, `${path}.changed_at_millis`),
		source: stringAt(entry.source, `${path}.source`),
		dynamic_id:
			entry.dynamic_id === undefined
				? null
				: nullable(entry.dynamic_id, `${path}.dynamic_id`, stringAt),
		pool_number:
			entry.pool_number === undefined
				? null
				: nullable(entry.pool_number, `${path}.pool_number`, integerAt),
		name: stringAt(entry.name, `${path}.name`),
		runtime_instance_id:
			entry.runtime_instance_id === undefined
				? null
				: nullable(
						entry.runtime_instance_id,
						`${path}.runtime_instance_id`,
						stringAt,
					),
		controller_id:
			entry.controller_id === undefined
				? null
				: nullable(entry.controller_id, `${path}.controller_id`, stringAt),
		lane_id:
			entry.lane_id === undefined
				? null
				: nullable(entry.lane_id, `${path}.lane_id`, stringAt),
		size:
			entry.size === undefined
				? null
				: nullable(entry.size, `${path}.size`, numberAt),
		activation_mix:
			entry.activation_mix === undefined
				? null
				: nullable(
						entry.activation_mix,
						`${path}.activation_mix`,
						normalizedAt,
					),
		paused: optionalBoolean("paused"),
		hidden: optionalBoolean("hidden"),
		pending: optionalBoolean("pending"),
		winning: optionalBoolean("winning"),
		summary_count:
			entry.summary_count === undefined
				? undefined
				: integerAt(entry.summary_count, `${path}.summary_count`),
		summary_title:
			entry.summary_title === undefined
				? undefined
				: stringAt(entry.summary_title, `${path}.summary_title`),
		value:
			entry.value === undefined
				? null
				: nullable(entry.value, `${path}.value`, decodeAttributeValue),
		resolved_value:
			entry.resolved_value === undefined
				? null
				: nullable(
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
	const entry = recordAt(value, path);
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
	const bytes = new TextEncoder().encode(text).byteLength;
	if (!response.ok)
		throw new VisualizationRuntimeHttpError(
			text || `${response.status} ${response.statusText}`,
			response.status,
		);
	try {
		return { value: text ? (JSON.parse(text) as unknown) : null, bytes };
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
