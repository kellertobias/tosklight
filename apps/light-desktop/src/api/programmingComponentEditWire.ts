import type {
	ProgrammingComponentEdit,
	ProgrammingNativeColorEdit,
	ProgrammingScalarEdit,
} from "./generated/light-wire";
import {
	arrayAt,
	enumAt,
	exactRecordAt,
	numberAt,
	recordAt,
} from "./playbackWirePrimitives";
import {
	decodeProgrammingColorXyz,
	decodeProgrammingComponent,
	decodeProgrammingScalarIntent,
	decodeProgrammingTargetReference,
	PROGRAMMING_U32_MAX,
	programmingNonNilUuidAt,
	programmingUnsignedAt,
} from "./programmingIntentWire";
import { WireValidationError } from "./wireValidation";

/** Mirrors the server's per-transaction limit; the server stays authoritative. */
export const MAX_PROGRAMMING_COMPONENT_EDITS = 512;

/**
 * Ordered semantic component edits for one value intent. Edits use the generated wire types
 * unchanged so Normal and Preload clients share one contract with the server.
 */
export interface ProgrammingComponentEditsOperation {
	type: "component_edits";
	edits: readonly ProgrammingComponentEdit[];
}

/** The complete Normal/Preload `apply_intent` operation accepted by the desktop clients. */
export type ProgrammerValueIntentOperation<TValue> =
	| { type: "absolute_set"; value: TValue }
	| { type: "relative_step"; delta: number }
	| ProgrammingComponentEditsOperation;

export type EncodedProgrammerValueIntentOperation<TValue> =
	| { type: "absolute_set"; value: TValue }
	| { type: "relative_step"; delta: number }
	| { type: "component_edits"; edits: ProgrammingComponentEdit[] };

const OPERATION_TYPES = [
	"absolute_set",
	"relative_step",
	"component_edits",
] as const;
const EDIT_KINDS = [
	"activate_angles",
	"scalar",
	"target",
	"coordinates",
	"native",
] as const;
/** These components have no scalar domain and must use their typed edit. */
const NON_SCALAR_COMPONENTS: ReadonlySet<string> = new Set([
	"target_reference",
	"color_wheel",
	"native_color",
]);

/**
 * Validate and copy one `apply_intent` operation for the wire. Every operation keeps its own
 * branch: component edits are never flattened into a whole-value set or a relative step, and
 * their order is preserved exactly. Validation is structural only; family coherence such as
 * Angle/Target exclusivity remains a server decision.
 */
export function encodeProgrammerValueIntentOperation<TValue>(
	operation: ProgrammerValueIntentOperation<TValue>,
	path: string,
): EncodedProgrammerValueIntentOperation<TValue> {
	const type = enumAt(
		recordAt(operation, path).type,
		`${path}.type`,
		OPERATION_TYPES,
	);
	if (type === "component_edits") {
		const edits = (operation as ProgrammingComponentEditsOperation).edits;
		exactRecordAt(operation, path, ["type", "edits"]);
		return {
			type,
			edits: encodeProgrammingComponentEdits(edits, `${path}.edits`),
		};
	}
	// Absolute and relative operations keep their historical copy-only-declared-fields shape.
	if (type === "relative_step") {
		return {
			type,
			delta: numberAt(
				(operation as { delta: unknown }).delta,
				`${path}.delta`,
			),
		};
	}
	return {
		type,
		value: (operation as { value: TValue }).value,
	};
}

/**
 * An empty list is a valid quiet no-op (matching the server); the list itself is never
 * reordered, deduplicated or merged.
 */
export function encodeProgrammingComponentEdits(
	value: unknown,
	path: string,
): ProgrammingComponentEdit[] {
	const edits = arrayAt(value, path);
	if (edits.length > MAX_PROGRAMMING_COMPONENT_EDITS)
		throw new WireValidationError(
			path,
			`at most ${MAX_PROGRAMMING_COMPONENT_EDITS} component edits`,
			value,
		);
	return edits.map((edit, index) =>
		encodeProgrammingComponentEdit(edit, `${path}[${index}]`),
	);
}

export function encodeProgrammingComponentEdit(
	value: unknown,
	path: string,
): ProgrammingComponentEdit {
	const kind = enumAt(recordAt(value, path).kind, `${path}.kind`, EDIT_KINDS);
	if (kind === "activate_angles") {
		exactRecordAt(value, path, ["kind"]);
		return { kind };
	}
	if (kind === "scalar") {
		const edit = exactRecordAt(value, path, ["kind", "component", "operation"]);
		const component = decodeProgrammingComponent(
			edit.component,
			`${path}.component`,
		);
		if (NON_SCALAR_COMPONENTS.has(component.kind))
			throw new WireValidationError(
				`${path}.component.kind`,
				"a component with a scalar domain",
				component.kind,
			);
		return {
			kind,
			component,
			operation: scalarEdit(edit.operation, `${path}.operation`),
		};
	}
	if (kind === "target") {
		const edit = exactRecordAt(value, path, ["kind", "reference"]);
		return {
			kind,
			reference: decodeProgrammingTargetReference(
				edit.reference,
				`${path}.reference`,
			),
		};
	}
	if (kind === "coordinates") {
		const edit = exactRecordAt(value, path, ["kind", "xyz"]);
		return { kind, xyz: decodeProgrammingColorXyz(edit.xyz, `${path}.xyz`) };
	}
	const edit = exactRecordAt(value, path, ["kind", "binding", "operation"]);
	const binding = exactRecordAt(edit.binding, `${path}.binding`, [
		"channel_id",
		"function_id",
	]);
	return {
		kind,
		binding: {
			channel_id: programmingNonNilUuidAt(
				binding.channel_id,
				`${path}.binding.channel_id`,
			),
			function_id: programmingNonNilUuidAt(
				binding.function_id,
				`${path}.binding.function_id`,
			),
		},
		operation: nativeEdit(edit.operation, `${path}.operation`),
	};
}

function scalarEdit(value: unknown, path: string): ProgrammingScalarEdit {
	const edit = exactRecordAt(value, path, ["kind", "value"]);
	const kind = enumAt(edit.kind, `${path}.kind`, ["set", "relative"]);
	if (kind === "relative")
		return { kind, value: numberAt(edit.value, `${path}.value`) };
	return {
		kind,
		value: decodeProgrammingScalarIntent(edit.value, `${path}.value`),
	};
}

function nativeEdit(value: unknown, path: string): ProgrammingNativeColorEdit {
	const edit = exactRecordAt(value, path, ["kind", "value"]);
	const kind = enumAt(edit.kind, `${path}.kind`, ["set", "spread", "relative"]);
	if (kind === "set")
		return { kind, value: programmingUnsignedAt(edit.value, `${path}.value`) };
	if (kind === "spread") {
		const points = arrayAt(edit.value, `${path}.value`);
		if (points.length < 2 || points.length > 4096)
			throw new WireValidationError(
				`${path}.value`,
				"2-4096 native control points",
				edit.value,
			);
		return {
			kind,
			value: points.map((point, index) =>
				programmingUnsignedAt(point, `${path}.value[${index}]`),
			),
		};
	}
	const delta = edit.value;
	if (
		!Number.isSafeInteger(delta) ||
		Math.abs(delta as number) > PROGRAMMING_U32_MAX
	)
		throw new WireValidationError(
			`${path}.value`,
			`integer step within ±${PROGRAMMING_U32_MAX}`,
			delta,
		);
	return { kind, value: delta as number };
}
