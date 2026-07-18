import type { PlaybackDefinition } from "../../../api/types";
import type {
	AuthoritativeControls,
	PlaybackServer,
	PlaybackSnapshotActive,
} from "./types";

export function emptyConfiguration(
	page: number,
	slot: number,
	buttons: number,
	hasFader: boolean,
	cueListId: string,
): PlaybackDefinition {
	return {
		number: 0,
		name: `Playback ${page}.${slot}`,
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go_minus", "go", "flash"],
		button_count: Math.max(0, Math.min(3, buttons)) as 0 | 1 | 2 | 3,
		fader: "master",
		has_fader: hasFader,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

export function playbackButtonLabel(
	action: PlaybackDefinition["buttons"][number],
) {
	return (
		(
			{
				go: "GO +",
				go_minus: "GO −",
				fast_forward: "FAST +",
				fast_rewind: "FAST −",
				select_contents: "SELECT CONTENTS",
				select_dereferenced: "SELECT FIXTURES",
				pause_dynamics: "PAUSE DYNAMICS",
				none: "DISABLED",
			} as Partial<Record<typeof action, string>>
		)[action] ?? action.toUpperCase()
	);
}

export function isHeldAction(action: PlaybackDefinition["buttons"][number]) {
	return action === "flash" || action === "swap";
}

export function buttonFeedbackClass(
	action: PlaybackDefinition["buttons"][number],
	active: PlaybackSnapshotActive | undefined,
	selected: boolean,
	blackout: boolean,
) {
	const on =
		action === "select"
			? selected
			: action === "flash"
				? Boolean(active?.flash)
				: action === "temp"
					? Boolean(active?.temporary_active)
					: action === "swap"
						? Boolean(active?.swap_active)
						: action === "pause"
							? Boolean(active?.paused)
							: action === "blackout"
								? blackout
								: action === "on" || action === "toggle"
									? Boolean(active?.enabled)
									: false;
	return on ? "playback-button-active" : "";
}

export function playbackFaderValue(
	playback: PlaybackDefinition | null,
	active: PlaybackSnapshotActive | undefined,
	groupMaster: number | undefined,
	configuration: PlaybackServer["configuration"],
	controls: AuthoritativeControls | undefined,
	grandMaster: number,
) {
	if (!playback) return 0;
	if (playback.target.type === "group") {
		const groupId = playback.target.group_id;
		return Math.round(
			(controls?.groups.find((item) => item.id === groupId)?.master ??
				groupMaster ??
				1) * 100,
		);
	}
	if (playback.target.type === "speed_group") {
		const speed =
			controls?.speed_groups[playback.target.group.charCodeAt(0) - 65];
		const bpm =
			speed?.effective_bpm ??
			configuration?.speed_groups_bpm[
				playback.target.group.charCodeAt(0) - 65
			] ??
			120;
		return playback.fader === "direct_bpm"
			? bpm / 3
			: playback.fader === "centered_relative"
				? speed
					? centeredRelativePosition(speed.speed_master_scale)
					: 50
				: Math.round(
						(speed?.speed_master_scale ??
							Math.min(1, bpm / Math.max(1, speed?.manual_bpm ?? 120))) * 100,
					);
	}
	if (playback.target.type === "programmer_fade")
		return (
			(controls?.programmer_fade_millis ??
				configuration?.programmer_fade_millis ??
				3_000) / 200
		);
	if (playback.target.type === "cue_fade")
		return (
			(controls?.cue_fade_millis ??
				configuration?.sequence_master_fade_millis ??
				3_000) / 600
		);
	if (playback.target.type === "grand_master")
		return Math.round((controls?.grand_master.level ?? grandMaster) * 100);
	if (playback.fader === "x_fade")
		return Math.round((active?.manual_xfade_position ?? 0) * 100);
	if (playback.fader === "temp")
		return Math.round((active?.temporary_master ?? 0) * 100);
	return Math.round((active?.fader_position ?? active?.master ?? 0) * 100);
}

export function playbackFaderLabel(playback: PlaybackDefinition | null) {
	if (!playback) return "Empty";
	if (playback.target.type === "group") return "Group master";
	if (playback.target.type === "speed_group")
		return `Speed Group ${playback.target.group}`;
	if (playback.target.type === "programmer_fade") return "Programmer Fade";
	if (playback.target.type === "cue_fade") return "Cue Fade";
	if (playback.target.type === "grand_master") return "Grand Master";
	return playback.fader === "x_fade"
		? "X-fade"
		: playback.fader === "temp"
			? "Temp"
			: "Master";
}

export function playbackFaderModeFeedback(
	playback: PlaybackDefinition | null,
	active: PlaybackSnapshotActive | undefined,
) {
	if (active?.fader_pickup_required) return "Pickup: lower to zero";
	if (playback?.fader === "x_fade")
		return active?.manual_xfade_direction === "towards_low"
			? "Travel towards low"
			: "Travel towards high";
	if (playback?.fader === "temp" && active?.temporary_active)
		return "Temporary active";
	return undefined;
}

export function playbackFaderDisplay(
	playback: PlaybackDefinition | null,
	active: PlaybackSnapshotActive | undefined,
	value: number,
	configuration: PlaybackServer["configuration"],
	controls: AuthoritativeControls | undefined,
	blackout: boolean,
) {
	if (!playback) return "Empty";
	if (playback.target.type === "speed_group") {
		const speed =
			controls?.speed_groups[playback.target.group.charCodeAt(0) - 65];
		const bpm =
			speed?.effective_bpm ??
			configuration?.speed_groups_bpm[
				playback.target.group.charCodeAt(0) - 65
			] ??
			120;
		return `${Math.round(bpm)} BPM · ${speed?.paused ? "PAUSED" : (speed?.source?.replaceAll("_", " ").toUpperCase() ?? `${value.toFixed(0)}%`)}`;
	}
	if (playback.target.type === "programmer_fade")
		return `${((controls?.programmer_fade_millis ?? configuration?.programmer_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`;
	if (playback.target.type === "cue_fade")
		return `${((controls?.cue_fade_millis ?? configuration?.sequence_master_fade_millis ?? 3_000) / 1_000).toFixed(1)} s`;
	if (playback.target.type === "grand_master") {
		const master = controls?.grand_master;
		return `${value}%${(master?.blackout ?? blackout) ? " · BLACKOUT" : ""}${master?.dynamics_paused ? " · DYNAMICS PAUSED" : ""}`;
	}
	if (playback.target.type === "group") return `${value}% master`;
	if (playback.fader === "x_fade") {
		const current =
			active?.current_cue_number ??
			(active?.cue_index == null ? "—" : active.cue_index + 1);
		return `Cue ${current} → ${active?.effective_next_cue_number ?? "—"} · ${Math.round((active?.manual_xfade_progress ?? 0) * 100)}%`;
	}
	if (playback.fader === "temp")
		return `${active?.temporary_active ? "TEMP" : "Temp"} · ${value}%`;
	if (active?.loaded_cue_number != null)
		return `Load ${active.loaded_cue_number} · ${value}%`;
	if (active?.enabled !== false && active)
		return `Cue ${active.current_cue_number ?? active.cue_index + 1} · ${value}%`;
	return `${value}%`;
}

function centeredRelativePosition(scale: number) {
	return Math.max(
		0,
		Math.min(
			100,
			(0.5 + Math.log(Math.max(0.25, Math.min(4, scale))) / Math.log(4) / 2) *
				100,
		),
	);
}
