import { LABEL_CAPACITY_MESSAGE } from "./labelPlacement";
import { FIXTURE_LABEL_LINE_HEIGHT, FIXTURE_LABEL_PADDING, FIXTURE_LABEL_SIZE, printFixtureLabelLayout } from "./printFixtureLabels";
import { path, text } from "./printPdfOps";
import type { PlanPoint } from "./projection";
import { type CadDrawing, type CadEntity, type CadPrintPage, printPageHeight } from "./types";

/** Leaders and complete vector text are drawn after fixture geometry, inside the plan clip. */
export function fixtureLabelCommands(page: CadPrintPage, entities: readonly CadEntity[], point: (value: PlanPoint) => PlanPoint, drawings: readonly CadDrawing[] = []): string[] {
	const layout = printFixtureLabelLayout(page, entities, drawings);
	if (layout.unplaced.length) throw new Error(`${LABEL_CAPACITY_MESSAGE} ${layout.unplaced.length} labels do not fit.`);
	const origin = point([page.centreMillimetres[0] - page.widthMillimetres / 2, page.centreMillimetres[1] + printPageHeight(page) / 2]);
	const pdfPoint = (p: [number, number]): PlanPoint => [origin[0] + p[0], origin[1] - p[1]];
	const commands = ["0 0.55 0.71 RG", "0 0.71 0.92 rg", "0.3 w"];
	for (const label of layout.labels) commands.push(path(label.leader.map(pdfPoint), false, false));
	for (const label of layout.labels) {
		const { x, y } = label.box;
		label.lines.forEach((line, index) => commands.push(text(line, origin[0] + x + FIXTURE_LABEL_PADDING, origin[1] - y - FIXTURE_LABEL_PADDING - FIXTURE_LABEL_SIZE - index * FIXTURE_LABEL_LINE_HEIGHT, FIXTURE_LABEL_SIZE, true)));
	}
	return commands;
}
