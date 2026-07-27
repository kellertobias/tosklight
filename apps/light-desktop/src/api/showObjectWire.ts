import type {
	ShowObject,
	ShowObjectKind,
} from "../features/showObjects/contracts";
import { integerAt, recordAt, stringAt } from "./playbackWirePrimitives";
import { decodeShowObjectBody } from "./showObjectBodyWire";
import { WireValidationError } from "./wireValidation";

export function decodeShowObject<K extends ShowObjectKind>(
	value: unknown,
	expectedKind: K,
	path = "$",
): ShowObject<K> {
	const object = recordAt(value, path);
	const kind = stringAt(object.kind, `${path}.kind`);
	if (kind !== expectedKind) invalid(`${path}.kind`, expectedKind, kind);
	const id = stringAt(object.id, `${path}.id`);
	const body = decodeShowObjectBody(
		expectedKind,
		object.body,
		`${path}.body`,
		id,
	);
	const bodyValidationError =
		expectedKind === "dynamic" &&
		typeof (body as unknown as { __validationError?: unknown })
			.__validationError === "string"
			? (body as unknown as { __validationError: string }).__validationError
			: null;
	return {
		...object,
		kind: expectedKind,
		id,
		revision: integerAt(object.revision, `${path}.revision`),
		updated_at: plainStringAt(object.updated_at, `${path}.updated_at`),
		validationError:
			object.validation_error == null
				? bodyValidationError
				: plainStringAt(object.validation_error, `${path}.validation_error`),
		body,
	} as ShowObject<K>;
}

function plainStringAt(value: unknown, path: string) {
	if (typeof value !== "string") invalid(path, "string", value);
	return value;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
