import { describe, expect, it } from "vitest";
import { PLANNED_DEMO_FIXTURES } from "../../support/plannedDemoManifest";
import { createPlannedDemoPatchInputs } from "../../support/plannedDemoPatch";

const footprintByName: Record<string, number> = {
	"Robin DLS Profile": 36,
	"JBLED A7": 19,
	"RGBW LED": 5,
	"Robin LEDBeam 150": 22,
	"Dimmer Profile": 1,
	"Dimmer Fresnel": 1,
	"Dimmer PAR Can": 1,
	"Sunstrip LED RGB 42206": 30,
	ACL: 1,
	Blinder: 2,
	Hazer: 2,
	"Media Server": 75,
	"Visualizer Laser": 12,
	"Cold Spark Fountain": 3,
	"Flame Jet": 3,
	"Kabuki Curtain": 1,
};
const uniqueProfiles = [
	...new Map(
		PLANNED_DEMO_FIXTURES.map((fixture) => [
			`${fixture.profile.manufacturer}:${fixture.profile.name}`,
			fixture.profile,
		]),
	).values(),
];
const profiles = uniqueProfiles.map((profile, index) => ({
	id: `profile-${index}`,
	revision: 1,
	manufacturer: profile.manufacturer,
	name: profile.name,
	modes: [
		{
			id: `mode-${index}`,
			name: profile.mode,
			splits: [{ number: 1, footprint: footprintByName[profile.name] }],
		},
	],
})) as any;
const layers = Object.fromEntries(
	[
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
		"Media Servers",
		"Audience Beams",
		"Sunstrips",
		"Lasers",
		"Sparklers",
		"Flame Jets",
		"Kabuki Curtain",
	].map((name) => [name, `layer-${name}`]),
);

