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
	exactRecordAt,
	integerAt,
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
				`${this.baseUrl}/api/v1/visualization${query}`,
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
		profile_output_values: decodeValues(
			snapshot.profile_output_values,
			"$.profile_output_values",
		),
	};
}

function decodeValues(value: unknown, path: string) {
	return arrayAt(value, path).map((entry, index) =>
		decodeVisualizationValue(entry, `${path}[${index}]`),
	);
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
