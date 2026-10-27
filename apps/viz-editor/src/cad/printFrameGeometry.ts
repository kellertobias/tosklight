import { type CadPrintPage, normaliseQuarterTurns, planeDelta, projectPoint } from "./types";

/** Pages keep their own printed basis; the CAD camera can look at that paper from another turn. */
export function pageFrameGeometry(page: CadPrintPage, viewportRotation: number) {
	const savedRotation = page.rotationQuarterTurns ?? 0;
	return {
		centreMillimetres: projectPoint(
			planeDelta(page.centreMillimetres, page.view, savedRotation),
			page.view,
			viewportRotation,
		),
		rotationDegrees: page.view === "top_down"
			? normaliseQuarterTurns(viewportRotation - savedRotation) * 90
			: 0,
	};
}

/** Pointer motion uses the viewport plane; the stored sheet centre and scale use its own plane. */
export function pageFrameDelta(
	page: CadPrintPage,
	viewportRotation: number,
	delta: [number, number],
): [number, number] {
	return projectPoint(
		planeDelta(delta, page.view, viewportRotation),
		page.view,
		page.rotationQuarterTurns ?? 0,
	);
}

/** Turning the printed view keeps its world-space centre and paper format. */
export function withPageViewRotation(page: CadPrintPage, rotation: number): CadPrintPage {
	const next = normaliseQuarterTurns(rotation);
	return {
		...page,
		rotationQuarterTurns: next,
		centreMillimetres: projectPoint(
			planeDelta(page.centreMillimetres, page.view, page.rotationQuarterTurns ?? 0),
			page.view,
			next,
		),
	};
}
