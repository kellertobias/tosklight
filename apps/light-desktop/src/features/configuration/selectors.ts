import type { DeskConfiguration } from "../../api/types";
import type { ConfigurationSnapshot } from "./store";

export function selectProgrammerFadeMillis(snapshot: ConfigurationSnapshot) {
	return snapshot.configuration?.programmer_fade_millis ?? null;
}

export function selectSequenceMasterFadeMillis(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.sequence_master_fade_millis ?? null;
}

export function selectSpeedGroupsBpm(snapshot: ConfigurationSnapshot) {
	return snapshot.configuration?.speed_groups_bpm ?? null;
}

export function selectPatchPreviewHighlightDmx(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.patch_preview_highlight_dmx ?? false;
}

export function selectMatterEnabled(snapshot: ConfigurationSnapshot) {
	return snapshot.configuration?.matter_enabled ?? false;
}

export function selectFileManagerSystemPickerFallback(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.file_manager_system_picker_fallback ?? false;
}

export function selectDeskConfiguration(
	snapshot: ConfigurationSnapshot,
): DeskConfiguration | null {
	return snapshot.configuration;
}
