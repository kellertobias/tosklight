import { replaceProgrammingSelection } from "./bench/command-selection/programmingSelection";
import { expect } from "./bench/core/fixtures";
import { pairedScenario } from "./bench/core/pairedScenario";
import { batchProgrammerValues } from "./bench/programmer/programmerValues";
import { loadCanonicalCopy, programmer } from "./support/catalog";

type PickerColor = { hue: number; saturation: number; brightness: number };

// Test-owned oracle mirroring the server's color-range contract (crates/shared/core
// `color_range_color` + `hsv_to_rgb`, expanded per supported channel): open arcs pin both
// endpoints, interior fixtures interpolate hue along the gesture's travel and saturation
// linearly, and every color uses the end brightness.
function interpolatePickerRange(
	count: number,
	start: PickerColor,
	end: PickerColor,
): PickerColor[] {
	if (count <= 0) return [];
	if (count === 1) return [end];
	return Array.from({ length: count }, (_, index) => {
		if (index === 0) return { ...start, brightness: end.brightness };
		if (index === count - 1) return end;
		const ratio = index / (count - 1);
		return {
			hue: start.hue + (end.hue - start.hue) * ratio,
			saturation:
				start.saturation + (end.saturation - start.saturation) * ratio,
			brightness: end.brightness,
		};
	});
}

function hsvToRgb({ hue, saturation, brightness }: PickerColor) {
	const i = Math.floor(hue * 6);
	const f = hue * 6 - i;
	const p = brightness * (1 - saturation);
	const q = brightness * (1 - f * saturation);
	const t = brightness * (1 - (1 - f) * saturation);
	return (
		[
			[brightness, t, p],
			[q, brightness, p],
			[p, brightness, t],
			[p, q, brightness],
			[t, p, brightness],
			[brightness, p, q],
		] as number[][]
	)[i % 6];
}

function colorProgrammerAssignments(
	selectedFixtures: readonly string[],
	patch: readonly any[],
	colors: PickerColor[],
): Assignment[] {
	return selectedFixtures.flatMap((fixtureId, index) => {
		const fixture = patch.find(
			(candidate: any) =>
				candidate.fixture_id === fixtureId ||
				candidate.logical_heads.some(
					(head: any) => head.fixture_id === fixtureId,
				),
		);
		if (!fixture) return [];
		const logicalHead = fixture.logical_heads.find(
			(head: any) => head.fixture_id === fixtureId,
		);
		const heads = logicalHead
			? fixture.definition.heads.filter(
					(head: any) => head.index === logicalHead.head_index,
				)
			: fixture.definition.heads.filter((head: any) => head.shared);
		const attributes = new Set(
			heads.flatMap((head: any) =>
				head.parameters.map((parameter: any) => parameter.attribute),
			),
		);
		const color = colors[index];
		if (!color) return [];
		const [red, green, blue] = hsvToRgb(color);
		const values: Array<[string, number]> = [
			["color.red", red],
			["color.green", green],
			["color.blue", blue],
			["color.cyan", 1 - red],
			["color.magenta", 1 - green],
			["color.yellow", 1 - blue],
		];
		return values.flatMap(([attribute, value]) =>
			attributes.has(attribute) ? [{ fixtureId, attribute, value }] : [],
		);
	});
}

type Assignment = { fixtureId: string; attribute: string; value: number };
type ColorRangeState = {
	showId: string;
	selected: string[];
	range: Assignment[];
	prior: Assignment[];
};

const start: PickerColor = { hue: 0.1, saturation: 0.8, brightness: 0.85 };
const end: PickerColor = { hue: 0.9, saturation: 0.2, brightness: 0.85 };

pairedScenario<ColorRangeState>({
	id: "COLOR-RANGE-001",
	title:
		"Shift-drag applies an ordered Color range from software and attached hardware",
	surfaces: ["api"],
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`color-range-001-${surface}`,
			"default-stage",
		);
		const patch = await api.patch();
		const colorTargets = patch.fixtures.flatMap((fixture: any) => {
			const logicalByIndex = new Map(
				fixture.logical_heads.map((head: any) => [
					head.head_index,
					head.fixture_id,
				]),
			);
			return fixture.definition.heads.flatMap((head: any) => {
				const fixtureId = head.shared
					? fixture.fixture_id
					: logicalByIndex.get(head.index);
				const attributes = new Set(
					head.parameters.map((parameter: any) => parameter.attribute),
				);
				const supported = [
					"color.red",
					"color.green",
					"color.blue",
					"color.cyan",
					"color.magenta",
					"color.yellow",
				].some((attribute) => attributes.has(attribute));
				return fixtureId && supported
					? [{ fixtureId: fixtureId as string, logical: !head.shared }]
					: [];
			});
		});
		expect(colorTargets.length).toBeGreaterThanOrEqual(3);
		const logical = colorTargets.find((target: any) => target.logical);
		const chosen = [
			logical ?? colorTargets[2],
			...colorTargets
				.filter((target: any) => target.fixtureId !== logical?.fixtureId)
				.slice(0, 2),
		];
		const nonColor = patch.fixtures.find((fixture: any) =>
			fixture.definition.heads.every((head: any) =>
				head.parameters.every(
					(parameter: any) => !parameter.attribute.startsWith("color."),
				),
			),
		);
		const selected = [
			chosen[0].fixtureId,
			chosen[1].fixtureId,
			...(nonColor ? [nonColor.fixture_id] : []),
			chosen[2].fixtureId,
		];
		const range = colorProgrammerAssignments(
			selected,
			patch.fixtures,
			interpolatePickerRange(selected.length, start, end),
		);
		const prior = range.map((assignment) => ({ ...assignment, value: 0.33 }));
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: show.id,
			fixtures: selected,
		});
		await setMany(api, show.id, prior);
		return { showId: show.id, selected, range, prior };
	},
	api: async ({ api }, state) => {
		// The server owns the interpolation: one fan-out action carries the ordered selection,
		// both endpoints, and the gesture's hue travel.
		await batchProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
			mutations: [
				{
					action: "set_selection_color_range",
					fixtureIds: state.selected,
					start: { hue: start.hue, saturation: start.saturation },
					end: { hue: end.hue, saturation: end.saturation },
					hueTravel: end.hue - start.hue,
					brightness: end.brightness,
					timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
				},
			],
		});
	},
	assert: async ({ api }, state) => {
		expect((await programmer(api)).selected).toEqual(state.selected);
		await expectAssignments(api, state.range);
	},
});

async function setMany(
	api: any,
	showId: string,
	assignments: Assignment[],
): Promise<void> {
	await batchProgrammerValues(api, {
		surface: "api",
		showId,
		mutations: assignments.map(({ fixtureId, attribute, value }) => ({
			action: "set_fixture",
			fixtureId,
			attribute,
			value: { kind: "normalized", value },
			timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
		})),
	});
}

async function expectAssignments(
	api: any,
	expected: Assignment[],
): Promise<void> {
	await expect
		.poll(async () => {
			const values = (await programmer(api)).values;
			return expected.every((assignment) => {
				const actual = values.find(
					(value) =>
						value.fixture_id === assignment.fixtureId &&
						value.attribute === assignment.attribute,
				);
				const value = actual?.value?.value ?? actual?.value;
				return (
					typeof value === "number" &&
					Math.abs(value - assignment.value) < 0.00001
				);
			});
		})
		.toBe(true);
}
