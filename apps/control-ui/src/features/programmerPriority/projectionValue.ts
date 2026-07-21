import type { ProgrammerPriorityProjection } from "./contracts";
import { ProgrammerPriorityProtocolError } from "./transport";

const RFC3339 =
	/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(?:Z|([+-])(\d{2}):(\d{2}))$/u;

export function canonicalPriorityProjection(
	projection: ProgrammerPriorityProjection,
): ProgrammerPriorityProjection {
	if (!projection.userId) throw protocolError("projection is missing its user");
	assertPriorityRevision(projection.revision);
	assertProgrammerPriority(projection.priority);
	assertPriorityTimestamp(projection.changedAt);
	return Object.freeze({ ...projection });
}

export function samePriorityProjection(
	left: ProgrammerPriorityProjection,
	right: ProgrammerPriorityProjection,
) {
	return (
		left.userId === right.userId &&
		left.revision === right.revision &&
		left.priority === right.priority &&
		left.changedAt === right.changedAt
	);
}

export function assertProgrammerPriority(priority: number) {
	if (!Number.isInteger(priority) || priority < -32_768 || priority > 32_767)
		throw protocolError("must be a signed 16-bit integer");
}

export function assertPriorityRevision(revision: number) {
	if (!Number.isSafeInteger(revision) || revision < 0)
		throw protocolError("revision must be a non-negative safe integer");
}

export function assertPriorityCursor(cursor: number) {
	if (!Number.isSafeInteger(cursor) || cursor < 0)
		throw protocolError("event cursor must be a non-negative safe integer");
}

export function assertPriorityTimestamp(timestamp: string) {
	const match = RFC3339.exec(timestamp);
	if (!match) throw protocolError("changed_at must be an RFC 3339 timestamp");
	const [, year, month, day, hour, minute, second, , offsetHour, offsetMinute] =
		match;
	const values = [year, month, day, hour, minute, second].map(Number);
	const [yearValue, monthValue, dayValue, hourValue, minuteValue, secondValue] =
		values;
	const valid =
		yearValue !== undefined &&
		monthValue !== undefined &&
		dayValue !== undefined &&
		hourValue !== undefined &&
		minuteValue !== undefined &&
		secondValue !== undefined &&
		monthValue >= 1 &&
		monthValue <= 12 &&
		dayValue >= 1 &&
		dayValue <= daysInMonth(yearValue, monthValue) &&
		hourValue <= 23 &&
		minuteValue <= 59 &&
		secondValue <= 59 &&
		(offsetHour === undefined || Number(offsetHour) <= 23) &&
		(offsetMinute === undefined || Number(offsetMinute) <= 59);
	if (!valid) throw protocolError("changed_at is not a valid calendar instant");
}

function daysInMonth(year: number, month: number) {
	if (month === 2)
		return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0) ? 29 : 28;
	return [4, 6, 9, 11].includes(month) ? 30 : 31;
}

function protocolError(message: string) {
	return new ProgrammerPriorityProtocolError(`Programmer priority ${message}`);
}
