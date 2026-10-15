import { describe, expect, it } from "vitest";
import { decodeTimecodeClock } from "./timecodeClockWire";

const clock = {
	revision: 4,
	selected_source: "artnet:127.0.0.1:2",
	active_source: "artnet:127.0.0.1:2",
	state: "locked",
	current: "00:00:02:22",
	source_rate: "25",
	desk_rate: "44",
	rate_warning: "Timecode converted from 25 to 44 fps",
};
describe("clock wire boundary", () => {
	it("retains exact source, converted time, warning and tolerates additional read fields", () => {
		expect(decodeTimecodeClock({ ...clock, future: true })).toMatchObject(
			clock,
		);
	});
	it.each([
		{ revision: -1 },
		{ revision: NaN },
		{ revision: 1.2 },
		{ selected_source: null },
		{ state: "invented" },
		{ active_source: "other" },
		{ current: "not time" },
		{ current: null },
		{ source_rate: 25 },
		{ rate_warning: false },
	])("refuses malformed or contradictory authority %j", (change) => {
		expect(() => decodeTimecodeClock({ ...clock, ...change })).toThrow();
	});
	it("accepts a source loss without treating a last frame as current authority", () => {
		expect(
			decodeTimecodeClock({
				...clock,
				state: "lost",
				active_source: null,
				current: null,
			}).current,
		).toBeNull();
		expect(() => decodeTimecodeClock({ ...clock, state: "lost" })).toThrow();
	});
});
