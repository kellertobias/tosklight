import type { FixtureDefinition } from "../../../api/types";
import { fixtureDefinitionKey } from "../fixtureProfileModel";

/** Qualify only visible collisions; immutable selection keys are never changed. */
export function modeChoiceQualifiers(
	modes: FixtureDefinition[],
	label: (mode: FixtureDefinition) => string,
): Map<string, string> {
	const groups = new Map<string, FixtureDefinition[]>();
	for (const mode of modes) {
		const text = label(mode);
		groups.set(text, [...(groups.get(text) ?? []), mode]);
	}
	const result = new Map<string, string>();
	for (const group of groups.values()) {
		if (new Set(group.map(fixtureDefinitionKey)).size < 2) continue;
		const profileLabel = (mode: FixtureDefinition) =>
			`Profile ${mode.profile_snapshot?.name || mode.name || mode.model} · Rev ${mode.revision}`;
		for (const mode of group) {
			const text = profileLabel(mode);
			const peers = group.filter(
				(peer) =>
					profileLabel(peer) === text &&
					fixtureDefinitionKey(peer) !== fixtureDefinitionKey(mode),
			);
			let qualifier = text;
			if (peers.length) {
				// Start with a short profile identity, extending through the composite key only
				// when identical prefixes/profile identities would otherwise remain ambiguous.
				const key = fixtureDefinitionKey(mode);
				let length = Math.min(8, key.length);
				while (
					length < key.length &&
					peers.some(
						(peer) =>
							fixtureDefinitionKey(peer).slice(0, length) ===
							key.slice(0, length),
					)
				)
					length += 1;
				qualifier += ` · ${key.slice(0, length)}`;
			}
			result.set(fixtureDefinitionKey(mode), qualifier);
		}
	}
	return result;
}
