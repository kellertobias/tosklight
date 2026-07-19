import type {
	ProgrammingInteractionEventMessage,
	ProgrammingSnapshot,
} from "../features/programmingInteraction/contracts";
import type { ProgrammingEventScope } from "../features/programmingInteraction/transport";
import type {
	EventActionSource,
	EventCapability,
	EventServerMessage,
} from "./generated/light-wire";
import {
	arrayAt,
	enumAt,
	integerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammingChange,
	decodeProgrammingCommandLine,
	decodeProgrammingProjection,
	plainProgrammingStringAt,
	programmingComponentPresence,
	programmingUuidAt,
} from "./programmingWireProjection";
import { WireValidationError } from "./wireValidation";

export { decodeProgrammingCommandLine } from "./programmingWireProjection";

type EventRoute = { capability: EventCapability; id: string };

function decodeCursor(message: Record<string, unknown>, path: string) {
	return integerAt(
		recordAt(message.cursor, `${path}.cursor`).sequence,
		`${path}.cursor.sequence`,
	);
}

function decodeRoute(value: unknown, path: string): EventRoute {
	const route = recordAt(value, path);
	return {
		capability: enumAt(route.capability, `${path}.capability`, [
			"programmer",
			"playback",
			"show",
			"desk",
			"output",
			"system",
		]),
		id: stringAt(route.id, `${path}.id`),
	};
}

function decodeRoutes(event: Record<string, unknown>): EventRoute[] {
	if (event.object == null)
		throw new WireValidationError(
			"$.event.object",
			"Programming interaction route",
			event.object,
		);
	const routes = [decodeRoute(event.object, "$.event.object")];
	if (event.related_objects != null)
		for (const [index, route] of arrayAt(
			event.related_objects,
			"$.event.related_objects",
		).entries())
			routes.push(
				decodeRoute(route, `$.event.related_objects[${index}]`),
			);
	return routes;
}

function expectedRoutes(
	deskId: string,
	components: ReturnType<typeof programmingComponentPresence>,
): EventRoute[] {
	return [
		...(components.commandLine
			? [
					{
						capability: "desk" as const,
						id: `programming-command-line:${deskId}`,
					},
				]
			: []),
		...(components.selection
			? [
					{
						capability: "desk" as const,
						id: `programming-selection:${deskId}`,
					},
				]
			: []),
	];
}

function assertExactRoutes(actual: EventRoute[], expected: EventRoute[]) {
	const matches =
		actual.length === expected.length &&
		actual.every(
			(route, index) =>
				route.capability === expected[index]?.capability &&
				route.id === expected[index]?.id,
		);
	if (!matches)
		throw new WireValidationError(
			"$.event.object",
			`exact Programming routes ${expected.map((route) => route.id).join(", ")}`,
			actual,
		);
}

function assertSubscribedComponent(
	components: ReturnType<typeof programmingComponentPresence>,
	scope: ProgrammingEventScope,
) {
	if (
		!(components.commandLine && scope.commandLine) &&
		!(components.selection && scope.selection)
	)
		throw new WireValidationError(
			"$.event.payload.change",
			"change routed through a subscribed Programming capability",
			components,
		);
}

function validateEventSource(value: unknown, path: string) {
	const source = recordAt(value, path);
	const kind = enumAt(source.kind, `${path}.kind`, ["runtime", "action"]);
	if (kind === "action")
		enumAt<EventActionSource>(source.source, `${path}.source`, [
			"user_interface",
			"keyboard",
			"osc",
			"http",
			"midi",
			"matter",
			"cue",
			"timecode",
			"scheduler",
			"macro",
			"system",
		]);
}

function assertDesk(actual: string, expected: string, path: string) {
	if (actual !== expected)
		throw new WireValidationError(path, `requested desk ${expected}`, actual);
}

function validateProgrammingEnvelope(
	event: Record<string, unknown>,
	expectedDeskId: string,
) {
	assertDesk(
		programmingUuidAt(event.desk_id, "$.event.desk_id"),
		expectedDeskId,
		"$.event.desk_id",
	);
	enumAt(event.class, "$.event.class", ["projection"]);
	enumAt(event.delivery, "$.event.delivery", ["lossless"]);
	plainProgrammingStringAt(event.occurred_at, "$.event.occurred_at");
	validateEventSource(event.source, "$.event.source");
	if (!("correlation_id" in event))
		throw new WireValidationError(
			"$.event.correlation_id",
			"UUID or null",
			undefined,
		);
	if (event.correlation_id != null)
		programmingUuidAt(event.correlation_id, "$.event.correlation_id");
}

function decodeEvent(
	event: Record<string, unknown>,
	expectedDeskId: string,
	scope: ProgrammingEventScope,
): ProgrammingInteractionEventMessage | null {
	const sequence = integerAt(event.sequence, "$.event.sequence");
	const payload = recordAt(event.payload, "$.event.payload");
	const type = stringAt(payload.type, "$.event.payload.type");
	if (type !== "programming_interaction_changed") return null;

	validateProgrammingEnvelope(event, expectedDeskId);
	const change = decodeProgrammingChange(
		payload.change,
		"$.event.payload.change",
	);
	assertDesk(
		change.deskId,
		expectedDeskId,
		"$.event.payload.change.desk_id",
	);
	const components = programmingComponentPresence(change);
	assertSubscribedComponent(components, scope);
	assertExactRoutes(
		decodeRoutes(event),
		expectedRoutes(expectedDeskId, components),
	);
	return { type: "event", sequence, change };
}

/** Decode a Programming event while enforcing the requested desk and view scope. */
export function decodeProgrammingEventMessage(
	value: unknown,
	expectedDeskId: string,
	scope: ProgrammingEventScope,
): ProgrammingInteractionEventMessage | null {
	programmingUuidAt(expectedDeskId, "$.requested_desk_id");
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
			afterSequence: integerAt(
				gap.after_sequence,
				"$.gap.after_sequence",
			),
			oldestAvailable: integerAt(
				gap.oldest_available,
				"$.gap.oldest_available",
			),
			latestSequence: integerAt(
				gap.latest_sequence,
				"$.gap.latest_sequence",
			),
		};
	}
	if (type === "event")
		return decodeEvent(
			recordAt(message.event, "$.event"),
			expectedDeskId,
			scope,
		);
	throw new WireValidationError("$.type", "v2 event message type", type);
}

/** Decode a complete repair snapshot and bind it to the requested desk. */
export function decodeProgrammingInteractionSnapshot(
	value: unknown,
	expectedDeskId: string,
): ProgrammingSnapshot {
	programmingUuidAt(expectedDeskId, "$.requested_desk_id");
	const snapshot = recordAt(value, "$");
	const projection = decodeProgrammingProjection(
		snapshot.projection,
		"$.projection",
	);
	assertDesk(projection.deskId, expectedDeskId, "$.projection.desk_id");
	return {
		cursor: decodeCursor(snapshot, "$"),
		projection,
	};
}
