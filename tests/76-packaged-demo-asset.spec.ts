import fs from "node:fs/promises";
import { expect, test } from "./bench/core/fixtures";
import {
	PLANNED_DEMO_BENCHMARK_ASSIGNMENTS,
	startPlannedDemoBenchmarkLook,
} from "./support/plannedDemoBenchmark";

test("PLAN-76-PACKAGED @api › shipped canonical demo retains its exact packaged benchmark contract", async ({
	api,
}) => {
	const bytes = await fs.readFile(
		new URL("../assets/demo.show", import.meta.url),
	);
	const show = await api.createShow<{ id: string }>({
		name: `canonical-demo-benchmark-${crypto.randomUUID()}`,
		data_base64: bytes.toString("base64"),
		overwrite: false,
	});
	await api.openShow(show.id, { transition: "hold_current" });

	const patch = await api.patch();
	const physicalInstances = patch.fixtures.reduce(
		(total, fixture) => total + 1 + (fixture.multipatch?.length ?? 0),
		0,
	);
	expect(patch.fixtures).toHaveLength(262);
	expect(physicalInstances).toBe(301);

	const runtime = await startPlannedDemoBenchmarkLook(api, show.id);
	const projections = runtime.projections as Array<{
		target: string;
		runtime?: { enabled?: boolean; state?: string };
	}>;
	expect(projections).toHaveLength(PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.length);
	expect(
		projections.every((projection) =>
			projection.target === "cue_list"
				? projection.runtime?.enabled === true
				: projection.target === "dynamic" &&
					projection.runtime?.state === "active",
		),
	).toBe(true);
});
