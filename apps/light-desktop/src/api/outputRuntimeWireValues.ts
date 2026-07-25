import { assertGrandMaster } from "../features/outputRuntime/projectionValue";
import { numberAt, stringAt } from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

export function outputGrandMasterAt(value: unknown, path: string) {
	const decoded = numberAt(value, path);
	try {
		assertGrandMaster(decoded);
		return decoded;
	} catch {
		throw new WireValidationError(
			path,
			"finite number from 0 through 1",
			value,
		);
	}
}

export function outputTimestampAt(value: unknown, path: string) {
	const timestamp = stringAt(value, path);
	const match =
		/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(?:Z|[+-](\d{2}):(\d{2}))$/u.exec(
			timestamp,
		);
	if (!match || !validCalendarInstant(match))
		throw new WireValidationError(path, "RFC 3339 timestamp", value);
	return timestamp;
}

function validCalendarInstant(match: RegExpExecArray) {
	const [year, month, day, hour, minute, second] = match
		.slice(1, 7)
		.map(Number);
	const offsetHour = match[7] === undefined ? undefined : Number(match[7]);
	const offsetMinute = match[8] === undefined ? undefined : Number(match[8]);
	if (
		year === undefined ||
		month === undefined ||
		day === undefined ||
		hour === undefined ||
		minute === undefined ||
		second === undefined
	)
		return false;
	return (
		month >= 1 &&
		month <= 12 &&
		day >= 1 &&
		day <= new Date(Date.UTC(year, month, 0)).getUTCDate() &&
		hour <= 23 &&
		minute <= 59 &&
		second <= 59 &&
		(offsetHour === undefined || offsetHour <= 23) &&
		(offsetMinute === undefined || offsetMinute <= 59)
	);
}
