import test from "node:test";
import assert from "node:assert/strict";
import {
	scanSemanticWorldSource,
	semanticWorldMarker,
} from "./test-semantic-world-boundaries.mjs";

test("unmarked low-level protocol scenarios remain explicit migration inventory work", () => {
	assert.deepEqual(
		scanSemanticWorldSource("tests/protocol.spec.ts", `
			import type { Page } from "@playwright/test";
			fetch("/api/v2/output/dmx");
		`),
		[],
	);
});

test("marked scenarios may use only the public semantic world", () => {
	assert.deepEqual(
		scanSemanticWorldSource("tests/migrated.spec.ts", `
			// ${semanticWorldMarker}
			import { scenario } from "../apps/control-ui/e2e/bench/scenario";
			scenario("EXAMPLE-001", "uses intent", async (t) => {
				await t.show.use(Show.Empty);
				await t.app.open();
			});
		`),
		[],
	);
});

test("marked scenarios reject every private interaction family", () => {
	const failures = scanSemanticWorldSource("tests/migrated.spec.ts", `
		// ${semanticWorldMarker}
		import type { Page } from "@playwright/test";
		import { ApiDriver } from "../apps/control-ui/e2e/bench/api";
		fetch("/api/v2/output/dmx");
		page.locator(".private").click(1, 2);
		const fixtureId = "private";
		const encoderSlot = 4;
		api.seedShowObject("show", "group", "1", {});
		dispatch({ type: "private" });
	`);
	assert.equal(failures.length, 9);
});
