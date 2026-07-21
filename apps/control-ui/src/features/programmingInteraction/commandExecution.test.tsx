import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { createContext, memo, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import type { CommandExecutionRequest } from "./commandExecution";
import type { CommandLineProjection } from "./contracts";
import { ProgrammingInteractionViewProvider } from "./ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "./store";
import {
	commandLine,
	DESK_ID,
	FakeProgrammingTransport,
	OTHER_SHOW_ID,
	programmingSnapshot,
	SHOW_ID,
} from "./testFixtures";

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((onResolve, onReject) => {
		resolve = onResolve;
		reject = onReject;
	});
	return { promise, resolve, reject };
}

interface SurfaceHandle {
	ready: boolean;
	text: string;
	target: string;
	replace(value: string): Promise<boolean>;
	reset(): Promise<boolean>;
	execute(value?: string): Promise<boolean>;
	cancelChoice(): Promise<boolean>;
}

let surface: SurfaceHandle | null = null;
let inactiveSurface: SurfaceHandle | null = null;

afterEach(() => {
	cleanup();
	surface = null;
	inactiveSurface = null;
});

function CommandSurfaceProbe({ onRender }: { onRender?: () => void } = {}) {
	onRender?.();
	const command = useCommandLineSurface();
	surface = command;
	return (
		<span data-testid="command">
			{command.ready ? `ready:${command.text}` : "not-ready"}
		</span>
	);
}

function InactiveCommandSurfaceProbe() {
	const command = useCommandLineSurface({ enabled: false });
	inactiveSurface = command;
	return <span data-testid="inactive-command">{command.ready ? "ready" : "not-ready"}</span>;
}

function view({
	showId = SHOW_ID,
	store,
	transport,
	loadSnapshot,
	replaceCommandLine,
	executeCommand,
	children = <CommandSurfaceProbe />,
}: {
	showId?: string;
	store: ProgrammingInteractionStore;
	transport: FakeProgrammingTransport;
	loadSnapshot: () => Promise<ReturnType<typeof programmingSnapshot>>;
	replaceCommandLine?: (
		deskId: string,
		text: string,
		expectedRevision: number,
	) => Promise<CommandLineProjection>;
	executeCommand?: (request: CommandExecutionRequest) => Promise<boolean>;
	children?: React.ReactNode;
}) {
	return (
		<ProgrammingInteractionViewProvider
			showId={showId}
			deskId={DESK_ID}
			store={store}
			transport={transport}
			loadSnapshot={loadSnapshot}
			replaceCommandLine={replaceCommandLine}
			executeCommand={executeCommand}
		>
			{children}
		</ProgrammingInteractionViewProvider>
	);
}

function scope() {
	return {
		store: new ProgrammingInteractionStore(),
		transport: new FakeProgrammingTransport(),
	};
}

