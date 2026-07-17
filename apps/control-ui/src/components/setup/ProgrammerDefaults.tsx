import type { UpdateSettings } from "../../api/types";
import {
	FormLayout,
	MultiValueToggleField,
	SelectField,
	SwitchField,
} from "../common";
import {
	cueUpdateModes,
	existingContentModes,
} from "../control/updateWorkflow";

export interface RecordSettings {
	mode: "merge" | "overwrite";
	cueOnly: boolean;
	mergeActiveCue: boolean;
}

export const defaultRecordSettings: RecordSettings = {
	mode: "merge",
	cueOnly: false,
	mergeActiveCue: false,
};

export function loadRecordSettings(): RecordSettings {
	return {
		mode:
			localStorage.getItem("light.store-mode") === "overwrite"
				? "overwrite"
				: "merge",
		cueOnly: localStorage.getItem("light.store-cue-only") === "true",
		mergeActiveCue:
			localStorage.getItem("light.store-merge-active-cue") === "true",
	};
}

export function saveRecordSettings(settings: RecordSettings) {
	localStorage.setItem("light.store-mode", settings.mode);
	localStorage.setItem("light.store-cue-only", String(settings.cueOnly));
	localStorage.setItem(
		"light.store-merge-active-cue",
		String(settings.mergeActiveCue),
	);
}

export function RecordDefaultsFields({
	settings,
	onChange,
	labelPlacement = "side",
}: {
	settings: RecordSettings;
	onChange: (settings: RecordSettings) => void;
	labelPlacement?: "side" | "top";
}) {
	return (
		<FormLayout labelPlacement={labelPlacement}>
			<MultiValueToggleField
				label="Record mode"
				ariaLabel="Default Record mode"
				value={settings.mode}
				onChange={(mode) => onChange({ ...settings, mode })}
				options={[
					{ value: "merge", label: "Merge" },
					{ value: "overwrite", label: "Overwrite" },
				]}
			/>
			<SwitchField
				label="Cue only"
				checked={settings.cueOnly}
				onChange={(event) =>
					onChange({ ...settings, cueOnly: event.target.checked })
				}
				description="Restores the recorded addresses in the following Cue while unrelated values keep tracking."
			/>
			<SwitchField
				label="Merge current values into the active Cue when recording to its playback"
				checked={settings.mergeActiveCue}
				onChange={(event) =>
					onChange({ ...settings, mergeActiveCue: event.target.checked })
				}
			/>
		</FormLayout>
	);
}

export function UpdateDefaultsFields({
	settings,
	onChange,
	labelPlacement = "side",
}: {
	settings: UpdateSettings;
	onChange: (settings: UpdateSettings) => void;
	labelPlacement?: "side" | "top";
}) {
	return (
		<FormLayout labelPlacement={labelPlacement}>
			<SelectField
				label="Cue/Cuelist default"
				value={settings.cue_mode}
				onChange={(value) =>
					onChange({ ...settings, cue_mode: value })
				}
				options={cueUpdateModes}
			/>
			<SelectField
				label="Preset default"
				value={settings.preset_mode}
				onChange={(value) =>
					onChange({ ...settings, preset_mode: value })
				}
				options={existingContentModes}
			/>
			<SelectField
				label="Group default"
				value={settings.group_mode}
				onChange={(value) =>
					onChange({ ...settings, group_mode: value })
				}
				options={existingContentModes}
			/>
			<SwitchField
				label="Show Update modal on touch"
				checked={settings.show_update_modal_on_touch}
				onChange={(event) =>
					onChange({
						...settings,
						show_update_modal_on_touch: event.target.checked,
					})
				}
				description="Command-line confirmation with Enter always applies the configured default directly."
			/>
		</FormLayout>
	);
}
