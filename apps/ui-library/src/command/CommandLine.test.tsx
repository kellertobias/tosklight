import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CommandLine, type CommandLineProps } from "./CommandLine";

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

function props(overrides: Partial<CommandLineProps> = {}): CommandLineProps {
	return {
		mode: "programmer",
		hardware: false,
		ready: true,
		completed: false,
		commandError: null,
		persistentError: null,
		persistentErrorOpen: false,
		commandLine: "FIXTURE 1 AT 68",
		commandTarget: "FIXTURE",
		preloadArmed: false,
		preloadActive: false,
		preloadReady: true,
		preloadLabel: "PRELOAD",
		pendingSummary: "",
		recordState: "ready",
		recordShiftArmed: false,
		history: [],
		historyOpen: false,
		status: {
			connection: "connected",
			frequency: 44,
			timecode: null,
			blackout: false,
			highlight: false,
		},
		onReplace: vi.fn(),
		onExecute: vi.fn(),
		onToggleMode: vi.fn(),
		onHistoryOpenChange: vi.fn(),
		onReuseHistory: vi.fn(),
		onOpenStatus: vi.fn(),
		onAcknowledgeCommandError: vi.fn(),
		onPersistentErrorOpenChange: vi.fn(),
		onAcknowledgePersistentError: vi.fn(),
		onRecordStart: vi.fn(),
		onRecordEnd: vi.fn(),
		onRecordCancel: vi.fn(),
		onRecordComplete: vi.fn(),
		onAdvancePreload: vi.fn(),
		onInspectPreload: vi.fn(),
		...overrides,
	};
}

