import type { PresetFamily } from "../../presetFamilies";
import {
	normalizePresetFamily,
	presetAddress,
	presetStorageKey,
} from "../../presetFamilies";
import type {
	PresetRecordingActions,
	PresetRecordingMode,
} from "./contracts";
import type { PresetCard } from "./presetCards";

interface SubmitPresetRecordingOptions {
	card: PresetCard | null;
	index: number;
	family: PresetFamily;
	mode: PresetRecordingMode;
	preloadActive: boolean;
	actions: PresetRecordingActions | null;
	storePreload: (
		input: {
			target: "preset";
			target_id: string;
			name: string;
			mode: PresetRecordingMode;
			family: PresetFamily;
		},
		revision: number,
	) => Promise<boolean>;
}

export async function submitPresetRecording(options: SubmitPresetRecordingOptions) {
	const { card, index, family, mode } = options;
	const targetFamily = normalizePresetFamily(card?.body.family, family);
	const address = presetAddress(targetFamily, index + 1);
	const canonicalId = presetStorageKey(address);
	const name = card?.body.name ?? `Preset ${index + 1}`;
	const revision = card?.revision ?? 0;
	if (options.preloadActive) {
		void options.storePreload(
			{
				target: "preset",
				target_id: canonicalId,
				name,
				mode,
				family: targetFamily,
			},
			revision,
		);
		return null;
	}
	if (!options.actions) return null;
	return options.actions.record({
		objectId: card?.id ?? canonicalId,
		address,
		name,
		mode,
		expectedObjectRevision: revision,
	});
}
