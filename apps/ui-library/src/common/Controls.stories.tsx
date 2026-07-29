import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { SearchBar, TouchSelect } from "./index";
import {
	CheckboxField,
	ColorPickerField,
	CyclingValueToggleField,
	FileDropField,
	FormField,
	FormLayout,
	GroupedSelectionField,
	IconPickerField,
	LargeTextField,
	MultiValueToggleField,
	NumberField,
	Select,
	SelectField,
	SwitchField,
	TextAreaField,
	TextField,
} from "../controls";
import { HorizontalFaderField } from "./FaderControls";

const meta = {
	title: "Controls/Forms",
	component: FormLayout,
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: { source: { type: "dynamic" } },
	},
	args: {
		children: null,
	},
} satisfies Meta<typeof FormLayout>;

export default meta;
type Story = StoryObj<typeof meta>;

function InputFieldsExample() {
	const [name, setName] = useState("Main Stage");
	const [password, setPassword] = useState("operator");
	const [rows, setRows] = useState("4");
	const [scale, setScale] = useState("1.25");
	const [faderValue, setFaderValue] = useState("42");
	const [presetValue, setPresetValue] = useState("50");
	const notes = [
		"House opens at 18:30.",
		"Preset the front wash at 42%.",
		"Keep the lectern special isolated.",
		"Check the stage-right practical.",
		"Confirm haze with venue staff.",
		"Hold audience blinders until finale.",
		"Followspot channel is intercom 3.",
		"Interval state uses cue 27.",
		"Emergency look is playback 10.",
		"Touring console receives universe 4.",
		"Save a revision after sound check.",
		"Operator handover begins at 19:15.",
	].join("\n");
	const valuePresets = {
		groups: [
			{
				label: "Intensity",
				options: [
					{ value: "0", label: "Off", description: "Release output to zero." },
					{ value: "25", label: "Quarter", description: "Low working level." },
					{
						value: "50",
						label: "Half",
						description: "Balanced working level.",
					},
					{ value: "100", label: "Full", description: "Maximum output." },
				],
			},
			{
				label: "Operator defaults",
				options: [
					{
						value: "42",
						label: "House preset",
						icon: "★",
						description: "Stored house intensity.",
					},
					{
						value: "68",
						label: "Show level",
						icon: "●",
						description: "Current show default.",
					},
				],
			},
		],
		selectedValue: presetValue,
	} as const;
	return (
		<div className="forms-story-canvas">
			<section>
				<h2>Text inputs</h2>
				<FormLayout columns={2} labelPlacement="top">
					<TextField
						label="Name"
						description="Visible operator name"
						required
						clearable
						value={name}
						onChange={(event) => setName(event.target.value)}
					/>
					<TextField
						label="Password"
						secure
						value={password}
						onChange={(event) => setPassword(event.target.value)}
					/>
					<TextField
						label="Compact name"
						controlSize="compact"
						defaultValue="Front Wash"
					/>
					<TextField label="Required field" error="Required" required />
					<TextAreaField
						label="Notes"
						defaultValue={notes}
						placeholder="Add operator notes"
					/>
					<LargeTextField
						label="Large text"
						defaultValue={"Line one\nLine two\nLine three"}
					/>
				</FormLayout>
			</section>
			<section>
				<h2>Number inputs</h2>
				<FormLayout columns={2} labelPlacement="top">
					<NumberField
						label="Rows"
						value={rows}
						min={1}
						max={18}
						onValueChange={setRows}
					/>
					<NumberField
						label="Scale"
						value={scale}
						allowDecimal
						step={0.05}
						unit="×"
						onValueChange={setScale}
					/>
					<NumberField
						label="Value with fader"
						value={faderValue}
						allowDecimal
						min={0}
						max={100}
						modalFader={{ maximum: 100, step: 0.1, accentColor: "#1bd6ec" }}
						onValueChange={setFaderValue}
					/>
					<NumberField
						label="Value with presets"
						value={presetValue}
						allowDecimal
						min={0}
						max={100}
						modalPresets={valuePresets}
						onValueChange={setPresetValue}
						onModalRelease={() => setPresetValue("")}
						modalReleaseLabel="Release value"
					/>
					<NumberField
						label="Fixed value"
						value="512"
						showStepButtons={false}
						disabled
					/>
				</FormLayout>
			</section>
		</div>
	);
}

export const InputFields: Story = {
	render: () => <InputFieldsExample />,
};

const formSelectionGroups = [
	{
		label: "Step Control",
		options: [
			{
				value: "go",
				label: "GO",
				icon: "▶",
				description: "Advance to the next cue.",
			},
			{
				value: "go-minus",
				label: "GO MINUS",
				description: "Return to the previous cue.",
			},
		],
	},
	{
		label: "Temporary State",
		options: [
			{
				value: "flash",
				label: "FLASH",
				icon: "⚡",
				description: "Output while the button is held.",
			},
		],
	},
] as const;

function FileDropStates() {
	return (
		<section>
			<h2>File drop states</h2>
			<FormLayout columns={3}>
				<FileDropField
					label="Loading"
					status="loading"
					statusMessage="Loading selected file…"
					constraints={{ extensions: [".gdtf"] }}
					onFiles={() => undefined}
					onOpenPicker={() => undefined}
				/>
				<FileDropField
					label="Success"
					status="success"
					statusMessage="touring-profile.gdtf"
					constraints={{ extensions: [".gdtf"] }}
					onFiles={() => undefined}
					onOpenPicker={() => undefined}
				/>
				<FileDropField
					label="Actionable error"
					status="error"
					statusMessage="The archive could not be read."
					constraints={{ extensions: [".gdtf"] }}
					onFiles={() => undefined}
					onOpenPicker={() => undefined}
				/>
			</FormLayout>
		</section>
	);
}

