import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { playbackAt } from "./api";
import type {
	PlaybackCheckpoint,
	PlaybackConfigurationObservation,
} from "./models";

export async function playbackConfigurationObservation(
	api: ApiDriver,
	page: number,
	slot: number,
	expectedCueListId: string,
): Promise<PlaybackConfigurationObservation> {
	const playback = await playbackAt(api, page, slot);
	return {
		page,
		slot,
		number: playback.body.number,
		targetType: playback.body.target.type,
		targetMatchesExpected:
			playback.body.target.type === "cue_list" &&
			playback.body.target.cue_list_id === expectedCueListId,
		buttons: [...playback.body.buttons],
		buttonCount: playback.body.button_count,
		fader: playback.body.fader,
		hasFader: playback.body.has_fader,
		color: playback.body.color,
	};
}

export function serializedCueTimings(cueList: {
	body: { cues: Array<{ fade_millis: number; delay_millis: number }> };
}): string {
	return JSON.stringify(
		cueList.body.cues.map((cue) => [cue.fade_millis, cue.delay_millis]),
	);
}

export function xfadeObservation(
	runtime: any,
	intensity: number,
): PlaybackCheckpoint {
	return {
		cue: runtime.current_cue_number,
		position: runtime.manual_xfade_position,
		progress: runtime.manual_xfade_progress,
		direction: runtime.manual_xfade_direction,
		intensity,
	};
}

export async function visualizationLevel(
	api: ApiDriver,
	fixtureId: string,
): Promise<number> {
	const snapshot = await api.request<any>("GET", "/api/v1/visualization");
	const value = snapshot.values.find(
		(entry: any) =>
			entry.fixture_id === fixtureId && entry.attribute === "intensity",
	)?.value;
	return typeof value === "number" ? value : (value?.value ?? 0);
}

export async function intensityLevels(
	api: ApiDriver,
	fixtures: Record<number, string>,
	numbers: number[],
): Promise<Record<number, number>> {
	return Object.fromEntries(
		await Promise.all(
			numbers.map(
				async (number) =>
					[number, await visualizationLevel(api, fixtures[number])] as const,
			),
		),
	);
}

export function hasTemporaryRuntime(snapshot: any, number: number): boolean {
	return snapshot.active.some(
		(item: any) => item.playback_number === number && item.temporary_active,
	);
}

export function hasSwapRuntime(snapshot: any, number: number): boolean {
	return snapshot.active.some(
		(item: any) => item.playback_number === number && item.swap_active,
	);
}

export function authoritativeMasterObservation(state: any) {
	const group = state.groups.find((candidate: any) => candidate.id === "1");
	const speed = state.speed_groups[0];
	const rounded = (value: number) => Math.round(value * 1_000) / 1_000;
	return {
		speed: {
			manualBpm: rounded(speed.manual_bpm),
			effectiveBpm: rounded(speed.effective_bpm),
			paused: speed.paused,
		},
		neighborBpms: state.speed_groups
			.slice(1)
			.map((candidate: any) => rounded(candidate.manual_bpm)),
		group: {
			master: rounded(group.master),
			flashLevel: rounded(group.flash_level),
		},
		grand: {
			level: rounded(state.grand_master.level),
			effectiveLevel: rounded(state.grand_master.effective_level),
			blackout: state.grand_master.blackout,
			dynamicsPaused: state.grand_master.dynamics_paused,
		},
		programmerFadeMillis: state.programmer_fade_millis,
		cueFadeMillis: state.cue_fade_millis,
	};
}
