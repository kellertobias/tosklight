// Where one output's synchronized play modes take their tempo.
//
// Unlike the DMX patch, this applies live: the next frame follows the new source. It is chosen once
// for the output, never per layer, so there is no priority race between a Speed Group and a
// layer's own Playback BPM channel.

import { SelectField } from "@tosklight/ui/controls";
import { requestId } from "../../shared/api/editing";
import type {
	OutputConfigurationView,
	UpdateOutputConfiguration,
} from "../../shared/api/generated/media-wire";

const CHANNEL = "playback-bpm-channel";
/** Tos Light Control's Speed Groups A–E, numbered 1–5 on the wire. */
const DESK_GROUPS = ["A", "B", "C", "D", "E"];

export function OutputTempoSource({
	output,
	onSave,
}: {
	output: OutputConfigurationView;
	onSave: (edit: UpdateOutputConfiguration) => void;
}) {
	const selected =
		output.tempoSource === "speed-group" && output.speedGroup !== null
			? String(output.speedGroup)
			: CHANNEL;
	const options = [
		{ value: CHANNEL, label: "Each layer's Playback BPM channel" },
		...DESK_GROUPS.map((letter, index) => ({
			value: String(index + 1),
			label: `Light desk Speed Group ${letter}`,
		})),
	];
	if (!options.some((option) => option.value === selected)) {
		options.push({ value: selected, label: `Speed Group ${selected}` });
	}
	return (
		<fieldset>
			<legend>Tempo</legend>
			<SelectField
				label="Synchronized playback follows"
				description="Applies immediately. A Speed Group that stops arriving keeps its last tempo and is shown as lost under Network."
				value={selected}
				options={options}
				onChange={(value) =>
					onSave(
						value === CHANNEL
							? { requestId: requestId(), tempoSource: CHANNEL }
							: {
									requestId: requestId(),
									tempoSource: "speed-group",
									speedGroup: Number(value),
								},
					)
				}
			/>
		</fieldset>
	);
}
