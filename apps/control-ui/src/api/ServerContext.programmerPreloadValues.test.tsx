import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useProgrammerCaptureModeView } from "../features/programmerCaptureMode/ProgrammerCaptureModeView";
import {
	useProgrammerPreloadValuesActions,
	useProgrammerPreloadValuesView,
} from "../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerPreloadValuesEventObserver } from "../features/programmerPreloadValues/transport";
import { LightApiClient } from "./LightApiClient";
import { ServerProvider, useServer } from "./ServerContext";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

const boundaries = vi.hoisted(() => {
	const subscribeCaptureMode = vi.fn();
	const subscribePreloadValues = vi.fn();
	return {
		loadCaptureMode: vi.fn(),
		subscribeCaptureMode,
		captureModeTransport: { subscribe: subscribeCaptureMode },
		loadPreloadValues: vi.fn(),
		applyPreloadValues: vi.fn(),
		subscribePreloadValues,
		preloadValuesTransport: { subscribe: subscribePreloadValues },
		reportError: vi.fn(),
	};
});

vi.mock("../features/server/useServerPolling", () => ({
	useServerPolling: vi.fn(),
}));
vi.mock("../features/server/useShowData", () => ({
	useShowObjects: () => vi.fn().mockResolvedValue(undefined),
	useServerRefresh: () => vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../features/server/useServerConnection", async () => {
	const { useEffect } = await import("react");
	return {
		useServerConnection: (state: {
			setBootstrap(value: unknown): void;
			setSession(value: unknown): void;
			setConnectionGeneration(value: number): void;
		}) => {
			useEffect(() => {
				const user = { id: USER_ID, name: "Operator", enabled: true };
				const desk = {
					id: DESK_ID,
					name: "Main",
					osc_alias: "main",
					columns: 1,
					rows: 1,
					buttons: 1,
				};
				state.setBootstrap({
					api_version: "2",
					attribute_registry: [],
					users: [user],
					desks: [desk],
					clients: [],
					active_show: {
						id: SHOW_ID,
						name: "Show",
						path: "show",
						revision: 1,
						updated_at: "",
					},
					active_programmers: [],
					frame_rate_hz: 44,
					output_health: {
						frames_sent: 0,
						packets_sent: 0,
						send_errors: 0,
						deadline_misses: 0,
						maximum_lateness_micros: 0,
						frame_hz: 44,
						last_tick_micros: 0,
						maximum_tick_micros: 0,
						scheduler_utilization: 0,
					},
					active_timecode_source: null,
					active_timecode: null,
					active_show_error: null,
					hardware_connected: false,
				});
				state.setSession({
					session_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
					client_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
					token: "session-token",
					user,
					desk,
				});
				state.setConnectionGeneration(1);
			}, []);
		},
	};
});
vi.mock("./useServerFeatureBoundaries", () => ({
	useServerFeatureBoundaries: () => ({
		showObjectsTransport: null,
		playbackTransport: null,
		programmingTransport: null,
		programmerValuesTransport: null,
		programmerPreloadValuesTransport: boundaries.preloadValuesTransport,
		programmerCaptureModeTransport: boundaries.captureModeTransport,
		programmerValuesAuthorityKey: "server-session-a",
		programmerPreloadValuesAuthorityKey: "server-session-a",
		programmerCaptureModeAuthorityKey: "server-session-a",
		loadPlaybackSnapshot: vi.fn(),
		loadProgrammingInteractionSnapshot: vi.fn(),
		loadProgrammerValuesSnapshot: vi.fn(),
		loadProgrammerPreloadValuesSnapshot: boundaries.loadPreloadValues,
		loadProgrammerCaptureModeSnapshot: boundaries.loadCaptureMode,
		applyProgrammerValuesAction: vi.fn(),
		applyProgrammerPreloadValuesAction: boundaries.applyPreloadValues,
		loadShowObjectCollection: vi.fn(),
		loadShowObject: vi.fn(),
		reportShowObjectError: vi.fn(),
		reportPlaybackError: vi.fn(),
		reportProgrammingSessionError: vi.fn(),
		reportProgrammingMutationError: vi.fn(),
		reportProgrammerValuesSessionError: vi.fn(),
		reportProgrammerValuesMutationError: vi.fn(),
		reportProgrammerPreloadValuesSessionError: boundaries.reportError,
		reportProgrammerPreloadValuesMutationError: boundaries.reportError,
		reportProgrammerCaptureModeSessionError: boundaries.reportError,
	}),
}));

let unrelatedRenders = 0;

function UnrelatedServerConsumer() {
	useServer();
	unrelatedRenders += 1;
	return null;
}

function CaptureProbe() {
	const projection = useProgrammerCaptureModeView();
	return <span>{projection?.revision ?? "Loading capture"}</span>;
}

function PreloadActionsProbe() {
	return (
		<span>
			{useProgrammerPreloadValuesActions()
				? "Preload actions ready"
				: "No Preload actions"}
		</span>
	);
}

function PreloadValuesProbe() {
	const projection = useProgrammerPreloadValuesView();
	return <span>{projection?.revision ?? "Loading Preload values"}</span>;
}

function Harness({ showValues }: { showValues: boolean }) {
	return (
		<ServerProvider>
			<UnrelatedServerConsumer />
			<CaptureProbe />
			<PreloadActionsProbe />
			{showValues ? <PreloadValuesProbe /> : null}
		</ServerProvider>
	);
}

function captureProjection(revision: number) {
	return {
		userId: USER_ID,
		revision,
		blind: true,
		preview: false,
		preloadCaptureProgrammer: true,
	};
}

function preloadProjection(revision: number) {
	return {
		userId: USER_ID,
		revision,
		fixtureValues: [],
		groupValues: [],
	};
}

describe("ServerProvider Programmer Preload-values boundary", () => {
	it("waits for both active capture and a mounted values view", async () => {
		boundaries.loadCaptureMode.mockReset();
		boundaries.subscribeCaptureMode.mockReset();
		boundaries.loadPreloadValues.mockReset();
		boundaries.applyPreloadValues.mockReset();
		boundaries.subscribePreloadValues.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		let observer: ProgrammerPreloadValuesEventObserver | null = null;
		boundaries.loadCaptureMode.mockResolvedValue({
			cursor: 10,
			projection: captureProjection(1),
		});
		boundaries.loadPreloadValues.mockResolvedValue({
			cursor: 20,
			projection: preloadProjection(3),
		});
		boundaries.subscribeCaptureMode.mockReturnValue({
			close: vi.fn(),
			repair: vi.fn(),
		});
		boundaries.subscribePreloadValues.mockImplementation(
			(_scope, _cursor, nextObserver) => {
				observer = nextObserver;
				return { close: vi.fn(), repair: vi.fn() };
			},
		);
		unrelatedRenders = 0;
		const rendered = render(<Harness showValues={false} />);

		await waitFor(() => expect(screen.getByText("1")).toBeInTheDocument());
		expect(screen.getByText("Preload actions ready")).toBeInTheDocument();
		expect(boundaries.loadPreloadValues).not.toHaveBeenCalled();
		expect(boundaries.subscribePreloadValues).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();

		rendered.rerender(<Harness showValues />);
		await waitFor(() => expect(screen.getByText("3")).toBeInTheDocument());
		expect(boundaries.loadPreloadValues).toHaveBeenCalledOnce();
		expect(boundaries.subscribePreloadValues).toHaveBeenCalledWith(
			{ showId: SHOW_ID, userId: USER_ID },
			20,
			expect.any(Object),
		);
		const rendersBeforeEvent = unrelatedRenders;

		act(() =>
			observer?.message({
				type: "event",
				sequence: 21,
				correlationId: null,
				projection: preloadProjection(4),
			}),
		);

		expect(screen.getByText("4")).toBeInTheDocument();
		expect(unrelatedRenders).toBe(rendersBeforeEvent);
		expect(broadBootstrap).not.toHaveBeenCalled();
		broadBootstrap.mockRestore();
	});
});
