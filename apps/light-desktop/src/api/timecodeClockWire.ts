import type { RuntimeTimecodeClockSnapshot } from "./generated/light-wire";
export type TimecodeClockSnapshot = RuntimeTimecodeClockSnapshot;
export function decodeTimecodeClock(value: unknown): TimecodeClockSnapshot {
	if (!value || typeof value !== "object" || Array.isArray(value))
		throw new Error("Invalid Timecode clock snapshot");
	const item = value as Record<string, unknown>;
	if (!Number.isSafeInteger(item.revision) || (item.revision as number) < 0)
		throw new Error("Invalid Timecode clock revision");
	for (const key of ["selected_source", "desk_rate"])
		if (typeof item[key] !== "string" || !item[key])
			throw new Error(`Invalid Timecode clock ${key}`);
	for (const key of ["active_source", "current", "source_rate", "rate_warning"])
		if (item[key] !== null && (typeof item[key] !== "string" || !item[key]))
			throw new Error(`Invalid Timecode clock ${key}`);
	if (!["internal", "waiting", "locked", "lost"].includes(String(item.state)))
		throw new Error("Invalid Timecode clock state");
	if (
		item.current !== null &&
		!/^\d{2}:\d{2}:\d{2}:\d{2}$/.test(String(item.current))
	)
		throw new Error("Invalid Timecode clock position");
	if (
		item.state === "locked" &&
		(item.active_source !== item.selected_source || !item.current)
	)
		throw new Error("Inconsistent Timecode clock lock");
	if (
		["waiting", "lost"].includes(String(item.state)) &&
		(item.current !== null || item.active_source !== null)
	)
		throw new Error("Inconsistent Timecode clock loss");
	return item as unknown as TimecodeClockSnapshot;
}
