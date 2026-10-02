import { expect, test } from "@playwright/test";

/**
 * Gate of the semantic programming specs (Position, semantic Color, Focus/Zoom).
 *
 * Since TL-552 the default E2E server reports contract 1 and publishes the semantic family pages;
 * a server that answers `semantic: false` (an older contract-0 build) skips with `reason`. In the
 * `e2e-semantic` project (`npm run test:e2e-semantic`) a missing semantic publication is a failure
 * instead, so a broken server can never pass as a run of skips.
 */
export function requireSemanticContract(published: boolean, reason: string) {
	const project = test.info().project.use as { semanticProgrammingContract?: boolean };
	if (project.semanticProgrammingContract === true) {
		expect(
			published,
			"the E2E semantic test server must publish the semantic family pages",
		).toBe(true);
		return;
	}
	test.skip(!published, reason);
}
