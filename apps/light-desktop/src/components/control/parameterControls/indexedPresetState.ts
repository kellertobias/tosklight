import { maxRaw } from "../../setup/fixtureProfileModel";
import type { IndexedPresetChoice } from "./indexedPresetChoices";
import { discreteProgrammerTarget, normalizedProgrammerTarget } from "./model";
import type { ParameterProjection } from "./useParameterProjection";

/** Selection feedback comes from authoritative values, never the last button pressed. */
export function indexedPresetState(
	projection: ParameterProjection,
	attribute: string,
	choices: readonly IndexedPresetChoice[],
): { selectedValue?: string; normalized?: number } {
	for (const choice of choices) {
		if (!choice.semanticId || choice.kind === "control") continue;
		const covered = new Set<string>();
		const positions: number[] = [];
		let matches = true;
		for (const target of choice.targets) {
			const fixture = projection.selectedFixtures.find(
				(f) =>
					f.fixture_id === target.fixtureId ||
					f.logical_heads.some((h) => h.fixture_id === target.fixtureId),
			);
			const mode = fixture?.definition.profile_snapshot?.modes.find(
				(m) => m.id === fixture.definition.mode_id,
			);
			const channel = mode?.channels.find(
				(c) =>
					c.attribute === attribute &&
					c.functions.some((fn) => fn.id === target.functionId),
			);
			const fn = channel?.functions.find((f) => f.id === target.functionId);
			if (
				!fixture ||
				!channel ||
				!fn ||
				!(fn.behavior.type === "fixed" || fn.behavior.type === "indexed")
			) {
				matches = false;
				break;
			}
			const entry = projection.selectedGroupId
				? projection.groupProgrammerValues.find(
						(e) => e.attribute === attribute,
					)
				: projection.programmerValues.find(
						(e) =>
							e.fixtureId === target.fixtureId && e.attribute === attribute,
					);
			// An authored numeric value takes precedence over a stale resolved discrete value.
			const discrete = entry
				? discreteProgrammerTarget(entry.value)
				: projection.selectedGroupId
					? undefined
					: projection.discreteByFixture.get(target.fixtureId)?.get(attribute);
			const normalized = entry
				? normalizedProgrammerTarget(entry.value)
				: projection.selectedGroupId
					? undefined
					: projection.normalizedByFixture
							.get(target.fixtureId)
							?.get(attribute);
			const raw =
				normalized == null || !Number.isFinite(normalized)
					? undefined
					: Math.round(normalized * maxRaw(channel.resolution));
			if (
				discrete != null
					? discrete !== choice.semanticId
					: raw == null || raw < fn.dmx_from || raw > fn.dmx_to
			) {
				matches = false;
				break;
			}
			positions.push(
				discrete == null
					? (normalized as number)
					: fn.behavior.raw_value / maxRaw(channel.resolution),
			);
			covered.add(target.fixtureId);
			if (projection.selectedFixtureIds.includes(fixture.fixture_id))
				covered.add(fixture.fixture_id);
		}
		if (
			!matches ||
			!projection.selectedFixtureIds.length ||
			!projection.selectedFixtureIds.every((id) => covered.has(id))
		)
			continue;
		return {
			selectedValue: choice.id,
			normalized: positions.every((p) => p === positions[0])
				? positions[0]
				: undefined,
		};
	}
	return {};
}