describe("CommandLine", () => {
	it("exposes command, mode, status, Record, and Preload interactions without services", () => {
		const view = props();
		render(<CommandLine {...view} />);

		fireEvent.change(screen.getByRole("textbox", { name: "Command line" }), {
			target: { value: "FIXTURE 2 AT 50" },
		});
		expect(view.onReplace).toHaveBeenCalledWith("FIXTURE 2 AT 50");
		fireEvent.keyDown(screen.getByRole("textbox", { name: "Command line" }), {
			key: "Enter",
		});
		expect(view.onExecute).toHaveBeenCalledWith("FIXTURE 1 AT 68");
		fireEvent.click(screen.getByRole("button", { name: /PROG/u }));
		expect(view.onToggleMode).toHaveBeenCalledOnce();
		fireEvent.click(screen.getByRole("button", { name: "ESC" }));
		expect(view.onReplace).toHaveBeenCalledWith("", true);
		fireEvent.click(
			screen.getByRole("button", {
				name: /Open Running & Output/u,
			}),
		);
		expect(view.onOpenStatus).toHaveBeenCalledOnce();
		fireEvent.click(screen.getByRole("button", { name: "REC" }));
		expect(view.onRecordComplete).toHaveBeenCalledWith(false);
		fireEvent.click(screen.getByRole("button", { name: "PRELOAD" }));
		expect(view.onAdvancePreload).toHaveBeenCalledOnce();
	});

	it.each([
		false,
		true,
	])("preserves a full select-all paste after completion (hardware=%s)", async (hardware) => {
		const execute = vi.fn();
		function Surface() {
			const [text, setText] = useState("FIXTURE 9001 AT 20");
			const [completed, setCompleted] = useState(true);
			return (
				<CommandLine
					{...props({
						hardware,
						commandLine: text,
						completed,
						onReplace: (next) => {
							setText(next);
							setCompleted(false);
						},
						onExecute: execute,
					})}
				/>
			);
		}
		render(<Surface />);
		const user = userEvent.setup();
		const input = screen.getByRole("textbox", {
			name: "Command line",
		}) as HTMLInputElement;
		await user.click(input);
		input.setSelectionRange(0, input.value.length);
		const command = "FIXTURE 9002 AT COLOR PRESET 100";
		await user.paste(command);
		expect(input).toHaveValue(command);
		await user.keyboard("{Enter}");
		expect(execute).toHaveBeenCalledExactlyOnceWith(command);
	});

	it.each([
		"insertFromDrop",
		"insertReplacementText",
		"insertFromPaste",
	])("keeps complete %s content instead of its final digit", (inputType) => {
		const view = props({ completed: true });
		render(<CommandLine {...view} />);
		const input = screen.getByRole("textbox", { name: "Command line" });
		fireEvent.input(input, {
			target: { value: "FIXTURE 9002 AT 100" },
			inputType,
			data: "FIXTURE 9002 AT 100",
		});
		expect(view.onReplace).toHaveBeenCalledExactlyOnceWith(
			"FIXTURE 9002 AT 100",
		);
	});

	it.each([
		false,
		true,
	])("still starts a fresh command on an ordinary next digit (hardware=%s)", (hardware) => {
		const view = props({ completed: true, hardware });
		render(<CommandLine {...view} />);
		fireEvent.input(screen.getByRole("textbox", { name: "Command line" }), {
			target: { value: `${view.commandLine}9` },
			inputType: "insertText",
			data: "9",
		});
		expect(view.onReplace).toHaveBeenCalledExactlyOnceWith("FIXTURE 9");
	});

	it("keeps primary captions visible beside the smaller Shift actions", () => {
		render(<CommandLine {...props({ recordShiftArmed: true })} />);

		const escapeButton = screen.getByRole("button", {
			name: "ESC, Shift: UNDO",
		});
		const record = screen.getByRole("button", {
			name: "REC, Shift: UPDATE",
		});
		const preload = screen.getByRole("button", {
			name: "PRELOAD, Shift: CLEAR PRELOAD",
		});
		expect(escapeButton).toHaveTextContent("ESCUNDO");
		expect(record).toHaveTextContent("RECUPDATE");
		expect(preload).toHaveTextContent("PRELOADCLEAR PRELOAD");
		expect(escapeButton.querySelector("small")).toHaveTextContent("UNDO");
		expect(record.querySelector("small")).toHaveTextContent("UPDATE");
		expect(preload.querySelector("small")).toHaveTextContent("CLEAR PRELOAD");
	});

	it("renders authoritative completion, blackout, errors, and hardware geometry", () => {
		const view = props({
			hardware: true,
			completed: true,
			commandError: "Command rejected",
			historyOpen: true,
			persistentError: "Output is unavailable",
			persistentErrorOpen: true,
			recordState: "update-armed",
			status: {
				connection: "error",
				frequency: "—",
				timecode: "01:02:03:12",
				blackout: true,
				highlight: false,
			},
		});
		const { container } = render(<CommandLine {...view} />);

		expect(screen.getByRole("img", { name: "Command applied" })).toBeVisible();
		expect(container.querySelector(".command-line-bar")).toHaveClass(
			"hardware-mode",
		);
		expect(screen.getByText("BLACKOUT")).toBeVisible();
		expect(screen.getByRole("alert")).toHaveTextContent("Command rejected");
		expect(screen.getByRole("alert").parentElement).toHaveClass(
			"command-field",
			"command-history-open",
		);
		expect(screen.getByRole("alertdialog")).toHaveTextContent(
			"Output is unavailable",
		);
		expect(
			screen.queryByRole("button", { name: "ESC" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "UPDATE ARMED" }),
		).toHaveAttribute("aria-pressed", "true");
	});

	it("replaces the DMX rate with active Highlight feedback", () => {
		const { container } = render(
			<CommandLine
				{...props({
					status: {
						connection: "connected",
						frequency: 44,
						timecode: null,
						blackout: false,
						highlight: true,
					},
				})}
			/>,
		);

		expect(screen.getByText("DMX · Highlight")).toBeVisible();
		expect(screen.queryByText("DMX 44Hz")).not.toBeInTheDocument();
		expect(container.querySelector(".highlight-status")).toBeVisible();
		expect(
			screen.getByRole("button", { name: /Highlight active/u }),
		).toBeVisible();
	});

	it("keeps the DMX opener visible and adds an accessible Desk State warning", () => {
		const open = vi.fn();
		const view = props({
			onOpenStatus: open,
			status: {
				connection: "connected",
				frequency: 44,
				timecode: "01:02:03:12",
				blackout: false,
				highlight: false,
				deskError: "Programmer authority conflict",
			},
		});
		const { rerender } = render(<CommandLine {...view} />);

		const warning = screen.getByRole("button", {
			name: /Desk State needs attention: Programmer authority conflict/u,
		});
		expect(
			warning.querySelector(".command-status-warning-triangle"),
		).toBeInTheDocument();
		expect(screen.getAllByText("DMX 44Hz")[0]).toBeVisible();
		expect(screen.getByText("01:02:03:12")).toBeVisible();
		expect(warning).toHaveAttribute("title", "Open Desk State");
		fireEvent.click(warning);
		expect(open).toHaveBeenCalledOnce();

		rerender(
			<CommandLine
				{...view}
				status={{ ...view.status, deskError: null, timecode: null }}
			/>,
		);
		expect(screen.getAllByText("DMX 44Hz")[0]).toBeVisible();
		expect(screen.getByText("No Timecode")).toBeVisible();
	});

	it("keeps pending preload details in the title without adding them beside PRELOAD GO", () => {
		render(
			<CommandLine
				{...props({
					preloadArmed: true,
					preloadActive: true,
					preloadLabel: "PRELOAD GO",
					pendingSummary: "PROG 4 · GO MINUS 2",
				})}
			/>,
		);

		const preload = screen.getByRole("button", { name: "PRELOAD GO" });
		expect(preload).toHaveTextContent(/^PRELOAD GO$/);
		expect(preload).toHaveAttribute(
			"title",
			"Pending Preload: PROG 4 · GO MINUS 2",
		);
	});

	it("holds Preload to inspect pending work without advancing it", () => {
		vi.useFakeTimers();
		const view = props({ preloadActive: true });
		render(<CommandLine {...view} />);
		const preload = screen.getByRole("button", { name: "PRELOAD" });
		fireEvent.pointerDown(preload);
		vi.advanceTimersByTime(650);
		fireEvent.pointerUp(preload);
		fireEvent.click(preload);
		expect(view.onInspectPreload).toHaveBeenCalledOnce();
		expect(view.onAdvancePreload).not.toHaveBeenCalled();
		vi.useRealTimers();
	});

	it("opens, reuses, and dismisses command history through controlled state", () => {
		const reused = vi.fn();
		function Harness() {
			const [open, setOpen] = useState(false);
			return (
				<CommandLine
					{...props({
						historyOpen: open,
						onHistoryOpenChange: setOpen,
						onReuseHistory: reused,
						history: [
							{
								id: "one",
								command: "GROUP 1 AT FULL",
								status: "accepted",
								feedback: "Applied.",
								source: "software",
								at: "2026-07-26T12:34:56.000Z",
							},
						],
					})}
				/>
			);
		}
		render(<Harness />);

		fireEvent.click(screen.getByRole("textbox", { name: "Command line" }));
		expect(
			screen.getByRole("dialog", { name: "Command line history" }),
		).toBeVisible();
		expect(
			screen.getByRole("dialog", { name: "Command line history" })
				.parentElement,
		).toHaveClass("command-field", "command-history-open");
		fireEvent.click(screen.getByRole("button", { name: "Reuse" }));
		expect(reused).toHaveBeenCalledWith("GROUP 1 AT FULL");
		fireEvent.keyDown(window, { key: "Escape" });
		expect(
			screen.queryByRole("dialog", { name: "Command line history" }),
		).not.toBeInTheDocument();
	});

	it("shows content-window errors in history without offering to execute them", () => {
		render(
			<CommandLine
				{...props({
					historyOpen: true,
					history: [
						{
							id: "window-error",
							command: "DYNAMICS ERROR",
							status: "rejected",
							feedback: "Dynamic 4 could not be loaded.",
							source: "window",
							at: "2026-07-29T12:34:56.000Z",
						},
					],
				})}
			/>,
		);

		expect(screen.getByText("Dynamic 4 could not be loaded.")).toBeVisible();
		expect(screen.getByText(/content window/u)).toBeVisible();
		expect(
			screen.queryByRole("button", { name: "Reuse" }),
		).not.toBeInTheDocument();
	});
});

