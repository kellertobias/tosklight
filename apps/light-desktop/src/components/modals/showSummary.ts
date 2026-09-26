import type { PatchedFixture } from "../../api/types";

/** Count physical DMX slots in the patch, including split and multi-patch bindings. */
export function showPatchSummary(fixtures: readonly PatchedFixture[]) {
	const universes = new Set<number>();
	let parameters = 0;
	for (const fixture of fixtures) {
		const mode = fixture.definition.profile_snapshot?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		) ?? fixture.definition.profile_snapshot?.modes.find(
			(candidate) => candidate.name === fixture.definition.mode,
		) ?? fixture.definition.profile_snapshot?.modes[0];
		const splits = mode?.splits.length
			? mode.splits
			: [{ number: 1, footprint: fixture.definition.footprint }];
		for (const owner of [fixture, ...(fixture.multipatch ?? [])]) {
			const configured = new Map((owner.split_patches ?? []).map((patch) => [patch.split, patch]));
			for (const [index, split] of splits.entries()) {
				const patch = configured.get(split.number) ?? {
					universe: index === 0 ? owner.universe : null,
					address: index === 0 ? owner.address : null,
				};
				if (patch.universe == null || patch.address == null) continue;
				universes.add(patch.universe);
				parameters += split.footprint;
			}
		}
	}
	return { universes: universes.size, parameters };
}
