import { describe, expect, it, vi } from "vitest";
import type { MacroExecutionSnapshot } from "../../api/generated/light-wire";
import type { RunningDynamicController } from "../../components/modals/systemControls/runningDynamicsAuthority";
import type { RunningCueListSource } from "../../components/modals/systemControls/runningPlaybackAuthority";
import { buildRunningRows, filterRunningRows } from "./model";

function playback(
	key: string,
	cueListId = "cue-list-a",
	playbackNumber: number | null = 4,
): RunningCueListSource {
	return {
		key,
		identity:
			playbackNumber == null
				? { kind: "cue_list", cue_list_id: cueListId }
				: { kind: "playback", playback_number: playbackNumber },
		cueListId,
		playbackNumber,
		label: "Wrong assignment label",
		cueList: {
			id: cueListId,
			name: "Act One",
			cues: [],
			mode: "sequence",
			priority: 0,
			looped: false,
		},
		cue: undefined,
		runtime: {
			cue_index: 4,
			current: { id: "cue-5", number: 5 },
			paused: false,
			master: 1,
		} as RunningCueListSource["runtime"],
	};
}

function dynamic(
	controllerId: string,
	source: string,
): RunningDynamicController {
	return {
		key: `instance:${controllerId}`,
		instanceId: "instance",
		dynamicId: "dynamic-a",
		poolNumber: 7,
		name: "Circle",
		targets: [],
		pending: false,
		instancePaused: false,
		speedSource: "Own",
		controllerId,
		source,
		priority: 0,
		size: 1,
		speedMultiplier: 1,
		phaseOffsetDegrees: 0,
		paused: false,
		winning: true,
		releasing: false,
		activationMix: 1,
	};
}

function macro(executionId: string): MacroExecutionSnapshot {
	return {
		execution_id: executionId,
		macro_id: "macro-a",
		macro_number: 9,
		macro_name: "Reset",
		source_revision: 2,
		desk_id: "desk",
		user_id: "user",
		session_id: "session",
		state: "running",
		trigger: { type: "pool" },
		started_at: "2026-08-10T00:00:00Z",
	};
}

describe("buildRunningRows", () => {
	it("deduplicates shared Cuelists and shows the Cuelist identity and current Cue", () => {
		const release = vi.fn();
		const rows = buildRunningRows({
			playbacks: [playback("mapped"), playback("virtual", "cue-list-a", null)],
			dynamics: [],
			timecodes: [],
			macros: [],
			releasePlayback: release,
			turnOffDynamic: vi.fn(),
			stopTimecode: vi.fn(),
			cancelMacro: vi.fn(),
		});
		expect(rows).toHaveLength(1);
		expect(rows[0]).toMatchObject({
			kind: "cue_list",
			number: 4,
			name: "Act One",
			cueNumber: 5,
		});
		rows[0]?.off();
		expect(release).toHaveBeenCalledTimes(1);
		expect(release).toHaveBeenCalledWith(
			expect.objectContaining({ key: "mapped" }),
		);
	});

	it("suppresses contained Dynamics and deduplicates standalone stop identities", () => {
		const off = vi.fn();
		const standalone = dynamic("controller-a", "Programmer");
		const rows = buildRunningRows({
			playbacks: [playback("mapped")],
			dynamics: [
				dynamic("cue-controller", "Cue"),
				dynamic("playback-controller", "Playback 4"),
				standalone,
				{ ...standalone, key: "duplicate-projection" },
			],
			timecodes: [],
			macros: [],
			releasePlayback: vi.fn(),
			turnOffDynamic: off,
			stopTimecode: vi.fn(),
			cancelMacro: vi.fn(),
		});
		expect(rows.map((row) => row.kind)).toEqual(["cue_list", "dynamic"]);
		rows[1]?.off();
		expect(off).toHaveBeenCalledTimes(1);
		expect(off).toHaveBeenCalledWith(standalone);
	});

	it("deduplicates Timecodes and Macro execution projections and routes one exact Off", () => {
		const stop = vi.fn();
		const cancel = vi.fn();
		const timecode = {
			timecode_id: "timecode-a",
			state: "playing" as const,
			frame: 44,
			duration_frame: 100,
			audio_linked: true,
		};
		const rows = buildRunningRows({
			playbacks: [],
			dynamics: [],
			timecodes: [
				timecode,
				timecode,
				{ ...timecode, timecode_id: "stopped", state: "stopped" },
			],
			timecodeDefinitions: [
				{
					id: "timecode-a",
					revision: 1,
					body: { id: "timecode-a", number: 3, name: "Intro" },
				} as never,
			],
			macros: [
				macro("execution-a"),
				macro("execution-a"),
				{ ...macro("done"), state: "succeeded" },
			],
			releasePlayback: vi.fn(),
			turnOffDynamic: vi.fn(),
			stopTimecode: stop,
			cancelMacro: cancel,
		});
		expect(rows).toHaveLength(2);
		expect(rows[0]).toMatchObject({
			kind: "timecode",
			number: 3,
			name: "Intro",
			cueNumber: null,
		});
		expect(rows[1]).toMatchObject({
			kind: "macro",
			number: 9,
			cueNumber: null,
		});
		rows[0]?.off();
		rows[1]?.off();
		expect(stop).toHaveBeenCalledTimes(1);
		expect(stop).toHaveBeenCalledWith("timecode-a");
		expect(cancel).toHaveBeenCalledTimes(1);
		expect(cancel).toHaveBeenCalledWith("execution-a");
	});

	it("filters to each supported kind", () => {
		const rows = buildRunningRows({
			playbacks: [playback("mapped")],
			dynamics: [dynamic("controller", "Programmer")],
			timecodes: [
				{
					timecode_id: "tc",
					state: "paused",
					frame: 0,
					duration_frame: 1,
					audio_linked: false,
				},
			],
			macros: [macro("macro")],
			releasePlayback: vi.fn(),
			turnOffDynamic: vi.fn(),
			stopTimecode: vi.fn(),
			cancelMacro: vi.fn(),
		});
		for (const kind of ["cue_list", "dynamic", "timecode", "macro"] as const) {
			expect(filterRunningRows(rows, kind).map((row) => row.kind)).toEqual([
				kind,
			]);
		}
		expect(filterRunningRows(rows, "all")).toHaveLength(4);
	});
});
