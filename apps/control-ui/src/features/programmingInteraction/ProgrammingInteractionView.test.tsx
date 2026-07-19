import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import {
	ProgrammingInteractionViewProvider,
	useProgrammingCommandLineView,
} from "./ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "./store";
import {
	commandChange,
	commandLine,
	DESK_ID,
	FakeProgrammingTransport,
	OTHER_SHOW_ID,
	programmingSnapshot,
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

function ActionProbe({ onRender }: { onRender: () => void }) {
	onRender();
	useCommandLineSurface({ observeCommand: false });
	return <span>Action surface</span>;
}

describe("ProgrammingInteractionViewProvider", () => {
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
});
