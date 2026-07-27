import type {
	AttributeValue,
	VisualizationSnapshot,
} from "./types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "../features/visualizationRuntime/contracts";
import type { VisualizationRuntimeTransport } from "../features/visualizationRuntime/transport";
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
	stringAt,
} from "./playbackWirePrimitives";
import { decodeAttributeValue } from "./programmerValuesWireProjection";
import { programmingUuidAt } from "./programmingWireProjection";
import { WireValidationError } from "./wireValidation";

export interface HttpVisualizationRuntimeTransportOptions {
	baseUrl: string;
	sessionToken: string;
	showId: string;
	sessionId: string;
	authorityKey: string;
	deskBoundaryToken?: string;
	fetch?: typeof globalThis.fetch;
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
		const query = lane === "preload" ? "?preload=true" : "";
		let response: Response;
		try {
			response = await this.fetchImplementation(
				`${this.baseUrl}/api/v2/output/visualization${query}`,
				{ headers: this.headers() },
			);
		} catch (reason) {
			throw new VisualizationRuntimeHttpError(asError(reason).message, 0);
		}
		const value = await responseValue(response);
		try {
			return decodeVisualizationRuntimeSnapshot(value, lane);
		} catch (reason) {
			if (reason instanceof VisualizationRuntimeProtocolError) throw reason;
			throw new VisualizationRuntimeProtocolError(asError(reason).message);
		}
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
		});
		if (this.options.deskBoundaryToken)
			headers.set("x-light-desk-token", this.options.deskBoundaryToken);
		return headers;
	}
}

export function decodeVisualizationRuntimeSnapshot(
	value: unknown,
	expectedLane: VisualizationRuntimeLane,
): VisualizationSnapshot {
	const snapshot = exactRecordAt(value, "$", [
		"revision",
		"generated_at",
		"grand_master",
		"blackout",
		"preload",
		"values",
		"dynamic_stack",
		"profile_output_values",
	]);
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
	const entry = exactRecordAt(value, path, [
		"fixture_id",
		"attribute",
		"entry_type",
		"priority",
		"changed_at_millis",
		"source",
		"dynamic_id",
		"pool_number",
		"name",
		"runtime_instance_id",
		"controller_id",
		"lane_id",
		"size",
		"activation_mix",
		"paused",
		"hidden",
		"pending",
		"winning",
		"value",
		"resolved_value",
	]);
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
		pool_number: nullable(
			entry.pool_number,
			`${path}.pool_number`,
			integerAt,
		),
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

function decodeVisualizationValue(value: unknown, path: string): {
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
