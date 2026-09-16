/**
 * Bringing a placed element up to the newest version of its profile.
 *
 * A patched fixture keeps the exact profile revision it was placed with, and that revision never
 * changes: a show opened next year draws what it drew when it was built. So when a shipped part is
 * corrected — a truss section that was 340 mm is now 290, a corner block that was 1068 mm across is
 * now 500 — the elements already in the show keep the old shape, and stand beside newly added ones
 * that no longer match. This offers the operator the new revision for the element they have selected
 * rather than making them delete it and place it again.
 *
 * Only the profile reference moves. Name, position, rotation, patch and the measurements the
 * operator set stay as they are; a measurement the new profile does not let the operator set goes
 * back to the new profile's own, which is exactly the section that was corrected.
 */
import type { FixtureDefinition, FixtureProfileScenery, PatchFixtureProjection } from "@tosklight/patch";
import { SIZE_AXES } from "./sceneryAxes";

/** The newest revision of a profile in this computer's library, as the patch would reference it. */
export interface UpgradeTarget {
	profileRevision: number;
	modeId: string;
	scenery: FixtureProfileScenery | null;
}

function profileIdOf(definition: FixtureDefinition): string | undefined {
	return definition.profile_id ?? definition.profile_snapshot?.id;
}

function revisionOf(definition: FixtureDefinition): number {
	return definition.profile_snapshot?.revision ?? definition.revision;
}

function modeIdOf(definition: FixtureDefinition): string | undefined {
	return (
		definition.mode_id ??
		definition.profile_snapshot?.modes.find((mode) => mode.name === definition.mode)?.id
	);
}

/**
 * The newer revision this fixture could be brought up to, or null when it is already the newest.
 *
 * `modeName` is what the fixture's own mode is called, so a fixture in a particular mode stays in
 * that mode across the upgrade even when the new revision numbered its modes differently.
 */
export function newerRevision(
	definitions: readonly FixtureDefinition[],
	fixture: Pick<PatchFixtureProjection, "profileId" | "profileRevision" | "modeId">,
	modeName?: string,
): UpgradeTarget | null {
	const mine = definitions.filter((definition) => profileIdOf(definition) === fixture.profileId);
	const newest = Math.max(...mine.map(revisionOf), Number.NEGATIVE_INFINITY);
	if (!Number.isFinite(newest) || newest <= fixture.profileRevision) return null;
	const candidates = mine.filter((definition) => revisionOf(definition) === newest);
	const chosen =
		candidates.find((definition) => modeIdOf(definition) === fixture.modeId) ??
		(modeName ? candidates.find((definition) => definition.mode === modeName) : undefined) ??
		candidates[0];
	const modeId = chosen && modeIdOf(chosen);
	if (!modeId) return null;
	return {
		profileRevision: newest,
		modeId,
		scenery: chosen.profile_snapshot?.scenery ?? null,
	};
}

/**
 * The same element, referencing the newer revision.
 *
 * A measurement the new profile still lets the operator set keeps the value they set; one it does
 * not — a truss section, a corner block's size — takes the new profile's own, so the correction
 * actually reaches the element.
 */
export function upgraded(
	fixture: PatchFixtureProjection,
	target: UpgradeTarget,
): PatchFixtureProjection {
	const stored = fixture.scenerySizeMetres;
	const scenery = target.scenery;
	const size =
		stored && scenery
			? SIZE_AXES.reduce(
					(next, { axis, key }) => ({
						...next,
						[key]: scenery.adjustable[axis]
							? stored[key]
							: Math.round(scenery.default_size_metres[key] * 1000),
					}),
					{} as { x: number; y: number; z: number },
				)
			: stored;
	return {
		...fixture,
		profileRevision: target.profileRevision,
		modeId: target.modeId,
		scenerySizeMetres: size,
	};
}
