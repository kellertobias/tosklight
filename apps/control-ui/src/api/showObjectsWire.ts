import type { EventServerMessage } from "./generated/light-wire";
import type {
	ShowObjectChange,
	ShowObjectKind,
	ShowObjectsEventMessage,
} from "../features/showObjects/contracts";
import { WireValidationError } from "./wireValidation";

function recordAt(value: unknown, path: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value))
		throw new WireValidationError(path, "object", value);
	return value as Record<string, unknown>;
}

function stringAt(value: unknown, path: string): string {
	if (typeof value !== "string" || !value)
		throw new WireValidationError(path, "non-empty string", value);
	return value;
}

function plainStringAt(value: unknown, path: string): string {
	if (typeof value !== "string")
		throw new WireValidationError(path, "string", value);
	return value;
}

function integerAt(value: unknown, path: string): number {
	if (!Number.isSafeInteger(value) || (value as number) < 0)
		throw new WireValidationError(path, "unsigned integer", value);
	return value as number;
}

function booleanAt(value: unknown, path: string): boolean {
	if (typeof value !== "boolean")
		throw new WireValidationError(path, "boolean", value);
	return value;
}

function validateRelatedObjects(event: Record<string, unknown>) {
	const value = event.related_objects;
	if (value == null) return;
	if (!Array.isArray(value))
		throw new WireValidationError("$.event.related_objects", "array", value);
	for (const [index, item] of value.entries()) {
		const path = `$.event.related_objects[${index}]`;
		const object = recordAt(item, path);
		const capability = stringAt(object.capability, `${path}.capability`);
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
		stringAt(object.id, `${path}.id`);
	}
}

function supportedKindAt(value: unknown, path: string): ShowObjectKind | null {
	const kind = stringAt(value, path);
	return kind === "group" || kind === "preset" ? kind : null;
}

function validateGroupBody(value: unknown, path: string) {
	const body = recordAt(value, path);
	if (!Array.isArray(body.fixtures) || body.fixtures.some((id) => typeof id !== "string"))
		throw new WireValidationError(`${path}.fixtures`, "string array", body.fixtures);
	return body;
}

function validatePresetBody(value: unknown, path: string) {
	const body = recordAt(value, path);
	plainStringAt(body.name, `${path}.name`);
	integerAt(body.number, `${path}.number`);
	recordAt(body.values, `${path}.values`);
	return body;
}

function decodeChange(value: unknown, path: string): ShowObjectChange | null {
	const change = recordAt(value, path);
	const kind = supportedKindAt(change.kind, `${path}.kind`);
	if (!kind) return null;
	const deleted = booleanAt(change.deleted, `${path}.deleted`);
	const body = change.body;
	if (deleted && body !== null)
		throw new WireValidationError(`${path}.body`, "null deletion body", body);
	if (!deleted && body == null)
		throw new WireValidationError(`${path}.body`, `${kind} body`, body);
	return {
		kind,
		objectId: stringAt(change.object_id, `${path}.object_id`),
		objectRevision: integerAt(
			change.object_revision,
			`${path}.object_revision`,
		),
		body: deleted
			? null
			: kind === "group"
				? validateGroupBody(body, `${path}.body`)
				: validatePresetBody(body, `${path}.body`),
		deleted,
	} as ShowObjectChange;
}

function decodeCursor(message: Record<string, unknown>, path: string) {
	return integerAt(recordAt(message.cursor, `${path}.cursor`).sequence, `${path}.cursor.sequence`);
}

/** Maps the generated v2 envelope into the feature-owned Group/Preset contract. */
export function decodeShowObjectsEventMessage(
	value: unknown,
): ShowObjectsEventMessage | null {
	const message = recordAt(value, "$");
	const type = stringAt(message.type, "$.type") as EventServerMessage["type"];
	switch (type) {
		case "ready":
		case "repaired":
			return { type, cursor: decodeCursor(message, "$") };
		case "gap": {
			const gap = recordAt(message.gap, "$.gap");
			return {
				type: "gap",
				afterSequence: integerAt(gap.after_sequence, "$.gap.after_sequence"),
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
		case "error":
			return { type: "error", error: stringAt(message.error, "$.error") };
		case "event": {
			const event = recordAt(message.event, "$.event");
			validateRelatedObjects(event);
			const payload = recordAt(event.payload, "$.event.payload");
			const payloadType = stringAt(payload.type, "$.event.payload.type");
			if (
				payloadType !== "show_objects_changed" &&
				payloadType !== "selective_import_applied"
			)
				return null;
			const change = recordAt(payload.change, "$.event.payload.change");
			const rawChanges =
				payloadType === "show_objects_changed" ? change.changes : change.objects;
			if (!Array.isArray(rawChanges))
				throw new WireValidationError(
					`$.event.payload.change.${payloadType === "show_objects_changed" ? "changes" : "objects"}`,
					"array",
					rawChanges,
				);
			return {
				type: "event",
				change: {
					showId: stringAt(change.show_id, "$.event.payload.change.show_id"),
					showRevision: integerAt(
						change.show_revision,
						"$.event.payload.change.show_revision",
					),
					eventSequence: integerAt(event.sequence, "$.event.sequence"),
					changes: rawChanges.flatMap((item, index) => {
						const candidate =
							payloadType === "selective_import_applied"
								? { ...recordAt(item, "$.event.payload.change.objects"), deleted: false }
								: item;
						const decoded = decodeChange(
							candidate,
							`$.event.payload.change.${payloadType === "show_objects_changed" ? "changes" : "objects"}[${index}]`,
						);
						return decoded ? [decoded] : [];
					}),
				},
			};
		}
		default:
			throw new WireValidationError("$.type", "v2 event message type", type);
	}
}
