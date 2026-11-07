import { MultiValueToggleField } from "@tosklight/ui";
import type { PlaybackDefinition } from "../../api/types";
import type { PaneFootprintDefault } from "./playbackFaderBank/types";

type Footprint = NonNullable<PlaybackDefinition["footprint"]>;

const physicalFootprintOptions = [
	{ value: "normal", label: "Normal" },
	{ value: "taller", label: "Taller" },
	{ value: "wider", label: "Wider" },
] satisfies { value: Footprint["type"]; label: string }[];

const paneFootprintOptions = [
	{ value: "single_button", label: "Single button" },
	{ value: "pane_normal", label: "Normal" },
	{ value: "pane_taller", label: "Taller" },
	{ value: "pane_wider", label: "Wider" },
] satisfies { value: Footprint["type"]; label: string }[];

export function PlaybackFootprintField({
	draft,
	paneFootprintDefault,
	onDraftChange,
}: {
	draft: PlaybackDefinition;
	paneFootprintDefault?: PaneFootprintDefault;
	onDraftChange: (playback: PlaybackDefinition) => void;
}) {
	const footprint = draft.footprint ?? { type: "normal" as const };
	const authoredPaneFootprint = paneFootprintOptions.some(
		({ value }) => value === footprint.type,
	);
	const pane = authoredPaneFootprint || paneFootprintDefault != null;
	let selected = footprint.type;
	if (!authoredPaneFootprint && paneFootprintDefault) {
		selected =
			paneFootprintDefault === "single_button" || footprint.type === "normal"
				? paneFootprintDefault
				: footprint.type === "taller"
					? "pane_taller"
					: "pane_wider";
	}
	return (
		<MultiValueToggleField<Footprint["type"]>
			label="Footprint"
			value={selected}
			onChange={(type) => {
				if (type !== footprint.type)
					onDraftChange(withFootprint(draft, footprint, type));
			}}
			options={pane ? paneFootprintOptions : physicalFootprintOptions}
			description={
				pane
					? "Single button shows one action and keeps the configured buttons. Normal needs two empty cells below. Taller also needs the cell above. Wider adds a column on the right. When the space is unavailable, the pane shows a single button."
					: "Expanded controls remain saved when this surface cannot expose them."
			}
		/>
	);
}

function withFootprint(
	draft: PlaybackDefinition,
	previous: Footprint,
	type: Footprint["type"],
): PlaybackDefinition {
	const defaults = defaultButtons(draft.target.type);
	let footprint: Footprint;
	switch (type) {
		case "taller":
		case "pane_taller":
			footprint = {
				type,
				upper_button: "upper_button" in previous ? previous.upper_button : "none",
			};
			break;
		case "wider":
		case "pane_wider":
			footprint = {
				type,
				right_buttons:
					"right_buttons" in previous ? previous.right_buttons : defaults,
				right_fader:
					"right_fader" in previous
						? previous.right_fader
						: defaultFader(draft.target.type),
			};
			break;
		default:
			footprint = { type };
	}

	const count = draft.button_count ?? 3;
	const paneExpanded =
		type === "pane_normal" || type === "pane_taller" || type === "pane_wider";
	if (paneExpanded && (count === 1 || count === 2)) {
		const buttons = [...draft.buttons] as PlaybackDefinition["buttons"];
		for (let index = count; index < buttons.length; index += 1)
			buttons[index] = defaults[index];
		return { ...draft, footprint, button_count: 3, buttons };
	}
	return { ...draft, footprint };
}

function defaultButtons(
	type: PlaybackDefinition["target"]["type"],
): PlaybackDefinition["buttons"] {
	if (type === "cue_list") return ["go_minus", "go", "flash"];
	if (type === "macro") return ["go", "none", "none"];
	if (type === "timecode") return ["go", "pause", "off"];
	if (type === "dynamic") return ["off", "pause", "flash"];
	if (type === "group") return ["select", "select_dereferenced", "flash"];
	if (type === "speed_group") return ["double", "half", "learn"];
	if (type === "grand_master") return ["blackout", "pause_dynamics", "flash"];
	return ["double", "half", "off"];
}

function defaultFader(
	type: PlaybackDefinition["target"]["type"],
): PlaybackDefinition["fader"] {
	return type === "speed_group" ? "learned_percentage" : "master";
}
