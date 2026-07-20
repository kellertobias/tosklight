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
import { useProgrammerValueTargets } from "./useProgrammerValueTargets";

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((settle) => {
		resolve = settle;
	});
	return { promise, resolve };
}

function harness(
	overrides: {
		capture?: () => Promise<ReturnType<typeof captureModeSnapshot>>;
		values?: () => Promise<ReturnType<typeof valuesSnapshot>>;
		preload?: () => Promise<ReturnType<typeof preloadSnapshot>>;
	} = {},
) {
	return {
		captureStore: new ProgrammerCaptureModeStore(),
		captureTransport: new FakeProgrammerCaptureModeTransport(),
		loadCapture: vi.fn(
			overrides.capture ?? (async () => captureModeSnapshot()),
		),
		valuesStore: new ProgrammerValuesStore(),
		valuesTransport: new FakeProgrammerValuesTransport(),
		loadValues: vi.fn(overrides.values ?? (async () => valuesSnapshot())),
		preloadStore: new ProgrammerPreloadValuesStore(),
		preloadTransport: new FakeProgrammerPreloadValuesTransport(),
		loadPreload: vi.fn(overrides.preload ?? (async () => preloadSnapshot())),
	};
}

type Harness = ReturnType<typeof harness>;

function Providers({
	state,
	children,
}: {
	state: Harness;
	children: ReactNode;
}) {
	return (
		<ProgrammerCaptureModeViewProvider
			showId={SHOW_ID}
			userId={USER_ID}
			store={state.captureStore}
			transport={state.captureTransport}
			loadSnapshot={state.loadCapture}
		>
			<ProgrammerValuesViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={state.valuesStore}
				transport={state.valuesTransport}
				loadSnapshot={state.loadValues}
			>
				<ProgrammerPreloadValuesViewProvider
					showId={SHOW_ID}
					userId={USER_ID}
					store={state.preloadStore}
					transport={state.preloadTransport}
					loadSnapshot={state.loadPreload}
				>
					{children}
				</ProgrammerPreloadValuesViewProvider>
			</ProgrammerValuesViewProvider>
		</ProgrammerCaptureModeViewProvider>
	);
}

function Probe({
	enabled = true,
	rendered,
}: {
	enabled?: boolean;
	rendered?: () => void;
}) {
	const targets = useProgrammerValueTargets(enabled);
	rendered?.();
	return (
		<output data-testid="targets">
			{targets
				? `${targets.fixtureIds.join(",")}|${targets.groupIds.join(",")}`
				: "loading"}
		</output>
	);
}

afterEach(cleanup);

describe("Programmer value targets", () => {
	it("stays dormant when disabled and resolves capture before normal targets", async () => {
		const capture = deferred<ReturnType<typeof captureModeSnapshot>>();
		const state = harness({
			capture: () => capture.promise,
			values: async () =>
				valuesSnapshot({
					fixtureValues: [
						fixtureValue(),
						fixtureValue(0.5, { attribute: "pan" }),
					],
					groupValues: [groupValue(), groupValue(0.2, { attribute: "pan" })],
				}),
		});
		const view = render(
			<Providers state={state}>
				<Probe enabled={false} />
			</Providers>,
		);
		expect(screen.getByTestId("targets")).toHaveTextContent("loading");
		expect(state.loadCapture).not.toHaveBeenCalled();
		expect(state.loadValues).not.toHaveBeenCalled();

		view.rerender(
			<Providers state={state}>
				<Probe />
			</Providers>,
		);
		await waitFor(() => expect(state.loadCapture).toHaveBeenCalledOnce());
		expect(state.loadValues).not.toHaveBeenCalled();
		capture.resolve(captureModeSnapshot());

		await waitFor(() =>
			expect(screen.getByTestId("targets")).toHaveTextContent(
				"11111111-1111-4111-8111-111111111111|front",
			),
		);
		expect(state.loadValues).toHaveBeenCalledOnce();
		expect(state.loadPreload).not.toHaveBeenCalled();
	});

	it("suppresses value-only renders while publishing target membership changes", async () => {
		const state = harness();
		const rendered = vi.fn();
		render(
			<Providers state={state}>
				<Probe rendered={rendered} />
			</Providers>,
		);
		await waitFor(() =>
			expect(screen.getByTestId("targets")).not.toHaveTextContent("loading"),
		);
		const renderCount = rendered.mock.calls.length;

		act(() =>
			state.valuesTransport.emit({
				type: "event",
				sequence: 11,
				correlationId: "same-target",
				projection: valuesProjection({
					revision: 2,
					fixtureValues: [fixtureValue(0.8, { attribute: "pan" })],
				}),
			}),
		);
		expect(state.valuesStore.getSnapshot().projection?.revision).toBe(2);
		expect(rendered).toHaveBeenCalledTimes(renderCount);

		act(() =>
			state.valuesTransport.emit({
				type: "event",
				sequence: 12,
				correlationId: "new-target",
				projection: valuesProjection({
					revision: 3,
					fixtureValues: [
						fixtureValue(0.8, { attribute: "pan" }),
						fixtureValue(0.4, { fixtureId: FIXTURE_2 }),
					],
				}),
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("targets")).toHaveTextContent(FIXTURE_2),
		);
		expect(rendered).toHaveBeenCalledTimes(renderCount + 1);
	});

	it("hides normal targets until the newly active pending authority hydrates", async () => {
		const pending = deferred<ReturnType<typeof preloadSnapshot>>();
		const state = harness({ preload: () => pending.promise });
		const view = render(
			<Providers state={state}>
				<Probe />
			</Providers>,
		);
		await waitFor(() =>
			expect(screen.getByTestId("targets")).not.toHaveTextContent("loading"),
		);

		act(() =>
			state.captureTransport.emit({
				type: "event",
				sequence: 11,
				correlationId: "preload",
				projection: captureModeProjection({
					revision: 2,
					blind: true,
					preloadCaptureProgrammer: true,
				}),
			}),
		);
		await waitFor(() => expect(state.loadPreload).toHaveBeenCalledOnce());
		expect(screen.getByTestId("targets")).toHaveTextContent("loading");
		pending.resolve(
			preloadSnapshot({
				fixtureValues: [preloadFixtureValue(0.4, { fixtureId: FIXTURE_2 })],
				groupValues: [preloadGroupValue(0.2, { groupId: "rear" })],
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("targets")).toHaveTextContent(
				`${FIXTURE_2}|rear`,
			),
		);

		view.unmount();
		await waitFor(() =>
			expect(
				state.captureTransport.subscriptions[0]?.close,
			).toHaveBeenCalledOnce(),
		);
		expect(
			state.preloadTransport.subscriptions[0]?.close,
		).toHaveBeenCalledOnce();
	});
});
