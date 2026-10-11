import type {
	PlaybackAction,
	PlaybackActionRequest,
	PlaybackAddress,

} from "../../api/types";

export type PlaybackControlAction =
	| "button"
	| "configured-fader"
	| "on"
	| "off"
	| "toggle"
	| "go"
	| "go-minus"
	| "go-to"
	| "load"
	| "fast-forward"
	| "fast-rewind"
	| "temp"
	| "temp-on"
	| "temp-off"
	| "swap"
	| "select"
	| "select-contents"
	| "select-dereferenced"
	| "learn"
	| "double"
	| "half"
	| "pause"
	| "blackout"
	| "pause-dynamics"
	| "flash"
	| "master"
	| "xfade-on"
	| "xfade-off";

export interface PlaybackControlInput {
	value?: number;
	pressed?: boolean;
	button?: number;
	fader?: number;
	cue_number?: string;
	control_source: { surface_id: string; control_id: number };
}

export function playbackRequest(
	address: { page: number; position: number },
	action: PlaybackControlAction,
	input: PlaybackControlInput,
): PlaybackActionRequest {
	return {
		address: { kind: "playback", ...address },
		action: structuredAction(action, input),
		control_source: input.control_source,
	};
}

function structuredAction(
	action: PlaybackControlAction,
	input: PlaybackControlInput,
): PlaybackAction {
	const pressed = input.pressed ?? true;
	if (action === "button")
		return {
			type: "configured_button",
			number: required(input.button, "button number"),
			pressed,
		};
	if (action === "master")
		return { type: "master", value: required(input.value, "master value") };
	if (action === "configured-fader")
		return {
			type: "configured_fader",
			number: required(input.fader, "fader number"),
			level: required(input.value, "fader value"),
		};
	if (action === "go-to" || action === "load")
		return {
			type: action === "go-to" ? "go_to" : "load",
			cue_number: required(input.cue_number, "cue number"),
		};
	if (action === "xfade-on" || action === "xfade-off")
		return { type: "crossfade", enabled: action === "xfade-on" };
	if (action === "temp-on" || action === "temp-off")
		return { type: "temporary", enabled: action === "temp-on", pressed };
	return {
		type: simpleActionType(action),
		pressed,
	} as PlaybackAction;
}

function simpleActionType(action: PlaybackControlAction) {
	return (
		(
			{
				"go-minus": "back",
				"fast-forward": "fast_forward",
				"fast-rewind": "fast_rewind",
				"select-contents": "select_contents",
				"select-dereferenced": "select_dereferenced",
				"pause-dynamics": "pause_dynamics",
			} as Record<string, string>
		)[action] ?? action
	);
}

function required<T>(value: T | undefined, label: string): T {
	if (value == null) throw new Error(`${label} is required`);
	return value;
}
