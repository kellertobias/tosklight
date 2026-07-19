import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	useProgrammerValuesActions,
	useProgrammerValuesView,
} from "../features/programmerValues/ProgrammerValuesView";
import { ServerProvider, useServer } from "./ServerContext";
import { LightApiClient } from "./LightApiClient";

const SHOW_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const DESK_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

const boundaries = vi.hoisted(() => ({
	loadValues: vi.fn(),
	applyValues: vi.fn(),
	subscribeValues: vi.fn(),
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
		programmerValuesTransport: { subscribe: boundaries.subscribeValues },
		programmerValuesAuthorityKey: "server-session-a",
		loadPlaybackSnapshot: vi.fn(),
		loadProgrammingInteractionSnapshot: vi.fn(),
		loadProgrammerValuesSnapshot: boundaries.loadValues,
		applyProgrammerValuesAction: boundaries.applyValues,
		loadShowObjectCollection: vi.fn(),
		loadShowObject: vi.fn(),
		reportShowObjectError: vi.fn(),
		reportPlaybackError: vi.fn(),
		reportProgrammingSessionError: vi.fn(),
		reportProgrammingMutationError: vi.fn(),
		reportProgrammerValuesSessionError: vi.fn(),
		reportProgrammerValuesMutationError: vi.fn(),
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

function Harness({ showValues }: { showValues: boolean }) {
	return (
		<ServerProvider>
			<UnrelatedServerConsumer />
			<ActionProbe />
			{showValues ? <ValuesProbe /> : null}
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

describe("ServerProvider Programmer values boundary", () => {
	it("is dormant until a values view mounts and isolates unrelated renders", async () => {
		boundaries.loadValues.mockReset();
		boundaries.applyValues.mockReset();
		boundaries.subscribeValues.mockReset();
		const broadBootstrap = vi.spyOn(LightApiClient.prototype, "bootstrap");
		let observer: { message(value: unknown): void } | null = null;
		boundaries.loadValues.mockResolvedValue({
			cursor: 10,
			projection: projection(1),
		});
		boundaries.subscribeValues.mockImplementation(
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

		expect(boundaries.loadValues).not.toHaveBeenCalled();
		expect(boundaries.subscribeValues).not.toHaveBeenCalled();
		expect(broadBootstrap).not.toHaveBeenCalled();

		rendered.rerender(<Harness showValues />);
		await waitFor(() => expect(screen.getByText("1")).toBeInTheDocument());
		expect(boundaries.loadValues).toHaveBeenCalledOnce();
		expect(boundaries.subscribeValues).toHaveBeenCalledOnce();
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
		broadBootstrap.mockRestore();
	});
});
