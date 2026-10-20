import { beforeEach, expect, it } from "vitest";
import { entityDepthRange, visibleEntities } from "./cutPlanes";
import { previewMarks } from "./depthPreview";
import {
	duplicateStep,
	forgetMoveAxes,
	rememberMoveAxis,
} from "./duplicateStep";
import { entityBounds } from "./marqueeSelection";
import type { CadEntity } from "./types";

const balcony = {
	id: "balcony",
	logicalFixtureId: "balcony",
	positionMillimetres: [1000, 2000, 500],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [6000, 4000, 200],
	scenery: {
		kind: "box",
		chords: 0,
		pattern: "standard",
		architecture: {
			kind: "slab",
			purpose: "balcony",
			outline_millimetres: [
				{ x: -1000, y: -2000 },
				{ x: 7000, y: -2000 },
				{ x: 7000, y: 1000 },
				{ x: -1000, y: 1000 },
			],
			elevation_millimetres: 4200,
			thickness_millimetres: 300,
		},
	},
	venueGeometry: {
		triangles: [
			[
				[-1000, 4200, -2000],
				[7000, 4200, -2000],
				[7000, 4200, 1000],
			],
			[
				[-1000, 4500, -2000],
				[7000, 4500, 1000],
				[-1000, 4500, 1000],
			],
		],
		plan: [
			[
				[-1000, -2000],
				[7000, -2000],
			],
			[
				[7000, -2000],
				[7000, 1000],
			],
			[
				[7000, 1000],
				[-1000, 1000],
			],
			[
				[-1000, 1000],
				[-1000, -2000],
			],
		],
	},
} as CadEntity;
beforeEach(forgetMoveAxes);
it("measures the authored asymmetric outline rather than the carrier box", () => {
	expect(entityBounds(balcony, "top_down")).toEqual({
		minimum: [0, 1000],
		maximum: [8000, 4000],
	});
	expect(duplicateStep([balcony], ["balcony"], "top_down", 0)).toEqual([
		8000, 0, 0,
	]);
});
it("keeps the slab's true elevation and thickness in elevation marquee and duplicate bounds", () => {
	expect(entityBounds(balcony, "front_to_back")).toEqual({
		minimum: [0, 4700],
		maximum: [8000, 5000],
	});
	rememberMoveAxis(["balcony"], [0, 0, 1000], "front_to_back", 0);
	expect(duplicateStep([balcony], ["balcony"], "front_to_back", 0)).toEqual([
		0, 0, -300,
	]);
});
it("turns native vertices once and then adds the placement datum", () => {
	const bounds = entityBounds(
		{ ...balcony, rotationDegrees: [0, 0, 90] },
		"top_down",
	);
	expect(bounds.minimum[0]).toBeCloseTo(-1000);
	expect(bounds.maximum[0]).toBeCloseTo(2000);
	expect(bounds.minimum[1]).toBeCloseTo(1000);
	expect(bounds.maximum[1]).toBeCloseTo(9000);
});

it("slices an elevated slab at its actual height and horizontal extent", () => {
	expect(entityDepthRange(balcony, "top_down")).toEqual({
		near: -5000,
		far: -4700,
	});
	expect(
		visibleEntities([balcony], "top_down", {
			nearMillimetres: -4900,
			farMillimetres: -4800,
		}),
	).toEqual([balcony]);
	expect(
		visibleEntities([balcony], "top_down", {
			nearMillimetres: -1000,
			farMillimetres: 0,
		}),
	).toEqual([]);
	expect(entityDepthRange(balcony, "left_to_right")).toEqual({
		near: 0,
		far: 8000,
	});
});

it("shows the raised balcony at its visible centre in the cut-plane companion preview", () => {
	expect(previewMarks([balcony], "top_down")).toEqual([
		{ id: "balcony", depth: -4850, lateral: 4000 },
	]);
});
