import { describe, expect, it } from "vitest";
import { buildCadPdf } from "./print";
import type { CadPrintPage, CadSceneSnapshot } from "./types";

const scene: CadSceneSnapshot = {
	showId: "show",
	sceneRevision: 1,
	selectionRevision: 1,
	selectedIds: [],
	attachments: [],
	drawings: [],
	entities: [
		{
			id: "fixture",
			name: "Moving Light",
			fixtureNumber: 1,
			fixtureDisplayId: "1",
			dmxAddress: "1.1",
			kind: "fixture",
			fixtureType: "moving_head",
			drawingId: "drawing",
			layerId: "layer",
			selectable: true,
			positionMillimetres: [0, 0, 1000],
			rotationDegrees: [0, 0, 0],
			sizeMillimetres: [400, 400, 600],
			outputDirection: [0, 1, 0],
		},
	],
};

const page = (id: string): CadPrintPage => ({
	id,
	tileId: "tile",
	name: id,
	view: "top_down",
	rotationQuarterTurns: 0,
	centreMillimetres: [0, 0],
	widthMillimetres: 5000,
	included: true,
});

describe("CAD PDF export", () => {
	it("writes one vector page per selected print outline", () => {
		const bytes = buildCadPdf(scene, [page("Page 1"), page("Page 2")]);
		const pdf = new TextDecoder().decode(bytes);
		expect(pdf.startsWith("%PDF-1.4")).toBe(true);
		expect(pdf).toContain("/Count 2");
		expect(pdf.match(/\/Type \/Page\b/g)).toHaveLength(2);
		expect(pdf).toContain(" re W n");
		expect(pdf).toMatch(/\d+(?:\.\d+)? \d+(?:\.\d+)? m .* l/);
		expect(pdf.endsWith("%%EOF\n")).toBe(true);
	});
});
