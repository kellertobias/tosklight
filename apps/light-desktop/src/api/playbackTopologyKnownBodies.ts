import type {
	Cue,
	CueList,
	PlaybackDefinition,
} from "./types";

export function sameKnownCueList(actual: CueList, requested: CueList) {
	return stableJson(knownCueList(actual)) === stableJson(knownCueList(requested));
}

export function sameKnownPlayback(
	actual: PlaybackDefinition,
	requested: PlaybackDefinition,
	number: number,
) {
	return stableJson(knownPlayback(actual, number)) ===
		stableJson(knownPlayback(requested, number));
}

function knownCueList(cueList: CueList) {
	return {
		id: cueList.id,
		name: cueList.name,
		priority: cueList.priority,
		mode: cueList.mode,
		looped: cueList.looped,
		intensity_priority_mode: cueList.intensity_priority_mode ?? "htp",
		wrap_mode: cueList.wrap_mode ?? null,
		restart_mode: cueList.restart_mode ?? "first_cue",
		force_cue_timing: cueList.force_cue_timing ?? false,
		disable_cue_timing: cueList.disable_cue_timing ?? false,
		chaser_step_millis: cueList.chaser_step_millis ?? 1_000,
		chaser_xfade_millis: cueList.chaser_xfade_millis ?? 0,
		chaser_xfade_percent: cueList.chaser_xfade_percent ?? null,
		speed_group: cueList.speed_group ?? null,
		speed_multiplier: cueList.speed_multiplier ?? 1,
		cues: cueList.cues.map(knownCue),
	};
}

function knownCue(cue: Cue) {
	return {
		id: cue.id ?? null,
		number: cue.number,
		name: cue.name,
		fade_millis: cue.fade_millis,
		delay_millis: cue.delay_millis,
		trigger: knownTrigger(cue.trigger),
		cue_only: cue.cue_only ?? false,
		changes: cue.changes.map(knownFixtureChange),
		group_changes: (cue.group_changes ?? []).map((change) =>
			knownGroupChange(change),
		),
		phasers: cue.phasers ?? [],
	};
}

function knownTrigger(trigger: Cue["trigger"]) {
	if (trigger.type === "manual") return { type: trigger.type };
	if (trigger.type === "timecode")
		return { type: trigger.type, frame: trigger.frame };
	return { type: trigger.type, delay_millis: trigger.delay_millis };
}

function knownFixtureChange(change: Cue["changes"][number]) {
	return { fixture_id: change.fixture_id, ...knownChangeFields(change) };
}

function knownGroupChange(
	change: NonNullable<Cue["group_changes"]>[number],
) {
	return { group_id: change.group_id, ...knownChangeFields(change) };
}

function knownChangeFields(
	change:
		| Cue["changes"][number]
		| NonNullable<Cue["group_changes"]>[number],
) {
	return {
		attribute: change.attribute,
		value: change.value,
		automatic_restore: change.automatic_restore ?? false,
		fade_millis: change.fade_millis ?? null,
		delay_millis: change.delay_millis ?? null,
	};
}

function knownPlayback(playback: PlaybackDefinition, number: number) {
	const fader =
		playback.target.type === "speed_group" && playback.fader === "speed"
			? "learned_percentage"
			: playback.fader;
	return {
		number,
		name: playback.name,
		target: knownTarget(playback.target),
		buttons: playback.buttons,
		button_count: playback.button_count ?? 3,
		fader,
		has_fader: playback.has_fader ?? true,
		go_activates: playback.go_activates,
		auto_off: playback.auto_off,
		xfade_millis: playback.xfade_millis,
		color: playback.color ?? "#20c997",
		flash_release: playback.flash_release ?? "release_all",
		protect_from_swap: playback.protect_from_swap ?? false,
		presentation_icon: playback.presentation_icon ?? null,
		presentation_image: playback.presentation_image ?? null,
	};
}

function knownTarget(target: PlaybackDefinition["target"]) {
	if (target.type === "cue_list")
		return { type: target.type, cue_list_id: target.cue_list_id };
	if (target.type === "group")
		return { type: target.type, group_id: target.group_id };
	if (target.type === "speed_group")
		return { type: target.type, group: target.group };
	return { type: target.type };
}

function stableJson(value: unknown): string {
	if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
	if (value && typeof value === "object") {
		const record = value as Record<string, unknown>;
		return `{${Object.keys(record)
			.sort()
			.map((key) => `${JSON.stringify(key)}:${stableJson(record[key])}`)
			.join(",")}}`;
	}
	return JSON.stringify(value) ?? "undefined";
}