describe("brief operator rejection feedback", () => {
	it("blinks red briefly, keeps the entered command grey, and expires its small toast", () => {
		vi.useFakeTimers();
		render(<CommandLine {...props({ commandError: "No Selection" })} />);
		const input = screen.getByRole("textbox", { name: "Command line" });
		expect(input).toHaveValue("FIXTURE 1 AT 68");
		expect(input).toHaveClass("error", "rejected");
		expect(screen.getByRole("alert")).toHaveTextContent("No Selection");
		act(() => vi.advanceTimersByTime(600));
		expect(input).not.toHaveClass("error");
		expect(input).toHaveClass("rejected");
		act(() => vi.advanceTimersByTime(2000));
		expect(screen.queryByRole("alert")).toBeNull();
		expect(input).toHaveValue("FIXTURE 1 AT 68");
		vi.useRealTimers();
	});
	it("restarts feedback for repeated rejection and clears it on edit", () => {
		vi.useFakeTimers();
		const view = props({
			commandError: "Preset not Recorded",
			commandErrorRevision: 1,
		});
		const { rerender } = render(<CommandLine {...view} />);
		act(() => vi.advanceTimersByTime(2500));
		rerender(<CommandLine {...view} commandErrorRevision={2} />);
		expect(screen.getByRole("textbox", { name: "Command line" })).toHaveClass(
			"error",
		);
		expect(screen.getByRole("alert")).toHaveTextContent("Preset not Recorded");
		rerender(
			<CommandLine {...view} commandError={null} commandLine="FIXTURE 2" />,
		);
		expect(screen.queryByRole("alert")).toBeNull();
		expect(
			screen.getByRole("textbox", { name: "Command line" }),
		).not.toHaveClass("rejected");
		vi.useRealTimers();
	});
	it("keeps operator mistakes out of the larger current-error panel", () => {
		render(
			<CommandLine
				{...props({ commandError: "Preset not Recorded", historyOpen: true })}
			/>,
		);
		expect(document.querySelector(".command-history-current-error")).toBeNull();
		expect(screen.getByRole("alert")).toHaveTextContent("Preset not Recorded");
	});
	it("retains detailed unexpected failures for diagnosis", () => {
		render(
			<CommandLine
				{...props({
					commandError: "Unexpected response\nserver diagnostic",
					commandErrorKind: "unexpected",
					historyOpen: true,
				})}
			/>,
		);
		expect(
			document.querySelector(".command-history-current-error"),
		).toHaveTextContent("server diagnostic");
	});
});

