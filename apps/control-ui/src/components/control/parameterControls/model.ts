import type { ControlActionKind, PatchedFixture } from "../../../api/types";

export const parameterFamilies = {
	Intensity: ["intensity", "shutter", "strobe", "master"],
	Color: [
		"color.red",
		"color.green",
		"color.blue",
		"color.white",
		"color.amber",
		"color.uv",
	],
	Position: ["pan", "tilt"],
	Beam: ["gobo", "gobo.2", "gobo.rotation", "prism", "prism.2", "iris"],
	Shapers: [
		"shaper.blade.1",
		"shaper.blade.2",
		"shaper.blade.3",
		"shaper.blade.4",
		"shaper.rotation",
	],
	Focus: ["focus", "zoom", "frost", "edge"],
	Control: ["control.reset", "control.lamp", "control.fan", "control.mode"],
	Media: ["media.layer", "media.clip", "media.opacity", "media.speed"],
} as const;

export type ParameterFamily = keyof typeof parameterFamilies;
export type SpecialParameterFamily =
	| "Color"
	| "Position"
	| "Beam"
	| "Shapers"
	| "Control";
export const alignModes = ["out", "center", "left", "right"] as const;
export type AlignMode = (typeof alignModes)[number];

export const compactFamilyLabels: Record<ParameterFamily, string> = {
	Intensity: "Int",
	Color: "Col",
	Position: "Pos",
	Beam: "Beam",
	Shapers: "Shapr",
	Focus: "Focus",
	Control: "Ctrl",
	Media: "Media",
};

export const parameterLabels: Record<string, string> = {
	intensity: "Dimmer",
	shutter: "Shutter",
	strobe: "Strobe",
	master: "Master",
	pan: "Pan",
	tilt: "Tilt",
	gobo: "Gobo 1",
	"gobo.2": "Gobo 2",
	"gobo.rotation": "Gobo rotation",
	prism: "Prism 1",
	"prism.2": "Prism 2",
	iris: "Iris",
	focus: "Focus",
	zoom: "Zoom",
	frost: "Frost",
	edge: "Edge",
};

export const specialParameterFamilies = new Set<SpecialParameterFamily>([
	"Color",
	"Position",
	"Beam",
	"Shapers",
	"Control",
]);

export interface ProgrammerValueEntry {
	fixture_id: string;
	attribute: string;
	value: unknown;
}

interface DirectValueAssignment {
	fixtureId: string;
	attribute: string;
}

export interface DirectValueChoice {
	key: string;
	label: string;
	semanticId: string;
	kind: "fixed" | "indexed";
	assignments: DirectValueAssignment[];
}

export interface DirectControlChoice {
	key: string;
	actionId: string;
	label: string;
	kind: ControlActionKind;
	durationMillis: number | null;
	fixtureIds: string[];
}

export function normalizedProgrammerTarget(value: unknown): number | undefined {
	if (typeof value === "number") return value;
	if (!value || typeof value !== "object") return undefined;
	const record = value as Record<string, unknown>;
	if (record.kind === "normalized" && typeof record.value === "number")
		return record.value;
	return record.value === value
		? undefined
		: normalizedProgrammerTarget(record.value);
}

export function discreteProgrammerTarget(value: unknown): string | undefined {
	if (!value || typeof value !== "object") return undefined;
	const record = value as Record<string, unknown>;
	if (record.kind === "discrete" && typeof record.value === "string")
		return record.value;
	return record.value === value
		? undefined
		: discreteProgrammerTarget(record.value);
}

export function formatNormalizedValue(value: number): string {
	return `${Math.round(value * 100)}%`;
}

export function formatNormalizedRange(values: number[]): string | undefined {
	if (!values.length) return undefined;
	const rounded = values.map((value) => Math.round(value * 100));
	const minimum = Math.min(...rounded);
	const maximum = Math.max(...rounded);
	return minimum === maximum ? `${minimum}%` : `${minimum}%...${maximum}%`;
}

