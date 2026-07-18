import type {
	FixtureChannel,
	FixtureDefinition,
	MultiPatchInstance,
	PatchedFixture,
	SplitPatch,
} from "../../../api/types";
import { isDmxPatchable } from "../patchUtils";

export function definitionSplits(definition: FixtureDefinition) {
	const profile = definition.profile_snapshot;
	const mode =
		profile?.modes.find((candidate) => candidate.id === definition.mode_id) ??
		profile?.modes.find((candidate) => candidate.name === definition.mode) ??
		profile?.modes[0];
	return mode?.splits.length
		? mode.splits
		: [{ number: 1, footprint: definition.footprint }];
}

export function definitionModeChannels(
	definition: FixtureDefinition,
): FixtureChannel[] {
	const profile = definition.profile_snapshot;
	return (
		profile?.modes.find((candidate) => candidate.id === definition.mode_id)
			?.channels ??
		profile?.modes.find((candidate) => candidate.name === definition.mode)
			?.channels ??
		profile?.modes[0]?.channels ??
		[]
	);
}

export function effectiveSplitPatches(
	definition: FixtureDefinition,
	patches: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
): SplitPatch[] {
	const configured = new Map(
		(patches ?? []).map((patch) => [patch.split, patch]),
	);
	return definitionSplits(definition).map(
		(split, index) =>
			configured.get(split.number) ?? {
				split: split.number,
				universe: index === 0 ? universe : null,
				address: index === 0 ? address : null,
			},
	);
}

export function reconcileSplitPatchOwner(
	currentDefinition: FixtureDefinition,
	nextDefinition: FixtureDefinition,
	patches: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
): Pick<PatchedFixture, "split_patches" | "universe" | "address"> {
	const previous = new Map(
		effectiveSplitPatches(currentDefinition, patches, universe, address).map(
			(patch) => [patch.split, patch],
		),
	);
	const split_patches = definitionSplits(nextDefinition).map((split) => {
		const match = previous.get(split.number);
		return {
			split: split.number,
			universe: match?.universe ?? null,
			address: match?.address ?? null,
		};
	});
	const primary =
		split_patches.find((patch) => patch.split === 1) ?? split_patches[0];
	return {
		split_patches,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
	};
}

export function reconcileModePatchChanges(
	fixture: PatchedFixture,
	definition: FixtureDefinition,
): Pick<
	PatchedFixture,
	"definition" | "split_patches" | "universe" | "address" | "multipatch"
> {
	if (!isDmxPatchable(definition)) {
		const clear = () => ({
			universe: null,
			address: null,
			split_patches: definitionSplits(definition).map((split) => ({
				split: split.number,
				universe: null,
				address: null,
			})),
		});
		return {
			definition,
			...clear(),
			multipatch: (fixture.multipatch ?? []).map((instance) => ({
				...instance,
				...clear(),
			})),
		};
	}
	const primary = reconcileSplitPatchOwner(
		fixture.definition,
		definition,
		fixture.split_patches,
		fixture.universe,
		fixture.address,
	);
	return {
		definition,
		...primary,
		multipatch: (fixture.multipatch ?? []).map((instance) => ({
			...instance,
			...reconcileSplitPatchOwner(
				fixture.definition,
				definition,
				instance.split_patches,
				instance.universe,
				instance.address,
			),
		})),
	};
}

export function unpatchFixtureChanges(
	fixture: PatchedFixture,
): Pick<
	PatchedFixture,
	"split_patches" | "universe" | "address" | "multipatch"
> {
	const clearOwner = (
		patches: SplitPatch[] | undefined,
		universe: number | null,
		address: number | null,
	) => ({
		universe: null,
		address: null,
		split_patches: effectiveSplitPatches(
			fixture.definition,
			patches,
			universe,
			address,
		).map((patch) => ({ split: patch.split, universe: null, address: null })),
	});
	return {
		...clearOwner(fixture.split_patches, fixture.universe, fixture.address),
		multipatch: (fixture.multipatch ?? []).map((instance) => ({
			...instance,
			...clearOwner(
				instance.split_patches,
				instance.universe,
				instance.address,
			),
		})),
	};
}

export function replaceSelectedSplitPatch(
	definition: FixtureDefinition,
	current: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
	selectedSplit: number,
	patch: { universe: number; address: number } | null,
): Pick<PatchedFixture, "split_patches" | "universe" | "address"> {
	const split_patches = effectiveSplitPatches(
		definition,
		current,
		universe,
		address,
	).map((candidate) =>
		candidate.split === selectedSplit
			? {
					...candidate,
					universe: patch?.universe ?? null,
					address: patch?.address ?? null,
				}
			: candidate,
	);
	const primary =
		split_patches.find((candidate) => candidate.split === 1) ??
		split_patches[0];
	return {
		split_patches,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
	};
}

export function splitPatchSetError(
	definition: FixtureDefinition,
	patches: SplitPatch[],
) {
	const footprints = new Map(
		definitionSplits(definition).map((split) => [
			split.number,
			split.footprint,
		]),
	);
	const ranges = patches.flatMap((patch) => {
		if (patch.universe == null && patch.address == null) return [];
		if (patch.universe == null || patch.address == null)
			return [
				{
					split: patch.split,
					universe: patch.universe ?? 0,
					start: patch.address ?? 0,
					end: -1,
				},
			];
		const footprint = footprints.get(patch.split) ?? 0;
		return [
			{
				split: patch.split,
				universe: patch.universe,
				start: patch.address,
				end: patch.address + footprint - 1,
			},
		];
	});
	const invalid = ranges.find(
		(range) =>
			range.universe < 1 ||
			range.start < 1 ||
			range.end < range.start ||
			range.end > 512,
	);
	if (invalid)
		return `Split ${invalid.split} must fit completely inside one 512-slot universe.`;
	for (let index = 0; index < ranges.length; index++)
		for (let other = index + 1; other < ranges.length; other++) {
			const left = ranges[index];
			const right = ranges[other];
			if (
				left.universe === right.universe &&
				left.start <= right.end &&
				right.start <= left.end
			)
				return `Split ${left.split} overlaps split ${right.split}. Give each patched split its own address range.`;
		}
	return null;
}

export function formatFixturePatch(fixture: PatchedFixture) {
	return formatPatchOwner(
		fixture.definition,
		fixture.split_patches,
		fixture.universe,
		fixture.address,
	);
}

export function formatInstancePatch(
	definition: FixtureDefinition,
	instance: MultiPatchInstance,
) {
	return formatPatchOwner(
		definition,
		instance.split_patches,
		instance.universe,
		instance.address,
	);
}

function formatPatchOwner(
	definition: FixtureDefinition,
	patches: SplitPatch[] | undefined,
	universe: number | null,
	address: number | null,
) {
	const effective = effectiveSplitPatches(
		definition,
		patches,
		universe,
		address,
	);
	if (effective.length === 1)
		return effective[0].universe && effective[0].address
			? `${effective[0].universe}.${effective[0].address}`
			: "Unpatched";
	return effective
		.map(
			(patch) =>
				`S${patch.split} ${patch.universe && patch.address ? `${patch.universe}.${patch.address}` : "—"}`,
		)
		.join(" · ");
}
