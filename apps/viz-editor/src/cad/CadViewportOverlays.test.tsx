import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { CadEntityLabels } from "./CadViewportOverlays";
import type { CadEntity } from "./types";

afterEach(cleanup);
const reference = "950 · Rear flybar · position reference";
const entity = {
	id: "fixture",
	kind: "fixture", fixtureType: "moving_head", rotationDegrees: [0, 0, 0],
	sizeMillimetres: [400, 400, 600], drawingId: "drawing",
	logicalFixtureId: "fixture",
	fixtureDisplayId: "117",
	dmxAddress: "1.1",
	positionMillimetres: [0, 0, 4000],
	positionReference: reference,
} as CadEntity;
function labels(showFixtureIds: boolean, showDmxAddresses = false) {
	return (
		<CadEntityLabels
			entities={[entity]}
			preview={null}
			view="top_down"
			rotationQuarterTurns={0}
			camera={{ pan: [0, 0], zoom: 0.1 }}
			showFixtureIds={showFixtureIds}
			showDmxAddresses={showDmxAddresses}
		/>
	);
}
it("hides shared position annotations when fixture labels are disabled", () => {
	const { container } = render(labels(false));
	expect(container).toBeEmptyDOMElement();
});
it("shows reference annotations when fixture IDs are explicitly enabled", () => {
	render(labels(true));
	expect(screen.getByText(`Follows 3D Point ${reference}`)).toBeInTheDocument();
	expect(screen.getByText(/ID 117/u)).toBeInTheDocument();
});
it("can show DMX addresses without restoring reference annotations", () => {
	render(labels(false, true));
	expect(screen.getByText("DMX 1.1")).toBeInTheDocument();
	expect(screen.queryByText(/Follows 3D Point/u)).not.toBeInTheDocument();
});
