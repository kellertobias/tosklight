import type {
	PlaybackAction,
	PlaybackActionRequest,
	PlaybackAddress,
	PlaybackSurface,
} from "../../api/types";

export type PoolPlaybackAction =
	| "button"
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

export interface PoolPlaybackInput {
	value?: number;
	pressed?: boolean;
	button?: number;
	cue_number?: number;
	surface?: PlaybackSurface;
}

export function poolPlaybackRequest(
	playbackNumber: number,
	action: PoolPlaybackAction,
	input: PoolPlaybackInput,
): PlaybackActionRequest {
	return actionRequest(
		{ kind: "playback", playback_number: playbackNumber },
		structuredAction(action, input),
		input.surface ?? "physical",
	);
}

export function cueListPlaybackRequest(
	cueListId: string,
	action: "go" | "back" | "pause" | "release",
): PlaybackActionRequest {
	return actionRequest(
		{ kind: "cue_list", cue_list_id: cueListId },
		action === "release"
			? { type: "release" }
			: { type: action, pressed: true },
		"virtual",
	);
}

function actionRequest(
	address: PlaybackAddress,
	action: PlaybackAction,
	surface: PlaybackSurface,
): PlaybackActionRequest {
	return { request_id: crypto.randomUUID(), address, action, surface };
}

function structuredAction(
	action: PoolPlaybackAction,
	input: PoolPlaybackInput,
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

function simpleActionType(action: PoolPlaybackAction) {
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

function required(value: number | undefined, label: string) {
	if (value == null) throw new Error(`${label} is required`);
	return value;
}