describe("overall demo show patch builder", () => {
	it("builds 254 stable controls and 287 physical instances with valid patches", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers);
		expect(built.fixtureRecords).toBe(254);
		expect(built.physicalInstances).toBe(287);
		expect(built.lastUniverse).toBeGreaterThan(1);
		expect(
			new Set(built.fixtures.map((fixture) => fixture.fixture_id)).size,
		).toBe(254);
		const occupied = new Set<string>();
		for (const fixture of built.fixtures) {
			for (const instance of [fixture, ...fixture.multipatch]) {
				for (const split of instance.split_patches) {
					if (split.universe == null) continue;
					const footprint =
						footprintByName[
							PLANNED_DEMO_FIXTURES.find(
								(entry) => entry.number === fixture.fixture_number,
							)!.profile.name
						];
					for (let offset = 0; offset < footprint; offset++) {
						const slot = `${split.universe}:${split.address + offset}`;
						expect(occupied.has(slot)).toBe(false);
						occupied.add(slot);
						expect(split.address + offset).toBeLessThanOrEqual(512);
					}
				}
			}
		}
		expect(occupied.size).toBe(3_378);
	});

	it("configures exactly one local CITP media server", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers);
		expect(
			built.fixtures.find((fixture) => fixture.fixture_number === 1001)
				?.direct_control,
		).toEqual({
			protocol: "citp",
			ip_address: "127.0.0.1",
			port: 4809,
		});
		expect(
			built.fixtures.find((fixture) => fixture.fixture_number === 1002)
				?.direct_control,
		).toBeNull();
	});

	it("keeps Conventional, Stage, Audience, and Auxiliary bands separate", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers);
		const fresnels = built.fixtures.filter(
			(fixture) => fixture.fixture_number >= 1 && fixture.fixture_number <= 8,
		);
		expect(
			fresnels.map((fixture) => [
				fixture.split_patches[0].universe,
				fixture.split_patches[0].address,
			]),
		).toEqual(Array.from({ length: 8 }, (_, index) => [1, index + 1]));
		const movers = built.fixtures.filter(
			(fixture) =>
				fixture.fixture_number >= 101 && fixture.fixture_number <= 107,
		);
		expect(
			movers.every((fixture) => fixture.split_patches[0].universe === 2),
		).toBe(true);
		const manifestByNumber = new Map(
			PLANNED_DEMO_FIXTURES.map((fixture) => [fixture.number, fixture]),
		);
		expect(
			built.fixtures
				.filter(
					(fixture) =>
						manifestByNumber.get(fixture.fixture_number)?.location ===
						"audience",
				)
				.every((fixture) => [5, 6].includes(fixture.split_patches[0].universe)),
		).toBe(true);
		expect(
			built.fixtures
				.filter((fixture) => {
					const manifest = manifestByNumber.get(fixture.fixture_number);
					return (
						manifest?.location === "aux" ||
						manifest?.roles.includes("Sunstrips")
					);
				})
				.every((fixture) => fixture.split_patches[0].universe === 8),
		).toBe(true);
		expect(
			movers.every(
				(fixture) =>
					fixture.rotation.x === 0 &&
					fixture.rotation.y === 0 &&
					fixture.rotation.z === 0,
			),
		).toBe(true);
	});

	it("orders progressive patching in visible layer batches", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers);
		const layerNameById = new Map(
			Object.entries(layers).map(([name, id]) => [id, name]),
		);
		const visibleLayerSequence = built.fixtures
			.map((fixture) => layerNameById.get(fixture.layer_id))
			.filter(
				(layer, index, sequence) =>
					index === 0 || layer !== sequence[index - 1],
			);
		expect(visibleLayerSequence).toEqual([
			"Conventional Light",
			"LED PAR Stage",
			"Profile Stage",
			"Wash Stage",
			"Profile Audience",
			"Wash Audience",
			"LED PAR Audience",
			"Profile Auxilliary",
			"Wash Auxilliary",
			"LED PAR Auxilliary",
			"Sunstrips",
			"Audience Beams",
			"Media Servers",
			"Lasers",
			"Sparklers",
			"Flame Jets",
			"Kabuki Curtain",
		]);
	});

	it("uses the literal two-rack conventional patch and true multipatch policy", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers);
		const byNumber = new Map(
			built.fixtures.map((fixture) => [fixture.fixture_number, fixture]),
		);
		for (const [fixtureNumber, address] of [
			[1, 1],
			[2, 2],
			[3, 3],
			[4, 4],
			[5, 5],
			[6, 6],
			[7, 7],
			[8, 8],
			[9, 9],
			[11, 10],
			[12, 11],
			[13, 12],
			[601, 13],
			[602, 14],
			[603, 15],
			[604, 16],
			[901, 17],
			[701, 21],
			[702, 23],
		] as const)
			expect(byNumber.get(fixtureNumber)?.split_patches[0]).toMatchObject({
				universe: 1,
				address,
			});
		expect(
			byNumber
				.get(901)
				?.multipatch.map((instance) => instance.split_patches[0].address),
		).toEqual([18, 19, 20]);
		for (const fixtureNumber of [9, 13, 601, 602, 603, 604])
			expect(
				byNumber
					.get(fixtureNumber)
					?.multipatch.every((instance) =>
						instance.split_patches.every((patch) => patch.universe == null),
					),
			).toBe(true);
		expect(byNumber.get(801)?.split_patches[0]).toMatchObject({
			universe: 1,
			address: 509,
		});
		expect(byNumber.get(802)?.split_patches[0]).toMatchObject({
			universe: 1,
			address: 511,
		});
	});

	it("places four eight-lamp ACL fans on their literal trusses", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers);
		const acls = built.fixtures.filter(
			(fixture) =>
				fixture.fixture_number >= 601 && fixture.fixture_number <= 604,
		);
		expect(acls.every((fixture) => fixture.multipatch.length === 7)).toBe(true);
		expect(acls.map((fixture) => fixture.layer_id)).toEqual(
			Array.from({ length: 4 }, () => "layer-Conventional Light"),
		);
		expect(
			acls
				.flatMap((fixture) => fixture.multipatch)
				.every((instance) =>
					instance.split_patches.every((patch) => patch.universe == null),
				),
		).toBe(true);
		const front = [acls[3], ...acls[3].multipatch].map(
			(fixture) => fixture.location.x,
		);
		expect(front.filter((x) => x < 0)).toHaveLength(4);
		expect(front.filter((x) => x > 0)).toHaveLength(4);
	});

	it("uses the editable demo script bands and THRU placements", () => {
		const built = createPlannedDemoPatchInputs(profiles, layers, {
			addressBands: {
				dimmers: "2.10 THRU 2.64",
				stageLedPars: "3.65 THRU 3.256",
				stageMovingLights: "4.257 THRU 6.508",
				audience: "7.1 THRU 8.512",
				auxiliary: "9.1 THRU 9.512",
				hazers: "5.509 THRU 5.512",
			},
			placements: [
				{
					targets: "1 THRU 4",
					location: { x: "-4 THRU -1", y: "-2", z: "5" },
				},
				{
					targets: "601 primary THRU multipatch 7",
					location: { x: "-2 THRU 2", y: "3", z: "4" },
					rotation: { x: "0", y: "-21 THRU 21", z: "0" },
				},
			],
			movingFixtureRotation: { x: 0, y: 0, z: 0 },
		});
		const fresnels = built.fixtures.filter(
			(fixture) => fixture.fixture_number >= 1 && fixture.fixture_number <= 4,
		);
		expect(fresnels.map((fixture) => fixture.location.x)).toEqual([
			-4000, -3000, -2000, -1000,
		]);
		expect(fresnels[0].split_patches[0]).toMatchObject({
			universe: 1,
			address: 1,
		});
		const acl = built.fixtures.find(
			(fixture) => fixture.fixture_number === 601,
		);
		expect(
			[acl, ...acl.multipatch].map((fixture) => fixture.location.x),
		).toEqual([-2000, -1429, -857, -286, 286, 857, 1429, 2000]);
		expect(
			[acl, ...acl.multipatch].map((fixture) => fixture.rotation.y),
		).toEqual([-21, -15, -9, -3, 3, 9, 15, 21]);
	});
});
