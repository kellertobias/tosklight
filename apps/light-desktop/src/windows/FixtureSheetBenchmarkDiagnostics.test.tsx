import { cleanup, render, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { appendPackagedStageBenchmarkSample } = vi.hoisted(() => ({
	appendPackagedStageBenchmarkSample: vi.fn(
		async (_sample: Record<string, unknown>) => undefined,
	),
}));
const bridge = { appendPackagedStageBenchmarkSample };
vi.mock("../platform/desktop", () => ({ useDesktopBridge: () => bridge }));

import {
	FixtureSheetBenchmarkDiagnostics,
	useFixtureSheetReadinessDiagnostics,
} from "./FixtureSheetBenchmarkDiagnostics";

const config = {
	fixtureSheet: true,
	profile: "demo",
	expectedFixtureRecords: 3,
	durationSeconds: 240,
	controlDurationSeconds: 30,
	additionalStageWindow: false,
};
const rows = [{ parentFixtureId: "one" }, { parentFixtureId: "two" }];
const state = {
	config,
	active: true,
	rows,
	activeValuesLoading: false,
	groupRuntimeLoading: false,
};

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	appendPackagedStageBenchmarkSample.mockReset().mockResolvedValue(undefined);
});

describe("packaged benchmark Fixture Sheet diagnostics", () => {
	it("distinguishes a mounted child awaiting configuration from unavailable configuration", () => {
		const view = renderHook(
			(props: { config: typeof config | null | undefined }) =>
				useFixtureSheetReadinessDiagnostics({ ...state, ...props }),
			{
				initialProps: { config: undefined as typeof config | null | undefined },
				wrapper: ({ children }) => (
					<FixtureSheetBenchmarkDiagnostics profile="demo">
						{children}
					</FixtureSheetBenchmarkDiagnostics>
				),
			},
		);
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({
				kind: "fixture-sheet-readiness",
				configState: "loading",
			}),
		);
		view.rerender({ config: null });
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({
				kind: "fixture-sheet-readiness",
				configState: "unavailable",
			}),
		);
	});

	it("does not scan or append diagnostics after readiness while the measured rows change", () => {
		const badRows = new Proxy(rows, {
			get() {
				throw new Error("post-readiness diagnostic scan");
			},
		});
		renderHook(() =>
			useFixtureSheetReadinessDiagnostics({
				...state,
				rows: badRows,
				ready: true,
			}),
		);
		expect(appendPackagedStageBenchmarkSample).not.toHaveBeenCalled();
	});
	it("retains container mount and child render failure even before the readiness hook mounts", async () => {
		vi.spyOn(console, "error").mockImplementation(() => {});
		function BrokenSheet(): never {
			throw new Error("missing fixture context");
		}
		const view = render(
			<FixtureSheetBenchmarkDiagnostics profile="demo">
				<BrokenSheet />
			</FixtureSheetBenchmarkDiagnostics>,
		);
		await waitFor(() =>
			expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
				expect.objectContaining({
					kind: "fixture-sheet-render-failed",
					errorMessage: "missing fixture context",
				}),
			),
		);
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({ kind: "fixture-sheet-container-mounted" }),
		);
		expect(
			appendPackagedStageBenchmarkSample.mock.calls.some(
				([sample]) => sample.kind === "fixture-sheet-ready",
			),
		).toBe(false);
		view.unmount();
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledWith(
			expect.objectContaining({ kind: "fixture-sheet-container-unmounted" }),
		);
	});

	it("deduplicates unchanged row identities and records changed loading predicates", () => {
		const view = renderHook(
			(props) => useFixtureSheetReadinessDiagnostics(props),
			{ initialProps: state },
		);
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledTimes(1);
		view.rerender({ ...state, rows: [...rows] });
		expect(appendPackagedStageBenchmarkSample).toHaveBeenCalledTimes(1);
		view.rerender({ ...state, groupRuntimeLoading: true });
		expect(appendPackagedStageBenchmarkSample).toHaveBeenLastCalledWith(
			expect.objectContaining({
				kind: "fixture-sheet-readiness",
				groupRuntimeLoading: true,
				fixtureRecords: 2,
				expectedFixtureRecords: 3,
			}),
		);
	});

	it("emits nothing in ordinary operator windows or when the benchmark Sheet is disabled", () => {
		renderHook(() =>
			useFixtureSheetReadinessDiagnostics({ ...state, config: null }),
		);
		renderHook(() =>
			useFixtureSheetReadinessDiagnostics({ ...state, config: undefined }),
		);
		renderHook(() =>
			useFixtureSheetReadinessDiagnostics({
				...state,
				config: { ...config, fixtureSheet: false },
			}),
		);
		expect(appendPackagedStageBenchmarkSample).not.toHaveBeenCalled();
	});

	it("qualifies a failed diagnostic append through the console without satisfying readiness", async () => {
		appendPackagedStageBenchmarkSample.mockRejectedValueOnce(
			new Error("report unavailable"),
		);
		const log = vi.spyOn(console, "error").mockImplementation(() => {});
		renderHook(() => useFixtureSheetReadinessDiagnostics(state));
		await waitFor(() =>
			expect(log).toHaveBeenCalledWith(
				"Fixture Sheet benchmark diagnostic append failed",
				expect.objectContaining({ kind: "fixture-sheet-readiness" }),
			),
		);
		expect(
			appendPackagedStageBenchmarkSample.mock.calls.some(
				([sample]) => sample.kind === "fixture-sheet-ready",
			),
		).toBe(false);
	});
});
