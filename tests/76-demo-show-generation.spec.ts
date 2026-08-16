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

test("DEMO-GENERATOR-001 @api › installs the one overall Desk and PreViz demo show", async ({
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
	expect(generatedShow.outputRoutes).toHaveLength(9);
	expect(await api.showObjects(showId, "route")).toHaveLength(9);
	const generated = generatedShow.patch;
	expect(generated).toMatchObject({
		fixtureRecords: 253,
		physicalInstances: 286,
		firstUniverse: 1,
	});
	expect(generated.lastUniverse).toBeGreaterThan(1);
	expect(generated.occupiedSlots).toBe(3_377);
	const frontLights = generated.fixtures.filter(
		(fixture) => fixture.fixture_number >= 1 && fixture.fixture_number <= 8,
	);
	expect(
		frontLights.map((fixture) => [
			fixture.split_patches[0].universe,
			fixture.split_patches[0].address,
		]),
	).toEqual(Array.from({ length: 8 }, (_, index) => [1, index + 1]));
	expect(generatedShow.scenery).toHaveLength(43);
	const completePatch = await api.patch();
	expect(completePatch.fixtures).toHaveLength(296);
	expect(
		completePatch.fixtures.reduce(
			(count, fixture) => count + 1 + fixture.multipatch.length,
			0,
		),
	).toBe(344);

	const demoFixtures = (first: number, last: number) =>
		generated.fixtures.filter(
			(fixture) =>
				fixture.fixture_number >= first && fixture.fixture_number <= last,
		);
	expect(demoFixtures(1001, 1002).map((fixture) => fixture.name)).toEqual([
		"Media Server 1",
		"Media Server 2",
	]);
	expect(
		demoFixtures(1001, 1002).map((fixture) => fixture.direct_control),
	).toEqual([
		{
			protocol: "citp",
			ip_address: "127.0.0.1",
			port: 4809,
		},
		null,
	]);
	expect(demoFixtures(1101, 1103).map((fixture) => fixture.name)).toEqual([
		"Laser Left",
		"Laser Center",
		"Laser Right",
	]);
	expect(demoFixtures(1101, 1103).map((fixture) => fixture.location.x)).toEqual(
		[-3_000, 0, 3_000],
	);
	expect(demoFixtures(1101, 1103).map((fixture) => fixture.location.y)).toEqual(
		[4_000, 4_000, 4_000],
	);
	expect(demoFixtures(1101, 1103).map((fixture) => fixture.location.z)).toEqual(
		[1_000, 3_550, 1_000],
	);
	expect(demoFixtures(1201, 1206)).toHaveLength(6);
	expect(demoFixtures(1201, 1206).map((fixture) => fixture.location.y)).toEqual(
		Array(6).fill(-550),
	);
	expect(demoFixtures(1301, 1303)).toHaveLength(3);
	expect(demoFixtures(1301, 1303).map((fixture) => fixture.location.y)).toEqual(
		Array(3).fill(-1_150),
	);
	expect(
		demoFixtures(1301, 1303).every(
			(fixture) => fixture.definition.model === "Flame Jet",
		),
	).toBe(true);
	expect(demoFixtures(451, 458)).toHaveLength(8);
	expect(
		demoFixtures(451, 458).every(
			(fixture) => fixture.definition.model === "Robin LEDBeam 150",
		),
	).toBe(true);
	expect(demoFixtures(1401, 1401)).toEqual([]);

	const storedLayers = await api.showObjects<any>(showId, "patch_layer");
	const layerNameById = new Map(
		storedLayers.map((layer) => [layer.id, layer.body.name]),
	);
	for (const [first, last, layer] of [
		[101, 128, "Profile Stage"],
		[129, 150, "Profile Audience"],
		[151, 154, "Profile Auxilliary"],
		[201, 226, "Wash Stage"],
		[227, 234, "Wash Audience"],
		[243, 246, "Wash Auxilliary"],
		[301, 316, "LED PAR Stage"],
		[317, 416, "LED PAR Audience"],
		[417, 426, "LED PAR Auxilliary"],
		[451, 458, "Audience Beams"],
		[501, 508, "Sunstrips"],
		[1001, 1002, "Media Servers"],
		[1101, 1103, "Lasers"],
		[1201, 1206, "Sparklers"],
		[1301, 1303, "Flame Jets"],
	] as const) {
		expect(
			demoFixtures(first, last).every(
				(fixture) => layerNameById.get(fixture.layer_id) === layer,
			),
			`${first} THRU ${last} must be in ${layer}`,
		).toBe(true);
	}

	const mediaServers = await api.showObjects<any>(showId, "media_server");
	const mediaSources = await api.showObjects<any>(showId, "media_source");
	const mediaSurfaces = await api.showObjects<any>(showId, "media_surface");
	const ledModules = await api.showObjects<any>(showId, "led_module_type");
	expect(mediaServers).toHaveLength(2);
	expect(mediaServers.map((server) => server.body.fixtureId)).toEqual([
		"00000000-0000-4001-8000-0000000003e9",
		"00000000-0000-4001-8000-0000000003ea",
	]);
	expect(mediaSources).toHaveLength(2);
	expect(mediaSources.map((source) => source.body.serverId).sort()).toEqual(
		mediaServers.map((server) => server.id).sort(),
	);
	expect(ledModules).toHaveLength(1);
	const projection = mediaSurfaces.find(
		(surface) => surface.body.name === "Projection Screens",
	);
	const led = mediaSurfaces.find(
		(surface) => surface.body.name === "Sunstrip LED Panels",
	);
	expect(projection.body.sections.map((section: any) => section.name)).toEqual([
		"Projection Screen Left",
		"Projection Screen Right",
	]);
	expect(
		projection.body.sections.every(
			(section: any) =>
				section.type === "projection_screen" &&
				section.module_type_id == null &&
				section.moduleTypeId == null,
		),
	).toBe(true);
	expect(led.body.sections.map((section: any) => section.name)).toEqual([
		"LED Panel Left of Sunstrips",
		"LED Panel Between Sunstrips",
		"LED Panel Right of Sunstrips",
	]);
	expect(
		led.body.sections.every(
			(section: any) =>
				section.type === "led" && section.module_type_id === ledModules[0].id,
		),
	).toBe(true);

	const venue = await api.showObjects<any>(showId, "venue");
	expect(venue.filter((object) => object.body.kind === "truss")).toHaveLength(
		24,
	);
	expect(venue.filter((object) => object.body.kind === "riser")).toHaveLength(
		20,
	);
	expect(venue.filter((object) => object.body.kind === "curtain")).toHaveLength(
		4,
	);
	expect(
		venue
			.filter((object) => object.body.name.startsWith("Audience "))
			.filter((object) => object.body.kind === "truss"),
	).toHaveLength(8);
	expect(
		venue
			.filter((object) => object.body.kind === "curtain")
			.map((object) => object.body.name),
	).toEqual([
		"Back Curtain 1",
		"Back Curtain 2",
		"Stage Left Curtain",
		"Stage Right Curtain",
	]);
	expect(
		venue.find((object) => object.body.kind === "mirror_ball")?.body,
	).toMatchObject({
		name: "Audience Mirror Ball",
		position: { x: 0, y: -3, z: 4.5 },
	});
	const crowd = completePatch.fixtures.find(
		(fixture) => fixture.name === "Dancefloor Crowd",
	);
	expect(crowd).toMatchObject({
		virtual_fixture_number: 38,
		definition: { model: "Crowd Area", mode: "Dancing — Dense" },
		location: { x: 0, y: -3_000, z: 0 },
	});

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
	).toHaveLength(42);
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
