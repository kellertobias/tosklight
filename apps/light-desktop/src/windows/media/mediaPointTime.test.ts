import { describe, expect, it } from "vitest";
import {
	clipLengthReadout,
	formatPointTime,
	parsePointFrames,
	parsePointTime,
	pointDisplay,
} from "./mediaPointTime";

describe("the selected clip's length", () => {
	it("reads at the points' rate, past the point range, and never from the points", () => {
		expect(clipLengthReadout({ kind: "known", seconds: 12.4 }, 25).value).toBe(
			"00:12.10",
		);
		// Longer than a 16-bit point reaches: the length is not clamped.
		expect(clipLengthReadout({ kind: "known", seconds: 3_600 }, 25).value).toBe(
			"60:00.00",
		);
		expect(clipLengthReadout({ kind: "known", seconds: 12.4 }, null).value).toBe(
			"12.40 s",
		);
	});

	it("says plainly when there is no length to show", () => {
		expect(clipLengthReadout({ kind: "unknown" }, 25)).toEqual({
			value: "Not reported",
			description: "The Media Server does not report a length for this clip.",
		});
		expect(clipLengthReadout({ kind: "still" }, 25).value).toBe("Still image");
		expect(clipLengthReadout({ kind: "none" }, 25).value).toBe("No clip");
	});
});

describe("Media In/Out point time", () => {
	it("formats frame counts as mm:ss.ff at the server's rate", () => {
		expect(formatPointTime(0, 25)).toBe("00:00.00");
		expect(formatPointTime(1234, 25)).toBe("00:49.09");
		expect(formatPointTime(1500, 25)).toBe("01:00.00");
		expect(formatPointTime(1799, 30)).toBe("00:59.29");
		expect(formatPointTime(65535, 25)).toBe("43:41.10");
		expect(formatPointTime(65535, 1)).toBe("1092:15.00");
		expect(formatPointTime(121, 120)).toBe("00:01.001");
	});

	it("reads typed times back to the same frame counts", () => {
		for (const [fps, frames] of [
			[25, 0],
			[25, 1234],
			[30, 1799],
			[60, 65535],
			[120, 121],
		] as const) {
			expect(parsePointTime(formatPointTime(frames, fps), fps)).toEqual({
				ok: true,
				frames,
			});
		}
		expect(parsePointTime("1:30.12", 25)).toEqual({ ok: true, frames: 2262 });
		expect(parsePointTime("90", 25)).toEqual({ ok: true, frames: 2250 });
		expect(parsePointTime(" 2.5 ", 25)).toEqual({ ok: true, frames: 55 });
	});

	it("refuses times that do not fit the format, the rate, or 16 bits", () => {
		expect(parsePointTime("", 25)).toMatchObject({ ok: false });
		expect(parsePointTime("1:2:3", 25)).toMatchObject({ ok: false });
		expect(parsePointTime("abc", 25)).toMatchObject({ ok: false });
		expect(parsePointTime("01:60.00", 25)).toEqual({
			ok: false,
			error: "Seconds must be below 60",
		});
		expect(parsePointTime("00:01.25", 25)).toEqual({
			ok: false,
			error: "Frames must be below 25 at 25 fps",
		});
		expect(parsePointTime("43:41.11", 25)).toEqual({
			ok: false,
			error: "The longest point is 43:41.10 at 25 fps",
		});
	});

	it("takes whole frame counts while no rate is known", () => {
		expect(parsePointFrames(" 1200 ")).toEqual({ ok: true, frames: 1200 });
		expect(parsePointFrames("12.5")).toMatchObject({ ok: false });
		expect(parsePointFrames("65536")).toMatchObject({ ok: false });
	});

	it("names the reference of each point", () => {
		expect(pointDisplay("start", 50, 25)).toBe("00:02.00");
		expect(pointDisplay("end", 50, 25)).toBe("00:02.00 before end");
		expect(pointDisplay("end", 0, 25)).toBe("End of clip");
		expect(pointDisplay("start", 50, null)).toBe("Frame 50");
		expect(pointDisplay("end", 50, null)).toBe("50 frames before end");
	});
});
