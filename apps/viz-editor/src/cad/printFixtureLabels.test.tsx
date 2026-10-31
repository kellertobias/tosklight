import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { PrintPageLabels } from "./CadPrintPageParts";
import { LABEL_CAPACITY_MESSAGE, boundsOverlap } from "./labelPlacement";
import { fixtureLabelCommands } from "./printFixtureLabelCommands";
import { buildCadPdf } from "./print";
import { FIXTURE_LABEL_SIZE, printFixtureLabelLayout } from "./printFixtureLabels";
import { printPageHeight } from "./types";
import type { CadEntity, CadPrintPage, CadSceneSnapshot } from "./types";

const page: CadPrintPage = {
	id: "ground", tileId: "top", name: "Ground plan", kind: "plan", view: "top_down",
	rotationQuarterTurns: 0, centreMillimetres: [0, 0], widthMillimetres: 18000,
	included: true, orientation: "landscape", showFixtureIds: true,
	showDmxAddresses: true, showMountingHardware: true,
};
function fixture(index: number, change: Partial<CadEntity> = {}): CadEntity {
	return { id: `placement-${index.toString().padStart(4, "0")}`, logicalFixtureId: `fixture-${index}`, fixtureDisplayId: String(index + 1),
		fixtureNumber: index + 1, name: "Moving Light", fixtureProfile: "Default", mode: "Standard", note: "", kind: "fixture", fixtureType: "moving_head",
		drawingId: "drawing", layerId: "layer", selectable: true, positionMillimetres: [0, 0, 4000], rotationDegrees: [0, 0, 0],
		sizeMillimetres: [400, 400, 600], outputDirection: [0, 1, 0], dmxAddress: `1.${index + 1}`, ...change };
}
function scene(entities: CadEntity[]): CadSceneSnapshot {
	return { showId: "show", sceneRevision: 1, selectionRevision: 1, selectedIds: [], attachments: [], drawings: [], entities };
}
afterEach(cleanup);

describe("complete fixture labels shared by paper and CAD", () => {
	it("keeps157 coincident fixture instances readable in the PDF and vector screen overlay", () => {
		const entities = Array.from({ length: 157 }, (_, index) => fixture(index));
		const layout = printFixtureLabelLayout(page, entities);
		expect(layout.unplaced).toEqual([]);
		expect(layout.labels).toHaveLength(157);
		for (let i = 0; i < layout.labels.length; i++) for (const next of layout.labels.slice(i + 1)) expect(boundsOverlap(layout.labels[i].box, next.box)).toBe(false);
		for (const { box } of layout.labels) {
			expect(box.y + box.height).toBeLessThanOrEqual(layout.bounds.y + layout.bounds.height + 0.001);
			for (const obstacle of layout.obstacles) expect(boundsOverlap(box, obstacle)).toBe(false);
		}
		const pdf = new TextDecoder().decode(buildCadPdf(scene(entities), [page]));
		expect([...pdf.matchAll(/\(ID \d+\) Tj/g)]).toHaveLength(157);
		expect([...pdf.matchAll(/\(DMX 1\.\d+\) Tj/g)]).toHaveLength(157);
		expect(pdf).toContain(`/F2 ${FIXTURE_LABEL_SIZE} Tf`);
		const { container } = render(<PrintPageLabels page={page} entities={entities} camera={{ pan: [0, 0], zoom: 0.05 }} height={printPageHeight(page)} millimetrePixels={3} />);
		expect(container.querySelectorAll("[data-fixture-instance]")).toHaveLength(157);
		expect(container.querySelectorAll("line")).toHaveLength(157);
		expect(container.querySelector("rect")).toBeNull();
		expect(fixtureLabelCommands(page, entities, (point) => point).some((command) => /re f/.test(command))).toBe(false);
		expect(screen.queryByRole("status")).toBeNull();
	});

	it("keeps labels clear of neighbouring rotated lamp drawings, without blanking the drawing below", () => {
		const entities = Array.from({ length: 24 }, (_, i) => fixture(i, {
			positionMillimetres: [(i % 8 - 4) * 550, Math.floor(i / 8) * 450, 4000],
			rotationDegrees: [0, 0, i * 15], sizeMillimetres: [500, 700, 600],
		}));
		const layout = printFixtureLabelLayout(page, entities);
		expect(layout.unplaced).toEqual([]);
		for (const label of layout.labels) for (const obstacle of layout.obstacles)
			expect(boundsOverlap(label.box, obstacle, 1)).toBe(false);
	});

	it("keeps multi-patch copies as separate labels and excludes off-page or cut-away fixtures", () => {
		const entities = [fixture(0), fixture(1, { fixtureDisplayId: "1", logicalFixtureId: "fixture-0" }),
			fixture(2, { positionMillimetres: [90000, 0, 4000] }), fixture(3, { positionMillimetres: [0, 0, 9000] })];
		const sliced = { ...page, cutPlanes: { nearMillimetres: -5000, farMillimetres: -3000 } };
		const layout = printFixtureLabelLayout(sliced, entities);
		expect(layout.labels.map((label) => label.key)).toEqual([entities[0].id, entities[1].id]);
		expect(layout.labels.map((label) => label.lines[0])).toEqual(["ID 1", "ID 1"]);
		expect(layout.labels[0].box).not.toEqual(layout.labels[1].box);
	});

	it("fails PDF export explicitly at6pt capacity and gives the screen an actionable count", () => {
		const entities = Array.from({ length: 1000 }, (_, index) => fixture(index));
		expect(() => buildCadPdf(scene(entities), [page])).toThrow(LABEL_CAPACITY_MESSAGE);
		render(<PrintPageLabels page={page} entities={entities} camera={{ pan: [0, 0], zoom: 0.05 }} height={printPageHeight(page)} millimetrePixels={3} />);
		expect(screen.getByRole("status")).toHaveTextContent("labels do not fit");
		expect(screen.getByRole("status")).toHaveTextContent("split the plan into pages");
	});
});
