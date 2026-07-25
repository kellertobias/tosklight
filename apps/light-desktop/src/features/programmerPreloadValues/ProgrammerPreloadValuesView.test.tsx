import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeViewProvider } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import {
	captureModeProjection,
	captureModeSnapshot,
} from "../programmerCaptureMode/testFixtures";
import type {
	ProgrammerPreloadValuesActionRequest,
	ProgrammerPreloadValuesActions,
	ProgrammerPreloadValuesScope,
} from "./contracts";
import {
	ProgrammerPreloadValuesViewProvider,
	useProgrammerPreloadValuesActions,
	useProgrammerPreloadValuesView,
} from "./ProgrammerPreloadValuesView";
import { ProgrammerPreloadValuesStore } from "./store";
import {
	FakeProgrammerPreloadValuesTransport,
	preloadSnapshot,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";

function ProjectionProbe() {
	const projection = useProgrammerPreloadValuesView();
	return (
		<span>{projection ? `Revision ${projection.revision}` : "Inactive"}</span>
	);
}

function ActionProbe({
	onActions,
}: {
	onActions?: (actions: ProgrammerPreloadValuesActions | null) => void;
}) {
	const actions = useProgrammerPreloadValuesActions();
	onActions?.(actions);
	return <span>{actions ? "Actions ready" : "No actions"}</span>;
}

function readyCaptureStore(active: boolean) {
	const store = new ProgrammerCaptureModeStore();
	store.reset(SHOW_ID, USER_ID, "session-a");
	store.installSnapshot(
		captureModeSnapshot({
			blind: active,
			preloadCaptureProgrammer: active,
		}),
	);
	return store;
}

function providers({
	children,
	captureModeStore,
	preloadStore,
	transport,
	loadSnapshot,
	applyAction,
	enabled = true,
}: {
	children: React.ReactNode;
	captureModeStore: ProgrammerCaptureModeStore;
	preloadStore: ProgrammerPreloadValuesStore;
	transport: FakeProgrammerPreloadValuesTransport;
	loadSnapshot: () => Promise<ReturnType<typeof preloadSnapshot>>;
	applyAction?: (
		scope: ProgrammerPreloadValuesScope,
		request: ProgrammerPreloadValuesActionRequest,
	) => Promise<ReturnType<typeof noChange>>;
	enabled?: boolean;
}) {
	return (
		<ProgrammerCaptureModeViewProvider
			showId={SHOW_ID}
			userId={USER_ID}
			authorityKey="session-a"
			store={captureModeStore}
			transport={null}
			loadSnapshot={async () => captureModeSnapshot()}
		>
			<ProgrammerPreloadValuesViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				authorityKey="session-a"
				enabled={enabled}
				store={preloadStore}
				transport={transport}
				loadSnapshot={loadSnapshot}
				applyAction={applyAction}
			>
				{children}
			</ProgrammerPreloadValuesViewProvider>
		</ProgrammerCaptureModeViewProvider>
	);
}

function noChange(requestId: string) {
	return {
		requestId,
		correlationId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
		status: "no_change" as const,
		preloadRevision: 1,
		captureModeRevision: 1,
		replayed: false,
		warning: null,
	};
}

afterEach(cleanup);

describe("ProgrammerPreloadValuesViewProvider", () => {
	it("keeps an action-only provider dormant", async () => {
		const captureModeStore = readyCaptureStore(true);
		const preloadStore = new ProgrammerPreloadValuesStore();
		const transport = new FakeProgrammerPreloadValuesTransport();
		const loadSnapshot = vi.fn(async () => preloadSnapshot());
		const actions: ProgrammerPreloadValuesActions = {
			setFixtureValue: vi.fn(async () => null),
			releaseFixtureValue: vi.fn(async () => null),
			setGroupValue: vi.fn(async () => null),
			releaseGroupValue: vi.fn(async () => null),
			batch: vi.fn(async () => null),
		};

		render(
			<ProgrammerCaptureModeViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				authorityKey="session-a"
				store={captureModeStore}
				transport={null}
				loadSnapshot={async () => captureModeSnapshot()}
			>
				<ProgrammerPreloadValuesViewProvider
					showId={SHOW_ID}
					userId={USER_ID}
					authorityKey="session-a"
					store={preloadStore}
					transport={transport}
					loadSnapshot={loadSnapshot}
					actions={actions}
				>
					<ActionProbe />
				</ProgrammerPreloadValuesViewProvider>
			</ProgrammerCaptureModeViewProvider>,
		);

		expect(screen.getByText("Actions ready")).toBeInTheDocument();
		await act(async () => Promise.resolve());
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("opens authority only while exact-user Preload capture is active", async () => {
		const captureModeStore = readyCaptureStore(false);
		const preloadStore = new ProgrammerPreloadValuesStore();
		const transport = new FakeProgrammerPreloadValuesTransport();
		const loadSnapshot = vi.fn(async () => preloadSnapshot());
		render(
			providers({
				children: (
					<>
						<ProjectionProbe />
						<ActionProbe />
					</>
				),
				captureModeStore,
				preloadStore,
				transport,
				loadSnapshot,
			}),
		);

		expect(screen.getByText("Inactive")).toBeInTheDocument();
		expect(screen.getByText("No actions")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		act(() => {
			captureModeStore.applyProjection(
				captureModeProjection({
					revision: 2,
					blind: true,
					preloadCaptureProgrammer: true,
				}),
				11,
			);
		});
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		await waitFor(() =>
			expect(screen.getByText("Revision 1")).toBeInTheDocument(),
		);
		expect(transport.subscriptions[0]?.scope).toEqual({
			showId: SHOW_ID,
			userId: USER_ID,
		});

		act(() => {
			captureModeStore.applyProjection(
				captureModeProjection({
					revision: 3,
					blind: false,
					preloadCaptureProgrammer: false,
				}),
				12,
			);
		});
		await waitFor(() =>
			expect(transport.subscriptions[0]?.close).toHaveBeenCalledOnce(),
		);
		expect(screen.getByText("Inactive")).toBeInTheDocument();
		expect(screen.getByText("No actions")).toBeInTheDocument();
	});

	it("respects a disabled composition gate even when capture is active", async () => {
		const captureModeStore = readyCaptureStore(true);
		const preloadStore = new ProgrammerPreloadValuesStore();
		const transport = new FakeProgrammerPreloadValuesTransport();
		const loadSnapshot = vi.fn(async () => preloadSnapshot());
		render(
			providers({
				children: <ProjectionProbe />,
				captureModeStore,
				preloadStore,
				transport,
				loadSnapshot,
				enabled: false,
			}),
		);

		await act(async () => Promise.resolve());
		expect(screen.getByText("Inactive")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("keeps its session and writer live through StrictMode replay", async () => {
		const captureModeStore = readyCaptureStore(true);
		const preloadStore = new ProgrammerPreloadValuesStore();
		const transport = new FakeProgrammerPreloadValuesTransport();
		const applyAction = vi.fn(
			async (
				_scope: ProgrammerPreloadValuesScope,
				request: ProgrammerPreloadValuesActionRequest,
			) => noChange(request.requestId),
		);
		const onActions = vi.fn();

		render(
			<StrictMode>
				{providers({
					children: (
						<>
							<ProjectionProbe />
							<ActionProbe onActions={onActions} />
						</>
					),
					captureModeStore,
					preloadStore,
					transport,
					loadSnapshot: async () => preloadSnapshot(),
					applyAction,
				})}
			</StrictMode>,
		);

		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		await waitFor(() =>
			expect(screen.getByText("Revision 1")).toBeInTheDocument(),
		);
		const writer = onActions.mock.calls.at(
			-1,
		)?.[0] as ProgrammerPreloadValuesActions | null;
		expect(writer).not.toBeNull();
		await expect(
			writer?.releaseFixtureValue({
				requestId: "strict-write",
				fixtureId: "11111111-1111-4111-8111-111111111111",
				attribute: "intensity",
			}),
		).resolves.toMatchObject({ status: "no_change" });
		expect(applyAction).toHaveBeenCalledOnce();
	});
});
