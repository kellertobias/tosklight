import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ProgrammerLifecycleRow } from "../programmerLifecycle/contracts";
import { ProgrammerLifecycleViewProvider } from "../programmerLifecycle/ProgrammerLifecycleView";
import { ProgrammerLifecycleStore } from "../programmerLifecycle/store";
import { FakeProgrammerLifecycleTransport } from "../programmerLifecycle/testFixtures";
import {
	ProgrammerPreloadLifecycleProvider,
	useProgrammerPreloadLifecycleView,
} from "./ProgrammerPreloadLifecycleView";
import {
	lifecycleWriterHarness,
	OTHER_ID,
	queue,
	SHOW_ID,
	USER_ID,
	values,
} from "./writerTestHarness";

const authorities = vi.hoisted(() => ({
	current: {} as Record<string, unknown>,
}));

vi.mock("../programmerCaptureMode/ProgrammerCaptureModeView", () => ({
	useProgrammerCaptureModeAuthority: () => authorities.current.capture,
}));
vi.mock(
	"../programmerPreloadValues/ProgrammerPreloadValuesView",
	() => ({
		useProgrammerPreloadValuesAuthority: () => authorities.current.values,
	}),
);
vi.mock(
	"../programmerPreloadPlaybackQueue/ProgrammerPreloadPlaybackQueueView",
	() => ({
		useProgrammerPreloadPlaybackQueueAuthority: () => authorities.current.queue,
	}),
);
vi.mock("../programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionAuthority: () => authorities.current.selection,
}));
vi.mock("../playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackRuntimeAuthority: () => authorities.current.runtime,
}));

const LIFECYCLE_KEY = "server|connection|session|client";
const PRELOAD_KEY = `${LIFECYCLE_KEY}|${USER_ID}|desk`;
let renderCount = 0;

function lifecycleRow(
	userId = USER_ID,
	preloadActive = false,
): ProgrammerLifecycleRow & { preloadActive: boolean } {
	return {
		programmerId: userId === USER_ID ? USER_ID : OTHER_ID,
		userId,
		connected: true,
		selectedFixtureCount: 1,
		normalValueCount: 2,
		preloadActive,
		sessions: [],
	};
}

function lifecycleSnapshot(cursor = 14, revision = 1) {
	return {
		cursor,
		projection: { revision, programmers: [lifecycleRow()] },
	};
}

function Probe({ enabled }: { enabled: boolean }) {
	renderCount++;
	const view = useProgrammerPreloadLifecycleView(enabled);
	return (
		<output data-testid="preload-view">
			{JSON.stringify({ ready: view.ready, active: view.active })}
		</output>
	);
}

function createAuthority<T extends { subscribe: unknown }>(
	store: T,
	activate = vi.fn(() => vi.fn()),
) {
	return {
		store,
		activate,
		repairAuthority: vi.fn(async () => undefined),
	};
}

function viewHarness() {
	const stores = lifecycleWriterHarness();
	const lifecycleStore = new ProgrammerLifecycleStore();
	const lifecycleTransport = new FakeProgrammerLifecycleTransport();
	const loadLifecycle = vi.fn(async () => lifecycleSnapshot());
	const capture = createAuthority(stores.captureModeStore);
	const preloadValues = createAuthority(stores.valuesStore);
	const preloadQueue = createAuthority(stores.queueStore);
	const selection = createAuthority(stores.selectionStore);
	const activateDesk = vi.fn(() => vi.fn());
	authorities.current = {
		capture,
		values: preloadValues,
		queue: preloadQueue,
		selection,
		runtime: {
			store: stores.runtimeStore,
			activate: vi.fn(() => vi.fn()),
			activateDesk,
			repairAuthority: vi.fn(async () => undefined),
		},
	};
	const actionTransport = { applyAction: vi.fn() };
	return {
		...stores,
		lifecycleStore,
		lifecycleTransport,
		loadLifecycle,
		capture,
		preloadValues,
		preloadQueue,
		selection,
		activateDesk,
		actionTransport,
	};
}

interface TreeProps {
	enabled: boolean;
	lifecycleKey?: string;
	expectedLifecycleKey?: string;
}

function tree(setup: ReturnType<typeof viewHarness>, props: TreeProps) {
	const lifecycleKey = props.lifecycleKey ?? LIFECYCLE_KEY;
	return (
		<ProgrammerLifecycleViewProvider
			authorityKey={lifecycleKey}
			store={setup.lifecycleStore}
			transport={setup.lifecycleTransport}
			loadSnapshot={setup.loadLifecycle}
		>
			<ProgrammerPreloadLifecycleProvider
				showId={SHOW_ID}
				userId={USER_ID}
				deskId={setup.selectionStore.getSnapshot().deskId}
				authorityKey={PRELOAD_KEY}
				lifecycleAuthorityKey={
					props.expectedLifecycleKey ?? LIFECYCLE_KEY
				}
				showStore={setup.showStore}
				store={setup.localStore}
				transport={setup.actionTransport}
			>
				<Probe enabled={props.enabled} />
			</ProgrammerPreloadLifecycleProvider>
		</ProgrammerLifecycleViewProvider>
	);
}

