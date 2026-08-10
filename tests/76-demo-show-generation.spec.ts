import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "./bench/core/fixtures";
import { activeShowId, loadCanonicalCopy } from "./support/catalog";
import {
	PLANNED_DEMO_BENCHMARK_ASSIGNMENTS,
	PLANNED_DEMO_BENCHMARK_SPEED_GROUPS,
	startPlannedDemoBenchmarkLook,
} from "./support/plannedDemoBenchmark";
import { generatePlannedDemo } from "./support/plannedDemoGenerator";
import { PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES } from "./support/plannedDemoVirtualPlaybackZones";

const DEMO_SHOW_ASSET = process.env.LIGHT_DEMO_SHOW_OUTPUT
	? path.resolve(process.env.LIGHT_DEMO_SHOW_OUTPUT)
	: fileURLToPath(new URL("../assets/demo.show", import.meta.url));

test("DEMO-GENERATOR-001 @api › installs the exact Plan 76 lighting patch from one manifest", async ({
	api,
	bench,
}) => {
	await loadCanonicalCopy(api, bench, "plan-76-generator", "default-stage");
	const showId = await activeShowId(api);
	const layerNames = [
		"Stage & Venue",
		"Trusses",
		"Profile Stage",
		"Profile Audience",
		"Profile Auxilliary",
		"Wash Stage",
		"Wash Audience",
		"Wash Auxilliary",
		"LED PAR Stage",
		"LED PAR Audience",
		"LED PAR Auxilliary",
		"Conventional Light",
	];
	const layers = Object.fromEntries(
		layerNames.map((name, index) => [
			name,
			`00000000-0000-4000-8100-${(index + 1).toString(16).padStart(12, "0")}`,
		]),
	);
	for (const [index, [name, id]] of Object.entries(layers).entries())
		await api.seedShowObject(
			showId,
			"patch_layer",
			id,
			{ id, name, order: index },
			0,
		);

	const generatedShow = await generatePlannedDemo(api, showId, layers);
	expect(generatedShow.outputRoutes).toHaveLength(8);
	expect(await api.showObjects(showId, "route")).toHaveLength(8);
	const generated = generatedShow.patch;
	expect(generated).toMatchObject({
		fixtureRecords: 231,
		physicalInstances: 264,
		firstUniverse: 1,
	});
	expect(generated.lastUniverse).toBeGreaterThan(1);
	expect(generated.occupiedSlots).toBe(2_988);
	const frontLights = generated.fixtures.filter(
		(fixture) => fixture.fixture_number >= 1 && fixture.fixture_number <= 8,
	);
	expect(
		frontLights.map((fixture) => [
			fixture.split_patches[0].universe,
			fixture.split_patches[0].address,
		]),
	).toEqual(Array.from({ length: 8 }, (_, index) => [1, index + 1]));
	expect(generatedShow.scenery).toHaveLength(33);
	const completePatch = await api.patch();
	expect(completePatch.fixtures).toHaveLength(264);
	expect(
		completePatch.fixtures.reduce(
			(count, fixture) => count + 1 + fixture.multipatch.length,
			0,
		),
	).toBe(306);

	const acls = generated.fixtures.filter(
		(fixture) => fixture.fixture_number >= 601 && fixture.fixture_number <= 604,
	);
	expect(acls.map((fixture) => fixture.name)).toEqual([
		"ACL Back Center",
		"ACL Back Outside",
		"ACL Midtruss",
		"ACL Side",
	]);
	expect(acls.every((fixture) => fixture.multipatch.length === 7)).toBe(true);
	const frontInstances = [acls[3], ...acls[3].multipatch];
	expect(
		frontInstances.filter((fixture) => fixture.location.x < 0),
	).toHaveLength(4);
	expect(
		frontInstances.filter((fixture) => fixture.location.x > 0),
	).toHaveLength(4);

	const groupSpecs = generatedShow.groups;
	expect(groupSpecs).toHaveLength(35);
	const groups = await api.showObjects<any>(showId, "group");
	expect(groups).toHaveLength(35);
	expect(
		groups.find((group) => group.body.name === "Beam Show")?.body.fixtures,
	).toHaveLength(34);
	expect(
		groups.find((group) => group.body.name === "Beam Auxiliary Show")?.body
			.fixtures,
	).toHaveLength(4);
	expect(
		groups.find((group) => group.body.name === "Strobe")?.body.fixtures,
	).toEqual([]);

	expect(generatedShow.presets).toEqual({
		colors: 13,
		positions: 7,
		beam: 10,
	});
	const presets = await api.showObjects<any>(showId, "preset");
	expect(presets).toHaveLength(30);
	expect(
		presets.filter((preset) => preset.body.family === "Color"),
	).toHaveLength(13);
	expect(
		presets.find((preset) => preset.body.name === "Tungsten White"),
	).toBeDefined();

	const topology = generatedShow.topology;
	expect(topology.cuelists).toHaveLength(8);
	expect(topology.playbacks).toHaveLength(14);
	const cuelists = await api.showObjects<any>(showId, "cue_list");
	expect(
		cuelists.find((cuelist) => cuelist.body.name === "ACL Chase")?.body,
	).toMatchObject({ mode: "chaser", speed_group: "D", looped: true });
	expect(
		cuelists.find((cuelist) => cuelist.body.name === "ACL Chase")?.body.cues,
	).toHaveLength(4);
	expect(
		cuelists.find((cuelist) => cuelist.body.name === "Front Light")?.body.cues,
	).toHaveLength(1);

	const dynamics = generatedShow.dynamics;
	expect(dynamics).toHaveLength(30);
	const storedDynamics = await api.showObjects<any>(showId, "dynamic");
	expect(storedDynamics).toHaveLength(30);
	expect(
		storedDynamics.find((dynamic) => dynamic.body.name === "Wash Row Waterfall")
			?.body.phase.ordering,
	).toEqual({ type: "grid_linear", angle_degrees: 90 });
	const [page] = await api.showObjects<any>(showId, "playback_page");
	expect(Object.keys(page.body.virtual_playbacks)).toHaveLength(30);
	const exclusionZones = await api.request<{ zones: unknown[] }>(
		"GET",
		"/api/v2/virtual-playback-exclusion-zones",
		undefined,
		true,
		undefined,
		{ showId },
	);
	expect(exclusionZones.zones).toEqual(
		PLANNED_DEMO_VIRTUAL_PLAYBACK_EXCLUSION_ZONES,
	);

	const layout = generatedShow.layout;
	expect(layout.desks.map((desk) => desk.name)).toEqual([
		"Group Programming",
		"Busking",
		"Cue Programming",
		"Programming",
		"Theater",
	]);
	const [storedLayout] = await api.showObjects<any>(showId, "user_layout");
	expect(
		storedLayout.body.desks
			.find((desk: any) => desk.name === "Programming")
			?.panes.map((pane: any) => pane.kind),
	).toEqual(["fixtures", "stage", "dmx"]);

	await api.openDefaultShow({ transition: "hold_current" });
	await api.openShow(showId, { transition: "hold_current" });
	expect(
		PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.map((assignment) => assignment.name),
	).toEqual([
		"ACL Chase",
		"Wash Show Waterfall",
		"Beam Show Circle",
		"Beam Show PWM",
		"LED Show Random",
		"LED Show Random Strobe",
		"Sunstrip Rain",
		"Beam Auxiliary Show Circle",
		"Beam Auxiliary Show PWM",
		"Wash Auxiliary Show Waterfall",
		"Wash Auxiliary Show Random",
		"LED Auxiliary Show Sinus",
	]);
	expect(PLANNED_DEMO_BENCHMARK_SPEED_GROUPS).toEqual({
		A: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
		B: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
		C: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
		D: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
		E: { bpm: 120, multiplier: 1, phaseOriginDegrees: 0 },
	});
	const runtime = await startPlannedDemoBenchmarkLook(api, showId);
	const runtimeSummary = runtime.projections.map((projection: any) => ({
		requested: projection.requested,
		target: projection.target,
		runtime: projection.runtime,
	}));
	expect(runtime.projections).toHaveLength(
		PLANNED_DEMO_BENCHMARK_ASSIGNMENTS.length,
	);
	expect(
		runtime.projections.every((projection: any) =>
			projection.target === "cue_list"
				? projection.runtime?.enabled === true
				: projection.target === "dynamic" &&
					projection.runtime?.state === "active" &&
					projection.runtime?.master > 0 &&
					projection.runtime?.size > 0,
		),
		JSON.stringify(runtimeSummary, null, 2),
	).toBe(true);

	if (process.env.LIGHT_UPDATE_DEMO_SHOW === "1") {
		for (const route of await api.showObjects<any>(showId, "route")) {
			const port = route.body.protocol === "sacn" ? 5568 : 6454;
			await api.seedShowObject(
				showId,
				"route",
				route.id,
				{ ...route.body, destination: `127.0.0.1:${port}` },
				route.revision,
			);
		}
		const generated = await api.downloadShow(showId);
		const temporary = `${DEMO_SHOW_ASSET}.${process.pid}.${crypto.randomUUID()}.tmp`;
		await fs.mkdir(path.dirname(DEMO_SHOW_ASSET), { recursive: true });
		await fs.writeFile(temporary, generated);
		await fs.rename(temporary, DEMO_SHOW_ASSET);
	}
});
