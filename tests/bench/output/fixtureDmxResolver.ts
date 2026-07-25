import type {
	FixtureChannel,
	FixtureMode,
	MultiPatchInstance,
	PatchedFixture,
	PatchSnapshot,
	SplitPatch,
} from "../../../apps/light-desktop/src/api/types";
import { derivePrimarySlots } from "../../../apps/light-desktop/src/components/setup/fixtureProfileModel";
import {
	displayAttributeName,
	type ExpectedDMXByte,
	type FixtureDMXTarget,
	type FixtureReference,
	fixtureReferences,
	normalizeDmxName,
} from "./fixtureDmxContract";

export interface ResolvedFixtureDmxComponent {
	readonly universe: number;
	readonly address: number;
	readonly expected: ExpectedDMXByte;
	readonly description: string;
}

interface PhysicalOwner {
	readonly name: string;
	readonly universe: number | null;
	readonly address: number | null;
	readonly split_patches?: SplitPatch[];
}

class FixtureDmxContractError extends Error {}

const COMPONENT_NAMES = ["coarse", "fine", "ultra", "fourth"] as const;

export function resolveFixtureDmxComponents(
	patch: PatchSnapshot,
	target: FixtureDMXTarget,
	entries: readonly [string, ExpectedDMXByte][],
): ResolvedFixtureDmxComponent[] {
	return resolveTargets(patch, target).flatMap(({ fixture, reference }) => {
		const { profile, mode, head, channels } = fixtureMode(fixture, reference);
		const primary = derivePrimarySlots(mode);
		if (primary.errors.length)
			throw fixtureError(
				fixture,
				profile.name,
				mode.name,
				`profile channel layout is invalid: ${primary.errors.join("; ")}`,
			);
		const owner = selectedOwner(fixture, reference);
		const catalog = channelCatalog(channels, primary.slots);
		return entries.map(([requestedName, expected]) => {
			const matches = catalog.filter(
				(candidate) =>
					normalizeDmxName(candidate.name) === normalizeDmxName(requestedName),
			);
			if (matches.length === 0)
				throw fixtureError(
					fixture,
					profile.name,
					mode.name,
					`DMX channel "${requestedName}" does not exist. Valid channels: ${validNames(catalog)}`,
				);
			if (matches.length > 1)
				throw fixtureError(
					fixture,
					profile.name,
					mode.name,
					`DMX channel "${requestedName}" is ambiguous: ${matches.map(describeCandidate).join("; ")}. Qualify the fixture head. Valid channels: ${validNames(catalog)}`,
				);
			const component = matches[0];
			const assignment = assignmentFor(owner, mode, component.channel.split);
			if (assignment?.universe == null || assignment.address == null)
				throw fixtureError(
					fixture,
					profile.name,
					mode.name,
					`${owner.name} has no DMX assignment for ${component.name} on split ${component.channel.split}`,
				);
			const address = assignment.address + component.slot - 1;
			if (address < 1 || address > 512)
				throw fixtureError(
					fixture,
					profile.name,
					mode.name,
					`${component.name} resolves outside its universe at U${assignment.universe}.${address}`,
				);
			return {
				universe: assignment.universe,
				address,
				expected,
				description: `${fixtureLabel(fixture)} · profile ${profile.name} r${profile.revision} · mode ${mode.name} · head ${head?.name ?? "all heads"} · ${owner.name} · ${component.name} · split ${component.channel.split} · U${assignment.universe}.${address}`,
			};
		});
	});
}

export function assignedFixtureDmxTargets(
	patch: PatchSnapshot,
	target: FixtureDMXTarget,
): string[] {
	return resolveTargets(patch, target).flatMap(({ fixture, reference }) => {
		const { mode, channels } = fixtureMode(fixture, reference);
		const owner = selectedOwner(fixture, reference);
		const assigned = new Set(
			channels
				.map((channel) => assignmentFor(owner, mode, channel.split))
				.filter(
					(
						assignment,
					): assignment is SplitPatch & {
						universe: number;
						address: number;
					} => assignment?.universe != null && assignment.address != null,
				)
				.map(
					(assignment) =>
						`split ${assignment.split} at U${assignment.universe}.${assignment.address}`,
				),
		);
		return assigned.size
			? [
					`${fixtureLabel(fixture)} ${owner.name} still has ${[...assigned].join(", ")}`,
				]
			: [];
	});
}

function resolveTargets(patch: PatchSnapshot, target: FixtureDMXTarget) {
	return fixtureReferences(target).map((reference) => {
		const matches = patch.fixtures.filter(
			(candidate) => candidate.fixture_number === reference.number,
		);
		if (matches.length === 0)
			throw new FixtureDmxContractError(
				`Fixture ${reference.number} is not present in the current patch`,
			);
		if (matches.length > 1)
			throw new FixtureDmxContractError(
				`Fixture ${reference.number} is ambiguous in the current patch (${matches.map((candidate) => candidate.fixture_id).join(", ")})`,
			);
		return { fixture: matches[0], reference };
	});
}