describe("scoped command-line execution", () => {
	it("performs no snapshot or subscription before a command-line view mounts", async () => {
		const { store, transport } = scope();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		const rendered = render(
			view({ store, transport, loadSnapshot, children: <span>Dormant</span> }),
		);

		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view({ store, transport, loadSnapshot }));
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());
		expect(transport.subscriptions).toHaveLength(1);
	});

	it("shows no command state and refuses writes while scoped authority loads", async () => {
		const { store, transport } = scope();
		const snapshot = deferred<ReturnType<typeof programmingSnapshot>>();
		const replaceCommandLine = vi.fn();
		const executeCommand = vi.fn();
		render(
			view({
				store,
				transport,
				loadSnapshot: () => snapshot.promise,
				replaceCommandLine,
				executeCommand,
			}),
		);

		expect(screen.getByTestId("command")).toHaveTextContent("not-ready");
		await expect(surface?.replace("FIXTURE 1")).resolves.toBe(false);
		await expect(surface?.reset()).resolves.toBe(false);
		await expect(surface?.execute("FIXTURE 1")).resolves.toBe(false);
		expect(replaceCommandLine).not.toHaveBeenCalled();
		expect(executeCommand).not.toHaveBeenCalled();

		await act(async () => snapshot.resolve(programmingSnapshot()));
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);
	});

	it("keeps a disabled surface read-only when another view owns loaded authority", async () => {
		const { store, transport } = scope();
		const replaceCommandLine = vi.fn();
		const executeCommand = vi.fn();
		render(
			view({
				store,
				transport,
				loadSnapshot: async () => programmingSnapshot(),
				replaceCommandLine,
				executeCommand,
				children: (
					<>
						<CommandSurfaceProbe />
						<InactiveCommandSurfaceProbe />
					</>
				),
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);

		expect(screen.getByTestId("inactive-command")).toHaveTextContent("not-ready");
		await expect(inactiveSurface?.replace("FIXTURE 1")).resolves.toBe(false);
		await expect(inactiveSurface?.reset()).resolves.toBe(false);
		await expect(inactiveSurface?.execute("FIXTURE 1")).resolves.toBe(false);
		expect(replaceCommandLine).not.toHaveBeenCalled();
		expect(executeCommand).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(1);
	});

	it("refuses every write when no command-line writer is installed", async () => {
		const { store, transport } = scope();
		const executeCommand = vi.fn();
		render(
			view({
				store,
				transport,
				loadSnapshot: async () => programmingSnapshot(),
				executeCommand,
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("not-ready"),
		);

		await expect(surface?.replace("FIXTURE 1")).resolves.toBe(false);
		await expect(surface?.reset()).resolves.toBe(false);
		await expect(surface?.execute("FIXTURE 1")).resolves.toBe(false);
		await expect(surface?.cancelChoice()).resolves.toBe(false);
		expect(executeCommand).not.toHaveBeenCalled();
	});

	it("settles a pending edit before Enter executes", async () => {
		const { store, transport } = scope();
		const order: string[] = [];
		const write = deferred<CommandLineProjection>();
		const replaceCommandLine = vi.fn(() => {
			order.push("replace");
			return write.promise;
		});
		const executeCommand = vi.fn(async () => {
			order.push("execute");
			return true;
		});
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValue(
				programmingSnapshot({ sequence: 12, command: commandLine(3) }),
			);
		render(
			view({
				store,
				transport,
				loadSnapshot,
				replaceCommandLine,
				executeCommand,
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);

		let executed!: Promise<boolean>;
		act(() => {
			void surface?.replace("FIXTURE 1");
			executed = surface?.execute() as Promise<boolean>;
		});
		await waitFor(() => expect(replaceCommandLine).toHaveBeenCalledOnce());
		expect(executeCommand).not.toHaveBeenCalled();

		await act(async () => write.resolve(commandLine(2, "FIXTURE 1")));
		await act(async () => {
			await expect(executed).resolves.toBe(true);
		});
		expect(order).toEqual(["replace", "execute"]);
		expect(executeCommand).toHaveBeenCalledWith({
			command: "FIXTURE 1",
			target: "FIXTURE",
			pristine: false,
		});
	});

	it("reconciles whether the execution response or the desk event arrives first", async () => {
		for (const eventFirst of [false, true]) {
			const { store, transport } = scope();
			const executed = deferred<boolean>();
			const loadSnapshot = vi
				.fn()
				.mockResolvedValueOnce(programmingSnapshot())
				.mockResolvedValue(
					programmingSnapshot({ sequence: 30, command: commandLine(4) }),
				);
			const rendered = render(
				view({
					store,
					transport,
					loadSnapshot,
					replaceCommandLine: vi.fn(),
					executeCommand: () => executed.promise,
				}),
			);
			await waitFor(() =>
				expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
			);

			let running!: Promise<boolean>;
			act(() => {
				running = surface?.execute("FIXTURE 1") as Promise<boolean>;
			});
			const emitAuthority = () =>
				act(() =>
					transport.emit({
						type: "event",
						sequence: 30,
						correlationId: null,
						change: { deskId: DESK_ID, commandLine: commandLine(4) },
					}),
				);

			if (eventFirst) emitAuthority();
			await act(async () => executed.resolve(true));
			if (!eventFirst) emitAuthority();

			await act(async () => {
				await expect(running).resolves.toBe(true);
			});
			expect(store.getSnapshot().commandLine).toEqual(commandLine(4));
			rendered.unmount();
		}
	});

	it("leaves recoverable authoritative state after a failed execution", async () => {
		const { store, transport } = scope();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(
				programmingSnapshot({ command: commandLine(1, "FIXTURE 1") }),
			)
			.mockResolvedValue(
				programmingSnapshot({
					sequence: 15,
					command: commandLine(2, "FIXTURE 1"),
				}),
			);
		render(
			view({
				store,
				transport,
				loadSnapshot,
				replaceCommandLine: vi.fn(),
				executeCommand: async () => false,
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent(
				"ready:FIXTURE 1",
			),
		);

		await act(async () => {
			await expect(surface?.execute()).resolves.toBe(false);
		});

		// The rejected command stays visible and writable, so the operator can
		// correct and resend it instead of losing the line.
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(2, "FIXTURE 1"),
		);
		expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE 1");
	});

	it("carries the exact target and pristine state of a GROUP toggle", async () => {
		const { store, transport } = scope();
		const executeCommand = vi.fn(async () => true);
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValue(
				programmingSnapshot({
					sequence: 18,
					command: commandLine(5, "GROUP", "GROUP"),
				}),
			);
		render(
			view({
				store,
				transport,
				loadSnapshot,
				replaceCommandLine: vi.fn(),
				executeCommand,
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);

		await act(async () => {
			await expect(surface?.execute("GROUP")).resolves.toBe(true);
		});

		expect(executeCommand).toHaveBeenCalledWith({
			command: "GROUP",
			target: "FIXTURE",
			pristine: false,
		});
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(5, "GROUP", "GROUP"),
		);
	});

	it("retains an authoritative pending choice and clears it on cancel", async () => {
		const { store, transport } = scope();
		const choice = {
			type: "cue_move_copy" as const,
			choiceId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
			showId: SHOW_ID,
			showRevision: 7,
			operation: "copy" as const,
			command: "COPY SET 1 CUE 2 AT SET 2 CUE 2",
			options: [],
			cancelLabel: "Cancel",
		};
		const cancelled = deferred<CommandLineProjection>();
		render(
			view({
				store,
				transport,
				loadSnapshot: async () =>
					programmingSnapshot({
						command: { ...commandLine(2, choice.command), pendingChoice: choice },
					}),
				replaceCommandLine: vi.fn(() => cancelled.promise),
				executeCommand: vi.fn(),
			}),
		);
		await waitFor(() =>
			expect(store.getSnapshot().commandLine?.pendingChoice).toEqual(choice),
		);

		let cancelling!: Promise<boolean>;
		act(() => {
			cancelling = surface?.cancelChoice() as Promise<boolean>;
		});
		// Cancel is optimistic: the choice closes before the desk confirms.
		expect(store.getSnapshot().commandLine?.pendingChoice).toBeNull();

		await act(async () => cancelled.resolve(commandLine(3)));
		await act(async () => {
			await expect(cancelling).resolves.toBe(true);
		});
		expect(store.getSnapshot().commandLine).toEqual(commandLine(3));
	});

	it("drops a late completion after the active show is replaced", async () => {
		const { store, transport } = scope();
		const executed = deferred<boolean>();
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValue(
				programmingSnapshot({
					sequence: 40,
					command: commandLine(9, "GROUP", "GROUP"),
				}),
			);
		const rendered = render(
			view({
				store,
				transport,
				loadSnapshot,
				replaceCommandLine: vi.fn(),
				executeCommand: () => executed.promise,
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);

		let running!: Promise<boolean>;
		act(() => {
			running = surface?.execute("FIXTURE 1") as Promise<boolean>;
		});

		rendered.rerender(
			view({
				showId: OTHER_SHOW_ID,
				store,
				transport,
				loadSnapshot,
				replaceCommandLine: vi.fn(),
				executeCommand: () => executed.promise,
			}),
		);
		await waitFor(() =>
			expect(store.getSnapshot().showId).toBe(OTHER_SHOW_ID),
		);

		await act(async () => executed.resolve(true));
		await act(async () => {
			// The replacement scope owns the command line now; the late completion
			// must not be reported as this desk's result.
			await expect(running).resolves.toBe(false);
		});
		expect(store.getSnapshot().commandLine).toEqual(
			commandLine(9, "GROUP", "GROUP"),
		);
	});

	it("does not rerender the command surface for unrelated server updates", async () => {
		const { store, transport } = scope();
		const UnrelatedContext = createContext(0);
		const onRender = vi.fn();
		const MemoProbe = memo(() => <CommandSurfaceProbe onRender={onRender} />);
		// Stable dependencies: only the unrelated context value changes, exactly as
		// an unrelated ServerContext update would.
		const scoped = view({
			store,
			transport,
			loadSnapshot: async () => programmingSnapshot(),
			replaceCommandLine: vi.fn(),
			executeCommand: vi.fn(),
			children: <MemoProbe />,
		});
		function Host() {
			const [unrelated, setUnrelated] = useState(0);
			return (
				<UnrelatedContext.Provider value={unrelated}>
					<button type="button" onClick={() => setUnrelated((n) => n + 1)}>
						Update unrelated state
					</button>
					{scoped}
				</UnrelatedContext.Provider>
			);
		}
		render(<Host />);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);
		const renders = onRender.mock.calls.length;

		act(() =>
			screen.getByRole("button", { name: "Update unrelated state" }).click(),
		);

		expect(onRender).toHaveBeenCalledTimes(renders);
	});

	it("hydrates from the scoped snapshot without any broad bootstrap read", async () => {
		const { store, transport } = scope();
		const bootstrap = vi.fn();
		const loadSnapshot = vi.fn(async () => programmingSnapshot());
		render(
			view({
				store,
				transport,
				loadSnapshot,
				replaceCommandLine: vi.fn(),
				executeCommand: vi.fn(async () => {
					bootstrap();
					return true;
				}),
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("command")).toHaveTextContent("ready:FIXTURE"),
		);

		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(bootstrap).not.toHaveBeenCalled();
		expect(transport.subscriptions[0].scope).toEqual({
			commandLine: true,
			selection: false,
		});
	});
});
