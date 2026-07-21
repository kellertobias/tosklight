import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { OutputRuntimeProvider } from "../../features/outputRuntime/OutputRuntimeView";
import { OutputRuntimeStore } from "../../features/outputRuntime/store";
import {
	changedOutcome,
	deferred,
	DESK_ID,
	FakeOutputRuntimeTransport,
	noChangeOutcome,
	outputProjection,
	outputSnapshot,
	settleOutputSession,
	SHOW_ID,
} from "../../features/outputRuntime/testFixtures";
import { OutputRuntimeTransportError } from "../../features/outputRuntime/transport";
import { SystemControlsModal } from "./SystemControlsModal";

const appState = { systemControlsOpen: true };
const dispatch = vi.fn();
let legacyOutputAccesses = 0;
const server = {
	selectedFixtures: [],
	patch: { fixtures: [] },
	session: { user: { id: "operator", name: "Operator" } },
	clearProgrammer: vi.fn().mockResolvedValue(undefined),
	controlFixtureAction: vi.fn().mockResolvedValue(undefined),
	get readVisualization() {
		legacyOutputAccesses++;
		throw new Error("Output controls must not poll visualization");
	},
	get setMaster() {
		legacyOutputAccesses++;
		throw new Error("Output controls must not use the legacy master facade");
	},
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: appState, dispatch }),
}));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: () => ({ selected: [] }),
	}),
);
vi.mock("../../features/programmerLifecycle/ProgrammerLifecycleView", () => ({
	useProgrammerLifecycleView: () => ({ programmers: [] }),
}));
vi.mock(
	"../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView",
	() => ({
		useProgrammerPreloadLifecycleView: () => ({
			ready: true,
			armed: false,
			active: false,
			pending: false,
			phase: "idle",
			error: null,
			actions: {
				enter: vi.fn(),
				go: vi.fn(),
				clearPending: vi.fn(),
				release: vi.fn(),
			},
		}),
	}),
);
vi.mock("./systemControls/runningPlaybackAuthority", () => ({
	useRunningPlaybackAuthority: () => ({
		ready: true,
		loading: false,
		canRelease: true,
		sources: [],
		mappedSources: [],
		virtualSources: [],
		dynamics: [],
		release: vi.fn(),
	}),
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	legacyOutputAccesses = 0;
});

function harness(
	transport: FakeOutputRuntimeTransport,
	store = new OutputRuntimeStore(),
	authorityKey = "session-a",
): { view: ReactNode; store: OutputRuntimeStore } {
	return {
		store,
		view: (
			<OutputRuntimeProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				authorityKey={authorityKey}
				store={store}
				transport={transport}
			>
				<SystemControlsModal />
			</OutputRuntimeProvider>
		),
	};
}

function outputControls() {
	return {
		slider: screen.getByRole("slider", { name: "Grand master" }),
		blackout: screen.getByRole("button", {
			name: /^(?:RELEASE )?BLACKOUT$/u,
		}),
	};
}

describe("SystemControlsModal scoped Output runtime", () => {
	it("refuses loading input, then uses one optimistic response-first action", async () => {
		const transport = new FakeOutputRuntimeTransport();
		const snapshot = deferred<ReturnType<typeof outputSnapshot>>();
		transport.loadSnapshot.mockReturnValueOnce(snapshot.promise);
		const current = harness(transport);
		render(current.view);

		const loading = outputControls();
		expect(loading.slider).toBeDisabled();
		expect(loading.blackout).toBeDisabled();
		expect(screen.getByText("—")).toBeInTheDocument();
		fireEvent.input(loading.slider, { target: { value: "40" } });
		fireEvent.click(loading.blackout);
		expect(transport.applyAction).not.toHaveBeenCalled();

		snapshot.resolve(outputSnapshot());
		await act(settleOutputSession);
		await waitFor(() => expect(outputControls().slider).toBeEnabled());
		transport.applyAction.mockImplementationOnce(async (_scope, request) =>
			changedOutcome(
				request.requestId,
				outputProjection({
					revision: 2,
					grandMaster: request.grandMaster ?? 1,
				}),
			),
		);

		fireEvent.input(outputControls().slider, { target: { value: "40" } });
		await waitFor(() => expect(screen.getByText("40%")).toBeInTheDocument());
		expect(transport.applyAction).toHaveBeenCalledOnce();
		expect(transport.applyAction.mock.calls[0]?.[1]).toMatchObject({
			expectedShowId: SHOW_ID,
			expectedRevision: 1,
			grandMaster: 0.4,
		});
		expect(transport.applyAction.mock.calls[0]?.[1].blackout).toBeUndefined();

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: {
					projection: outputProjection({
						revision: 2,
						grandMaster: 0.4,
					}),
				},
			}),
		);
		expect(screen.getByText("40%")).toBeInTheDocument();
		expect(legacyOutputAccesses).toBe(0);
	});

	it("reconciles an event-first blackout and external desk changes", async () => {
		const transport = new FakeOutputRuntimeTransport();
		const response = deferred<ReturnType<typeof changedOutcome>>();
		transport.applyAction.mockReturnValueOnce(response.promise);
		const current = harness(transport);
		render(current.view);
		await waitFor(() => expect(outputControls().blackout).toBeEnabled());

		fireEvent.click(outputControls().blackout);
		expect(
			screen.getByRole("button", { name: "RELEASE BLACKOUT" }),
		).toBeInTheDocument();
		await waitFor(() => expect(transport.applyAction).toHaveBeenCalledOnce());
		const request = transport.applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing Output request");
		const outcome = changedOutcome(
			request.requestId,
			outputProjection({ revision: 2, blackout: true }),
		);
		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: { projection: outcome.projection },
			}),
		);
		response.resolve(outcome);
		await act(async () => response.promise);
		expect(current.store.getSnapshot().pendingRequestIds).toEqual([]);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 12,
				correlationId: null,
				change: {
					projection: outputProjection({
						revision: 3,
						grandMaster: 0.65,
						blackout: false,
					}),
				},
			}),
		);
		expect(screen.getByText("65%")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "BLACKOUT" })).toBeEnabled();
	});

	it("rolls back rejection, repairs conflict, and settles replayed no-change", async () => {
		const transport = new FakeOutputRuntimeTransport();
		const rejection = deferred<ReturnType<typeof changedOutcome>>();
		transport.applyAction.mockReturnValueOnce(rejection.promise);
		const current = harness(transport);
		render(current.view);
		await waitFor(() => expect(outputControls().blackout).toBeEnabled());

		fireEvent.click(outputControls().blackout);
		expect(
			screen.getByRole("button", { name: "RELEASE BLACKOUT" }),
		).toBeInTheDocument();
		rejection.reject(
			new OutputRuntimeTransportError("rejected", "invalid", 400, null, false),
		);
		await waitFor(() =>
			expect(screen.getByRole("button", { name: "BLACKOUT" })).toBeEnabled(),
		);

		transport.loadSnapshot.mockResolvedValueOnce(
			outputSnapshot({ cursor: 20, revision: 2, grandMaster: 0.7 }),
		);
		transport.applyAction.mockRejectedValueOnce(
			new OutputRuntimeTransportError(
				"revision conflict",
				"conflict",
				409,
				2,
				false,
			),
		);
		fireEvent.click(outputControls().blackout);
		await waitFor(() => expect(screen.getByText("70%")).toBeInTheDocument());
		expect(current.store.getSnapshot()).toMatchObject({
			authorityRevision: 2,
			pendingRequestIds: [],
		});

		transport.applyAction.mockImplementationOnce(async (_scope, request) =>
			noChangeOutcome(
				request.requestId,
				outputProjection({ revision: 2, grandMaster: 0.7 }),
				true,
			),
		);
		fireEvent.input(outputControls().slider, { target: { value: "70" } });
		await waitFor(() =>
			expect(current.store.getSnapshot().pendingRequestIds).toEqual([]),
		);
		expect(screen.getByText("70%")).toBeInTheDocument();
	});

	it("drops a late response after session authority replacement", async () => {
		const store = new OutputRuntimeStore();
		const first = new FakeOutputRuntimeTransport();
		const second = new FakeOutputRuntimeTransport();
		const response = deferred<ReturnType<typeof changedOutcome>>();
		first.applyAction.mockReturnValueOnce(response.promise);
		const rendered = render(harness(first, store).view);
		await waitFor(() => expect(outputControls().blackout).toBeEnabled());
		fireEvent.click(outputControls().blackout);
		await waitFor(() => expect(first.applyAction).toHaveBeenCalledOnce());
		const request = first.applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing Output request");

		rendered.rerender(harness(second, store, "session-b").view);
		await waitFor(() =>
			expect(store.getSnapshot().authorityKey).toBe("session-b"),
		);
		response.resolve(
			changedOutcome(
				request.requestId,
				outputProjection({ revision: 2, blackout: true }),
			),
		);
		await act(async () => response.promise);

		expect(store.getSnapshot()).toMatchObject({
			authorityKey: "session-b",
			authorityRevision: 1,
			projection: { blackout: false },
		});
		expect(screen.getByRole("button", { name: "BLACKOUT" })).toBeEnabled();
	});
});
