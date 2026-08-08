import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";
import {
	fixtureProfileOutputValue,
	fixtureValue,
} from "../fixtureVisualization";
import type { StageFixturePresentation } from "../stageWindow/types";

/**
 * One fixture as a Layout cell shows it: its number, its name, and what it is currently doing.
 *
 * This used to belong to the Stage, back when the desk drew the Stage itself. The renderer draws
 * every Stage now and reads the desk's own output universes rather than this, so the Layout window
 * is the only surface left that projects live values into the interface — and the projection
 * belongs with it.
 */
export function fixturePresentation(
	fixture: PatchedFixture,
	index: number,
	visualization: VisualizationSnapshot | null,
	patchPreviewSelected: boolean,
): StageFixturePresentation {
	const physicalAxis = (axis: "pan" | "tilt", inverted: boolean) => {
		const value = fixtureValue(visualization, fixture, axis);
		return inverted ? 1 - Math.max(0, Math.min(1, value)) : value;
	};
	const profileIntensity = fixtureProfileOutputValue(
		visualization,
		fixture,
		"intensity",
	);
	const intensity = patchPreviewSelected
		? 1
		: (profileIntensity ??
			(visualization?.blackout
				? 0
				: fixtureValue(visualization, fixture, "intensity")) *
				(visualization?.grand_master ?? 1));
	const red = fixtureValue(visualization, fixture, "color.red", 1);
	const green = fixtureValue(visualization, fixture, "color.green", 1);
	const blue = fixtureValue(visualization, fixture, "color.blue", 1);
	return {
		fixtureId: fixture.fixture_id,
		fixtureNumber:
			fixture.virtual_fixture_number != null
				? `0.${fixture.virtual_fixture_number}`
				: (fixture.fixture_number ?? index + 1),
		name: fixture.definition.name ?? fixture.definition.model,
		icon: fixture.definition.icon_asset ?? null,
		color: `rgb(${Math.round(red * 255)},${Math.round(green * 255)},${Math.round(blue * 255)})`,
		dimmer: Math.round(intensity * 100),
		pan: physicalAxis("pan", fixture.invert_pan ?? false),
		tilt: physicalAxis("tilt", fixture.invert_tilt ?? false),
	};
}
