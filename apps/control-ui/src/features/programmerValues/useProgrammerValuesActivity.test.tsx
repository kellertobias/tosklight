import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeViewProvider } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import {
	captureModeProjection,
	captureModeSnapshot,
	FakeProgrammerCaptureModeTransport,
} from "../programmerCaptureMode/testFixtures";
import { ProgrammerPreloadValuesViewProvider } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import { ProgrammerPreloadValuesStore } from "../programmerPreloadValues/store";
import {
	FakeProgrammerPreloadValuesTransport,
	preloadFixtureValue,
	preloadGroupValue,
	preloadSnapshot,
} from "../programmerPreloadValues/testFixtures";
import { ProgrammerValuesViewProvider } from "./ProgrammerValuesView";
import { ProgrammerValuesStore } from "./store";
import {
	FakeProgrammerValuesTransport,
	FIXTURE_2,
	fixtureValue,
	groupValue,
	SHOW_ID,
	USER_ID,
	valuesProjection,
	valuesSnapshot,
} from "./testFixtures";
import {
	useNormalProgrammerValueCount,
	useProgrammerValuesActivity,
} from "./useProgrammerValuesActivity";

type CaptureSnapshot = ReturnType<typeof captureModeSnapshot>;
type ValuesSnapshot = ReturnType<typeof valuesSnapshot>;
type PreloadSnapshot = ReturnType<typeof preloadSnapshot>;

