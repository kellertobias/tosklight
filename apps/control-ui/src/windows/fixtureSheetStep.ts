import type { HighlightState } from "../api/types";
import type { FixtureSheetRow } from "./fixtureSheetProjection";

export interface FixtureStepPresentation {
	base: boolean;
	containedBase: boolean;
	containedCurrent: boolean;
	current: boolean;
}

export type FixtureStepPresenter = (
	fixture: FixtureSheetRow,
) => FixtureStepPresentation;

const noStep: FixtureStepPresentation = {
	base: false,
	containedBase: false,
	containedCurrent: false,
	current: false,
};

export function createFixtureStepPresenter(
	highlight: HighlightState | null | undefined,
): FixtureStepPresenter {
	if (highlight?.mode !== "step") return () => noStep;

	const rememberedIds = new Set(
		highlight.remembered.map((fixture) => fixture.fixture_id),
	);
	const currentId =
		highlight.active_fixture?.fixture_id ??
		(highlight.active_index == null
			? null
			: (highlight.remembered[highlight.active_index]?.fixture_id ?? null));

	return (fixture) => ({
		base: rememberedIds.has(fixture.fixtureId),
		containedBase:
			fixture.targetKind === "master" &&
			fixture.childFixtureIds.some((fixtureId) => rememberedIds.has(fixtureId)),
		containedCurrent:
			fixture.targetKind === "master" &&
			currentId != null &&
			fixture.childFixtureIds.includes(currentId),
		current: currentId === fixture.fixtureId,
	});
}
