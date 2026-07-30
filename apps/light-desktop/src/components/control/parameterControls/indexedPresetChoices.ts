import type {
	ControlAction,
	FixtureMode,
	PatchedFixture,
} from "../../../api/types";

export interface IndexedPresetTarget {
	fixtureId: string;
	functionId?: string;
	profileRevision?: number;
	actionId?: string;
}

export interface IndexedPresetChoice {
	id: string;
	label: string;
	description: string;
	kind: "fixed" | "indexed" | "control";
	semanticId: string | null;
	controlKind: ControlAction["kind"] | null;
	targets: IndexedPresetTarget[];
	disabled: boolean;
}

interface MutableChoice extends IndexedPresetChoice {
	targetIds: Set<string>;
}

export function indexedPresetChoices(
	fixtures: readonly PatchedFixture[],
	selectedFixtureIds: readonly string[],
	attribute: string,
): IndexedPresetChoice[] {
	const selected = new Set(selectedFixtureIds);
	const choices = new Map<string, MutableChoice>();
	for (const fixture of fixtures) {
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		if (!profile || !mode) continue;
		for (const channel of mode.channels) {
			if (channel.attribute !== attribute) continue;
			const owner = profileHeadOwner(fixture, mode, channel.head_id);
			if (!owner || (!selected.has(fixture.fixture_id) && !selected.has(owner)))
				continue;
			for (const fn of channel.functions) {
				if (fn.attribute !== attribute) continue;
				const behavior = fn.behavior;
				if (behavior.type === "fixed" || behavior.type === "indexed") {
					const { semantic_id: semanticId, label } = behavior;
					const key = `${behavior.type}:${semanticId}:${label}`;
					appendChoice(choices, key, {
						id: key,
						label,
						description: "",
						kind: behavior.type,
						semanticId,
						controlKind: null,
						targets: [
							{
								fixtureId: owner,
								functionId: fn.id,
								profileRevision: profile.revision,
							},
						],
						targetIds: new Set([owner]),
						disabled: false,
					});
				} else if (behavior.type === "control") {
					const action = mode.control_actions.find(
						(candidate) => candidate.id === behavior.action_id,
					);
					if (!action) continue;
					const key = controlCompatibilityKey(action);
					appendChoice(choices, key, {
						id: key,
						label: action.name,
						description: "",
						kind: "control",
						semanticId: null,
						controlKind: action.kind,
						targets: [
							{
								fixtureId: owner,
								actionId: action.id,
								profileRevision: profile.revision,
							},
						],
						targetIds: new Set([owner]),
						disabled: fixture.definition.hazardous,
					});
				}
			}
		}
	}
	const selectedCount = selected.size;
	return [...choices.values()]
		.map(({ targetIds, ...choice }) => ({
			...choice,
			description:
				targetIds.size === selectedCount
					? "All selected fixtures"
					: fixtureScope(fixtures, targetIds),
		}))
		.sort(
			(left, right) =>
				left.label.localeCompare(right.label) ||
				left.description.localeCompare(right.description) ||
				left.id.localeCompare(right.id),
		);
}

function appendChoice(
	choices: Map<string, MutableChoice>,
	key: string,
	next: MutableChoice,
) {
	const existing = choices.get(key);
	if (!existing) {
		choices.set(key, next);
		return;
	}
	for (const target of next.targets)
		if (!existing.targetIds.has(target.fixtureId)) {
			existing.targets.push(target);
			existing.targetIds.add(target.fixtureId);
		}
	existing.disabled ||= next.disabled;
}

function profileHeadOwner(
	fixture: PatchedFixture,
	mode: FixtureMode,
	headId: string,
) {
	const index = mode.heads.findIndex((head) => head.id === headId);
	if (index < 0) return null;
	if (mode.heads[index]?.master_shared) return fixture.fixture_id;
	return (
		fixture.logical_heads.find((head) => head.head_index === index)
			?.fixture_id ??
		fixture.logical_heads.find((head) => head.head_index === index + 1)
			?.fixture_id ??
		null
	);
}

function controlCompatibilityKey(action: ControlAction) {
	return [
		"control",
		action.name,
		action.semantic,
		action.kind,
		action.duration_millis ?? "",
	].join(":");
}

function fixtureScope(
	fixtures: readonly PatchedFixture[],
	targetIds: ReadonlySet<string>,
) {
	const labels = [];
	for (const fixture of fixtures) {
		if (targetIds.has(fixture.fixture_id))
			labels.push(fixtureLabel(fixture, fixture.fixture_id));
		for (const head of fixture.logical_heads)
			if (targetIds.has(head.fixture_id))
				labels.push(fixtureLabel(fixture, head.fixture_id));
	}
	return labels.length ? labels.join(", ") : "Selected fixture subset";
}

function fixtureLabel(fixture: PatchedFixture, fixtureId: string) {
	const number =
		fixture.fixture_number ??
		fixture.virtual_fixture_number ??
		fixtureId.slice(0, 8);
	const name =
		fixture.name || fixture.definition.name || fixture.definition.model;
	return `Fixture ${number}${name ? ` ${name}` : ""}`;
}
