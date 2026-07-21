import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SpeedGroupId, SpeedGroupSoundState } from "../../api/types";
import { useSoundToLight } from "./useSoundToLight";

const analyzer = vi.hoisted(() => ({
	start: vi.fn(async () => undefined),
	stop: vi.fn(),
	update: vi.fn(),
}));
const refreshInputs = vi.hoisted(() => vi.fn(async () => undefined));
const deviceSelection = vi.hoisted(() => ({
	devices: [{ deviceId: "input-a", label: "Input A" }],
	deviceIds: { A: "input-a" },
	permission: "granted",
	setPermission: vi.fn(),
	refreshInputs,
	setDevice: vi.fn(),
}));
const server = vi.hoisted(() => ({
	session: {
		session_id: "session-a",
		desk: { id: "00000000-0000-4000-8000-000000000101" },
	},
	speedGroup: vi.fn(),
	updateSpeedGroup: vi.fn(),
	observeSpeedGroup: vi.fn(),
	speedGroupAction: vi.fn(),
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("./useSoundDeviceSelection", () => ({
	useSoundDeviceSelection: () => deviceSelection,
}));
vi.mock("./soundToLightAnalyzer", async (importOriginal) => {
	const actual =
		await importOriginal<typeof import("./soundToLightAnalyzer")>();
	return {
		...actual,
		SoundToLightAudioAnalyzer: class {
			start = analyzer.start;
			stop = analyzer.stop;
			updateConfiguration = analyzer.update;
		},
	};
});

function soundState(group: SpeedGroupId): SpeedGroupSoundState {
	return {
		group,
		configuration: {
			enabled: group === "A",
			analysis_mode: "tempo_bpm",
			frequency: { type: "preset", preset: "low" },
			input_gain_db: 0,
			confidence_threshold: 0.65,
			smoothing: 0.35,
			minimum_bpm: 40,
			maximum_bpm: 240,
			signal_hold_millis: 2_000,
			multiplier: 1,
		},
		snapshot: {
			manual_bpm: 120,
			sound_bpm: null,
			effective_bpm: 120,
			source: "manual",
			sound_status:
				group === "A"
					? { state: "manual_fallback", reason: "waiting_for_analysis" }
					: { state: "disabled" },
			paused: false,
			phase_advancing: true,
			speed_master_scale: 1,
			sound_multiplier: 1,
			source_available: false,
			usable_signal: false,
			input_level: 0,
			selected_band_level: 0,
		},
	};
}

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	server.session = {
		session_id: "session-a",
		desk: { id: "00000000-0000-4000-8000-000000000101" },
	};
});

describe("useSoundToLight lifecycle", () => {
	it("opens no v1 Sound requests or analyzer before a modal requests it", async () => {
		server.speedGroup.mockImplementation(async (group: SpeedGroupId) =>
			soundState(group),
		);
		const rendered = renderHook(
			({ enabled }: { enabled: boolean }) => useSoundToLight(enabled),
			{ initialProps: { enabled: false } },
		);
		await Promise.resolve();
		expect(server.speedGroup).not.toHaveBeenCalled();
		expect(analyzer.start).not.toHaveBeenCalled();

		rendered.rerender({ enabled: true });
		await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(5));
		expect(server.speedGroup.mock.calls.map(([group]) => group)).toEqual([
			"A",
			"B",
			"C",
			"D",
			"E",
		]);
		await waitFor(() => expect(analyzer.start).toHaveBeenCalledWith("input-a"));

		rendered.rerender({ enabled: false });
		await waitFor(() => expect(analyzer.stop).toHaveBeenCalled());
		expect(rendered.result.current.states).toEqual({});

		rendered.rerender({ enabled: true });
		await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(10));
		await waitFor(() => expect(analyzer.start).toHaveBeenCalledTimes(2));
		expect(server.speedGroup.mock.calls.map(([group]) => group)).toEqual([
			"A",
			"B",
			"C",
			"D",
			"E",
			"A",
			"B",
			"C",
			"D",
			"E",
		]);
	});

	it("cancels late five-group hydration when the modal closes", async () => {
		let resolveA!: (value: SpeedGroupSoundState) => void;
		server.speedGroup.mockImplementation(async (group: SpeedGroupId) =>
			group === "A"
				? new Promise((resolve) => {
						resolveA = resolve;
					})
				: soundState(group),
		);
		const rendered = renderHook(
			({ enabled }: { enabled: boolean }) => useSoundToLight(enabled),
			{ initialProps: { enabled: true } },
		);
		await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(5));
		rendered.rerender({ enabled: false });
		resolveA(soundState("A"));
		await Promise.resolve();
		await Promise.resolve();
		expect(rendered.result.current.states).toEqual({});
		expect(analyzer.start).not.toHaveBeenCalled();
	});
});
