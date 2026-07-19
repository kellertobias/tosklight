import { act, render, screen, waitFor } from "@testing-library/react";
import { useCallback } from "react";
import { describe, expect, it, vi } from "vitest";
import { ProgrammerCaptureModeViewProvider } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { ProgrammerCaptureModeStore } from "../programmerCaptureMode/store";
import {
	captureModeProjection,
	captureModeSnapshot,
	FakeProgrammerCaptureModeTransport,
} from "../programmerCaptureMode/testFixtures";
import type { ProgrammerValuesActions } from "./contracts";
import {
	ProgrammerValuesViewProvider,
	useProgrammerValuesActions,
	useProgrammerValuesSelector,
	useProgrammerValuesView,
} from "./ProgrammerValuesView";
import { ProgrammerValuesStore } from "./store";
import {
	FakeProgrammerValuesTransport,
	fixtureValue,
	groupValue,
	SHOW_ID,
	USER_ID,
	valuesProjection,
	valuesSnapshot,
} from "./testFixtures";

function ProjectionProbe({
	enabled,
	onRender,
}: {
	enabled: boolean;
	onRender: () => void;
}) {
	onRender();
	const projection = useProgrammerValuesView(enabled);
	return (
		<span>{enabled ? (projection?.revision ?? "Loading") : "Hidden"}</span>
	);
}

function FixtureLevelProbe({ onRender }: { onRender: () => void }) {
	onRender();
	const selector = useCallback(
		(state: ReturnType<ProgrammerValuesStore["getSnapshot"]>) => {
			const value = state.projection?.fixtureValues[0]?.value;
			return value?.kind === "normalized" ? value.value : null;
		},
		[],
	);
	const level = useProgrammerValuesSelector(selector);
	return <span>{level ?? "Loading level"}</span>;
}

function ActionProbe({ onRender }: { onRender: () => void }) {
	onRender();
	const actions = useProgrammerValuesActions();
	return <span>{actions ? "Actions ready" : "No actions"}</span>;
}

function WriterIdentityProbe({
	onWriter,
}: {
	onWriter: (actions: ProgrammerValuesActions | null) => void;
}) {
	useProgrammerValuesView();
	const actions = useProgrammerValuesActions();
	onWriter(actions);
	return <span>{actions ? "Writer ready" : "No writer"}</span>;
}

function actions(): ProgrammerValuesActions {
	return {
		setFixtureValue: vi.fn(async () => null),
		releaseFixtureValue: vi.fn(async () => null),
		setGroupValue: vi.fn(async () => null),
		releaseGroupValue: vi.fn(async () => null),
		batch: vi.fn(async () => null),
		clear: vi.fn(async () => null),
	};
}

