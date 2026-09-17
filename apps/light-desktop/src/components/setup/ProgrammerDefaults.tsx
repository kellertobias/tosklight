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
				description="Merge keeps what the target already stores and adds the programmer values. Overwrite replaces the stored values with the programmer values."
			/>
			<SwitchField
				label="Cue only"
				offLabel="Tracking"
				onLabel="Cue only"
				checked={settings.cueOnly}
				onChange={(event) =>
					onChange({ ...settings, cueOnly: event.target.checked })
				}
				description="On: the recorded values last for this Cue only. The next Cue returns those fixture attributes to their earlier values, or releases them. Everything else keeps tracking. Off: values track into later Cues as usual."
			/>
			<SwitchField
				label="Merge into active Cue"
				offLabel="Off"
				onLabel="Merge"
				checked={settings.mergeActiveCue}
				onChange={(event) =>
					onChange({ ...settings, mergeActiveCue: event.target.checked })
				}
				description="Recording onto a playback adds the programmer values to the Cue that playback is on. Fixture attributes in both are replaced; all other values stored in that Cue stay."
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
