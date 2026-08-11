import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CommandLine, type CommandLineProps } from "./CommandLine";

afterEach(cleanup);

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
		onReleasePreload: vi.fn(),
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
				name: /Open running and output controls/u,
			}),
		);
		expect(view.onOpenStatus).toHaveBeenCalledOnce();
		fireEvent.click(screen.getByRole("button", { name: "REC" }));
		expect(view.onRecordComplete).toHaveBeenCalledWith(false);
		fireEvent.click(screen.getByRole("button", { name: "PRELOAD" }));
		expect(view.onAdvancePreload).toHaveBeenCalledOnce();
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
		expect(
			screen.getByRole("alert").closest(".command-history-panel")
				?.parentElement,
		).toHaveClass("command-field", "command-history-open");
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

		expect(screen.getByText("Highlight")).toBeVisible();
		expect(screen.queryByText("DMX 44Hz")).not.toBeInTheDocument();
		expect(container.querySelector(".highlight-status")).toBeVisible();
		expect(
			screen.getByRole("button", { name: /Highlight active/u }),
		).toBeVisible();
	});

	it("replaces both healthy status rows with one desk-error route and restores them when healthy", () => {
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
			name: "Desk error. Open Running & Output Desk state",
		});
		expect(
			warning.querySelector(".command-status-warning-triangle"),
		).toBeInTheDocument();
		expect(screen.queryByText("DMX 44Hz")).not.toBeInTheDocument();
		expect(screen.queryByText("01:02:03:12")).not.toBeInTheDocument();
		fireEvent.click(warning);
		expect(open).toHaveBeenCalledOnce();

		rerender(
			<CommandLine
				{...view}
				status={{ ...view.status, deskError: null, timecode: null }}
			/>,
		);
		expect(screen.getByText("DMX 44Hz")).toBeVisible();
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
