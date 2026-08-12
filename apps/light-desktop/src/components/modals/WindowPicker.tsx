import { Button, ModalFrame, SelectionCardContent } from "@tosklight/ui";
import { useState } from "react";
import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";

export type WindowCategoryId =
	| "programming"
	| "playback"
	| "show"
	| "miscellaneous";

export interface WindowChoice {
	kind: BuiltInWindow;
	title: string;
	description: string;
}

export interface WindowChoiceCategory {
	id: WindowCategoryId;
	label: string;
	choices: readonly WindowChoice[];
}

export const windowCategories: readonly WindowChoiceCategory[] = [
	{
		id: "programming",
		label: "Programming",
		choices: [
			{
				kind: "presets",
				title: "Preset pool",
				description: "Store and recall reusable attribute values.",
			},
			{
				kind: "groups",
				title: "Group pool",
				description: "Build and recall ordered fixture selections.",
			},
			{
				kind: "fixtures",
				title: "Fixture sheet",
				description: "Select fixtures and inspect their current values.",
			},
			{
				kind: "channels",
				title: "Channels",
				description: "Inspect output by DMX channel.",
			},
			{
				kind: "dynamics",
				title: "Dynamics",
				description: "Create and control animated attribute values.",
			},
			{
				kind: "macros",
				title: "Macro Pool",
				description: "Run and edit reusable command sequences.",
			},
		],
	},
	{
		id: "playback",
		label: "Playback & Automation",
		choices: [
			{
				kind: "cuelists",
				title: "Cuelists",
				description: "Work across the pool, Cues, and Cuelist settings.",
			},
			{
				kind: "cues",
				title: "Cues",
				description: "View Cues for a selected Cuelist.",
			},
			{
				kind: "virtual_playbacks",
				title: "Virtual Playbacks",
				description: "Place playback actions on a configurable grid.",
			},
			{
				kind: "timecode",
				title: "Timecode",
				description: "Program and control timed show automation.",
			},
		],
	},
	{
		id: "show",
		label: "Show & Visual",
		choices: [
			{
				kind: "stage",
				title: "Stage",
				description: "View and arrange the show in 2D or 3D.",
			},
			{
				kind: "media",
				title: "Media",
				description: "Configure media layers and reconcile live CITP data.",
			},
			{
				kind: "dmx",
				title: "DMX output",
				description: "Inspect live universe values and diagnostics.",
			},
		],
	},
	{
		id: "miscellaneous",
		label: "Miscellaneous",
		choices: [
			{
				kind: "running",
				title: "Running",
				description: "Monitor and stop active runtime objects.",
			},
			{
				kind: "scheduler",
				title: "Scheduler",
				description: "Schedule show actions by date and time.",
			},
			{
				kind: "file_manager",
				title: "File Manager",
				description: "Manage files exposed by the desk server.",
			},
			{
				kind: "help",
				title: "Help",
				description: "Read the operator manual inside ToskLight.",
			},
			{
				kind: "text_editor",
				title: "Text Editor",
				description: "Open and edit show text files.",
			},
		],
	},
];

export const windowChoices: Array<[BuiltInWindow, string]> =
	windowCategories.flatMap((category) =>
		category.choices.map(({ kind, title }) => [kind, title]),
	);

export const availableWindowChoices = () => windowChoices;

export const availableWindowCategoryChoices = (category: WindowCategoryId) =>
	windowCategories.find(({ id }) => id === category)?.choices ?? [];

export function WindowPicker() {
	const { state, dispatch } = useApp();
	const [category, setCategory] = useState<WindowCategoryId>("programming");
	if (!state.windowPicker) return null;
	const close = () => dispatch({ type: "OPEN_WINDOW_PICKER", rect: null });
	const activeCategory = windowCategories.find(({ id }) => id === category);
	if (!activeCategory) return null;
	const choices = availableWindowCategoryChoices(category);
	return (
		<ModalFrame
			id="window-picker"
			ariaLabel="Open Window"
			title="Open Window"
			className="window-picker-layer"
			dialogClassName="window-picker-modal"
			tabs={windowCategories.map(({ id, label }) => ({ id, label }))}
			activeTab={category}
			onTabChange={(id) => setCategory(id as WindowCategoryId)}
			onClose={close}
			closeLabel="Close Open Window"
		>
			<div
				className="ui-grouped-selection-options window-picker-options"
				role="tabpanel"
				aria-label={activeCategory.label}
			>
				{choices.map((choice) => (
					<Button
						key={choice.kind}
						contentAlign="left"
						onClick={() => dispatch({ type: "ADD_WINDOW", kind: choice.kind })}
					>
						<SelectionCardContent
							label={choice.title}
							description={choice.description}
						/>
					</Button>
				))}
			</div>
		</ModalFrame>
	);
}
