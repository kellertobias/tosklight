import { WireValidationError } from "./wireValidation";

export function recordAt(
	value: unknown,
	path: string,
): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value))
		throw new WireValidationError(path, "object", value);
	return value as Record<string, unknown>;
}

export function arrayAt(value: unknown, path: string) {
	if (!Array.isArray(value))
		throw new WireValidationError(path, "array", value);
	return value;
}

export function stringAt(value: unknown, path: string) {
	if (typeof value !== "string" || value.length === 0)
		throw new WireValidationError(path, "non-empty string", value);
	return value;
}

export function integerAt(value: unknown, path: string) {
	if (!Number.isSafeInteger(value) || (value as number) < 0)
		throw new WireValidationError(path, "unsigned integer", value);
	return value as number;
}

export function positiveIntegerAt(value: unknown, path: string) {
	const integer = integerAt(value, path);
	if (integer < 1)
		throw new WireValidationError(path, "positive integer", value);
	return integer;
}

export function numberAt(value: unknown, path: string) {
	if (typeof value !== "number" || !Number.isFinite(value))
		throw new WireValidationError(path, "finite number", value);
	return value;
}

export function booleanAt(value: unknown, path: string) {
	if (typeof value !== "boolean")
		throw new WireValidationError(path, "boolean", value);
	return value;
}

export function nullable<T>(
	value: unknown,
	path: string,
	decode: (value: unknown, path: string) => T,
) {
	return value == null ? null : decode(value, path);
}

export function enumAt<const T extends string>(
	value: unknown,
	path: string,
	values: readonly T[],
): T {
	const decoded = stringAt(value, path);
	if (!values.includes(decoded as T))
		throw new WireValidationError(path, values.join(" | "), value);
	return decoded as T;
}
