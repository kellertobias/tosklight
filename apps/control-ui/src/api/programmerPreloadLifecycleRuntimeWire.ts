import type {
	ProgrammerPreloadPlaybackAction,
	ProgrammerPreloadPlaybackQueueEntry,
	ProgrammerPreloadPlaybackSurface,
} from "../features/programmerPreloadPlaybackQueue/contracts";
import type { PlaybackProjection } from "../features/playbackRuntime/contracts";
import { decodePlaybackProjection } from "./playbackWireProjection";
import {
	enumAt,
	exactRecordAt,
	integerAt,
	recordAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

const ACTIONS = [
	"toggle",
	"go",
	"back",
	"off",
	"on",
	"temporary_on",
	"temporary_off",
] as const satisfies readonly ProgrammerPreloadPlaybackAction[];
const SURFACES = [
	"physical",
	"virtual",
	"osc",
	"matter",
] as const satisfies readonly ProgrammerPreloadPlaybackSurface[];

export function decodePreloadExecutedAction(
	value: unknown,
	path: string,
): ProgrammerPreloadPlaybackQueueEntry {
	const item = exactRecordAt(value, path, [
		"playback_number",
		"page",
		"action",
		"surface",
	]);
	const playbackNumber = integerAt(
		item.playback_number,
		`${path}.playback_number`,
	);
	const page = item.page == null ? null : integerAt(item.page, `${path}.page`);
	if (playbackNumber > 65_535)
		throw new WireValidationError(
			`${path}.playback_number`,
			"16-bit playback number",
			playbackNumber,
		);
	if (page !== null && page > 255)
		throw new WireValidationError(`${path}.page`, "8-bit page", page);
	return {
		playbackNumber,
		page,
		action: enumAt(item.action, `${path}.action`, ACTIONS),
		surface: enumAt(item.surface, `${path}.surface`, SURFACES),
	};
}

export function decodeStrictPlaybackProjection(
	value: unknown,
	path: string,
): PlaybackProjection {
	assertProjectionShape(value, path);
	return decodePlaybackProjection(value, path);
}

function assertProjectionShape(value: unknown, path: string) {
	const projection = recordAt(value, path);
	const target = enumAt(projection.target, `${path}.target`, [
		"missing",
		"cue_list",
		"group",
		"speed_group",
		"grand_master",
		"programmer_fade",
		"cue_fade",
	]);
	const variant =
		target === "cue_list"
			? ["cue_list_id", "runtime"]
			: target === "group"
				? ["group_id", "master", "flash_level"]
				: target === "speed_group"
					? ["group", "runtime"]
					: target === "grand_master"
						? ["runtime"]
						: target === "programmer_fade" || target === "cue_fade"
							? ["millis"]
							: [];
	exactRecordAt(value, path, [
		"scope",
		"requested",
		"playback_number",
		"target",
		...variant,
	]);
	exactRecordAt(projection.scope, `${path}.scope`, [
		"show_id",
		"show_revision",
	]);
	assertIdentity(projection.requested, `${path}.requested`);
	if (target === "cue_list" && projection.runtime != null)
		assertCueRuntime(projection.runtime, `${path}.runtime`);
	if (target === "speed_group")
		assertSpeedRuntime(projection.runtime, `${path}.runtime`);
	if (target === "grand_master")
		exactRecordAt(projection.runtime, `${path}.runtime`, [
			"level",
			"effective_level",
			"blackout",
			"flash_active",
			"dynamics_paused",
		]);
}

function assertIdentity(value: unknown, path: string) {
	const identity = recordAt(value, path);
	const kind = enumAt(identity.kind, `${path}.kind`, [
		"playback",
		"cue_list",
		"group",
	]);
	exactRecordAt(value, path, [
		"kind",
		kind === "playback"
			? "playback_number"
			: kind === "cue_list"
				? "cue_list_id"
				: "group_id",
	]);
}

function assertCueRuntime(value: unknown, path: string) {
	const runtime = exactRecordAt(value, path, [
		"cue_index",
		"previous_index",
		"current",
		"loaded",
		"normal_next",
		"effective_next",
		"effective_next_is_loaded",
		"paused",
		"activated_at",
		"master",
		"fader_position",
		"fader_pickup_required",
		"flash",
		"temporary",
		"temporary_active",
		"temporary_master",
		"swap_active",
		"enabled",
		"transition_timing_bypassed",
		"manual_xfade_position",
		"manual_xfade_direction",
		"manual_xfade_progress",
	]);
	for (const key of ["current", "loaded", "normal_next", "effective_next"])
		if (runtime[key] != null)
			exactRecordAt(runtime[key], `${path}.${key}`, ["id", "number"]);
}

function assertSpeedRuntime(value: unknown, path: string) {
	const runtime = exactRecordAt(value, path, [
		"manual_bpm",
		"sound_bpm",
		"effective_bpm",
		"source",
		"sound_status",
		"paused",
		"phase_advancing",
		"speed_master_scale",
		"sound_multiplier",
		"source_available",
		"usable_signal",
		"input_level",
		"selected_band_level",
		"synchronized_with",
		"phase_origin_millis",
		"beat_phase",
	]);
	const sound = recordAt(runtime.sound_status, `${path}.sound_status`);
	const status = enumAt(sound.status, `${path}.sound_status.status`, [
		"disabled",
		"active",
		"holding",
		"manual_fallback",
	]);
	exactRecordAt(runtime.sound_status, `${path}.sound_status`, [
		"status",
		...(status === "active"
			? ["detected_bpm", "confidence"]
			: status === "holding"
				? ["reason", "remaining_millis"]
				: status === "manual_fallback"
					? ["reason"]
					: []),
	]);
}
