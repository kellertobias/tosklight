import { describe, expect, it, vi } from "vitest";
import {
	buildCadPdf,
	printGridMillimetres,
	printPageTitle,
	printPageLayout,
	printPageScale,
	printScaleDenominator,
	rotatePrintPage,
} from "./print";
import type { CadPrintPage, CadSceneSnapshot } from "./types";
import type { CadUnderlay } from "./underlays";

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
			fixtureProfile: "ToskLight Moving Light",
			mode: "Standard 24ch",
			note: "Hang on downstage truss",
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
	showMountingHardware: true,
});

const underlay: CadUnderlay = {
	id: "venue",
	name: "Ground plan.dxf",
	sourceFormat: "dxf",
	view: "top_down",
	originMillimetres: [0, 0],
	scale: 1,
	rotationDegrees: 0,
	visible: true,
	units: "millimetres",
	geometry: {
		polylines: [
			{
				// A wall along the plan, well inside the page window.
				points: [
					[-2000, -1000],
					[2000, -1000],
				],
				closed: false,
				layer: "A-WALL",
			},
		],
		extentsMillimetres: [-2000, -1000, 2000, -1000],
		units: "millimetres",
	},
};

/** The line work of a page, without its grid, border or title block. */
function drawnLines(pdf: string): string[] {
	return pdf
		.split("\n")
		.filter((line) => /\d+(\.\d+)? \d+(\.\d+)? m /.test(line));
}

