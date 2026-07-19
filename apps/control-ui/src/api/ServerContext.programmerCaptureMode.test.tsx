import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	useProgrammerCaptureModeStatus,
	useProgrammerCaptureModeView,
} from "../features/programmerCaptureMode/ProgrammerCaptureModeView";
import type { ProgrammerCaptureModeEventObserver } from "../features/programmerCaptureMode/transport";
import { LightApiClient } from "./LightApiClient";
import { ServerProvider, useServer } from "./ServerContext";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

const boundaries = vi.hoisted(() => ({
	loadCaptureMode: vi.fn(),
	subscribeCaptureMode: vi.fn(),
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
		programmerValuesTransport: null,
		programmerCaptureModeTransport: {
			subscribe: boundaries.subscribeCaptureMode,
		},
		programmerValuesAuthorityKey: "server-session-a",
		programmerCaptureModeAuthorityKey: "server-session-a",
		loadPlaybackSnapshot: vi.fn(),
		loadProgrammingInteractionSnapshot: vi.fn(),
		loadProgrammerValuesSnapshot: vi.fn(),
		loadProgrammerCaptureModeSnapshot: boundaries.loadCaptureMode,
		applyProgrammerValuesAction: vi.fn(),
		loadShowObjectCollection: vi.fn(),
		loadShowObject: vi.fn(),
		reportShowObjectError: vi.fn(),
		reportPlaybackError: vi.fn(),
		reportProgrammingSessionError: vi.fn(),
		reportProgrammingMutationError: vi.fn(),
		reportProgrammerValuesSessionError: vi.fn(),
		reportProgrammerValuesMutationError: vi.fn(),
		reportProgrammerCaptureModeSessionError: vi.fn(),
	}),
}));

let unrelatedRenders = 0;

function UnrelatedServerConsumer() {
	useServer();
	unrelatedRenders += 1;
	return null;
}

function CaptureModeStatusProbe() {
	const status = useProgrammerCaptureModeStatus();
	return <span data-testid="capture-status">{status.status}</span>;
}

function CaptureModeProbe() {
	const projection = useProgrammerCaptureModeView();
	return (
		<span data-testid="capture-revision">
			{projection?.revision ?? "Loading capture mode"}
		</span>
	);
}

function Harness({ showCaptureMode }: { showCaptureMode: boolean }) {
	return (
		<ServerProvider>
			<UnrelatedServerConsumer />
			<CaptureModeStatusProbe />
			{showCaptureMode ? <CaptureModeProbe /> : null}
		</ServerProvider>
	);
}

function projection(revision: number) {
	return {
		userId: USER_ID,
		revision,
		blind: false,
		preview: false,
		preloadCaptureProgrammer: false,
	};
}

describe("ServerProvider Programmer capture-mode boundary", () => {
	it("is dormant until an explicit view mounts and isolates context consumers", async () => {
		boundaries.loadCaptureMode.mockReset();
		boundaries.subscribeCaptureMode.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		let observer: ProgrammerCaptureModeEventObserver | null = null;
		boundaries.loadCaptureMode.mockResolvedValue({
			cursor: 10,
			projection: projection(1),
		});
		boundaries.subscribeCaptureMode.mockImplementation(
			(_scope, _cursor, nextObserver) => {
				observer = nextObserver;
				return { close: vi.fn(), repair: vi.fn() };
			},
		);
		unrelatedRenders = 0;
		const rendered = render(<Harness showCaptureMode={false} />);
		await waitFor(() =>
			expect(screen.getByTestId("capture-status")).toHaveTextContent("idle"),
		);

		expect(boundaries.loadCaptureMode).not.toHaveBeenCalled();
		expect(boundaries.subscribeCaptureMode).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();

		rendered.rerender(<Harness showCaptureMode />);
		await waitFor(() =>
			expect(screen.getByTestId("capture-revision")).toHaveTextContent("1"),
		);
		expect(boundaries.loadCaptureMode).toHaveBeenCalledOnce();
		expect(boundaries.subscribeCaptureMode).toHaveBeenCalledOnce();
		expect(boundaries.subscribeCaptureMode).toHaveBeenCalledWith(
			{ showId: SHOW_ID, userId: USER_ID },
			10,
			expect.any(Object),
		);
		const rendersBeforeEvent = unrelatedRenders;

		act(() =>
			observer?.message({
				type: "event",
				sequence: 11,
				correlationId: null,
				projection: projection(2),
			}),
		);

		expect(screen.getByTestId("capture-revision")).toHaveTextContent("2");
		expect(unrelatedRenders).toBe(rendersBeforeEvent);
		expect(broadBootstrap).not.toHaveBeenCalled();
		broadBootstrap.mockRestore();
	});
});
