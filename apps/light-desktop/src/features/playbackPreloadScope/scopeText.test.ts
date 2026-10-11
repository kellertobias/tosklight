import { describe, it, expect } from "vitest";
import { parsePlaybackPreloadScope, preloadScopeText } from "./scopeText";
describe("Preload address eligibility", () => {
	it("defaults all pages and unions overlapping position ranges", () =>
		expect(
			parsePlaybackPreloadScope("1-15,10-20,101-115", true, "invalid ignored"),
		).toEqual({
			position_ranges: [
				{ start: 1, end: 20 },
				{ start: 101, end: 115 },
			],
			pages: null,
		}));
	it("supports sparse positions and optional page range restrictions", () =>
		expect(
			parsePlaybackPreloadScope("401-680,301-305", false, "99,2-4,3"),
		).toEqual({
			position_ranges: [
				{ start: 301, end: 305 },
				{ start: 401, end: 680 },
			],
			pages: [2, 3, 4, 99],
		}));
	it.each([
		"0",
		"1001",
		"9-3",
		"1.2",
		"1-",
	])("rejects invalid position range %s", (v) =>
		expect(() => parsePlaybackPreloadScope(v, true, "")).toThrow());
	it("rejects out-of-range optional page restriction", () =>
		expect(() => parsePlaybackPreloadScope("1-1000", false, "100")).toThrow());
	it("roundtrips exact default display", () =>
		expect(
			preloadScopeText({
				position_ranges: [{ start: 1, end: 1000 }],
				pages: null,
			}),
		).toEqual({ positions: "1-1000", allPages: true, pages: "" }));
	it("preserves explicit empty position or page eligibility", () => {
		expect(parsePlaybackPreloadScope("", true, "")).toEqual({
			position_ranges: [],
			pages: null,
		});
		expect(parsePlaybackPreloadScope("1-1000", false, "").pages).toEqual([]);
	});
});
