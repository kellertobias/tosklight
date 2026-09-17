import {
	FormLayout,
	MultiValueToggleField,
	SelectField,
	SwitchField,
} from "@tosklight/ui";
import type { RecordUpdateOption, UpdateSettings } from "../../api/types";
import { RECORD_UPDATE_OPTIONS } from "../../features/recordUpdateOptions/options";
import {
	cueUpdateModes,
	existingContentModes,
} from "../control/updateWorkflow";

/** Browser-local Record preferences. The Record default itself is desk data (`record_default`). */
export interface RecordSettings {
	cueOnly: boolean;
}

export const defaultRecordSettings: RecordSettings = {
	cueOnly: false,
};

export function loadRecordSettings(): RecordSettings {
	const stored =
		typeof globalThis.localStorage?.getItem === "function"
			? globalThis.localStorage.getItem("light.store-cue-only")
			: null;
	return { cueOnly: stored === "true" };
}

export function saveRecordSettings(settings: RecordSettings) {
	localStorage.setItem("light.store-cue-only", String(settings.cueOnly));
}

export function RecordUpdateDefaultField({
	kind,
	value,
	onChange,
	disabled = false,
}: {
	kind: "record" | "update";
	value: RecordUpdateOption;
	onChange: (value: RecordUpdateOption) => void;
	disabled?: boolean;
}) {
	const verb = kind === "record" ? "Record" : "Update";
	const selected = RECORD_UPDATE_OPTIONS.find(
		(option) => option.value === value,
	);
	return (
		<MultiValueToggleField
			label={`${verb} default`}
			ariaLabel={`Default ${verb} mode`}
			value={value}
			disabled={disabled}
			onChange={onChange}
			options={RECORD_UPDATE_OPTIONS.map(({ value, label }) => ({
				value,
				label,
			}))}
			description={kind === "record" ? selected?.record : selected?.update}
		/>
	);
}

export function RecordDefaultsFields({
	settings,
	onChange,
	recordDefault,
	onRecordDefault,
	recordDefaultDisabled = false,
	updateDefault,
	onUpdateDefault,
	labelPlacement = "side",
	columns = 1,
	minColumnWidth = 240,
}: {
	settings: RecordSettings;
	onChange: (settings: RecordSettings) => void;
	recordDefault: RecordUpdateOption;
	onRecordDefault: (value: RecordUpdateOption) => void;
	recordDefaultDisabled?: boolean;
	/** Desk Setup keeps both plain-key defaults side by side. */
	updateDefault?: RecordUpdateOption;
	onUpdateDefault?: (value: RecordUpdateOption) => void;
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
			<RecordUpdateDefaultField
				kind="record"
				value={recordDefault}
				onChange={onRecordDefault}
				disabled={recordDefaultDisabled}
			/>
			{updateDefault && onUpdateDefault && (
				<RecordUpdateDefaultField
					kind="update"
					value={updateDefault}
					onChange={onUpdateDefault}
				/>
			)}
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
		</FormLayout>
	);
}

export function UpdateDefaultsFields({
	settings,
	onChange,
	showDefault = true,
	labelPlacement = "side",
	columns = 1,
	minColumnWidth = 240,
}: {
	settings: UpdateSettings;
	onChange: (settings: UpdateSettings) => void;
	/** Off where the Update default is already shown beside the Record default. */
	showDefault?: boolean;
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
			{showDefault && (
				<RecordUpdateDefaultField
					kind="update"
					value={settings.update_default}
					onChange={(value) =>
						onChange({ ...settings, update_default: value })
					}
				/>
			)}
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
