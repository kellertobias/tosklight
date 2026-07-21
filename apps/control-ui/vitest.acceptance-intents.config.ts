import { defineConfig } from "vitest/config";

export default defineConfig({
	test: {
		environment: "node",
		include: [
			"e2e/bench/mapExistingPlaybackToSlot.test.ts",
			"e2e/bench/outputRuntime.test.ts",
			"e2e/bench/programmerPreloadLifecycle.test.ts",
			"e2e/bench/programmerPriority.test.ts",
			"e2e/bench/programmingSelection.test.ts",
			"e2e/bench/programmerValues.test.ts",
			"e2e/bench/presetRecall.test.ts",
		],
	},
});
