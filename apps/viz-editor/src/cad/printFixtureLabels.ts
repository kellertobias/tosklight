import { visibleEntities } from "./cutPlanes";
import { labelWidth, placeLabels, type LabelRequest } from "./labelPlacement";
import { PRINT_BORDER_MM, PRINT_TITLE_HEIGHT_MM, PT_MM, printPageScale } from "./printLayout";
import { type CadEntity, type CadPrintPage, printPageHeight, printPaperSize, projectPoint } from "./types";

export const FIXTURE_LABEL_SIZE = 6;
export const FIXTURE_LABEL_LINE_HEIGHT = 8;
export const FIXTURE_LABEL_PADDING = 2;

export function fixtureLabelLines(entity: CadEntity, ids: boolean, dmx: boolean) {
	return [ids ? `ID ${entity.fixtureDisplayId}` : "", dmx && entity.dmxAddress !== "—" ? `DMX ${entity.dmxAddress}` : ""].filter(Boolean);
}

/** One layout in PDF points, relative to the page's top-left plan window, also mapped to CAD. */
export function printFixtureLabelLayout(page: CadPrintPage, entities: readonly CadEntity[]) {
	const scale = printPageScale(page) * PT_MM;
	const height = printPageHeight(page);
	const widthPoints = page.widthMillimetres * scale;
	const heightPoints = height * scale;
	const paper = printPaperSize(page);
	// The common safe region also leaves the screen sheet's border and complete footer clear.
	const inset = PRINT_BORDER_MM / paper.width * widthPoints;
	const footer = (PRINT_BORDER_MM + PRINT_TITLE_HEIGHT_MM) / paper.height * heightPoints;
	const bounds = { x: inset, y: inset, width: widthPoints - inset * 2, height: heightPoints - inset - footer };
	const requests: LabelRequest[] = [];
	for (const entity of visibleEntities(entities, page.view, page.cutPlanes)) {
		if (entity.kind === "venue") continue;
		const lines = fixtureLabelLines(entity, page.showFixtureIds, page.showDmxAddresses);
		if (!lines.length) continue;
		const point = projectPoint(entity.positionMillimetres, page.view, page.rotationQuarterTurns);
		const anchor: [number, number] = [
			(point[0] - page.centreMillimetres[0] + page.widthMillimetres / 2) * scale,
			(page.centreMillimetres[1] + height / 2 - point[1]) * scale,
		];
		if (anchor[0] < 0 || anchor[0] > widthPoints || anchor[1] < 0 || anchor[1] > heightPoints) continue;
		requests.push({ key: entity.id, anchor, lines, width: labelWidth(lines, FIXTURE_LABEL_SIZE), height: lines.length * FIXTURE_LABEL_LINE_HEIGHT + FIXTURE_LABEL_PADDING * 2 });
	}
	return { ...placeLabels(requests, bounds), bounds, scale, widthPoints, heightPoints };
}
