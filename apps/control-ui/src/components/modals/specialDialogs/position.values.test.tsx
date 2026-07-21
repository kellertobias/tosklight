import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import type { ProgrammerValuesMutationQueueController } from "../../../features/programmerValues/useProgrammerValuesMutationQueue";
import { selectFixturesForSelection } from "../../../features/patch/selectors";
import { usePositionDialog } from "./position";

vi.mock("../../../features/patch/PatchState", async (importOriginal) => ({
	...(await importOriginal<Record<string, unknown>>()),
	useSelectedPatchedFixtures: (
		selectedFixtureIds: readonly string[],
		enabled = true,
	) =>
		enabled
			? selectFixturesForSelection(
					{ fixtures: server.patch.fixtures } as never,
					new Set(selectedFixtureIds),
				)
			: [],
}));

const server = vi.hoisted(() => ({
	configuration: { programmer_fade_millis: 750 },
	patch: { fixtures: [] as PatchedFixture[] },
	readVisualization: vi.fn(async () => ({
		revision: 1,
		generated_at: "2026-07-20T00:00:00Z",
		grand_master: 1,
		blackout: false,
		values: [
			{
				fixture_id: "lamp-a",
				attribute: "pan",
				value: { kind: "normalized" as const, value: 0.2 },
			},
			{
				fixture_id: "lamp-a",
				attribute: "tilt",
				value: { kind: "normalized" as const, value: 0.7 },
			},
			{
				fixture_id: "lamp-b",
				attribute: "pan",
				value: { kind: "normalized" as const, value: 0.8 },
			},
			{
				fixture_id: "lamp-b",
				attribute: "tilt",
				value: { kind: "normalized" as const, value: 0.3 },
			},
		],
	})),
}));

vi.mock("../../../api/ServerContext", () => ({ useServer: () => server }));

const writes = {
	canWrite: true,
	route: "normal",
	submitLatest: vi.fn(async () => ({})),
	submitBarrier: vi.fn(async () => ({})),
} satisfies ProgrammerValuesMutationQueueController;

beforeEach(() => {
	vi.useFakeTimers();
	server.patch.fixtures = [
		fixture("lamp-a", 0.4, 0.5),
		fixture("lamp-b", 0.6, 0.3),
	];
});

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	vi.useRealTimers();
});

describe("Position special-dialog value writes", () => {
	it("sends one ordered latest batch per joystick tick", async () => {
		const { result } = renderHook(() =>
			usePositionDialog(true, ["lamp-a", "lamp-b"], writes),
		);
		await act(async () => {
			await Promise.resolve();
			await Promise.resolve();
		});
		result.current.joystick.current = { x: 1, y: 0 };

		act(() => vi.advanceTimersByTime(32));

		expect(writes.submitLatest).toHaveBeenCalledOnce();
		expect(writes.submitLatest).toHaveBeenCalledWith(expect.any(String), [
			setMutation("lamp-a", "pan", 0.23),
			setMutation("lamp-a", "tilt", 0.7),
			setMutation("lamp-b", "pan", 0.8300000000000001),
			setMutation("lamp-b", "tilt", 0.3),
		]);
	});

	it("returns every selected fixture home through one ordered barrier", async () => {
		const { result } = renderHook(() =>
			usePositionDialog(true, ["lamp-b", "lamp-a"], writes),
		);

		await act(() => result.current.returnHome());

		expect(writes.submitBarrier).toHaveBeenCalledOnce();
		expect(writes.submitBarrier).toHaveBeenCalledWith([
			setMutation("lamp-b", "pan", 0.6),
			setMutation("lamp-b", "tilt", 0.3),
			setMutation("lamp-a", "pan", 0.4),
			setMutation("lamp-a", "tilt", 0.5),
		]);
	});
});

function setMutation(fixtureId: string, attribute: string, value: number) {
	return {
		action: "set_fixture",
		fixtureId,
		attribute,
		value: { kind: "normalized", value },
		timing: { fade: true, fadeMillis: 750, delayMillis: null },
	};
}

function fixture(id: string, pan: number, tilt: number): PatchedFixture {
	return {
		fixture_id: id,
		universe: 1,
		address: 1,
		logical_heads: [],
		definition: {
			schema_version: 1,
			id,
			revision: 1,
			manufacturer: "Test",
			device_type: "moving light",
			name: id,
			model: id,
			mode: "default",
			footprint: 2,
			color_calibration: null,
			physical: {},
			hazardous: false,
			direct_control_protocols: [],
			signal_loss_policy: { type: "hold_last" },
			safe_values: {},
			heads: [
				{
					index: 0,
					name: "Main",
					shared: true,
					parameters: [parameter("pan", pan), parameter("tilt", tilt)],
				},
			],
		},
	};
}

function parameter(attribute: string, defaultValue: number) {
	return {
		attribute,
		components: [],
		default: defaultValue,
		virtual_dimmer: false,
		capabilities: [],
	};
}
