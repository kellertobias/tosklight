import {
	BrowserDiscreteEncoders,
	type DiscreteChoice,
} from "../../apps/control-ui/e2e/bench/discreteEncoderScenario";
import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { BrowserProgrammerSpecials } from "../../apps/control-ui/e2e/bench/programmerSpecialScenario";
import { BrowserSelection } from "../../apps/control-ui/e2e/bench/selectionScenario";
import {
	blankFixtureProfile,
	fixtureDefinitionFromProfileMode,
} from "../../apps/control-ui/src/components/setup/fixtureProfileModel";
import type {
	ChannelFunction,
	FixtureChannel,
	FixtureProfile,
} from "../../apps/control-ui/src/api/types";
import {
	loadCanonicalCopy,
	objects,
	programmer,
	putObject,
} from "../support/catalog";
import { replaceProgrammingSelection } from "../../apps/control-ui/e2e/bench/programmingSelection";
import { applyProgrammerSelectionValue } from "../../apps/control-ui/e2e/bench/programmerValues";

test("BENCH-DISCRETE-SPECIAL-001 @bench @ui › profile-derived discrete values remain semantic through API set and visible release", async ({
	api,
	bench,
	desk,
	page,
}) => {
	const show = await loadCanonicalCopy(api, bench, "bench-06d-discrete");
	const profile = specialProfile();
	const fixtureIds = await installFixtures(api, profile, [901]);
	await replaceProgrammingSelection(api, {
		surface: "api",
		showId: show.id,
		fixtures: fixtureIds,
	});
	await desk.open(api.baseUrl);

	const discrete = new BrowserDiscreteEncoders(
		api,
		new BrowserSelection(api),
		desk,
		page,
	);
	const choices = await discrete.choices("gobo");
	expect(choices).toEqual<DiscreteChoice[]>([
		{ semanticId: "gobo.dots", label: "Dots" },
		{ semanticId: "gobo.open", label: "Open" },
	]);
	await discrete.set("gobo", "gobo.dots");
	await expectProgrammerValue(api, fixtureIds[0], "gobo", {
		kind: "discrete",
		value: "gobo.dots",
	});
	await discrete.releaseVisible("gobo");
	await expectProgrammerReleased(api, fixtureIds[0], "gobo");

	await discrete.set("gobo", "gobo.open");
	await discrete.clear();
	expect((await programmer(api)).values).toEqual([]);
});

test("BENCH-DISCRETE-SPECIAL-002 @bench @ui › Position, Beam, Shapers, and Control helpers use production actions", async ({
	api,
	bench,
	desk,
	page,
}) => {
	const show = await loadCanonicalCopy(api, bench, "bench-06d-specials");
	const profile = specialProfile();
	const fixtureIds = await installFixtures(api, profile, [901, 902]);
	await replaceProgrammingSelection(api, {
		surface: "api",
		showId: show.id,
		fixtures: fixtureIds,
	});
	await desk.open(api.baseUrl);
	const selection = new BrowserSelection(api);
	const special = new BrowserProgrammerSpecials(api, page, desk, selection);

	await applyProgrammerSelectionValue(api, {
		surface: "api",
		showId: show.id,
		fixtureIds,
		attribute: "pan",
		operation: { type: "absolute_set", value: { kind: "normalized", value: 0.4 } },
		timing: { fade: false, fadeMillis: null, delayMillis: null },
	});
	await special.position.alignViaApi("left");
	await expect.poll(async () => programmerValues(api, "pan")).toEqual([0, 1]);
	await special.position.returnHome();
	const home = 0.5019608;
	await expect.poll(async () => programmerValues(api, "pan")).toEqual([home, home]);

	expect(await special.beam.available()).toContain("prism");
	await special.beam.set("prism", 40);
	await expect
		.poll(async () => programmerValuesClose(api, "prism", 0.4, 2))
		.toBe(true);
	expect(await special.shapers.available()).toContain("shaper.blade.1");
	await special.shapers.set("shaper.blade.1", 30);
	await expect
		.poll(async () => programmerValuesClose(api, "shaper.blade.1", 0.3, 2))
		.toBe(true);

	await special.control.invoke("reset");
	await special.control.invokeViaApi("reset");
});

