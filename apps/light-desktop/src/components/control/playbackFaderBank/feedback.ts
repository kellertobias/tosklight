import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../../api/types";
import type { PlaybackCardSummary } from "@tosklight/ui/playback";
import { formatSpeedGroupBpm } from "../speedGroupFormatting";
import type { PlaybackSnapshotActive } from "./types";

export function emptyConfiguration(
	page: number,
	slot: number,
	buttons: number,
	hasFader: boolean,
	cueListId: string,
): PlaybackDefinition {
	return {
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
				dynamic_restart: "RESTART",
				dynamic_double_speed: "DOUBLE SPEED",
				dynamic_half_speed: "HALF SPEED",
				dynamic_learn_speed: "LEARN SPEED",
				none: "DISABLED",
			} as Partial<Record<typeof action, string>>
		)[action] ?? action.toUpperCase()
	);
}

export function isHeldAction(
	action: PlaybackDefinition["buttons"][number],
): action is "flash" | "swap" {
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
	projection: PlaybackRuntimeProjection | undefined,
	hardware: boolean,
) {
	if (!playback) return 0;
	if (playback.target.type === "group")
		return projection?.target === "group" ? projection.master * 100 : 0;
	if (playback.target.type === "speed_group") {
		if (projection?.target !== "speed_group") return 0;
		const runtime = projection.runtime;
		const bpm = runtime.effective_bpm;
		return playback.fader === "direct_bpm"
			? bpm / 3
			: playback.fader === "centered_relative"
				? centeredRelativePosition(runtime.speed_master_scale)
				: runtime.speed_master_scale * 100;
	}
	if (playback.target.type === "dynamic")
		return projection?.target === "dynamic" && projection.runtime
			? projection.runtime.fader_value * 100
			: 0;
	if (playback.target.type === "programmer_fade")
		return projection?.target === "programmer_fade"
			? projection.millis / 200
			: 0;
	if (playback.target.type === "cue_fade")
		return projection?.target === "cue_fade" ? projection.millis / 600 : 0;
	if (playback.target.type === "grand_master")
		return projection?.target === "grand_master"
			? projection.runtime.level * 100
			: 0;
	if (playback.fader === "x_fade")
		return (active?.manual_xfade_position ?? 0) * 100;
	if (playback.fader === "temp") return (active?.temporary_master ?? 0) * 100;
	// Software touch controls set the shared master absolutely. The physical input position
	// remains independent and may be zero while an assigned/virtual owner runs at full master.
	// Only the hardware representation should show that physical position and pickup feedback.
	return (
		(hardware
			? (active?.fader_position ?? active?.master ?? 0)
			: (active?.master ?? 0)) * 100
	);
}

export function playbackFaderLabel(playback: PlaybackDefinition | null) {
	if (!playback) return "Empty";
	if (playback.target.type === "group") return "Group master";
	if (playback.target.type === "speed_group")
		return `Speed Group ${playback.target.group}`;
	if (playback.target.type === "dynamic")
		return `Dynamic ${playback.target.assignment.last_known_pool_number} · ${playback.target.assignment.fader_mode.replaceAll("_", " + ")}`;
	if (playback.target.type === "programmer_fade") return "Programmer Fade";
	if (playback.target.type === "cue_fade") return "Cue Fade";
	if (playback.target.type === "grand_master") return "Grand Master";
	return playback.fader === "x_fade"
		? "X-fade"
		: playback.fader === "temp"
			? "Temp"
			: "Master";
}

/** The fader shows its configured control position, not a runtime diagnostic paragraph. */
export function playbackFaderReadout(
	playback: PlaybackDefinition | null,
	active: PlaybackSnapshotActive | undefined,
	value: number,
	projection?: PlaybackRuntimeProjection,
) {
	return playback?.target.type === "dynamic" && projection?.target === "dynamic" && projection.runtime !== undefined
		? `${Math.round(value)}%`
		: playbackFaderDisplay(playback, active, value, projection);
}