describe("CAD PDF export", () => {
	it("prints a placed drawing under the rig, and leaves it off a page that hides it", () => {
		const shown = new TextDecoder().decode(
			buildCadPdf(scene, [page("Shown")], undefined, [underlay]),
		);
		const hidden = new TextDecoder().decode(
			buildCadPdf(
				scene,
				[{ ...page("Hidden"), hiddenUnderlayIds: [underlay.id] }],
				undefined,
				[underlay],
			),
		);
		// The drawing's own stroke weight is only set when a drawing is printed.
		expect(shown).toContain("0.55 G");
		expect(hidden).not.toContain("0.55 G");
		expect(drawnLines(shown).length).toBeGreaterThan(drawnLines(hidden).length);
	});

	it("leaves a drawing of another axis off the page", () => {
		const pdf = new TextDecoder().decode(
			buildCadPdf(scene, [page("Plan")], undefined, [
				{ ...underlay, view: "front_to_back" },
			]),
		);
		expect(pdf).not.toContain("0.55 G");
	});

	it("prints only the slice a page's cut planes leave", () => {
		const upstage = {
			...scene.entities[0],
			id: "upstage",
			logicalFixtureId: "upstage",
			fixtureDisplayId: "7",
			dmxAddress: "2.1",
			// Eight metres along the axis the side view looks down.
			positionMillimetres: [8_000, 0, 1_000] as [number, number, number],
		};
		const both: CadSceneSnapshot = {
			...scene,
			entities: [...scene.entities, upstage],
		};
		const sliced: CadPrintPage = {
			...page("Sliced"),
			view: "left_to_right",
			// Only the downstage half of the drawing, measured along the view.
			cutPlanes: { nearMillimetres: -1_000, farMillimetres: 1_000 },
		};
		const whole: CadPrintPage = {
			...sliced,
			id: "Whole",
			cutPlanes: undefined,
		};
		const slicedPdf = new TextDecoder().decode(buildCadPdf(both, [sliced]));
		const wholePdf = new TextDecoder().decode(buildCadPdf(both, [whole]));
		expect(wholePdf).toContain("ID 7");
		expect(slicedPdf).not.toContain("ID 7");
		expect(slicedPdf).toContain("ID 1");
	});

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
		expect(pdf).toContain("ToskLight Architect");
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

	it("restores dark title text after the white title background and fits long details inside their column", () => {
		const project = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
		const pdf = new TextDecoder().decode(
			buildCadPdf(scene, [page("Venue")], {
				showVersion: "",
				venue: "",
				contactEmail: "",
				contactPhone: "",
				showDate: "",
				lastSavedAt: 0,
				fixtureCount: 1,
				universeCount: 1,
				project,
				showName: "Venue show",
				lightingDesigner: "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
			}),
		);
		expect(pdf).toMatch(/0\.08 0\.08 0\.08 rg\nBT \/F2 12 Tf [^\n]+Venue/);
		expect(pdf).toContain("0 0.71 0.92 rg");
		expect(pdf).toContain("Generated with ToskLight Architect");
		// Long broad glyphs stay readable and are shortened instead of shrinking to invisibility.
		const field = pdf.match(/BT \/F1 ([\d.]+) Tf [^\n]+\(WWWW[^\n]+/);
		expect(Number(field?.[1])).toBeGreaterThanOrEqual(6);
		expect(field?.[0]).toContain("...");
		expect(pdf).not.toContain(project);
	});

	it("uses a custom page title first, with project/show below and one actual export timestamp", () => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date("2026-10-27T09:14:00Z"));
		try {
			const pdf = new TextDecoder().decode(buildCadPdf(scene, [page("Lighting Overview"), page("Page 2")], {
				showName: "Festival", project: "Summer Show", lightingDesigner: "Alex Designer",
				showVersion: "2.4", venue: "Grand Hall", showDate: "2027-01-02",
				contactEmail: "tobias.keller@example.org", contactPhone: "+49 1234 567890",
				lastSavedAt: 1_700_000_000, fixtureCount: 128, universeCount: 4,
			}));
			expect(pdf).toMatch(/BT \/F2 12 Tf [^\n]+\(Lighting Overview\)/);
			expect(pdf).toContain("Summer Show / Festival");
			expect(pdf).toMatch(/BT \/F2 12 Tf [^\n]+\(Summer Show\)/);
			expect(pdf.match(/Printed 2026-10-27 09:14 UTC/g)).toHaveLength(2);
			expect(pdf).not.toContain("Saved  ");
			expect(pdf).toContain("128 fixtures / 4 universes");
			expect(pdf).toContain("tobias.keller@example.org");
			expect(pdf).toContain("+49 1234 567890");
			expect(pdf).toContain("Show date 2027-01-02");
			expect(pdf).toContain("Version 2.4");
			expect(pdf).toContain("Scale 1:");
			expect(pdf).not.toContain("0.02 0.71 1 rg");
		} finally {
			vi.useRealTimers();
		}
		expect(printPageTitle(page("Page 42"))).toBe("");
		expect(printPageTitle(page("Page42"))).toBe("");
	});

	it("keeps portrait footer and every compact title field inside their own columns", () => {
		const sheet = { ...page("W".repeat(120)), orientation: "portrait" as const };
		const pdf = new TextDecoder().decode(buildCadPdf(scene, [sheet], {
			showName: "W".repeat(100), project: "W".repeat(100), lightingDesigner: "W".repeat(100),
			venue: "W".repeat(100), showDate: "W".repeat(100), showVersion: "W".repeat(100),
			contactEmail: "W".repeat(100), contactPhone: "W".repeat(100),
			lastSavedAt: 0, printedAt: 1_793_091_600, fixtureCount: 128, universeCount: 4,
		}));
		const { widthPoints, titleWidthMillimetres } = printPageLayout(sheet);
		const border = 7 * 72 / 25.4;
		const titleLeft = widthPoints - border - titleWidthMillimetres * 72 / 25.4;
		const fields = [...pdf.matchAll(/BT \/F[12] ([\d.]+) Tf ([\d.]+) ([\d.]+) Td \(([^\n]*)\) Tj ET/g)]
			.filter((field) => Number(field[3]) < border + 24 * 72 / 25.4);
		expect(fields.length).toBeGreaterThan(8);
		for (const field of fields) {
			const size = Number(field[1]);
			const x = Number(field[2]);
			const y = Number(field[3]);
			expect(size).toBeGreaterThanOrEqual(6);
			expect(y).toBeGreaterThan(border);
			if (x > titleLeft) {
				// Broad W glyphs are a useful worst-case check for field truncation.
				const columnsEnd = x < titleLeft + 214 ? titleLeft + 208 : widthPoints - border - 6;
				const broadGlyphs = [...field[4]].filter((character) => character === "W").length;
				expect(x + broadGlyphs * size * 0.944).toBeLessThan(columnsEnd);
			} else {
				expect(field[4]).toContain("Scale 1:");
				expect(x + field[4].length * size * 0.6).toBeLessThan(titleLeft);
			}
		}
	});

	it("rotates an A4 page while retaining its drawing scale", () => {
		const landscape = page("Rotate");
		const denominator = printScaleDenominator(landscape);
		const portrait = rotatePrintPage(landscape);
		expect(portrait.orientation).toBe("portrait");
		expect(printScaleDenominator(portrait)).toBe(denominator);
		expect(printPageScale(portrait)).toBeCloseTo(printPageScale(landscape), 12);
		expect(printPageLayout(portrait)).toMatchObject({
			paperWidthMillimetres: 210,
			paperHeightMillimetres: 297,
			titleWidthMillimetres: 116.42,
			titleHeightMillimetres: 24,
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

	it.each(["landscape", "portrait"] as const)("prints a scale legend matching the measured vector geometry on %s paper", (orientation) => {
		const sheet = { ...page("Measured scale"), orientation };
		const measured: CadUnderlay = {
			...underlay,
			geometry: { ...underlay.geometry, polylines: [{ points: [[0, 0], [1000, 0]], closed: false, layer: "MEASURE" }] },
		};
		const pdf = new TextDecoder().decode(buildCadPdf({ ...scene, entities: [] }, [sheet], undefined, [measured]));
		const line = pdf.match(/0\.55 G\n0\.4 w\n([\d.]+) ([\d.]+) m ([\d.]+) ([\d.]+) l S/);
		expect(line).not.toBeNull();
		const paperDistance = (Number(line?.[3]) - Number(line?.[1])) * 25.4 / 72;
		const actualDenominator = 1000 / paperDistance;
		const printedDenominator = printScaleDenominator(sheet);
		expect(Math.abs(actualDenominator - printedDenominator)).toBeLessThan(0.51);
		expect(pdf).toContain(`Scale 1:${printedDenominator}`);
		expect(paperDistance).toBeCloseTo(1000 * printPageScale(sheet), 1);
	});

	it("derives a stable drawing scale and useful grid from each outline", () => {
		expect(printScaleDenominator(page("Scale"))).toBeGreaterThan(1);
		expect(printGridMillimetres(page("Grid"))).toBe(500);
	});

	it("adds deterministic fixture-list pages with repeated headers and notes", () => {
		const entities = Array.from({ length: 35 }, (_, index) => ({
			...scene.entities[0],
			id: `fixture-${index + 1}`,
			logicalFixtureId: `fixture-${index + 1}`,
			fixtureDisplayId: String(index + 1),
			dmxAddress: index === 1 ? "Unpatched" : `1.${index + 1}`,
			note: index === 0 ? "Use frost and safety bond" : "",
		}));
		entities.push({
			...entities[0],
			id: "fixture-1-copy",
			dmxAddress: "2.101",
		});
		const fixtureList = {
			...page("Fixture List"),
			kind: "fixture_list" as const,
		};
		const pdf = new TextDecoder().decode(
			buildCadPdf({ ...scene, entities }, [fixtureList]),
		);
		expect(pdf).toContain("/Count 2");
		expect(pdf.match(/Fixture ID/g)).toHaveLength(2);
		expect(pdf.match(/DMX Patch/g)).toHaveLength(2);
		expect(pdf).toContain("ToskLight Moving Light");
		expect(pdf).toContain("Standard 24ch");
		expect(pdf).toContain("Use frost and safety bond");
		expect(pdf).toContain("1.1 / 2.101");
		expect(pdf).toContain("Unpatched");
	});
});
