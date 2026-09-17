import { NumberField, SelectField, SwitchField } from "@tosklight/ui";
import type { FixedScreenPane } from "../../../api/types";
import { DEFAULT_FIXED_SCREEN_PANE } from "../screenConfiguration";

export type CuelistOption = { id: string; name: string };
export type TextFileOption = {
	root: string;
	rootLabel: string;
	path: string;
	name: string;
};

export const fixedPaneLabels: Record<FixedScreenPane["type"], string> = {
	fixture_sheet: "Fixture Sheet",
	stage_2d: "Stage - 2D",
	stage_3d: "Stage - 3D",
	cues: "Cues - Cuelist",
	text: "Text",
};

export function defaultFixedPane(
	type: FixedScreenPane["type"],
): FixedScreenPane {
	switch (type) {
		case "fixture_sheet":
			return DEFAULT_FIXED_SCREEN_PANE;
		case "stage_2d":
			return { type, follow_preload: false, show_floor_grid: true };
		case "stage_3d":
			return {
				type,
				follow_preload: false,
				show_floor_grid: true,
				show_beam_guides: true,
				render_quality: "lines_and_beams",
				environment_brightness: 1,
			};
		case "cues":
			return { type, cue_list_id: "" };
		case "text":
			return { type, root: "", path: "", mode: "plain" };
	}
}

function FixtureSheetFixedSettings({
	pane,
	cueLists,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "fixture_sheet" }>;
	cueLists: readonly CuelistOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	const columnOptions = [
		["id", "Fixture ID"],
		["icon", "Icon"],
		["name", "Name"],
		["patch", "Patch address"],
		["intensity", "Intensity"],
		["color", "Color"],
		["position", "Position"],
		["beam", "Beam"],
		["shapers", "Shapers"],
		["focus", "Focus"],
		["control", "Control"],
		["media", "Media"],
	] as const;
	return (
		<>
			<SelectField
				label="Compact mode"
				value={pane.compact_mode}
				onChange={(compact_mode) => update({ ...pane, compact_mode })}
				options={[
					{ value: "off", label: "Off" },
					{ value: "icon_only", label: "Icon only" },
					{ value: "text_only", label: "Text only" },
				]}
			/>
			<SelectField
				label="Fixture heads"
				value={pane.included_heads}
				onChange={(included_heads) => update({ ...pane, included_heads })}
				options={[
					{ value: "all", label: "All" },
					{ value: "no_sub_heads", label: "No sub heads" },
					{ value: "no_master_heads", label: "No master heads" },
				]}
			/>
			<SelectField
				label="Ordering"
				value={pane.order}
				onChange={(order) => update({ ...pane, order })}
				options={[
					{ value: "fixture_id", label: "Fixture ID" },
					{ value: "active", label: "Active fixtures first" },
				]}
			/>
			<SwitchField
				label="Fixture filter"
				offLabel="All fixtures"
				onLabel="Active only"
				checked={pane.active_only}
				onChange={(event) =>
					update({ ...pane, active_only: event.target.checked })
				}
			/>
			<SelectField
				label="Cuelist filter"
				value={pane.cue_list_id ?? ""}
				onChange={(cue_list_id) =>
					update({ ...pane, cue_list_id: cue_list_id || null })
				}
				options={[
					{ value: "", label: "All fixtures" },
					...(pane.cue_list_id &&
					!cueLists.some((cueList) => cueList.id === pane.cue_list_id)
						? [
								{
									value: pane.cue_list_id,
									label: "Configured Cuelist is unavailable",
								},
							]
						: []),
					...cueLists.map((cueList) => ({
						value: cueList.id,
						label: cueList.name,
					})),
				]}
			/>
			<SwitchField
				label="Name details"
				offLabel="Names only"
				onLabel="Show fixture type"
				checked={pane.show_type}
				onChange={(event) =>
					update({ ...pane, show_type: event.target.checked })
				}
			/>
			<SwitchField
				label="Group shortcuts"
				offLabel="Hidden"
				onLabel="Visible"
				checked={pane.show_group_shortcuts}
				onChange={(event) =>
					update({ ...pane, show_group_shortcuts: event.target.checked })
				}
			/>
			<fieldset className="fixed-screen-column-settings">
				<legend>Columns</legend>
				{columnOptions.map(([column, label]) => (
					<SwitchField
						key={column}
						label={label}
						offLabel="Hidden"
						onLabel="Visible"
						checked={pane.columns.includes(column)}
						disabled={pane.columns.length === 1 && pane.columns[0] === column}
						onChange={(event) =>
							update({
								...pane,
								columns: event.target.checked
									? [...pane.columns, column]
									: pane.columns.filter((candidate) => candidate !== column),
							})
						}
					/>
				))}
			</fieldset>
		</>
	);
}

