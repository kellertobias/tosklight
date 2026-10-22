import { act, cleanup, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PackagedStageBenchmarkConfig } from "./platform/desktop/types";

const mock = vi.hoisted(() => ({
	append: vi.fn(async (_sample: Record<string, unknown>) => undefined),
	take: vi.fn(async () => ({ frames: [], heartbeats: [] })),
	prepared: vi.fn(async () => true),
	dispatch: vi.fn(),
	stage: vi.fn(),
	sheet: vi.fn(),
}));
vi.mock("./api/ServerRuntime", () => ({
	ServerRuntime: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("./state/AppContext", () => ({
	AppProvider: ({ children }: { children: ReactNode }) => children,
	useApp: () => ({
		state: { stageVizQuality: "standard" },
		dispatch: mock.dispatch,
	}),
}));
vi.mock("./features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("./components/shell/ConnectionState", () => ({
	ConnectionState: () => null,
}));
vi.mock("./components/shell/DeskLoadingOverlay", () => ({
	DeskLoadingOverlay: () => null,
}));
vi.mock("./windows/FixtureSheetBenchmarkDiagnostics", () => ({
	FixtureSheetBenchmarkDiagnostics: ({ children }: { children: ReactNode }) =>
		children,
}));
vi.mock("./windows/FixtureSheetWindow", () => ({
	FixtureSheetWindow: (props: { active: boolean }) => {
		mock.sheet(props);
		return <div data-testid="sheet" data-active={String(props.active)} />;
	},
}));
vi.mock("./windows/StageWindow", () => ({
	StageWindow: (props: unknown) => {
		mock.stage(props);
		return <div data-testid="stage" />;
	},
}));
const bridge = {
	appendPackagedStageBenchmarkSample: mock.append,
	takeStagePaneBenchmarkSamples: mock.take,
	packagedStageBenchmarkPrepared: mock.prepared,
};
vi.mock("./platform/desktop", () => ({ useDesktopBridge: () => bridge }));

import { NativePackagedStageBenchmarkApp } from "./NativePackagedStageBenchmarkApp";
import {
	benchmarkStageEnabled,
	NativeHostObservations,
} from "./nativePackagedBenchmarkMode";

const config: PackagedStageBenchmarkConfig = {
	stageEnabled: false,
	fixtureSheet: true,
	expectedFixtureRecords: 576,
	additionalStageWindow: false,
	profile: "sixteen_universe",
	durationSeconds: 3,
	controlDurationSeconds: 30,
};
const samples = () => mock.append.mock.calls.map(([sample]) => sample);
async function mount(stageEnabled = false) {
	await act(async () => {
		render(
			<NativePackagedStageBenchmarkApp config={{ ...config, stageEnabled }} />,
		);
	});
}

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(new Date("2026-10-17T12:00:00Z"));
	mock.append.mockClear();
	mock.take.mockClear();
	mock.stage.mockClear();
	mock.sheet.mockClear();
	mock.dispatch.mockClear();
	mock.take.mockResolvedValue({ frames: [], heartbeats: [] });
});
afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe("packaged benchmark Stage mode", () => {
	it("keeps the prepared 576 Fixture Sheet active without mounting Stage or exercising its lifecycle", async () => {
		await mount();
		expect(screen.getByTestId("sheet").getAttribute("data-active")).toBe(
			"true",
		);
		expect(screen.queryByTestId("stage")).toBeNull();
		await act(async () => {
			await vi.advanceTimersByTimeAsync(31_100);
		});
		expect(mock.stage).not.toHaveBeenCalled();
		expect(mock.dispatch).not.toHaveBeenCalled();
		expect(samples().some((sample) => sample.kind === "stage-started")).toBe(
			false,
		);
		const started = samples().find((sample) => sample.kind === "started");
		expect(started).toMatchObject({
			stageEnabled: false,
			config: { stageEnabled: false, expectedFixtureRecords: 576 },
		});
		const ack = samples().find((sample) => sample.kind === "stage-mode-ack");
		expect(ack).toMatchObject({ stageEnabled: false, hostObservationCount: 1 });
		const complete = samples().find((sample) => sample.kind === "complete");
		expect(complete).toMatchObject({
			stageEnabled: false,
			config: { stageEnabled: false },
			activeUiSurfaces: ["fixture-sheet"],
			nativeStage: {
				frames: [],
				activity: [],
				heartbeats: [],
				rendererLifecycleExercise: "disabled",
				observerOverflow: { errors: 0 },
			},
		});
		expect(
			(complete?.nativeStage as { hostObservationCount: number })
				.hostObservationCount,
		).toBeGreaterThanOrEqual(4);
		expect(mock.take.mock.calls.length).toBeGreaterThanOrEqual(4);
	});

	it("retains the enabled Stage start delay and both panes", async () => {
		await mount(true);
		expect(screen.queryByTestId("stage")).toBeNull();
		expect(screen.getByTestId("sheet").getAttribute("data-active")).toBe(
			"false",
		);
		await act(async () => {
			await vi.advanceTimersByTimeAsync(30_000);
		});
		expect(screen.getAllByTestId("stage")).toHaveLength(2);
		expect(mock.stage).toHaveBeenCalledWith(
			expect.objectContaining({ stageView: "3d", followPreload: false }),
		);
		expect(mock.stage).toHaveBeenCalledWith(
			expect.objectContaining({ stageView: "2d", followPreload: true }),
		);
		expect(mock.dispatch).toHaveBeenCalledWith({
			type: "SET_STAGE_OPTIONS",
			vizQuality: "draft",
		});
		expect(screen.getByTestId("sheet").getAttribute("data-active")).toBe(
			"true",
		);
		expect(
			samples().filter((sample) => sample.kind === "stage-started"),
		).toHaveLength(1);
	});

	it("records host query errors without inventing successful empty observations", async () => {
		mock.take.mockRejectedValueOnce(new Error("host unavailable"));
		await mount();
		expect(samples().some((sample) => sample.kind === "stage-mode-ack")).toBe(
			false,
		);
		await act(async () => {
			await vi.advanceTimersByTimeAsync(3_100);
		});
		const complete = samples().find((sample) => sample.kind === "complete");
		expect(complete).toMatchObject({
			nativeStage: { observerOverflow: { errors: 1 } },
		});
		expect(
			(complete?.nativeStage as { hostObservationCount: number })
				.hostObservationCount,
		).toBe(mock.take.mock.calls.length - 1);
	});

	it("rejects an unknown mode instead of silently enabling or disabling Stage", () => {
		expect(() =>
			benchmarkStageEnabled({
				...config,
				stageEnabled: undefined,
			} as unknown as PackagedStageBenchmarkConfig),
		).toThrow("Invalid packaged benchmark Stage mode");
	});

	it("retains genuine host observation counts and rejects malformed responses", () => {
		const observations = new NativeHostObservations();
		observations.record({ frames: [], heartbeats: [] }, 123);
		expect(observations.rows).toEqual([
			{ recordedAt: 123, frames: 0, heartbeats: 0 },
		]);
		expect(() => observations.record({} as never)).toThrow(
			"Invalid host Stage observation",
		);
		expect(observations.rows).toHaveLength(1);
	});
});
