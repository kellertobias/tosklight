import { describe, expect, it } from "vitest";
import { normalizePath, ROUTES } from "./routes";

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

	it("keeps the retired page URLs reachable through the new information architecture", () => {
		expect(normalizePath("/layers")).toBe("/media");
		for (const path of ["/audio", "/dmx", "/logs"]) {
			expect(normalizePath(path)).toBe("/settings");
		}
	});

	it("exposes exactly the six operator dock sections in order", () => {
		expect(ROUTES.map((route) => route.label)).toEqual([
			"Dashboard",
			"Media",
			"Library",
			"Visualizers",
			"Text",
			"Settings",
		]);
	});
});
