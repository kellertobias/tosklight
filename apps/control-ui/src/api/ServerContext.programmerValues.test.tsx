import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useProgrammerLifecycleView } from "../features/programmerLifecycle/ProgrammerLifecycleView";
import { useProgrammerPreloadPlaybackQueueView } from "../features/programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView";
import {
	useProgrammerValuesActions,
	useProgrammerValuesView,
} from "../features/programmerValues/ProgrammerValuesView";
import { LightApiClient } from "./LightApiClient";
import { ServerProvider, useServer } from "./ServerContext";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

const boundaries = vi.hoisted(() => ({
	loadValues: vi.fn(),
	applyValues: vi.fn(),
	subscribeValues: vi.fn(),
	loadCaptureMode: vi.fn(),
	subscribeCaptureMode: vi.fn(),
	loadLifecycle: vi.fn(),
	subscribeLifecycle: vi.fn(),
	loadQueue: vi.fn(),
	subscribeQueue: vi.fn(),
}));

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
		programmerLifecycleTransport: {
			subscribe: boundaries.subscribeLifecycle,
		},
		programmerValuesTransport: { subscribe: boundaries.subscribeValues },
		programmerPreloadPlaybackQueueTransport: {
			subscribe: boundaries.subscribeQueue,
		},
		programmerCaptureModeTransport: {
			subscribe: boundaries.subscribeCaptureMode,
		},
		programmerValuesAuthorityKey: "server-session-a",
		programmerCaptureModeAuthorityKey: "server-session-a",
		programmerLifecycleAuthorityKey: "server-session-a",
		programmerPreloadPlaybackQueueAuthorityKey: "server-session-a",
		loadPlaybackSnapshot: vi.fn(),
		loadProgrammingInteractionSnapshot: vi.fn(),
		loadProgrammerValuesSnapshot: boundaries.loadValues,
		loadProgrammerCaptureModeSnapshot: boundaries.loadCaptureMode,
		loadProgrammerLifecycleSnapshot: boundaries.loadLifecycle,
		loadProgrammerPreloadPlaybackQueueSnapshot: boundaries.loadQueue,
		applyProgrammerValuesAction: boundaries.applyValues,
		loadShowObjectCollection: vi.fn(),
		loadShowObject: vi.fn(),
		reportShowObjectError: vi.fn(),
		reportPlaybackError: vi.fn(),
		reportProgrammingSessionError: vi.fn(),
		reportProgrammingMutationError: vi.fn(),
		reportProgrammerValuesSessionError: vi.fn(),
		reportProgrammerValuesMutationError: vi.fn(),
		reportProgrammerCaptureModeSessionError: vi.fn(),
		reportProgrammerLifecycleSessionError: vi.fn(),
		reportProgrammerPreloadPlaybackQueueSessionError: vi.fn(),
	}),
}));

let unrelatedRenders = 0;

function UnrelatedServerConsumer() {
	useServer();
	unrelatedRenders += 1;
	return null;
}

function ActionProbe() {
	return (
		<span>{useProgrammerValuesActions() ? "Actions ready" : "No actions"}</span>
	);
}

function ValuesProbe() {
	const projection = useProgrammerValuesView();
	return <span>{projection?.revision ?? "Loading values"}</span>;
}

function LifecycleProbe() {
	const projection = useProgrammerLifecycleView();
	return <span>Lifecycle {projection?.revision ?? "loading"}</span>;
}

function QueueProbe() {
	const projection = useProgrammerPreloadPlaybackQueueView();
	return <span>Queue {projection?.revision ?? "loading"}</span>;
}

function Harness({
	showValues,
	showLifecycle = false,
	showQueue = false,
}: {
	showValues: boolean;
	showLifecycle?: boolean;
	showQueue?: boolean;
}) {
	return (
		<ServerProvider>
			<UnrelatedServerConsumer />
			<ActionProbe />
			{showValues ? <ValuesProbe /> : null}
			{showLifecycle ? <LifecycleProbe /> : null}
			{showQueue ? <QueueProbe /> : null}
		</ServerProvider>
	);
}

function projection(revision: number) {
	return {
		userId: USER_ID,
		revision,
		fixtureValues: [],
		groupValues: [],
	};
}

function captureModeProjection(revision: number) {
	return {
		userId: USER_ID,
		revision,
		blind: false,
		preview: false,
		preloadCaptureProgrammer: false,
	};
}

function lifecycleProjection(revision: number) {
	return {
		revision,
		programmers: [
			{
				programmerId: "programmer-a",
				userId: USER_ID,
				connected: true,
				selectedFixtureCount: 0,
				normalValueCount: 0,
				sessions: [],
			},
		],
	};
}

function queueProjection(revision: number) {
	return {
		userId: USER_ID,
		revision,
		actions: [{ playbackNumber: 7, action: "go", surface: "virtual" }],
	};
}