function specialProfile(): FixtureProfile {
	const profile = blankFixtureProfile();
	profile.id = crypto.randomUUID();
	profile.revision = 1;
	profile.manufacturer = "Bench";
	profile.name = "Discrete and special controls";
	profile.short_name = "Bench special";
	profile.fixture_type = "moving_head";
	const mode = profile.modes[0];
	mode.splits = [{ number: 1, footprint: 5 }];
	const headId = mode.heads[0].id;
	mode.channels = [
		channel("gobo", headId, [
			discreteFunction("gobo.open", "Open", 0),
			discreteFunction("gobo.dots", "Dots", 255),
		]),
		channel("prism", headId),
		channel("shaper.blade.1", headId),
		channel("pan", headId, undefined, 0.5),
		channel("tilt", headId, undefined, 0.5),
	];
	mode.control_actions = [
		{
			id: crypto.randomUUID(),
			name: "Reset",
			semantic: "reset",
			kind: "latched",
			duration_millis: null,
			assignments: [
				{ channel_id: mode.channels[1].id, active_raw: 255, inactive_raw: 0 },
			],
		},
	];
	return profile;
}

function channel(
	attribute: string,
	headId: string,
	functions?: ChannelFunction[],
	defaultValue = 0,
): FixtureChannel {
	return {
		id: crypto.randomUUID(),
		head_id: headId,
		split: 1,
		attribute,
		resolution: "u8",
		secondary_slots: [],
		default_raw: Math.round(defaultValue * 255),
		highlight_raw: 255,
		physical_min: 0,
		physical_max: 1,
		unit: null,
		invert: false,
		snap: Boolean(functions),
		reacts_to_virtual_intensity: false,
		reacts_to_sequence_master: false,
		reacts_to_group_master: false,
		reacts_to_grand_master: false,
		behavior: "controlled",
		functions:
			functions ?? [
				{
					id: crypto.randomUUID(),
					name: attribute,
					dmx_from: 0,
					dmx_to: 255,
					attribute,
					priority: 0,
					behavior: {
						type: "continuous",
						physical_min: 0,
						physical_max: 1,
						unit: null,
					},
				},
			],
	};
}

function discreteFunction(
	semanticId: string,
	label: string,
	rawValue: number,
): ChannelFunction {
	return {
		id: crypto.randomUUID(),
		name: label,
		dmx_from: rawValue,
		dmx_to: rawValue,
		attribute: "gobo",
		priority: 1,
		behavior: {
			type: "indexed",
			semantic_id: semanticId,
			label,
			raw_value: rawValue,
		},
	};
}

async function installFixtures(
	api: Parameters<typeof putObject>[0],
	profile: FixtureProfile,
	numbers: number[],
): Promise<string[]> {
	const definition = fixtureDefinitionFromProfileMode(profile, profile.modes[0]);
	const fixtureIds = numbers.map(() => crypto.randomUUID());
	const source = (await objects<Record<string, unknown>>(api, "patched_fixture"))[0]
		.body;
	for (const [index, fixtureId] of fixtureIds.entries()) {
		await putObject(api, "patched_fixture", fixtureId, {
			...source,
			fixture_id: fixtureId,
			fixture_number: numbers[index],
			name: `Bench special ${numbers[index]}`,
			universe: null,
			address: null,
			definition,
			logical_heads: [],
			location: { x: index, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
			split_patches: [{ split: 1, universe: null, address: null }],
			highlight_overrides: {},
		});
	}
	return fixtureIds;
}

async function programmerValues(
	api: Parameters<typeof programmer>[0],
	attribute: string,
): Promise<number[]> {
	return (await programmer(api)).values
		.filter((entry) => entry.attribute === attribute)
		.map((entry) => {
			const value = entry.value as { kind?: string; value?: number };
			return typeof value === "number" ? value : (value.value ?? Number.NaN);
		})
		.sort((left, right) => left - right);
}

async function programmerValuesClose(
	api: Parameters<typeof programmer>[0],
	attribute: string,
	expected: number,
	count: number,
): Promise<boolean> {
	const values = await programmerValues(api, attribute);
	return (
		values.length === count &&
		values.every((value) => Math.abs(value - expected) < 0.01)
	);
}

async function expectProgrammerValue(
	api: Parameters<typeof programmer>[0],
	fixtureId: string,
	attribute: string,
	expected: unknown,
): Promise<void> {
	await expect
		.poll(async () =>
			(await programmer(api)).values.find(
				(entry) =>
					entry.fixture_id === fixtureId && entry.attribute === attribute,
			)?.value,
		)
		.toEqual(expected);
}

async function expectProgrammerReleased(
	api: Parameters<typeof programmer>[0],
	fixtureId: string,
	attribute: string,
): Promise<void> {
	await expect
		.poll(async () =>
			(await programmer(api)).values.some(
				(entry) =>
					entry.fixture_id === fixtureId && entry.attribute === attribute,
			),
		)
		.toBe(false);
}
