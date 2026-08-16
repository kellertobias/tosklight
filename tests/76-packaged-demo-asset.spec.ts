import fs from "node:fs/promises";
import { expect, test } from "./bench/core/fixtures";
import {
	PLANNED_DEMO_BENCHMARK_ASSIGNMENTS,
	startPlannedDemoBenchmarkLook,
} from "./support/plannedDemoBenchmark";

test("OVERALL-DEMO-PACKAGED @api › shipped canonical demo retains the Desk and PreViz contract", async ({
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
	expect(patch.fixtures).toHaveLength(296);
	expect(physicalInstances).toBe(344);
	expect(await api.showObjects(show.id, "media_server")).toHaveLength(2);
	const surfaces = await api.showObjects<any>(show.id, "media_surface");
	expect(surfaces).toHaveLength(2);
	expect(
		surfaces.find((surface) => surface.body.name === "Projection Screens")?.body
			.sections,
	).toHaveLength(2);
	expect(
		surfaces
			.find((surface) => surface.body.name === "Projection Screens")
			?.body.sections.every(
				(section: any) =>
					section.type === "projection_screen" &&
					section.module_type_id == null &&
					section.moduleTypeId == null,
			),
	).toBe(true);
	expect(
		surfaces.find((surface) => surface.body.name === "Sunstrip LED Panels")
			?.body.sections,
	).toHaveLength(3);
	const venue = await api.showObjects<any>(show.id, "venue");
	expect(venue).toHaveLength(57);
	expect(venue.filter((object) => object.body.kind === "truss")).toHaveLength(
		24,
	);
	expect(venue.filter((object) => object.body.kind === "curtain")).toHaveLength(
		4,
	);
	expect(venue.filter((object) => object.body.kind === "riser")).toHaveLength(
		20,
	);
	expect(
		venue.find((object) => object.body.kind === "mirror_ball")?.body.position,
	).toEqual({ x: 0, y: -3, z: 4.5 });

	const byNumber = new Map(
		patch.fixtures.flatMap((fixture) =>
			fixture.fixture_number == null ? [] : [[fixture.fixture_number, fixture]],
		),
	);
	expect(
		Array.from({ length: 8 }, (_, index) => byNumber.get(451 + index)).every(
			(fixture) => fixture?.definition.model === "Robin LEDBeam 150",
		),
	).toBe(true);
	expect(
		Array.from({ length: 3 }, (_, index) => byNumber.get(1301 + index)).every(
			(fixture) => fixture?.definition.model === "Flame Jet",
		),
	).toBe(true);
	expect(byNumber.has(1401)).toBe(false);

	const runtime = await startPlannedDemoBenchmarkLook(api, show.id);
	const projections = runtime.projections as Array<{
		target: string;
		runtime?: {
			enabled?: boolean;
			state?: string;
			master?: number;
			size?: number;
		};
	}>;
	expect(projections).toHaveLength(PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.length);
	expect(
		projections.every((projection) =>
			projection.target === "cue_list"
				? projection.runtime?.enabled === true
				: projection.target === "dynamic" &&
					projection.runtime?.state === "active" &&
					Number(projection.runtime?.master) > 0 &&
					Number(projection.runtime?.size) > 0,
		),
	).toBe(true);
});
