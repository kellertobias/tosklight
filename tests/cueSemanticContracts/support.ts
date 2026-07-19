import { test } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	type PairedScenario,
	pairedScenario,
} from "../../apps/control-ui/e2e/bench/pairedScenario";

export * from "./contractAssertions";
export * from "./playbackFixtures";

export const CUE_SEMANTIC_CONTRACTS =
	"docs/testing/02-cues-tracking-and-arbitration.md";

export function registerPairedCueScenario<State>(
	scenario: PairedScenario<State>,
): void {
	test.describe(CUE_SEMANTIC_CONTRACTS, () => {
		pairedScenario(scenario);
	});
}
