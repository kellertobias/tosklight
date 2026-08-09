import type { DeskConfiguration } from "../../api/types";
import type { ConfigurationSnapshot } from "./store";

export function selectProgrammerFadeMillis(snapshot: ConfigurationSnapshot) {
	return snapshot.configuration?.programmer_fade_millis ?? null;
}

export function selectDirectEntryUsesProgrammerFade(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.command_line_at_uses_programmer_fade ?? true;
}

export function selectSequenceMasterFadeMillis(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.sequence_master_fade_millis ?? null;
}

export function selectCuelistAutoOffAtZeroDefault(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.cuelist_auto_off_at_zero_default ?? false;
}

export function selectCuelistAutoOffFlashReleaseDefault(
	snapshot: ConfigurationSnapshot,
) {
	return (
		snapshot.configuration?.cuelist_auto_off_flash_release_default ?? false
	);
}

export function selectStartAfterFirstRecording(
	snapshot: ConfigurationSnapshot,
) {
	return snapshot.configuration?.start_after_first_recording ?? false;
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