afterEach(() => {
	cleanup();
	renderCount = 0;
	authorities.current = {};
});

describe("ProgrammerPreloadLifecycleView", () => {
	it("constructs dormant and activates exact non-Playback authority on demand", async () => {
		const setup = viewHarness();
		const rendered = render(tree(setup, { enabled: false }));

		expect(setup.loadLifecycle).not.toHaveBeenCalled();
		expect(setup.capture.activate).not.toHaveBeenCalled();
		expect(setup.preloadValues.activate).not.toHaveBeenCalled();
		expect(setup.preloadQueue.activate).not.toHaveBeenCalled();
		expect(setup.selection.activate).not.toHaveBeenCalled();
		expect(setup.activateDesk).not.toHaveBeenCalled();
		expect(setup.actionTransport.applyAction).not.toHaveBeenCalled();

		rendered.rerender(tree(setup, { enabled: true }));
		await waitFor(() =>
			expect(screen.getByTestId("preload-view")).toHaveTextContent(
				'"ready":true',
			),
		);
		expect(setup.loadLifecycle).toHaveBeenCalledOnce();
		expect(setup.capture.activate).toHaveBeenCalledOnce();
		expect(setup.preloadValues.activate).toHaveBeenCalledOnce();
		expect(setup.preloadQueue.activate).toHaveBeenCalledOnce();
		expect(setup.selection.activate).toHaveBeenCalledOnce();
		expect(setup.activateDesk).not.toHaveBeenCalled();
	});

	it("uses the lifecycle capability key and fails closed across replacement", async () => {
		const setup = viewHarness();
		const rendered = render(tree(setup, { enabled: true }));
		await waitFor(() =>
			expect(screen.getByTestId("preload-view")).toHaveTextContent(
				'"ready":true',
			),
		);

		rendered.rerender(
			tree(setup, {
				enabled: true,
				lifecycleKey: "replacement-lifecycle-key",
				expectedLifecycleKey: LIFECYCLE_KEY,
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("preload-view")).toHaveTextContent(
				'"ready":false',
			),
		);

		rendered.rerender(
			tree(setup, {
				enabled: true,
				lifecycleKey: "replacement-lifecycle-key",
				expectedLifecycleKey: "replacement-lifecycle-key",
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("preload-view")).toHaveTextContent(
				'"ready":true',
			),
		);
	});

	it("suppresses unrelated projection and foreign-user rerenders", async () => {
		const setup = viewHarness();
		render(tree(setup, { enabled: true }));
		await waitFor(() =>
			expect(screen.getByTestId("preload-view")).toHaveTextContent(
				'"ready":true',
			),
		);
		const settledRenders = renderCount;

		act(() => {
			setup.valuesStore.applyProjection(values(2), 20);
			setup.queueStore.applyProjection(queue(3), 21);
			setup.selectionStore.applyChange(
				{
					deskId: setup.selectionStore.getSnapshot().deskId!,
					selection: {
						...setup.selectionStore.getSnapshot().selection!,
						revision: 2,
					},
				},
				22,
			);
			setup.lifecycleStore.applyChange(
				{
					revision: 2,
					delta: { type: "upsert", programmer: lifecycleRow(OTHER_ID, true) },
				},
				23,
			);
		});
		expect(renderCount).toBe(settledRenders);
	});

	it("repairs a lifecycle cursor gap and remains exact-user ready", async () => {
		const setup = viewHarness();
		setup.loadLifecycle
			.mockResolvedValueOnce(lifecycleSnapshot())
			.mockResolvedValueOnce(lifecycleSnapshot(30, 1));
		render(tree(setup, { enabled: true }));
		await waitFor(() => expect(setup.loadLifecycle).toHaveBeenCalledOnce());

		act(() => {
			setup.lifecycleTransport.emit({
				type: "gap",
				afterSequence: 14,
				oldestAvailable: 20,
				latestSequence: 30,
			});
		});
		await waitFor(() => expect(setup.loadLifecycle).toHaveBeenCalledTimes(2));
		expect(
			setup.lifecycleTransport.subscriptions[0]?.repair,
		).toHaveBeenCalledWith(30);
		expect(screen.getByTestId("preload-view")).toHaveTextContent(
			'"ready":true',
		);
	});
});
