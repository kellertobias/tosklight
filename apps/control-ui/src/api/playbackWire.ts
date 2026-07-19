import type { PlaybackRuntimeEventMessage } from "../features/playbackRuntime/contracts";
import type {
	EventServerMessage,
	PlaybackActionOutcome,
	PlaybackAddress,
	PlaybackDurability,
	PlaybackOutcome,
	PlaybackRuntimeSnapshot,
	ResolvedPlaybackAddress,
} from "./generated/light-wire";
import {
	arrayAt,
	booleanAt,
	enumAt,
	integerAt,
	nullable,
	positiveIntegerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodePlaybackDesk,
	decodePlaybackProjection,
} from "./playbackWireProjection";
import { WireValidationError } from "./wireValidation";

function decodeCursor(message: Record<string, unknown>, path: string) {
	return integerAt(
		recordAt(message.cursor, `${path}.cursor`).sequence,
		`${path}.cursor.sequence`,
	);
}

function validateRoute(value: unknown, path: string) {
	const route = recordAt(value, path);
	const capability = stringAt(route.capability, `${path}.capability`);
	if (
		!["programmer", "playback", "show", "desk", "output", "system"].includes(
			capability,
		)
	)
		throw new WireValidationError(
			`${path}.capability`,
			"event capability",
			capability,
		);
	return { capability, id: stringAt(route.id, `${path}.id`) };
}

function eventRoutes(event: Record<string, unknown>) {
	const routes =
		event.object == null ? [] : [validateRoute(event.object, "$.event.object")];
	if (event.related_objects != null)
		for (const [index, route] of arrayAt(
			event.related_objects,
			"$.event.related_objects",
		).entries())
			routes.push(validateRoute(route, `$.event.related_objects[${index}]`));
	return routes;
}

function assertRuntimeRoute(
	routes: Array<{ capability: string; id: string }>,
	projection: ReturnType<typeof decodePlaybackProjection>,
) {
	const expected =
		projection.playback_number == null
			? projection.target === "cue_list"
				? `cuelist:${projection.cue_list_id}`
				: projection.requested.kind === "cue_list"
					? `cuelist:${projection.requested.cue_list_id}`
					: `playback:${projection.requested.playback_number}`
			: `playback:${projection.playback_number}`;
	if (
		!routes.some(
			(route) => route.capability === "playback" && route.id === expected,
		)
	)
		throw new WireValidationError(
			"$.event.object",
			`Playback route ${expected}`,
			routes,
		);
}

function decodeEvent(
	event: Record<string, unknown>,
): PlaybackRuntimeEventMessage | null {
	// Only validated fields cross this boundary; unused envelope metadata is discarded.
	const sequence = integerAt(event.sequence, "$.event.sequence");
	const routes = eventRoutes(event);
	const payload = recordAt(event.payload, "$.event.payload");
	const type = stringAt(payload.type, "$.event.payload.type");
	if (type === "playback_runtime_changed") {
		const change = recordAt(payload.change, "$.event.payload.change");
		const projection = decodePlaybackProjection(
			change.projection,
			"$.event.payload.change.projection",
		);
		assertRuntimeRoute(routes, projection);
		return {
			type: "event",
			sequence,
			payload: { type: "runtime", projection },
		};
	}
	if (type === "playback_view_changed") {
		const projection = decodePlaybackDesk(
			payload.projection,
			"$.event.payload.projection",
		);
		const route = `playback-view:${projection.desk_id}`;
		if (!routes.some((item) => item.capability === "desk" && item.id === route))
			throw new WireValidationError(
				"$.event.object",
				`desk route ${route}`,
				routes,
			);
		return { type: "event", sequence, payload: { type: "desk", projection } };
	}
	return null;
}

export function decodePlaybackEventMessage(
	value: unknown,
): PlaybackRuntimeEventMessage | null {
	const message = recordAt(value, "$");
	const type = stringAt(message.type, "$.type") as EventServerMessage["type"];
	if (type === "ready" || type === "repaired")
		return { type, cursor: decodeCursor(message, "$") };
	if (type === "error")
		return { type, error: stringAt(message.error, "$.error") };
	if (type === "gap") {
		const gap = recordAt(message.gap, "$.gap");
		return {
			type,
			afterSequence: integerAt(gap.after_sequence, "$.gap.after_sequence"),
			oldestAvailable: integerAt(
				gap.oldest_available,
				"$.gap.oldest_available",
			),
			latestSequence: integerAt(gap.latest_sequence, "$.gap.latest_sequence"),
		};
	}
	if (type === "event") return decodeEvent(recordAt(message.event, "$.event"));
	throw new WireValidationError("$.type", "v2 event message type", type);
}