export function formatDiscreteValues(values: string[]): string | undefined {
	if (!values.length) return undefined;
	const unique = [...new Set(values)];
	return unique.length === 1 ? unique[0] : "Mixed";
}

function profileHeadOwner(
	fixture: PatchedFixture,
	headId: string,
): string | null {
	const profile = fixture.definition.profile_snapshot;
	const mode = profile?.modes.find(
		(candidate) => candidate.id === fixture.definition.mode_id,
	);
	const headIndex = mode?.heads.findIndex((head) => head.id === headId) ?? -1;
	if (!mode || headIndex < 0) return null;
	if (mode.heads[headIndex].master_shared) return fixture.fixture_id;
	return (
		(
			fixture.logical_heads.find((head) => head.head_index === headIndex) ??
			fixture.logical_heads.find((head) => head.head_index === headIndex + 1)
		)?.fixture_id ?? null
	);
}

function collectDirectValues(
	fixture: PatchedFixture,
	selected: Set<string>,
	values: Map<string, DirectValueChoice>,
	fixtureIds: Set<string>,
) {
	const physicalSelected = selected.has(fixture.fixture_id);
	const profile = fixture.definition.profile_snapshot;
	const mode = profile?.modes.find(
		(candidate) => candidate.id === fixture.definition.mode_id,
	);
	if (!profile || !mode) return;
	for (const channel of mode.channels) {
		const owner = profileHeadOwner(fixture, channel.head_id);
		if (!owner || (!physicalSelected && !selected.has(owner))) continue;
		for (const fn of channel.functions) {
			if (fn.behavior.type !== "fixed" && fn.behavior.type !== "indexed")
				continue;
			const key = `${fn.behavior.type}:${fn.behavior.semantic_id}`;
			const choice = values.get(key) ?? {
				key,
				label: fn.behavior.label,
				semanticId: fn.behavior.semantic_id,
				kind: fn.behavior.type,
				assignments: [],
			};
			if (fn.behavior.label.localeCompare(choice.label) < 0)
				choice.label = fn.behavior.label;
			if (
				!choice.assignments.some(
					(assignment) =>
						assignment.fixtureId === owner &&
						assignment.attribute === fn.attribute,
				)
			)
				choice.assignments.push({ fixtureId: owner, attribute: fn.attribute });
			values.set(key, choice);
			fixtureIds.add(fixture.fixture_id);
		}
	}
}

function collectControlActions(
	fixture: PatchedFixture,
	actions: Map<string, DirectControlChoice>,
) {
	const profile = fixture.definition.profile_snapshot;
	const mode = profile?.modes.find(
		(candidate) => candidate.id === fixture.definition.mode_id,
	);
	if (!profile || !mode) return;
	for (const action of mode.control_actions) {
		const key = `${profile.id}:${mode.id}:${action.id}`;
		const choice = actions.get(key) ?? {
			key,
			actionId: action.id,
			label: action.name,
			kind: action.kind,
			durationMillis: action.duration_millis,
			fixtureIds: [],
		};
		if (!choice.fixtureIds.includes(fixture.fixture_id))
			choice.fixtureIds.push(fixture.fixture_id);
		actions.set(key, choice);
	}
}

export function directProgrammerChoices(
	fixtures: PatchedFixture[],
	selectedFixtures: string[],
) {
	const selected = new Set(selectedFixtures);
	const values = new Map<string, DirectValueChoice>();
	const actions = new Map<string, DirectControlChoice>();
	const fixtureIds = new Set<string>();
	for (const fixture of fixtures) {
		const selectedFixture =
			selected.has(fixture.fixture_id) ||
			fixture.logical_heads.some((head) => selected.has(head.fixture_id));
		if (!selectedFixture) continue;
		collectDirectValues(fixture, selected, values, fixtureIds);
		collectControlActions(fixture, actions);
	}
	return {
		values: [...values.values()].sort((left, right) =>
			left.label.localeCompare(right.label),
		),
		actions: [...actions.values()].sort((left, right) =>
			left.label.localeCompare(right.label),
		),
		fixtureIds: [...fixtureIds],
	};
}