export function dynamicPlaybackSummary(
	projection: Extract<PlaybackRuntimeProjection, { target: "dynamic" }>,
): PlaybackCardSummary {
	const runtime = projection.runtime;
	if (!runtime) return { label: runtime === null ? "OFF" : "Unavailable" };
	const warnings: string[] = [];
	if (runtime.missing_target_count) warnings.push(`${runtime.missing_target_count} missing`);
	if (runtime.unpatched_target_count) warnings.push(`${runtime.unpatched_target_count} unpatched`);
	for (const warning of runtime.warning?.split(" · ") ?? []) {
		if (warning.startsWith("Aliasing:")) warnings.push("Aliasing");
		else if (/^\d+ of \d+ target\/lane addresses run;/.test(warning))
			warnings.push(`${runtime.supported_address_count}/${runtime.target_count * runtime.lane_count} addr`);
		else if (warning === "Dynamic Playback controller disappeared after an accepted start")
			warnings.push("Controller lost");
		else if (warning) warnings.push(warning);
	}
	return {
		label: runtime.state.toUpperCase(),
		detail: `${runtime.compatible_target_count}/${runtime.target_count} targets`,
		warning: warnings.length ? warnings.join(" · ") : undefined,
	};
}

export function playbackFaderModeFeedback(
	playback: PlaybackDefinition | null,
	active: PlaybackSnapshotActive | undefined,
) {
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
	projection?: PlaybackRuntimeProjection,
) {
	if (!playback) return "Empty";
	const percent = Math.round(value);
	if (playback.target.type === "speed_group") {
		if (projection?.target !== "speed_group") return "Unavailable";
		const runtime = projection.runtime;
		return `${formatSpeedGroupBpm(runtime.effective_bpm)} BPM · ${runtime.paused ? "PAUSED" : runtime.source.replaceAll("_", " ").toUpperCase()}`;
	}
	if (playback.target.type === "dynamic") {
		if (projection?.target !== "dynamic") return "Unavailable";
		// A valid assignment has no runtime until its first activation.
		if (projection.runtime === null) return "OFF · 0%";
		if (!projection.runtime) return "Unavailable";
		const runtime = projection.runtime;
		const learned = runtime.learned_duration_millis
			? ` · Learned ${(runtime.learned_duration_millis / 1_000).toFixed(2)}s`
			: "";
		const duration = runtime.effective_duration_millis
			? ` · ${(runtime.effective_duration_millis / 1_000).toFixed(2)}s`
			: "";
		const warning = runtime.warning ? ` · ⚠ ${runtime.warning}` : "";
		const identity = runtime.instance_id
			? ` · I ${runtime.instance_id.slice(0, 8)} · C ${runtime.controller_id.slice(0, 8)}`
			: ` · C ${runtime.controller_id.slice(0, 8)}`;
		const controllerStatus =
			(runtime.state === "off" || runtime.state === "pending") &&
			runtime.controller_status === "missing"
				? ""
				: ` · ${runtime.controller_status.toUpperCase()}`;
		return `${runtime.state.toUpperCase()}${controllerStatus} · Size ${Math.round(runtime.size * 100)}% · Master ${Math.round(runtime.master * 100)}% · ${runtime.effective_speed_multiplier.toFixed(2)}× ${runtime.speed_source.replaceAll("_", " ")}${duration}${learned} · ${runtime.compatible_target_count}/${runtime.target_count} compatible targets · ${runtime.supported_address_count}/${runtime.target_count * runtime.lane_count} target/lane addresses · ${runtime.missing_target_count} missing · ${runtime.unpatched_target_count} unpatched${identity}${warning}`;
	}
	if (playback.target.type === "programmer_fade")
		return projection?.target === "programmer_fade"
			? `${(projection.millis / 1_000).toFixed(1)} s`
			: "Unavailable";
	if (playback.target.type === "cue_fade")
		return projection?.target === "cue_fade"
			? `${(projection.millis / 1_000).toFixed(1)} s`
			: "Unavailable";
	if (playback.target.type === "grand_master") {
		if (projection?.target !== "grand_master") return "Unavailable";
		const master = projection.runtime;
		return `${percent}%${master.blackout ? " · BLACKOUT" : ""}${master.dynamics_paused ? " · DYNAMICS PAUSED" : ""}`;
	}
	if (playback.target.type === "group")
		return value === 0 ? "0% master · ▲" : `${percent}% master`;
	if (playback.fader === "x_fade") {
		const current =
			active?.current_cue_number ??
			(active?.cue_index == null ? "—" : active.cue_index + 1);
		return `Cue ${current} → ${active?.effective_next_cue_number ?? "—"} · ${Math.round((active?.manual_xfade_progress ?? 0) * 100)}%`;
	}
	if (playback.fader === "temp")
		return `${active?.temporary_active ? "TEMP" : "Temp"} · ${percent}%`;
	if (active?.loaded_cue_number != null)
		return `Load ${active.loaded_cue_number} · ${percent}%`;
	if (active?.enabled !== false && active)
		return `Cue ${active.current_cue_number ?? active.cue_index + 1} · ${percent}%`;
	return `${percent}%`;
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