describe("ServerProvider Programmer values boundary", () => {
	it("is dormant until a values view mounts and isolates unrelated renders", async () => {
		boundaries.loadValues.mockReset();
		boundaries.applyValues.mockReset();
		boundaries.subscribeValues.mockReset();
		boundaries.loadCaptureMode.mockReset();
		boundaries.subscribeCaptureMode.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		let observer: { message(value: unknown): void } | null = null;
		boundaries.loadValues.mockResolvedValue({
			cursor: 10,
			projection: projection(1),
		});
		boundaries.loadCaptureMode.mockResolvedValue({
			cursor: 9,
			projection: captureModeProjection(1),
		});
		boundaries.subscribeValues.mockImplementation(
			(_scope, _cursor, nextObserver) => {
				observer = nextObserver;
				return { close: vi.fn(), repair: vi.fn() };
			},
		);
		boundaries.subscribeCaptureMode.mockReturnValue({
			close: vi.fn(),
			repair: vi.fn(),
		});
		unrelatedRenders = 0;
		const rendered = render(<Harness showValues={false} />);
		await waitFor(() =>
			expect(screen.getByText("Actions ready")).toBeInTheDocument(),
		);

		expect(boundaries.loadValues).not.toHaveBeenCalled();
		expect(boundaries.subscribeValues).not.toHaveBeenCalled();
		expect(boundaries.loadCaptureMode).not.toHaveBeenCalled();
		expect(boundaries.subscribeCaptureMode).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();

		rendered.rerender(<Harness showValues />);
		await waitFor(() => expect(screen.getByText("1")).toBeInTheDocument());
		expect(boundaries.loadValues).toHaveBeenCalledOnce();
		expect(boundaries.subscribeValues).toHaveBeenCalledOnce();
		expect(boundaries.loadCaptureMode).toHaveBeenCalledOnce();
		expect(boundaries.subscribeCaptureMode).toHaveBeenCalledOnce();
		const rendersBeforeEvent = unrelatedRenders;

		act(() =>
			observer?.message({
				type: "event",
				sequence: 11,
				correlationId: null,
				projection: projection(2),
			}),
		);

		expect(screen.getByText("2")).toBeInTheDocument();
		expect(unrelatedRenders).toBe(rendersBeforeEvent);
		rendered.unmount();
		broadBootstrap.mockRestore();
	});

	it("keeps the aggregate lifecycle dormant and outside global context renders", async () => {
		boundaries.loadLifecycle.mockReset();
		boundaries.subscribeLifecycle.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		let observer: { message(value: unknown): void } | null = null;
		boundaries.loadLifecycle.mockResolvedValue({
			cursor: 20,
			projection: lifecycleProjection(1),
		});
		boundaries.subscribeLifecycle.mockImplementation(
			(_cursor, nextObserver) => {
				observer = nextObserver;
				return { close: vi.fn(), repair: vi.fn() };
			},
		);
		unrelatedRenders = 0;
		const rendered = render(<Harness showValues={false} />);
		await waitFor(() =>
			expect(screen.getByText("Actions ready")).toBeInTheDocument(),
		);

		expect(boundaries.loadLifecycle).not.toHaveBeenCalled();
		expect(boundaries.subscribeLifecycle).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();

		rendered.rerender(<Harness showValues={false} showLifecycle />);
		await waitFor(() =>
			expect(screen.getByText("Lifecycle 1")).toBeInTheDocument(),
		);
		expect(boundaries.loadLifecycle).toHaveBeenCalledOnce();
		expect(boundaries.subscribeLifecycle).toHaveBeenCalledOnce();
		const rendersBeforeEvent = unrelatedRenders;

		act(() =>
			observer?.message({
				type: "event",
				sequence: 22,
				correlationId: null,
				change: {
					revision: 2,
					delta: {
						type: "upsert",
						programmer: {
							...lifecycleProjection(1).programmers[0],
							normalValueCount: 1,
						},
					},
				},
			}),
		);

		expect(screen.getByText("Lifecycle 2")).toBeInTheDocument();
		expect(unrelatedRenders).toBe(rendersBeforeEvent);
		expect(broadBootstrap).not.toHaveBeenCalled();
		rendered.unmount();
		broadBootstrap.mockRestore();
	});

	it("keeps the exact-user Preload playback queue dormant and locally reactive", async () => {
		boundaries.loadQueue.mockReset();
		boundaries.subscribeQueue.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		let observer: { message(value: unknown): void } | null = null;
		boundaries.loadQueue.mockResolvedValue({
			cursor: 30,
			projection: queueProjection(1),
		});
		boundaries.subscribeQueue.mockImplementation(
			(_scope, _cursor, nextObserver) => {
				observer = nextObserver;
				return { close: vi.fn(), repair: vi.fn() };
			},
		);
		unrelatedRenders = 0;
		const rendered = render(<Harness showValues={false} />);
		await waitFor(() =>
			expect(screen.getByText("Actions ready")).toBeInTheDocument(),
		);

		expect(boundaries.loadQueue).not.toHaveBeenCalled();
		expect(boundaries.subscribeQueue).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();

		rendered.rerender(<Harness showValues={false} showQueue />);
		await waitFor(() =>
			expect(screen.getByText("Queue 1")).toBeInTheDocument(),
		);
		expect(boundaries.loadQueue).toHaveBeenCalledOnce();
		expect(boundaries.subscribeQueue).toHaveBeenCalledOnce();
		const rendersBeforeEvent = unrelatedRenders;

		act(() =>
			observer?.message({
				type: "event",
				sequence: 31,
				correlationId: null,
				projection: queueProjection(2),
			}),
		);

		expect(screen.getByText("Queue 2")).toBeInTheDocument();
		expect(unrelatedRenders).toBe(rendersBeforeEvent);
		expect(broadBootstrap).not.toHaveBeenCalled();
		rendered.unmount();
		broadBootstrap.mockRestore();
	});
});