function Stage2dFixedSettings({
	pane,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "stage_2d" }>;
	update: (pane: FixedScreenPane) => void;
}) {
	return (
		<SwitchField
			label="Preload source"
			offLabel="Live"
			onLabel="Follow preload"
			checked={pane.follow_preload}
			onChange={(event) =>
				update({ ...pane, follow_preload: event.target.checked })
			}
		/>
	);
}

function Stage3dFixedSettings({
	pane,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "stage_3d" }>;
	update: (pane: FixedScreenPane) => void;
}) {
	return (
		<>
			<SwitchField
				label="Preload source"
				offLabel="Live"
				onLabel="Follow preload"
				checked={pane.follow_preload}
				onChange={(event) =>
					update({ ...pane, follow_preload: event.target.checked })
				}
			/>
			<SwitchField
				label="Floor grid"
				offLabel="Hidden"
				onLabel="Visible"
				checked={pane.show_floor_grid}
				onChange={(event) =>
					update({ ...pane, show_floor_grid: event.target.checked })
				}
			/>
			<SwitchField
				label="Beam direction guidelines"
				offLabel="Hidden"
				onLabel="Visible"
				checked={pane.show_beam_guides}
				onChange={(event) =>
					update({ ...pane, show_beam_guides: event.target.checked })
				}
			/>
			<SelectField
				label="Render quality"
				value={pane.render_quality}
				onChange={(render_quality) => update({ ...pane, render_quality })}
				options={[
					{ value: "lines_only", label: "Lines only" },
					{ value: "lines_and_beams", label: "Lines and beams" },
					{ value: "full", label: "Full" },
				]}
			/>
			<NumberField
				label="Environment brightness"
				min="0"
				max="1"
				step="0.05"
				value={pane.environment_brightness}
				onChange={(event) =>
					update({
						...pane,
						environment_brightness: Number(event.target.value),
					})
				}
			/>
		</>
	);
}

function CuesFixedSettings({
	pane,
	cueLists,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "cues" }>;
	cueLists: readonly CuelistOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	return (
		<SelectField
			label="Cuelist"
			value={pane.cue_list_id}
			onChange={(cue_list_id) => update({ ...pane, cue_list_id })}
			options={[
				...(!pane.cue_list_id
					? [{ value: "", label: "Unavailable - choose a Cuelist" }]
					: cueLists.some((cueList) => cueList.id === pane.cue_list_id)
						? []
						: [
								{
									value: pane.cue_list_id,
									label: "Configured Cuelist is unavailable",
								},
							]),
				...cueLists.map((cueList) => ({
					value: cueList.id,
					label: cueList.name,
				})),
			]}
		/>
	);
}

function TextFixedSettings({
	pane,
	textFiles,
	update,
}: {
	pane: Extract<FixedScreenPane, { type: "text" }>;
	textFiles: readonly TextFileOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	const selectedTextValue =
		pane.root && pane.path ? `${pane.root}\u0000${pane.path}` : "";
	return (
		<>
			<SelectField
				label="Text"
				value={selectedTextValue}
				onChange={(value) => {
					const [root = "", path = ""] = value.split("\u0000");
					update({ ...pane, root, path });
				}}
				options={[
					...(!selectedTextValue
						? [{ value: "", label: "Unavailable - choose a text file" }]
						: textFiles.some(
									(file) => file.root === pane.root && file.path === pane.path,
								)
							? []
							: [
									{
										value: selectedTextValue,
										label: "Configured text is unavailable",
									},
								]),
					...textFiles.map((file) => ({
						value: `${file.root}\u0000${file.path}`,
						label: `${file.rootLabel} · ${file.name}`,
					})),
				]}
			/>
			<SelectField
				label="Text view"
				value={pane.mode}
				onChange={(mode) => update({ ...pane, mode })}
				options={[
					{ value: "plain", label: "Plain Text" },
					{ value: "markdown", label: "Rendered Markdown" },
				]}
			/>
		</>
	);
}

export function FixedPaneSettings({
	pane,
	cueLists,
	textFiles,
	update,
}: {
	pane: FixedScreenPane;
	cueLists: readonly CuelistOption[];
	textFiles: readonly TextFileOption[];
	update: (pane: FixedScreenPane) => void;
}) {
	if (pane.type === "fixture_sheet")
		return (
			<FixtureSheetFixedSettings
				pane={pane}
				cueLists={cueLists}
				update={update}
			/>
		);
	if (pane.type === "stage_2d")
		return <Stage2dFixedSettings pane={pane} update={update} />;
	if (pane.type === "stage_3d")
		return <Stage3dFixedSettings pane={pane} update={update} />;
	if (pane.type === "cues")
		return (
			<CuesFixedSettings pane={pane} cueLists={cueLists} update={update} />
		);
	return (
		<TextFixedSettings pane={pane} textFiles={textFiles} update={update} />
	);
}
