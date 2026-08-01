import { expect, test } from "./bench/core/fixtures";
import { activeShowId, loadCanonicalCopy } from "./support/catalog";
import {
	PLANNED_DEMO_BENCHMARK_ASSIGNMENTS,
	PLANNED_DEMO_BENCHMARK_SPEED_GROUPS,
	startPlannedDemoBenchmarkLook,
} from "./support/plannedDemoBenchmark";
import { generatePlannedDemo } from "./support/plannedDemoGenerator";

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
		"Front Lights",
		"Front Profiles",
		"ACLs & Blinder",
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
	const generated = generatedShow.patch;
	expect(generated).toMatchObject({
		fixtureRecords: 262,
		physicalInstances: 301,
		firstUniverse: 1,
	});
	expect(generated.lastUniverse).toBeGreaterThan(1);
	expect(generated.occupiedSlots).toBe(3_783);
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
	expect(completePatch.fixtures).toHaveLength(295);
	expect(
		completePatch.fixtures.reduce(
			(count, fixture) => count + 1 + fixture.multipatch.length,
			0,
		),
	).toBe(343);

	const acls = generated.fixtures.filter(
		(fixture) => fixture.fixture_number >= 601 && fixture.fixture_number <= 604,
	);
	expect(acls.map((fixture) => fixture.name)).toEqual([
		"Back Centre ACL",
		"Back Split ACL",
		"Mid Split ACL",
		"Front Split ACL",
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
	).toHaveLength(50);
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
	expect(topology.cuelists).toHaveLength(7);
	expect(topology.playbacks).toHaveLength(13);
	const cuelists = await api.showObjects<any>(showId, "cue_list");
	expect(
		cuelists.find((cuelist) => cuelist.body.name === "ACL Chase")?.body,
	).toMatchObject({ mode: "chaser", speed_group: "D", looped: true });
	expect(
		cuelists.find((cuelist) => cuelist.body.name === "ACL Chase")?.body.cues,
	).toHaveLength(4);

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

	const layout = generatedShow.layout;
	expect(layout.desks.map((desk) => desk.name)).toEqual([
		"Group Programming",
		"Busking",
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
					projection.runtime?.state === "active",
		),
		JSON.stringify(runtimeSummary, null, 2),
	).toBe(true);
});
