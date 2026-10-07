import { defineConfig } from "@playwright/test";
import type { BenchWorkerOptions } from "./tests/bench/core/fixtures";
import base from "./playwright.config";

/**
 * The semantic programming specs on the E2E semantic test server (`npm run test:e2e-semantic`).
 *
 * Extends the repository configuration (canonical results/report paths, browser, timeouts) and
 * runs only the semantic scenarios. Every bench of this project starts the separately built
 * `e2e-semantic-contract` server; a case that finds no semantic publication fails here instead of
 * skipping. Since TL-552 production reports contract 1 as well, so the default `npm run test:e2e`
 * run executes these cases too (the feature is a retained no-op).
 */
export const SEMANTIC_SPECS = [
	/117-position-operator-controls\.spec\.ts$/,
	/112-color-intent\.spec\.ts$/,
	/118-focus-zoom-operator-controls\.spec\.ts$/,
	/119-semantic-color-controls\.spec\.ts$/,
	/120-direct-color-pages\.spec\.ts$/,
	/121-intention-programming-frame-contract\.spec\.ts$/,
	/125-typed-family-values\.spec\.ts$/,
	/128-position-points\.spec\.ts$/,
];
export const SEMANTIC_SCENARIOS = /\b(POSITION-CONTROLS|SEMANTIC-COLOR|DIRECT-COLOR|FOCUS-ZOOM|INTENT-FRAME|TYPED-FAMILY)-\d{3}\b/;

export default defineConfig<BenchWorkerOptions>(base, {
	projects: [
		{
			name: "e2e-semantic",
			testMatch: SEMANTIC_SPECS,
			grep: SEMANTIC_SCENARIOS,
			use: { semanticProgrammingContract: true },
		},
	],
});
