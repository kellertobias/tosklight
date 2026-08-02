import { describe, expect, it } from "vitest";
import {
	buildProductDemoEditTimeline,
	PRODUCT_DEMO_SCRIPT,
} from "./productDemoScenario";

function titleMarkers() {
	return new Map(
		PRODUCT_DEMO_SCRIPT.sections.map((section, index) => [
			section.id,
			index * 10_000,
		]),
	);
}

describe("product-demo edit timeline", () => {
	it("keeps deterministic chapter frames while fixing the repeated layer window at five seconds", () => {
		const markers = titleMarkers();
		const timeline = buildProductDemoEditTimeline(
			markers,
			new Map([
				["remaining-patch-layers-start", 12_000],
				["remaining-patch-layers-end", 15_000],
				["front-truss-left-start", 15_500],
				["front-truss-left-end", 16_000],
			]),
			PRODUCT_DEMO_SCRIPT.sections.length * 10_000,
		);

		expect(timeline.version).toBe(2);
		expect(timeline.totalFrames).toBe(
			PRODUCT_DEMO_SCRIPT.sections.reduce(
				(total, section) => total + section.frames,
				0,
			) -
				(PRODUCT_DEMO_SCRIPT.sections.length - 1) *
					PRODUCT_DEMO_SCRIPT.transitionFrames,
		);
		expect(timeline.fixedEditWindows).toEqual([
			expect.objectContaining({
				id: "remaining-patch-layers",
				frames: 125,
				durationMillis: 5_000,
			}),
		]);
		const showSetupSegments = timeline.segments.filter(
			(segment) => segment.chapterId === "show-setup",
		);
		expect(
			showSetupSegments.reduce((total, segment) => total + segment.frames, 0),
		).toBe(
			PRODUCT_DEMO_SCRIPT.sections.find(
				(section) => section.id === "show-setup",
			)?.frames,
		);
		expect(
			timeline.boundaries.find(
				(boundary) => boundary.id === "front-truss-left-start",
			),
		).toMatchObject({ sourceMillis: 15_500 });
	});

	it("rejects a fixed edit window without both recording markers", () => {
		expect(() =>
			buildProductDemoEditTimeline(
				titleMarkers(),
				new Map([["remaining-patch-layers-start", 12_000]]),
				PRODUCT_DEMO_SCRIPT.sections.length * 10_000,
			),
		).toThrow("Missing product-demo edit markers for remaining-patch-layers");
	});
});
