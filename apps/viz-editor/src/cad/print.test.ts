import { describe, expect, it } from "vitest";
import {
	buildCadPdf,
	printGridMillimetres,
	printPageLayout,
	printScaleDenominator,
	rotatePrintPage,
} from "./print";
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
			logicalFixtureId: "fixture",
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
	orientation: "landscape",
	showFixtureIds: true,
	showDmxAddresses: true,
});

describe("CAD PDF export", () => {
	it("writes one vector page per selected print outline", () => {
		const bytes = buildCadPdf(scene, [page("Page 1"), page("Page 2")], {
			showName: "Festival",
			lightingDesigner: "Alex Designer",
			showVersion: "2.4",
			venue: "Grand Hall",
			contactEmail: "alex@example.com",
			contactPhone: "+49 123",
			project: "Summer Show",
			showDate: "2026-08-17",
			lastSavedAt: 1_700_000_000,
			fixtureCount: 128,
			universeCount: 4,
		});
		const pdf = new TextDecoder().decode(bytes);
		expect(pdf.startsWith("%PDF-1.4")).toBe(true);
		expect(pdf).toContain("/Count 2");
		expect(pdf.match(/\/Type \/Page\b/g)).toHaveLength(2);
		expect(pdf).toContain(" re W n");
		expect(pdf).toContain("Tasklight Architect");
		expect(pdf).toContain("Festival");
		expect(pdf).toContain("Alex Designer");
		expect(pdf).toContain("Grand Hall");
		expect(pdf).toContain("ID 1");
		expect(pdf).toContain("DMX 1.1");
		expect(pdf).toContain("0 0.71 0.92 rg");
		expect(pdf).toContain("128 fixtures / 4 universes");
		expect(pdf).toContain("/BaseFont /Helvetica-Bold");
		expect(pdf).toMatch(/\d+(?:\.\d+)? \d+(?:\.\d+)? m .* l/);
		expect(pdf.endsWith("%%EOF\n")).toBe(true);
	});

	it("rotates an A4 page while retaining its drawing scale", () => {
		const landscape = page("Rotate");
		const denominator = printScaleDenominator(landscape);
		const portrait = rotatePrintPage(landscape);
		expect(portrait.orientation).toBe("portrait");
		expect(printScaleDenominator(portrait)).toBe(denominator);
		expect(printPageLayout(portrait)).toMatchObject({
			paperWidthMillimetres: 210,
			paperHeightMillimetres: 297,
			titleWidthMillimetres: 116.42,
			titleHeightMillimetres: 30,
		});
		const pdf = new TextDecoder().decode(buildCadPdf(scene, [portrait]));
		expect(pdf).toContain("/MediaBox [0 0 595.28 841.89]");
	});

	it("omits blank optional project rows rather than printing placeholders", () => {
		const pdf = new TextDecoder().decode(
			buildCadPdf(scene, [page("Blank")], {
				showName: "Named show",
				lightingDesigner: "",
				showVersion: "",
				venue: "",
				contactEmail: "",
				contactPhone: "",
				project: "",
				showDate: "",
				lastSavedAt: 0,
				fixtureCount: 1,
				universeCount: 1,
			}),
		);
		expect(pdf).not.toContain("Show date  ");
		expect(pdf).not.toContain("Venue  ");
		expect(pdf).not.toContain("Lighting designer  ");
		expect(pdf).not.toContain("Untitled.show");
	});

	it("derives a stable drawing scale and useful grid from each outline", () => {
		expect(printScaleDenominator(page("Scale"))).toBeGreaterThan(1);
		expect(printGridMillimetres(page("Grid"))).toBe(500);
	});
});
