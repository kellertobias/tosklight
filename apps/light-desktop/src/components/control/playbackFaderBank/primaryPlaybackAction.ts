import type { PlaybackDefinition } from "../../../api/types";

/** Compact tiles follow their target until their button binding is explicitly edited. */
export function primaryPlaybackAction(playback: PlaybackDefinition | null): {
	action: PlaybackDefinition["buttons"][number];
	button: number;
	direct?: "go" | "toggle";
} {
	const configured = playback?.buttons.slice(0, playback.button_count ?? 3) ?? [];
	if (
		playback?.footprint?.type === "single_button" &&
		playback.footprint.button != null
	) {
		const button = playback.footprint.button - 1;
		return { action: configured[button] ?? "none", button };
	}
	if (playback?.button_count === 1) return { action: configured[0], button: 0 };
	if (!configured.some((action) => action !== "none"))
		return { action: "none", button: 0 };
	const target = playback?.target.type;
	const preferred =
		target === "dynamic"
			? "toggle"
			: target === "cue_list" || target === "macro" || target === "timecode"
				? "go"
				: undefined;
	const matching = preferred
		? configured.indexOf(preferred)
		: configured.findIndex((action) => action !== "none");
	if (matching >= 0) return { action: configured[matching], button: matching };
	return { action: preferred ?? "none", button: 0, direct: preferred };
}
