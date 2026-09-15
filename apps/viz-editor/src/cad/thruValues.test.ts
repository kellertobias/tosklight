import { describe, expect, it } from "vitest";
import { describeRange, describeThru, parseThru, spreadThru } from "./thruValues";

describe("THRU values", () => {
	it("reads one value, or two ends joined by THRU, an ellipsis or three dots", () => {
		expect(parseThru("2.5")).toEqual({ first: 2.5, last: 2.5 });
		expect(parseThru("1 THRU 5")).toEqual({ first: 1, last: 5 });
		expect(parseThru("1 thru -5")).toEqual({ first: 1, last: -5 });
		expect(parseThru("1 … 5")).toEqual({ first: 1, last: 5 });
		expect(parseThru("1…5")).toEqual({ first: 1, last: 5 });
		expect(parseThru("1 ... 5")).toEqual({ first: 1, last: 5 });
		expect(parseThru("1.5...2,5")).toEqual({ first: 1.5, last: 2.5 });
		expect(parseThru("")).toBeNull();
		expect(parseThru("1 THRU")).toBeNull();
		expect(parseThru("1THRU5")).toBeNull();
		expect(parseThru("one")).toBeNull();
	});

	it("spreads a range evenly over the selection", () => {
		expect(spreadThru({ first: 0, last: 3 }, 4)).toEqual([0, 1, 2, 3]);
		expect(spreadThru({ first: 7, last: 9 }, 1)).toEqual([7]);
	});

	it("shows a shared value, an even spread as a range, and the covered range for mixed values", () => {
		expect(describeThru([2, 2, 2], 3)).toEqual({ text: "2", mixed: false });
		expect(describeThru([0, 1.5, 3], 3)).toEqual({ text: "0 THRU 3", mixed: false });
		// Mixed values leave nothing to type over, but still say where the selection stands.
		expect(describeThru([0.4, -1.2, 1.2], 3)).toEqual({
			text: "",
			mixed: true,
			range: { min: -1.2, max: 1.2 },
		});
		expect(describeThru([null, null], 1)).toEqual({ text: "", mixed: false });
		expect(describeThru([null, 20], 1)).toEqual({ text: "", mixed: true, range: { min: 20, max: 20 } });
	});

	it("names a mixed range lowest to highest in the field's unit", () => {
		expect(describeRange({ min: -1.2, max: 1.2 }, 3, "m")).toBe("-1.2m THRU 1.2m");
		expect(describeRange({ min: 15, max: 90 }, 1, "°")).toBe("15° THRU 90°");
		expect(describeRange({ min: 20, max: 20 }, 1, "°")).toBe("20°");
	});
});
