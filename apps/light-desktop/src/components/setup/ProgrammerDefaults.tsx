import {
	FormLayout,
	MultiValueToggleField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import type { UpdateSettings } from "../../api/types";
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
	const stored = (key: string) =>
		typeof globalThis.localStorage?.getItem === "function"
			? globalThis.localStorage.getItem(key)
			: null;
	return {
		mode: stored("light.store-mode") === "overwrite" ? "overwrite" : "merge",
		cueOnly: stored("light.store-cue-only") === "true",
		mergeActiveCue: stored("light.store-merge-active-cue") === "true",
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
	columns = 1,
	minColumnWidth = 240,
}: {
	settings: RecordSettings;
	onChange: (settings: RecordSettings) => void;
	labelPlacement?: "side" | "top";
	columns?: number;
	minColumnWidth?: number;
}) {
	return (
		<FormLayout
			labelPlacement={labelPlacement}
			columns={columns}
			minColumnWidth={minColumnWidth}
		>
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
				offLabel="Tracking"
				onLabel="Cue only"
				checked={settings.cueOnly}
				onChange={(event) =>
					onChange({ ...settings, cueOnly: event.target.checked })
				}
				description="Restores the recorded addresses in the following Cue while unrelated values keep tracking."
			/>
			<SwitchField
				label="Merge current values into the active Cue when recording to its playback"
				offLabel="Keep cue"
				onLabel="Merge values"
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
	columns = 1,
	minColumnWidth = 240,
}: {
	settings: UpdateSettings;
	onChange: (settings: UpdateSettings) => void;
	labelPlacement?: "side" | "top";
	columns?: number;
	minColumnWidth?: number;
}) {
	return (
		<FormLayout
			labelPlacement={labelPlacement}
			columns={columns}
			minColumnWidth={minColumnWidth}
		>
			<SelectField
				label="Cue/Cuelist default"
				value={settings.cue_mode}
				onChange={(value) => onChange({ ...settings, cue_mode: value })}
				options={cueUpdateModes}
			/>
			<SelectField
				label="Preset default"
				value={settings.preset_mode}
				onChange={(value) => onChange({ ...settings, preset_mode: value })}
				options={existingContentModes}
			/>
			<SelectField
				label="Group default"
				value={settings.group_mode}
				onChange={(value) => onChange({ ...settings, group_mode: value })}
				options={existingContentModes}
			/>
			<SwitchField
				label="Show Update modal on touch"
				offLabel="Use default"
				onLabel="Show modal"
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