function fixtureMode(fixture: PatchedFixture, target: FixtureReference) {
	const profile = fixture.definition.profile_snapshot;
	if (!profile)
		throw fixtureError(
			fixture,
			fixture.definition.name,
			fixture.definition.mode,
			`exact profile revision ${fixture.definition.revision} has no parameterized snapshot`,
		);
	if (profile.patch_policy === "visual_only")
		throw fixtureError(
			fixture,
			profile.name,
			fixture.definition.mode,
			"profile is visual-only and has no DMX channels",
		);
	const mode = profile.modes.find(
		(candidate) => candidate.id === fixture.definition.mode_id,
	);
	if (!mode)
		throw fixtureError(
			fixture,
			profile.name,
			fixture.definition.mode,
			`mode identity ${fixture.definition.mode_id ?? "(missing)"} is unavailable`,
		);
	const head =
		target.head === undefined ? undefined : mode.heads[target.head - 1];
	if (target.head !== undefined && !head)
		throw fixtureError(
			fixture,
			profile.name,
			mode.name,
			`head ${target.head} does not exist. Valid heads: ${mode.heads.map((candidate, index) => `${index + 1} ${candidate.name}`).join(", ") || "(none)"}`,
		);
	const channels = head
		? mode.channels.filter((channel) => channel.head_id === head.id)
		: mode.channels;
	if (channels.length === 0)
		throw fixtureError(
			fixture,
			profile.name,
			mode.name,
			`${head ? `head ${target.head} ${head.name}` : "mode"} has no raw DMX channels`,
		);
	return { profile, mode, head, channels };
}

function selectedOwner(
	fixture: PatchedFixture,
	target: FixtureReference,
): PhysicalOwner {
	if (!target.multipatch)
		return {
			name: "primary fixture patch",
			universe: fixture.universe,
			address: fixture.address,
			split_patches: fixture.split_patches,
		};
	const key = normalizeDmxName(target.multipatch);
	const matches = (fixture.multipatch ?? []).filter(
		(instance) =>
			normalizeDmxName(instance.id) === key ||
			normalizeDmxName(instance.name) === key,
	);
	if (matches.length !== 1)
		throw new FixtureDmxContractError(
			`${fixtureLabel(fixture)} multi-patch "${target.multipatch}" ${matches.length ? "is ambiguous" : "does not exist"}. Available multi-patches: ${multipatchNames(fixture.multipatch ?? [])}`,
		);
	return { ...matches[0], name: `multi-patch "${matches[0].name}"` };
}

function assignmentFor(
	owner: PhysicalOwner,
	mode: FixtureMode,
	split: number,
): SplitPatch | undefined {
	if (owner.split_patches?.length)
		return owner.split_patches.find((candidate) => candidate.split === split);
	if (mode.splits[0]?.number !== split) return undefined;
	return { split, universe: owner.universe, address: owner.address };
}

function channelCatalog(
	channels: readonly FixtureChannel[],
	primarySlots: ReadonlyMap<string, number>,
) {
	return channels.flatMap((channel) => {
		const primary = primarySlots.get(channel.id);
		if (primary === undefined)
			throw new FixtureDmxContractError(
				`Profile channel ${channel.attribute} has no derived primary slot`,
			);
		const slots = [primary, ...channel.secondary_slots];
		return slots.map((slot, index) => ({
			name:
				slots.length === 1
					? displayAttributeName(channel.attribute)
					: `${displayAttributeName(channel.attribute)} ${COMPONENT_NAMES[index] ?? `byte ${index + 1}`}`,
			channel,
			slot,
		}));
	});
}

function validNames(catalog: readonly { name: string }[]): string {
	return [...new Set(catalog.map((candidate) => candidate.name))].join(", ");
}

function describeCandidate(candidate: {
	name: string;
	channel: FixtureChannel;
	slot: number;
}): string {
	return `${candidate.name} (head ${candidate.channel.head_id}, split ${candidate.channel.split}, slot ${candidate.slot})`;
}

function fixtureLabel(fixture: PatchedFixture): string {
	return `Fixture ${fixture.fixture_number ?? fixture.fixture_id}${fixture.name ? ` "${fixture.name}"` : ""}`;
}

function fixtureError(
	fixture: PatchedFixture,
	profile: string,
	mode: string,
	detail: string,
): FixtureDmxContractError {
	return new FixtureDmxContractError(
		`${fixtureLabel(fixture)} · profile ${profile} · mode ${mode}: ${detail}`,
	);
}

function multipatchNames(instances: readonly MultiPatchInstance[]): string {
	return (
		instances
			.map((instance) => `${instance.name || "(unnamed)"} [${instance.id}]`)
			.join(", ") || "(none)"
	);
}