describe("ProgrammerValuesViewProvider", () => {
	it("keeps an action-only provider dormant", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const loadSnapshot = vi.fn(async () => valuesSnapshot());
		render(
			<ProgrammerValuesViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
				actions={actions()}
			>
				<ActionProbe onRender={vi.fn()} />
			</ProgrammerValuesViewProvider>,
		);

		expect(screen.getByText("Actions ready")).toBeInTheDocument();
		await act(async () => Promise.resolve());
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("subscribes only while a values view is active", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const loadSnapshot = vi.fn(async () => valuesSnapshot());
		const captureModeStore = new ProgrammerCaptureModeStore();
		const captureModeTransport = new FakeProgrammerCaptureModeTransport();
		const loadCaptureModeSnapshot = vi.fn(async () => captureModeSnapshot());
		const onRender = vi.fn();
		const view = (enabled: boolean) => (
			<ProgrammerCaptureModeViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={captureModeStore}
				transport={captureModeTransport}
				loadSnapshot={loadCaptureModeSnapshot}
			>
				<ProgrammerValuesViewProvider
					showId={SHOW_ID}
					userId={USER_ID}
					store={store}
					transport={transport}
					loadSnapshot={loadSnapshot}
				>
					<ProjectionProbe enabled={enabled} onRender={onRender} />
				</ProgrammerValuesViewProvider>
			</ProgrammerCaptureModeViewProvider>
		);
		const rendered = render(view(false));

		expect(screen.getByText("Hidden")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(loadCaptureModeSnapshot).not.toHaveBeenCalled();

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		await waitFor(() =>
			expect(captureModeTransport.subscriptions).toHaveLength(1),
		);
		await waitFor(() => expect(screen.getByText("1")).toBeInTheDocument());
		expect(transport.subscriptions[0].scope).toEqual({
			showId: SHOW_ID,
			userId: USER_ID,
		});

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
		expect(captureModeTransport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(screen.getByText("Hidden")).toBeInTheDocument();
	});

	it("suppresses renders for unchanged selected data and action context", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const levelRenders = vi.fn();
		const actionRenders = vi.fn();
		const injectedActions = actions();
		render(
			<ProgrammerValuesViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={store}
				transport={transport}
				loadSnapshot={async () => valuesSnapshot()}
				actions={injectedActions}
			>
				<FixtureLevelProbe onRender={levelRenders} />
				<ActionProbe onRender={actionRenders} />
			</ProgrammerValuesViewProvider>,
		);
		await waitFor(() => expect(screen.getByText("0.25")).toBeInTheDocument());
		const levelCount = levelRenders.mock.calls.length;
		const actionCount = actionRenders.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: "osc-group-value",
				projection: valuesProjection({
					revision: 2,
					fixtureValues: [fixtureValue(0.25)],
					groupValues: [groupValue(0.8)],
				}),
			}),
		);

		expect(levelRenders).toHaveBeenCalledTimes(levelCount);
		expect(actionRenders).toHaveBeenCalledTimes(actionCount);
	});

	it("keeps the writer and values subscription stable across capture events", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const captureModeStore = new ProgrammerCaptureModeStore();
		const captureModeTransport = new FakeProgrammerCaptureModeTransport();
		const onWriter = vi.fn();
		const applyAction = vi.fn();
		const loadSnapshot = vi.fn(async () => valuesSnapshot());
		const loadCaptureModeSnapshot = vi.fn(async () => captureModeSnapshot());
		const tree = () => (
			<ProgrammerCaptureModeViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				store={captureModeStore}
				transport={captureModeTransport}
				loadSnapshot={loadCaptureModeSnapshot}
			>
				<ProgrammerValuesViewProvider
					showId={SHOW_ID}
					userId={USER_ID}
					store={store}
					transport={transport}
					loadSnapshot={loadSnapshot}
					applyAction={applyAction}
				>
					<WriterIdentityProbe onWriter={onWriter} />
				</ProgrammerValuesViewProvider>
			</ProgrammerCaptureModeViewProvider>
		);
		const rendered = render(tree());
		await waitFor(() =>
			expect(screen.getByText("Writer ready")).toBeInTheDocument(),
		);
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		const writer = onWriter.mock.calls.at(-1)?.[0];

		act(() =>
			captureModeTransport.emit({
				type: "event",
				sequence: 11,
				correlationId: "preview-on",
				projection: captureModeProjection({ revision: 2, preview: true }),
			}),
		);
		rendered.rerender(tree());

		expect(onWriter.mock.calls.at(-1)?.[0]).toBe(writer);
		expect(transport.subscriptions).toHaveLength(1);
		expect(transport.subscriptions[0].close).not.toHaveBeenCalled();
	});

	it("replaces authority when the server session key changes", async () => {
		const store = new ProgrammerValuesStore();
		const transport = new FakeProgrammerValuesTransport();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(valuesSnapshot())
			.mockResolvedValueOnce(valuesSnapshot({ cursor: 1, revision: 7 }));
		const view = (authorityKey: string) => (
			<ProgrammerValuesViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				authorityKey={authorityKey}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<ProjectionProbe enabled onRender={vi.fn()} />
			</ProgrammerValuesViewProvider>
		);
		const rendered = render(view("session-a"));
		await waitFor(() => expect(screen.getByText("1")).toBeInTheDocument());

		rendered.rerender(view("session-b"));
		await waitFor(() => expect(screen.getByText("7")).toBeInTheDocument());

		expect(loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions).toHaveLength(2);
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1].after).toBe(1);
	});
});
