import type {
	CueListRuntimeProjection,
	PlaybackDeskProjection,
	PlaybackRuntimeIdentity,
	PlaybackRuntimeProjection,
	PlaybackTargetProjection,
	SoundLossReason,
	SoundStatus,
	SpeedGroupRuntimeProjection,
} from "./generated/light-wire";
import {
	booleanAt,
	enumAt,
	integerAt,
	nullable,
	numberAt,
	positiveIntegerAt,
	recordAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

export function decodePlaybackIdentity(
	value: unknown,
	path: string,
): PlaybackRuntimeIdentity {
	const identity = recordAt(value, path);
	const kind = enumAt(identity.kind, `${path}.kind`, ["playback", "cue_list"]);
	return kind === "playback"
		? {
				kind,
				playback_number: positiveIntegerAt(
					identity.playback_number,
					`${path}.playback_number`,
				),
			}
		: {
				kind,
				cue_list_id: stringAt(identity.cue_list_id, `${path}.cue_list_id`),
			};
}

function decodeScope(value: unknown, path: string) {
	const scope = recordAt(value, path);
	return {
		show_id: stringAt(scope.show_id, `${path}.show_id`),
		show_revision: integerAt(scope.show_revision, `${path}.show_revision`),
	};
}

function decodeCueReference(value: unknown, path: string) {
	const cue = recordAt(value, path);
	return {
		id: stringAt(cue.id, `${path}.id`),
		number: numberAt(cue.number, `${path}.number`),
	};
}

function decodeCueRuntime(
	value: unknown,
	path: string,
): CueListRuntimeProjection {
	const runtime = recordAt(value, path);
	return {
		cue_index: integerAt(runtime.cue_index, `${path}.cue_index`),
		previous_index: nullable(
			runtime.previous_index,
			`${path}.previous_index`,
			integerAt,
		),
		current: nullable(runtime.current, `${path}.current`, decodeCueReference),
		loaded: nullable(runtime.loaded, `${path}.loaded`, decodeCueReference),
		normal_next: nullable(
			runtime.normal_next,
			`${path}.normal_next`,
			decodeCueReference,
		),
		effective_next: nullable(
			runtime.effective_next,
			`${path}.effective_next`,
			decodeCueReference,
		),
		effective_next_is_loaded: booleanAt(
			runtime.effective_next_is_loaded,
			`${path}.effective_next_is_loaded`,
		),
		paused: booleanAt(runtime.paused, `${path}.paused`),
		activated_at: stringAt(runtime.activated_at, `${path}.activated_at`),
		master: numberAt(runtime.master, `${path}.master`),
		fader_position: numberAt(runtime.fader_position, `${path}.fader_position`),
		fader_pickup_required: booleanAt(
			runtime.fader_pickup_required,
			`${path}.fader_pickup_required`,
		),
		flash: booleanAt(runtime.flash, `${path}.flash`),
		temporary: booleanAt(runtime.temporary, `${path}.temporary`),
		temporary_active: booleanAt(
			runtime.temporary_active,
			`${path}.temporary_active`,
		),
		temporary_master: numberAt(
			runtime.temporary_master,
			`${path}.temporary_master`,
		),
		swap_active: booleanAt(runtime.swap_active, `${path}.swap_active`),
		enabled: booleanAt(runtime.enabled, `${path}.enabled`),
		transition_timing_bypassed: booleanAt(
			runtime.transition_timing_bypassed,
			`${path}.transition_timing_bypassed`,
		),
		manual_xfade_position: numberAt(
			runtime.manual_xfade_position,
			`${path}.manual_xfade_position`,
		),
		manual_xfade_direction: enumAt(
			runtime.manual_xfade_direction,
			`${path}.manual_xfade_direction`,
			["towards_high", "towards_low"],
		),
		manual_xfade_progress: numberAt(
			runtime.manual_xfade_progress,
			`${path}.manual_xfade_progress`,
		),
	};
}

const SOUND_LOSS_REASONS = [
	"source_unavailable",
	"no_usable_signal",
	"low_confidence",
	"tempo_outside_range",
	"waiting_for_analysis",
] as const satisfies readonly SoundLossReason[];

function decodeSoundStatus(value: unknown, path: string): SoundStatus {
	const status = recordAt(value, path);
	const kind = enumAt(status.status, `${path}.status`, [
		"disabled",
		"active",
		"holding",
		"manual_fallback",
	]);
	if (kind === "disabled") return { status: kind };
	if (kind === "active")
		return {
			status: kind,
			detected_bpm: numberAt(status.detected_bpm, `${path}.detected_bpm`),
			confidence: numberAt(status.confidence, `${path}.confidence`),
		};
	const reason = enumAt(status.reason, `${path}.reason`, SOUND_LOSS_REASONS);
	return kind === "holding"
		? {
				status: kind,
				reason,
				remaining_millis: integerAt(
					status.remaining_millis,
					`${path}.remaining_millis`,
				),
			}
		: { status: kind, reason };
}

function decodeSpeedRuntime(
	value: unknown,
	path: string,
): SpeedGroupRuntimeProjection {
	const runtime = recordAt(value, path);
	return {
		manual_bpm: numberAt(runtime.manual_bpm, `${path}.manual_bpm`),
		sound_bpm: nullable(runtime.sound_bpm, `${path}.sound_bpm`, numberAt),
		effective_bpm: numberAt(runtime.effective_bpm, `${path}.effective_bpm`),
		source: enumAt(runtime.source, `${path}.source`, [
			"manual",
			"sound",
			"held_sound",
			"manual_fallback",
		]),
		sound_status: decodeSoundStatus(
			runtime.sound_status,
			`${path}.sound_status`,
		),
		paused: booleanAt(runtime.paused, `${path}.paused`),
		phase_advancing: booleanAt(
			runtime.phase_advancing,
			`${path}.phase_advancing`,
		),
		speed_master_scale: numberAt(
			runtime.speed_master_scale,
			`${path}.speed_master_scale`,
		),
		sound_multiplier: numberAt(
			runtime.sound_multiplier,
			`${path}.sound_multiplier`,
		),
		source_available: booleanAt(
			runtime.source_available,
			`${path}.source_available`,
		),
		usable_signal: booleanAt(runtime.usable_signal, `${path}.usable_signal`),
		input_level: numberAt(runtime.input_level, `${path}.input_level`),
		selected_band_level: numberAt(
			runtime.selected_band_level,
			`${path}.selected_band_level`,
		),
		synchronized_with: nullable(
			runtime.synchronized_with,
			`${path}.synchronized_with`,
			integerAt,
		),
		phase_origin_millis: integerAt(
			runtime.phase_origin_millis,
			`${path}.phase_origin_millis`,
		),
		beat_phase: numberAt(runtime.beat_phase, `${path}.beat_phase`),
	};
}

function decodeTarget(
	projection: Record<string, unknown>,
	path: string,
): PlaybackTargetProjection {
	const target = enumAt(projection.target, `${path}.target`, [
		"missing",
		"cue_list",
		"group",
		"speed_group",
		"grand_master",
		"programmer_fade",
		"cue_fade",
	]);
	if (target === "missing") return { target };
	if (target === "cue_list") {
		return {
			target,
			cue_list_id: stringAt(projection.cue_list_id, `${path}.cue_list_id`),
			runtime: nullable(
				projection.runtime,
				`${path}.runtime`,
				decodeCueRuntime,
			),
		};
	}
	if (target === "group") {
		return {
			target,
			group_id: stringAt(projection.group_id, `${path}.group_id`),
			master: numberAt(projection.master, `${path}.master`),
			flash_level: numberAt(projection.flash_level, `${path}.flash_level`),
		};
	}
	if (target === "speed_group") {
		return {
			target,
			group: stringAt(projection.group, `${path}.group`),
			runtime: decodeSpeedRuntime(projection.runtime, `${path}.runtime`),
		};
	}
	if (target === "grand_master") {
		const runtime = recordAt(projection.runtime, `${path}.runtime`);
		return {
			target,
			runtime: {
				level: numberAt(runtime.level, `${path}.runtime.level`),
				effective_level: numberAt(
					runtime.effective_level,
					`${path}.runtime.effective_level`,
				),
				blackout: booleanAt(runtime.blackout, `${path}.runtime.blackout`),
				flash_active: booleanAt(
					runtime.flash_active,
					`${path}.runtime.flash_active`,
				),
				dynamics_paused: booleanAt(
					runtime.dynamics_paused,
					`${path}.runtime.dynamics_paused`,
				),
			},
		};
	}
	return { target, millis: numberAt(projection.millis, `${path}.millis`) };
}

export function decodePlaybackProjection(
	value: unknown,
	path: string,
): PlaybackRuntimeProjection {
	const projection = recordAt(value, path);
	return {
		scope: decodeScope(projection.scope, `${path}.scope`),
		requested: decodePlaybackIdentity(
			projection.requested,
			`${path}.requested`,
		),
		playback_number: nullable(
			projection.playback_number,
			`${path}.playback_number`,
			positiveIntegerAt,
		),
		...decodeTarget(projection, path),
	};
}

export function decodePlaybackDesk(
	value: unknown,
	path: string,
): PlaybackDeskProjection {
	const projection = recordAt(value, path);
	return {
		scope: decodeScope(projection.scope, `${path}.scope`),
		desk_id: stringAt(projection.desk_id, `${path}.desk_id`),
		active_page: positiveIntegerAt(
			projection.active_page,
			`${path}.active_page`,
		),
		selected_playback: nullable(
			projection.selected_playback,
			`${path}.selected_playback`,
			positiveIntegerAt,
		),
	};
}

export function assertSameShow(
	projection: PlaybackRuntimeProjection | PlaybackDeskProjection,
	showId: string,
) {
	if (projection.scope.show_id !== showId)
		throw new WireValidationError(
			"$.scope.show_id",
			`active show ${showId}`,
			projection.scope.show_id,
		);
}