function FormComponentsExample() {
	const [mode, setMode] = useState("software");
	const [stageView, setStageView] = useState("2d");
	const [curveMethod, setCurveMethod] = useState("keyframes");
	const [level, setLevel] = useState(68);
	const [fullscreen, setFullscreen] = useState(true);
	const [locked, setLocked] = useState(false);
	const [icon, setIcon] = useState("◇");
	const [color, setColor] = useState("#1bd6ec");
	const [buttonAction, setButtonAction] = useState("go");
	const [faderAction, setFaderAction] = useState("master");
	const [fileState, setFileState] = useState("No file selected");
	const [universe, setUniverse] = useState(1);
	return (
		<div className="forms-story-canvas">
			<section>
				<h2>Selection and state controls</h2>
				<FormLayout columns={2} labelPlacement="top">
					<SelectField
						label="Mode"
						value={mode}
						options={[
							{ value: "software", label: "Software" },
							{ value: "hardware", label: "Hardware" },
							{ value: "unavailable", label: "Unavailable", disabled: true },
						]}
						onChange={setMode}
					/>
					<MultiValueToggleField
						label="Stage view"
						value={stageView}
						options={[
							{ value: "2d", label: "2D" },
							{ value: "3d", label: "3D" },
						]}
						onChange={setStageView}
					/>
					<CyclingValueToggleField
						label="Curve method"
						description="Press repeatedly to cycle through the available values."
						ariaLabel="Curve method"
						value={curveMethod}
						options={[
							{ value: "keyframes", label: "Keyframes" },
							{ value: "max_min", label: "Max / min" },
							{ value: "middle_amplitude", label: "Middle / amplitude" },
						]}
						onChange={setCurveMethod}
					/>
					<SwitchField
						label="Window mode"
						offLabel="Windowed"
						onLabel="Fullscreen"
						checked={fullscreen}
						onChange={(event) => setFullscreen(event.target.checked)}
					/>
					<CheckboxField
						label="Desktop lock"
						stateLabel="Prevent layout changes"
						checked={locked}
						onChange={(event) => setLocked(event.target.checked)}
					/>
					<IconPickerField
						label="Icon"
						value={icon}
						defaultGroup="gobo"
						onChange={setIcon}
					/>
					<ColorPickerField label="Color" value={color} onChange={setColor} />
					<FormField label="Native select">
						<Select aria-label="Universe type" defaultValue="1">
							<option value="1">Universe 1</option>
							<option value="2">Universe 2</option>
						</Select>
					</FormField>
					<TouchSelect
						label="Universe"
						value={universe}
						options={[1, 2, 3, 4]}
						onChange={setUniverse}
					/>
				</FormLayout>
			</section>
			<section>
				<h2>Side labels and compatibility controls</h2>
				<FormLayout columns={2} labelPlacement="side" labelWidth={130}>
					<FileDropField
						label="Fixture profile"
						constraints={{
							extensions: [".gdtf", ".toskfixture"],
							mimeTypes: ["application/zip"],
							multiple: false,
						}}
						selectedLabel={fileState}
						onFiles={(files) =>
							setFileState(files[0]?.name ?? "No file selected")
						}
						onRejected={setFileState}
						onOpenPicker={() => setFileState("ToskLight File Manager opened")}
					/>
					<HorizontalFaderField
						label="Level"
						value={level}
						display={`${level}%`}
						onChange={setLevel}
					/>
				</FormLayout>
			</section>
			<section>
				<h2>Grouped selections</h2>
				<FormLayout columns={2}>
					<GroupedSelectionField
						label="Top button"
						value={buttonAction}
						groups={formSelectionGroups}
						onChange={setButtonAction}
						clearAction={{ label: "Empty Button", value: "none" }}
					/>
					<GroupedSelectionField
						label="Fader"
						value={faderAction}
						groups={[
							{
								label: "Level Control",
								options: [
									{
										value: "master",
										label: "Master",
										description: "Control playback intensity.",
									},
									{
										value: "temp",
										label: "Temp",
										description: "Temporarily bring the playback in.",
									},
								],
							},
						]}
						onChange={setFaderAction}
					/>
				</FormLayout>
			</section>
			<FileDropStates />
		</div>
	);
}

export const FormComponents: Story = {
	render: () => <FormComponentsExample />,
};

function SearchExample() {
	const [search, setSearch] = useState("wash");
	const [filter, setFilter] = useState("");
	return (
		<div style={{ width: 720, display: "grid", gap: 16 }}>
			<SearchBar value={search} onChange={setSearch} />
			<SearchBar
				value={search}
				onChange={setSearch}
				settings={[
					{
						kind: "select",
						id: "type",
						label: "Fixture type",
						value: filter,
						options: [
							{ value: "", label: "All" },
							{ value: "Dimmer", label: "Dimmer" },
							{ value: "Moving light", label: "Moving light" },
						],
					},
				]}
				onSettingChange={(_, value) => setFilter(String(value))}
				onClearSettings={() => setFilter("")}
			/>
		</div>
	);
}

export const Search: Story = {
	render: () => <SearchExample />,
};
