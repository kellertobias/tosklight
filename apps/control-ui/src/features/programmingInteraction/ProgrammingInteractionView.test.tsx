import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useLayoutEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { Button } from "../../components/common/controls";
import type {
	SelectionActionOutcome,
	SelectionActionRequest,
} from "./contracts";
import {
	ProgrammingInteractionViewProvider,
	useProgrammingCommandLineView,
	useProgrammingSelectionActions,
} from "./ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "./store";
import {
	commandChange,
	commandLine,
	DESK_ID,
	FakeProgrammingTransport,
	FIXTURE_1,
	FIXTURE_2,
	FIXTURE_3,
	OTHER_SHOW_ID,
	programmingSnapshot,
	selection,
	selectionChange,
	SHOW_ID,
} from "./testFixtures";

const server = vi.hoisted(() => ({
	commandLine: "FIXTURE",
	commandTargetMode: "FIXTURE" as const,
	commandLinePristine: true,
	selectedFixtures: [] as string[],
	selectedGroupId: null,
	pendingCommandChoice: null,
	setCommandLine: vi.fn(),
	resetCommandLine: vi.fn(),
	executeCommandLine: vi.fn(),
	cancelCommandChoice: vi.fn(),
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => {
	server.selectedFixtures = [];
});

function CommandProbe({
	visible,
	onRender,
}: {
	visible: boolean;
	onRender: () => void;
}) {
	onRender();
	const command = useProgrammingCommandLineView(visible);
	return <span>{command?.text ?? (visible ? "Loading" : "Hidden")}</span>;
}

function SurfaceProbe({ active }: { active: boolean }) {
	const command = useCommandLineSurface({
		selection: true,
		enabled: active,
	});
	return <span>{active ? command.text : "Inactive surface"}</span>;
}

function SelectionSurfaceProbe() {
	const command = useCommandLineSurface({ selection: true });
	return <span>{command.selected.join(",") || "No scoped selection"}</span>;
}

function ActionProbe({ onRender }: { onRender: () => void }) {
	onRender();
	useCommandLineSurface({ observeCommand: false });
	return <span>Action surface</span>;
}

function SelectionActionProbe({
	enabled,
	onRender,
}: {
	enabled: boolean;
	onRender: () => void;
}) {
	onRender();
	useProgrammingSelectionActions(enabled);
	return <span>Selection action surface</span>;
}

function SelectionMutationProbe({
	onWrite,
}: {
	onWrite: (write: Promise<SelectionActionOutcome | null>) => void;
}) {
	const actions = useProgrammingSelectionActions();
	return (
		<Button
			onClick={() => {
				if (actions)
					onWrite(actions.replace({ resolvedFixtures: [FIXTURE_2] }));
			}}
		>
			Select fixture 2
		</Button>
	);
}

function LayoutSelectionMutationProbe({
	armed,
	onWrite,
}: {
	armed: boolean;
	onWrite: (write: Promise<SelectionActionOutcome | null>) => void;
}) {
	const actions = useProgrammingSelectionActions();
	useLayoutEffect(() => {
		if (armed && actions)
			onWrite(actions.replace({ resolvedFixtures: [FIXTURE_2] }));
	}, [actions, armed, onWrite]);
	return <span>Layout selection surface</span>;
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("ProgrammingInteractionViewProvider", () => {
	it("never falls back to stale global selection while scoped authority loads", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const snapshot = deferred<ReturnType<typeof programmingSnapshot>>();
		server.selectedFixtures = [FIXTURE_3];
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={() => snapshot.promise}
			>
				<SelectionSurfaceProbe />
			</ProgrammingInteractionViewProvider>,
		);

		expect(screen.getByText("No scoped selection")).toBeInTheDocument();
		snapshot.resolve(programmingSnapshot());
		await screen.findByText(FIXTURE_1);
	});

	it("subscribes a command surface only while its view is active", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const view = (active: boolean) => (
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<SurfaceProbe active={active} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(false));

		expect(screen.getByText("Inactive surface")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: true,
			selection: true,
		});

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
	});

	it("does not rerender action-only consumers for command text changes", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const onRender = vi.fn();
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={async () => programmingSnapshot()}
			>
				<ActionProbe onRender={onRender} />
			</ProgrammingInteractionViewProvider>,
		);
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		const renderCount = onRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: commandChange({ revision: 2, text: "FIXTURE 20" }),
			}),
		);

		expect(onRender).toHaveBeenCalledTimes(renderCount);
	});

	it("activates selection actions only while their surface is visible", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const onRender = vi.fn();
		const view = (visible: boolean) => (
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
				applySelection={vi.fn()}
			>
				<SelectionActionProbe enabled={visible} onRender={onRender} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(false));
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: false,
			selection: true,
		});
		const renderCount = onRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 20,
				correlationId: null,
				change: selectionChange({ revision: 2 }),
			}),
		);

		expect(onRender).toHaveBeenCalledTimes(renderCount);

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
	});

	it("performs no work for a hidden view and isolates irrelevant events", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const onRender = vi.fn();
		const view = (visible: boolean) => (
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<CommandProbe visible={visible} onRender={onRender} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(false));

		expect(screen.getByText("Hidden")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: true,
			selection: false,
		});
		await waitFor(() => expect(screen.getByText("FIXTURE")).toBeInTheDocument());

		const beforeIrrelevant = onRender.mock.calls.length;
		act(() =>
			transport.emit({
				type: "event",
				sequence: 24,
				correlationId: null,
				change: selectionChange({ revision: 2 }),
			}),
		);
		expect(onRender).toHaveBeenCalledTimes(beforeIrrelevant);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 31,
				correlationId: null,
				change: commandChange({ revision: 2, text: "FIXTURE 31" }),
			}),
		);
		await waitFor(() =>
			expect(screen.getByText("FIXTURE 31")).toBeInTheDocument(),
		);

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
		expect(screen.getByText("Hidden")).toBeInTheDocument();
	});

	it("invalidates an active selection write when the show changes", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const lateResponse = deferred<SelectionActionOutcome>();
		const applySelection = vi.fn().mockReturnValue(lateResponse.promise);
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(
				programmingSnapshot({ selected: selection(1, [FIXTURE_1]) }),
			)
			.mockResolvedValueOnce(
				programmingSnapshot({
					sequence: 20,
					selected: selection(5, [FIXTURE_3]),
				}),
			);
		let activeWrite: Promise<SelectionActionOutcome | null> | null = null;
		const view = (showId: string) => (
			<ProgrammingInteractionViewProvider
				showId={showId}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
				applySelection={applySelection}
			>
				<SelectionMutationProbe onWrite={(write) => (activeWrite = write)} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(SHOW_ID));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());
		await waitFor(() =>
			expect(store.getSnapshot().selection).toEqual(selection(1, [FIXTURE_1])),
		);

		fireEvent.click(screen.getByRole("button", { name: "Select fixture 2" }));
		await waitFor(() => expect(applySelection).toHaveBeenCalledOnce());
		expect(store.getSnapshot().selection?.selected).toEqual([FIXTURE_2]);

		rendered.rerender(view(OTHER_SHOW_ID));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(2));
		await expect(activeWrite).resolves.toBeNull();
		await waitFor(() =>
			expect(store.getSnapshot()).toMatchObject({
				showId: OTHER_SHOW_ID,
				selection: selection(5, [FIXTURE_3]),
			}),
		);

		const request = applySelection.mock.calls[0]?.[1] as SelectionActionRequest;
		lateResponse.resolve({
			requestId: request.requestId,
			correlationId: request.requestId,
			action: "replaced",
			applied: 1,
			selection: selection(2, [FIXTURE_2]),
			eventSequence: 11,
			replayed: false,
			warning: null,
		});
		await Promise.resolve();
		await Promise.resolve();
		expect(store.getSnapshot().selection).toEqual(selection(5, [FIXTURE_3]));
	});

	it("resets and rehydrates when the active show changes", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValueOnce(
				programmingSnapshot({
					sequence: 20,
					command: commandLine(3, "GROUP", "GROUP"),
				}),
			);
		const view = (showId: string) => (
			<ProgrammingInteractionViewProvider
				showId={showId}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<CommandProbe visible onRender={() => undefined} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(SHOW_ID));
		await waitFor(() => expect(screen.getByText("FIXTURE")).toBeInTheDocument());

		rendered.rerender(view(OTHER_SHOW_ID));
		await waitFor(() => expect(screen.getByText("GROUP")).toBeInTheDocument());

		expect(loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions[0].close).toHaveBeenCalledOnce();
		expect(transport.subscriptions[1].after).toBe(20);
		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			eventSequence: 20,
		});
	});

	it("rejects a new-view layout mutation until that view owns the store scope", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const applySelection = vi.fn();
		const onWrite = vi.fn();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValueOnce(
				programmingSnapshot({
					sequence: 20,
					selected: selection(5, [FIXTURE_3]),
				}),
			);
		const view = (showId: string, armed: boolean) => (
			<ProgrammingInteractionViewProvider
				showId={showId}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
				applySelection={applySelection}
			>
				<LayoutSelectionMutationProbe armed={armed} onWrite={onWrite} />
			</ProgrammingInteractionViewProvider>
		);
		const rendered = render(view(SHOW_ID, false));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());

		rendered.rerender(view(OTHER_SHOW_ID, true));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(2));
		await expect(onWrite.mock.calls[0]?.[0]).resolves.toBeNull();

		expect(applySelection).not.toHaveBeenCalled();
		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			selection: selection(5, [FIXTURE_3]),
		});
	});
});
