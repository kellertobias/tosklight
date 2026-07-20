import type { StoredPreset } from "../../api/types";
import {
	normalizePresetFamily,
	presetAddress,
	presetStorageKey,
	type PresetFamily,
} from "../../presetFamilies";

export interface PresetCard {
	id: string;
	revision?: number;
	body: StoredPreset;
}

export function resolvePresetCards(
	stored: readonly PresetCard[],
	family: PresetFamily,
	count = 200,
) {
	return Array.from({ length: count }, (_, index) => {
		const address = presetAddress(family, index + 1);
		const canonicalId = presetStorageKey(address);
		return (
			stored.find((preset) => preset.id === canonicalId) ??
			stored.find(
				(preset) =>
					normalizePresetFamily(preset.body.family) === family &&
					preset.body.number === address.number,
			) ??
			null
		);
	});
}
