import fs from "node:fs/promises";
import { programmerValues, selectFixtures } from "./bench/color/semanticColorScenario";
import { expect, test } from "./bench/core/fixtures";
import { recallPreset } from "./bench/groups-presets/presetRecall";
import {
	PLANNED_DEMO_BENCHMARK_ASSIGNMENTS,
	startPlannedDemoBenchmarkLook,
} from "./support/plannedDemoBenchmark";
import { plannedDemoFamilyNumbers } from "./support/plannedDemoManifest";

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
	expect(patch.fixtures).toHaveLength(312);
	expect(physicalInstances).toBe(345);
	expect(await api.showObjects(show.id, "media_server")).toHaveLength(2);
	const surfaces = await api.showObjects<any>(show.id, "media_surface");
	expect(surfaces).toHaveLength(3);
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
	expect(
		surfaces.find((surface) => surface.body.name === "Upstage Header Screen")
			?.body.sections,
	).toHaveLength(1);
	// The scenery lives in the patch as Venue fixtures; no standalone venue records remain.
	expect(await api.showObjects(show.id, "venue")).toHaveLength(0);
	const scenery = (model: string) =>
		patch.fixtures.filter((fixture) => fixture.definition.model === model);
	// Every two-metre span of a truss run is its own Venue object.
	expect(scenery("Four-Point Truss")).toHaveLength(20);
	expect(scenery("Curtain 5 m")).toHaveLength(4);
	expect(scenery("Stage Element 2 × 1 m")).toHaveLength(20);
	expect(
		patch.fixtures.filter((fixture) => fixture.name?.includes("Railing")),
	).toHaveLength(8);
	expect(
		patch.fixtures.find((fixture) => fixture.name === "Audience Mirror Ball")
			?.location,
	).toEqual({
		x: 0,
		y: -3000,
		z: 4500,
	});

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
	expect(byNumber.get(1401)?.definition.model).toBe("Kabuki");

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

test("OVERALL-DEMO-PACKAGED @api › the shipped demo's universal Colour presets recall on any fixture type", async ({
	api,
}) => {
	const bytes = await fs.readFile(
		new URL("../assets/demo.show", import.meta.url),
	);
	const show = await api.createShow<{ id: string }>({
		name: `canonical-demo-presets-${crypto.randomUUID()}`,
		data_base64: bytes.toString("base64"),
		overwrite: false,
	});
	await api.openShow(show.id, { transition: "hold_current" });
	const patch = await api.patch();
	const byNumber = new Map(
		patch.fixtures.flatMap((fixture) =>
			fixture.fixture_number == null ? [] : [[fixture.fixture_number, fixture]],
		),
	);
	// A universal preset names no fixture. One fixture of each colour family, plus the Robin
	// LEDBeam 150 (451), which per-fixture demo presets never named.
	const selected = [
		...(["led", "wash", "profile"] as const).map(
			(family) => plannedDemoFamilyNumbers(family)[0],
		),
		451,
	].map((number) => {
		const fixture = byNumber.get(number);
		if (!fixture) throw new Error(`no demo fixture ${number}`);
		return fixture;
	});
	await selectFixtures(
		api,
		show.id,
		selected.map((fixture) => fixture.fixture_id),
	);
	await recallPreset(api, {
		surface: "api",
		showId: show.id,
		preset: { objectId: "2.1", family: "Color", number: 1 },
	});
	const colored = new Set(
		(await programmerValues(api))
			.filter((value) => value.attribute === "color")
			.map((value) => value.fixture_id),
	);
	for (const fixture of selected) {
		const owners = [
			fixture.fixture_id,
			...(fixture.logical_heads ?? []).map((head) => head.fixture_id),
		];
		expect(
			owners.some((owner) => colored.has(owner)),
			`${fixture.name} receives the Red preset`,
		).toBe(true);
	}
});

test("OVERALL-DEMO-PACKAGED @api › every shipped demo Dynamic starts on its Group under programming contract 1", async ({
	api,
}) => {
	const bytes = await fs.readFile(
		new URL("../assets/demo.show", import.meta.url),
	);
	const show = await api.createShow<{ id: string }>({
		name: `canonical-demo-dynamics-${crypto.randomUUID()}`,
		data_base64: bytes.toString("base64"),
		overwrite: false,
	});
	await api.openShow(show.id, { transition: "hold_current" });
	const dynamics = await api.showObjects<{
		name: string;
		lanes: Array<{ attribute?: string }>;
	}>(show.id, "dynamic");
	expect(dynamics).toHaveLength(30);
	// TL-648: family lanes are typed; only Intensity keeps scalar lanes.
	expect(
		dynamics.flatMap((dynamic) =>
			dynamic.body.lanes.flatMap((lane) =>
				lane.attribute === undefined || lane.attribute === "intensity"
					? []
					: [`${dynamic.body.name}: ${lane.attribute}`],
			),
		),
	).toEqual([]);
	for (const dynamic of dynamics)
		await api.request(
			"POST",
			`/api/v2/dynamics/${encodeURIComponent(dynamic.id)}/start`,
			{ targets: [] },
			true,
			undefined,
			{ showId: show.id },
		);
	const runtime = await api.request<{
		instances: Array<{ dynamic_id: string; targets: string[] }>;
	}>("GET", "/api/v2/dynamics/runtime", undefined, true, undefined, {
		showId: show.id,
	});
	expect(
		dynamics
			.filter(
				(dynamic) =>
					!runtime.instances.some(
						(instance) =>
							instance.dynamic_id === dynamic.id && instance.targets.length > 0,
					),
			)
			.map((dynamic) => dynamic.body.name),
	).toEqual([]);
});