export function decodePlaybackSnapshot(
	value: unknown,
): PlaybackRuntimeSnapshot {
	const snapshot = recordAt(value, "$");
	return {
		cursor: { sequence: decodeCursor(snapshot, "$") },
		desk: decodePlaybackDesk(snapshot.desk, "$.desk"),
		projections: arrayAt(snapshot.projections, "$.projections").map(
			(projection, index) =>
				decodePlaybackProjection(projection, `$.projections[${index}]`),
		),
	};
}

export function decodePlaybackOutcome(value: unknown): PlaybackActionOutcome {
	const outcome = recordAt(value, "$");
	return {
		request_id: stringAt(outcome.request_id, "$.request_id"),
		correlation_id: stringAt(outcome.correlation_id, "$.correlation_id"),
		requested: decodeRequestedAddress(outcome.requested, "$.requested"),
		resolved: decodeResolvedAddress(outcome.resolved, "$.resolved"),
		outcome: decodeActionOutcome(outcome.outcome, "$.outcome"),
		durability: enumAt(outcome.durability, "$.durability", [
			"durable",
			"persistence_pending",
		]) satisfies PlaybackDurability,
		projection: decodePlaybackProjection(outcome.projection, "$.projection"),
		desk: nullable(outcome.desk, "$.desk", decodePlaybackDesk),
		event_sequence: nullable(
			outcome.event_sequence,
			"$.event_sequence",
			integerAt,
		),
		desk_event_sequence: nullable(
			outcome.desk_event_sequence,
			"$.desk_event_sequence",
			integerAt,
		),
		replayed: booleanAt(outcome.replayed, "$.replayed"),
	};
}

function decodeRequestedAddress(value: unknown, path: string): PlaybackAddress {
	const address = recordAt(value, path);
	const kind = enumAt(address.kind, `${path}.kind`, [
		"cue_list",
		"playback",
		"current_page",
		"explicit_page",
	]);
	if (kind === "cue_list")
		return {
			kind,
			cue_list_id: stringAt(address.cue_list_id, `${path}.cue_list_id`),
		};
	if (kind === "playback")
		return {
			kind,
			playback_number: positiveIntegerAt(
				address.playback_number,
				`${path}.playback_number`,
			),
		};
	if (kind === "current_page")
		return {
			kind,
			slot: positiveIntegerAt(address.slot, `${path}.slot`),
		};
	return {
		kind,
		page: positiveIntegerAt(address.page, `${path}.page`),
		slot: positiveIntegerAt(address.slot, `${path}.slot`),
	};
}

function decodeResolvedAddress(
	value: unknown,
	path: string,
): ResolvedPlaybackAddress {
	const address = recordAt(value, path);
	const kind = enumAt(address.kind, `${path}.kind`, ["cue_list", "playback"]);
	if (kind === "cue_list")
		return {
			kind,
			cue_list_id: stringAt(address.cue_list_id, `${path}.cue_list_id`),
		};
	return {
		kind,
		playback_number: positiveIntegerAt(
			address.playback_number,
			`${path}.playback_number`,
		),
		page: nullable(address.page, `${path}.page`, positiveIntegerAt),
		slot: nullable(address.slot, `${path}.slot`, positiveIntegerAt),
	};
}

function decodeActionOutcome(value: unknown, path: string): PlaybackOutcome {
	const outcome = recordAt(value, path);
	const status = enumAt(outcome.status, `${path}.status`, [
		"applied",
		"no_change",
		"captured",
	]);
	if (status !== "captured") return { status };
	return {
		status,
		pending: enumAt(outcome.pending, `${path}.pending`, [
			"toggle",
			"go",
			"back",
			"off",
			"on",
			"temporary_on",
			"temporary_off",
		]),
	};
}
