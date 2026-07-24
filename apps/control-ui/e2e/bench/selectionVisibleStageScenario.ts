import type { Page } from "@playwright/test";
import type { SelectionExpression } from "../../src/features/programmingInteraction/contracts";

export interface StageShiftSelectionResult {
	order: "stage-visible";
	anchor: number;
	target: number;
	selection: readonly number[];
	expression: SelectionExpression | null;
}

export interface SelectionObservation {
	targets: ReadonlyArray<{ number: number }>;
	expression: SelectionExpression | null;
}

export async function waitForObservedStageFixture(
	page: Page,
	observeSelection: () => Promise<SelectionObservation>,
	target: number,
): Promise<SelectionObservation> {
	const deadline = Date.now() + 2_000;
	do {
		const observation = await observeSelection();
		if (observation.targets.some((fixture) => fixture.number === target))
			return observation;
		await page.waitForTimeout(5);
	} while (Date.now() < deadline);
	throw new Error(
		`Timed out observing Stage Shift-click target Fixture ${target}`,
	);
}