interface Loaders {
	capture?: () => Promise<CaptureSnapshot>;
	values?: () => Promise<ValuesSnapshot>;
	preload?: () => Promise<PreloadSnapshot>;
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

function createHarness(loaders: Loaders = {}) {
	return {
		captureStore: new ProgrammerCaptureModeStore(),
		captureTransport: new FakeProgrammerCaptureModeTransport(),
		loadCapture: vi.fn(
			loaders.capture ?? (async () => captureModeSnapshot()),
		),
		valuesStore: new ProgrammerValuesStore(),
		valuesTransport: new FakeProgrammerValuesTransport(),
		loadValues: vi.fn(loaders.values ?? (async () => valuesSnapshot())),
		preloadStore: new ProgrammerPreloadValuesStore(),
		preloadTransport: new FakeProgrammerPreloadValuesTransport(),
		loadPreload: vi.fn(loaders.preload ?? (async () => preloadSnapshot())),
	};
}

type Harness = ReturnType<typeof createHarness>;

function providers(harness: Harness, children: ReactNode) {
	return (
		<ProgrammerCaptureModeViewProvider
			showId={SHOW_ID}
			userId={USER_ID}
			authorityKey="session-a"
			store={harness.captureStore}
			transport={harness.captureTransport}
			loadSnapshot={harness.loadCapture}
		>
			<ProgrammerValuesViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				authorityKey="session-a"
				store={harness.valuesStore}
				transport={harness.valuesTransport}
				loadSnapshot={harness.loadValues}
			>
				<ProgrammerPreloadValuesViewProvider
					showId={SHOW_ID}
					userId={USER_ID}
					authorityKey="session-a"
					store={harness.preloadStore}
					transport={harness.preloadTransport}
					loadSnapshot={harness.loadPreload}
				>
					{children}
				</ProgrammerPreloadValuesViewProvider>
			</ProgrammerValuesViewProvider>
		</ProgrammerCaptureModeViewProvider>
	);
}

function ActivityProbe({
	enabled = true,
	onRender,
}: {
	enabled?: boolean;
	onRender?: () => void;
}) {
	const activity = useProgrammerValuesActivity(enabled);
	onRender?.();
	return (
		<output data-testid="activity">
			<span data-testid="authority">{activity.authority}</span>
			<span data-testid="ready">{String(activity.ready)}</span>
			<span data-testid="value-count">{activity.valueCount}</span>
			<span data-testid="pending-count">{activity.pendingValueCount}</span>
		</output>
	);
}

function NormalCountProbe({ onRender }: { onRender: () => void }) {
	const count = useNormalProgrammerValueCount();
	onRender();
	return <output data-testid="normal-count">{count ?? "loading"}</output>;
}

function expectActivity(
	authority: "loading" | "normal" | "preload",
	ready: boolean,
	valueCount: number,
	pendingCount: number,
) {
	expect(screen.getByTestId("authority")).toHaveTextContent(authority);
	expect(screen.getByTestId("ready")).toHaveTextContent(String(ready));
	expect(screen.getByTestId("value-count")).toHaveTextContent(
		String(valueCount),
	);
	expect(screen.getByTestId("pending-count")).toHaveTextContent(
		String(pendingCount),
	);
}

afterEach(cleanup);

describe("Programmer values activity", () => {
	it("stays dormant until enabled and resolves capture before normal authority", async () => {
		const capture = deferred<CaptureSnapshot>();
		const harness = createHarness({
			capture: () => capture.promise,
			values: async () =>
				valuesSnapshot({
					fixtureValues: [fixtureValue(), fixtureValue(0.5, { fixtureId: FIXTURE_2 })],
					groupValues: [groupValue()],
				}),
		});
		const rendered = render(
			providers(harness, <ActivityProbe enabled={false} />),
		);

		await act(async () => Promise.resolve());
		expectActivity("loading", false, 0, 0);
		expect(harness.loadCapture).not.toHaveBeenCalled();
		expect(harness.loadValues).not.toHaveBeenCalled();
		expect(harness.loadPreload).not.toHaveBeenCalled();

		rendered.rerender(providers(harness, <ActivityProbe />));
		await waitFor(() => expect(harness.loadCapture).toHaveBeenCalledOnce());
		expect(harness.loadValues).not.toHaveBeenCalled();
		expect(harness.loadPreload).not.toHaveBeenCalled();
		expect(harness.valuesTransport.subscriptions).toHaveLength(0);
		expect(harness.preloadTransport.subscriptions).toHaveLength(0);

		capture.resolve(captureModeSnapshot());
		await waitFor(() => expectActivity("normal", true, 3, 0));
		expect(harness.loadValues).toHaveBeenCalledOnce();
		expect(harness.loadPreload).not.toHaveBeenCalled();
		expect(harness.captureTransport.subscriptions).toHaveLength(1);
		expect(harness.valuesTransport.subscriptions).toHaveLength(1);
		expect(harness.preloadTransport.subscriptions).toHaveLength(0);

		rendered.unmount();
		await waitFor(() =>
			expect(
				harness.captureTransport.subscriptions[0]?.close,
			).toHaveBeenCalledOnce(),
		);
		expect(
			harness.valuesTransport.subscriptions[0]?.close,
		).toHaveBeenCalledOnce();
	});

	it("activates only pending values while Preload capture is authoritative", async () => {
		const harness = createHarness({
			capture: async () =>
				captureModeSnapshot({
					blind: true,
					preloadCaptureProgrammer: true,
				}),
			preload: async () =>
				preloadSnapshot({
					fixtureValues: [preloadFixtureValue()],
					groupValues: [preloadGroupValue()],
				}),
		});
		const rendered = render(providers(harness, <ActivityProbe />));

		await waitFor(() => expectActivity("preload", true, 2, 2));
		expect(harness.loadValues).not.toHaveBeenCalled();
		expect(harness.valuesTransport.subscriptions).toHaveLength(0);
		expect(harness.loadPreload).toHaveBeenCalledOnce();
		expect(harness.preloadTransport.subscriptions).toHaveLength(1);

		rendered.unmount();
		await waitFor(() =>
			expect(
				harness.captureTransport.subscriptions[0]?.close,
			).toHaveBeenCalledOnce(),
		);
		expect(
			harness.preloadTransport.subscriptions[0]?.close,
		).toHaveBeenCalledOnce();
	});

	it("hides the old count while a capture-route replacement hydrates", async () => {
		const pending = deferred<PreloadSnapshot>();
		const harness = createHarness({
			values: async () =>
				valuesSnapshot({
					fixtureValues: [fixtureValue()],
					groupValues: [groupValue()],
				}),
			preload: () => pending.promise,
		});
		render(providers(harness, <ActivityProbe />));
		await waitFor(() => expectActivity("normal", true, 2, 0));

		act(() =>
			harness.captureTransport.emit({
				type: "event",
				sequence: 11,
				correlationId: "activate-preload",
				projection: captureModeProjection({
					revision: 2,
					blind: true,
					preloadCaptureProgrammer: true,
				}),
			}),
		);

		await waitFor(() => expect(harness.loadPreload).toHaveBeenCalledOnce());
		expectActivity("preload", false, 0, 0);
		await waitFor(() =>
			expect(
				harness.valuesTransport.subscriptions[0]?.close,
			).toHaveBeenCalledOnce(),
		);

		pending.resolve(
			preloadSnapshot({
				fixtureValues: [
					preloadFixtureValue(),
					preloadFixtureValue(0.75, { fixtureId: FIXTURE_2 }),
				],
				groupValues: [preloadGroupValue()],
			}),
		);
		await waitFor(() => expectActivity("preload", true, 3, 3));
	});

	it("selects the normal count as a scalar and suppresses same-count renders", async () => {
		const harness = createHarness({
			values: async () =>
				valuesSnapshot({
					fixtureValues: [fixtureValue()],
					groupValues: [groupValue()],
				}),
		});
		const onRender = vi.fn();
		render(providers(harness, <NormalCountProbe onRender={onRender} />));
		await waitFor(() =>
			expect(screen.getByTestId("normal-count")).toHaveTextContent("2"),
		);
		await waitFor(() =>
			expect(harness.captureTransport.subscriptions).toHaveLength(1),
		);
		const renderCount = onRender.mock.calls.length;

		act(() =>
			harness.valuesTransport.emit({
				type: "event",
				sequence: 11,
				correlationId: "same-count",
				projection: valuesProjection({
					revision: 2,
					fixtureValues: [
						fixtureValue(0.5),
						fixtureValue(0.75, { fixtureId: FIXTURE_2 }),
					],
					groupValues: [],
				}),
			}),
		);
		expect(harness.valuesStore.getSnapshot().projection?.revision).toBe(2);
		expect(onRender).toHaveBeenCalledTimes(renderCount);

		act(() =>
			harness.valuesTransport.emit({
				type: "event",
				sequence: 12,
				correlationId: "changed-count",
				projection: valuesProjection({
					revision: 3,
					fixtureValues: [
						fixtureValue(0.5),
						fixtureValue(0.75, { fixtureId: FIXTURE_2 }),
					],
					groupValues: [groupValue()],
				}),
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("normal-count")).toHaveTextContent("3"),
		);
		expect(onRender).toHaveBeenCalledTimes(renderCount + 1);
		expect(harness.loadPreload).not.toHaveBeenCalled();
	});
});
