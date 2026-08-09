import { describe, expect, it } from "vitest";
import { ROUTES, normalizePath } from "./routes";

describe("routing", () => {
	it("recognizes every declared route, with or without a trailing slash", () => {
		for (const route of ROUTES) {
			expect(normalizePath(route.path)).toBe(route.path);
			expect(normalizePath(`${route.path}/`)).toBe(route.path);
		}
	});

	it("sends an unknown path to the dashboard rather than to a blank page", () => {
		expect(normalizePath("/not-a-page")).toBe("/");
		expect(normalizePath("")).toBe("/");
	});
});
