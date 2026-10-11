import { describe, it, expect } from "vitest";
import { playbackSurfacePatch } from "./playbackSurfacePatch";
const original = {
	name: "Pane",
	layout: { rows: [] },
	page_source: { type: "own" as const, page: 3 },
};
describe("sparse surface intents", () => {
	it("creates complete configuration without unrelated surfaces", () => {
		expect(playbackSurfacePatch(undefined, original)).toEqual(original);
	});
	it("sends only layout when taking a reviewed screen layout", () => {
		const layout = {
			rows: [
				{
					first_position: 401,
					count: 20,
					presentation: "buttons" as const,
					button_count: 1,
				},
			],
		};
		expect(playbackSurfacePatch(original, { ...original, layout })).toEqual({
			layout,
		});
	});
	it("preserves absent fields for isolated page source changes", () => {
		const page_source = { type: "follow" as const, surface_id: "main" };
		expect(
			playbackSurfacePatch(original, { ...original, page_source }),
		).toEqual({ page_source });
		expect(playbackSurfacePatch(original, { ...original })).toEqual({});
	});
});