describe("authoritative Playback command context", () => {
	it.each([
		false,
		true,
	])("keeps context inside the command field with hardware=%s", (hardware) => {
		const view = props({ hardware });
		const { rerender } = render(
			<CommandLine
				{...view}
				{...{ commandContext: { name: "Main", page: 2 } }}
			/>,
		);
		const field = screen
			.getByRole("textbox", { name: "Command line" })
			.closest(".command-field");
		expect(field).toContainElement(screen.getByText("Main"));
		expect(field).toContainElement(screen.getByText("Page 2"));
		expect(screen.getByRole("textbox", { name: "Command line" })).toHaveValue(
			"FIXTURE 1 AT 68",
		);
		rerender(
			<CommandLine
				{...view}
				{...{ commandContext: { name: "Screen 1", page: 7 } }}
			/>,
		);
		expect(field).toContainElement(screen.getByText("Screen 1"));
		expect(field).toContainElement(screen.getByText("Page 7"));
		expect(screen.queryByText("Page 2")).not.toBeInTheDocument();
	});
});

it("holds the mode control for650ms to open settings without a short toggle", () => {
	vi.useFakeTimers();
	const settings = vi.fn();
	const view = props({ onModeSettings: settings });
	render(<CommandLine {...view} />);
	const mode = screen.getByRole("button", { name: /PROG.*PLAYBK/ });
	fireEvent.pointerDown(mode);
	act(() => vi.advanceTimersByTime(649));
	expect(settings).not.toHaveBeenCalled();
	act(() => vi.advanceTimersByTime(1));
	fireEvent.pointerUp(mode);
	fireEvent.click(mode);
	expect(settings).toHaveBeenCalledOnce();
	expect(view.onToggleMode).not.toHaveBeenCalled();
	fireEvent.pointerDown(mode);
	act(() => vi.advanceTimersByTime(100));
	fireEvent.pointerUp(mode);
	fireEvent.click(mode);
	expect(view.onToggleMode).toHaveBeenCalledOnce();
});
it("shows Programmer placement instead of Playback context and preserves command text", () => {
	render(<CommandLine {...props({ programmerAssignment: "Programmer is assigned to Screen1", commandContext: { name: "Main", page: 4 } })} />);
	expect(screen.getByText("Programmer is assigned to Screen1")).toBeInTheDocument();
	expect(screen.queryByLabelText("Playback command context")).not.toBeInTheDocument();
	expect(screen.getByRole("textbox", { name: "Command line" })).toHaveValue("FIXTURE 1 AT 68");
});
it("replaces the command controls with the supplied Solo editor without invoking desk actions", () => {
	render(<CommandLine {...props({ replacementContent: <><input aria-label="Solo Region name" /><button>Save Solo Region</button><button>Cancel</button></> })} />);
	expect(screen.getByRole("button", { name: "Save Solo Region" })).toBeInTheDocument();
	expect(screen.queryByRole("textbox", { name: "Command line" })).not.toBeInTheDocument();
	expect(screen.queryByRole("button", { name: "REC" })).not.toBeInTheDocument();
});

it("cancels a mode-settings hold when its pointer cancels or its component unmounts", () => {
	vi.useFakeTimers();
	const settings = vi.fn();
	const view = render(<CommandLine {...props({ onModeSettings: settings })} />);
	const mode = screen.getByRole("button", { name: /PROG.*PLAYBK/ });
	fireEvent.pointerDown(mode);
	fireEvent.pointerCancel(mode);
	act(() => vi.advanceTimersByTime(1000));
	expect(settings).not.toHaveBeenCalled();
	fireEvent.pointerDown(mode);
	view.unmount();
	act(() => vi.advanceTimersByTime(1000));
	expect(settings).not.toHaveBeenCalled();
});
