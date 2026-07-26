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
		expect(view.onExecute).toHaveBeenCalledOnce();
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
			persistentError: "Output is unavailable",
			persistentErrorOpen: true,
			recordState: "update-armed",
			status: {
				connection: "error",
				frequency: "—",
				timecode: "01:02:03:12",
				blackout: true,
			},
		});
		const { container } = render(<CommandLine {...view} />);

		expect(screen.getByRole("img", { name: "Command applied" })).toBeVisible();
		expect(container.querySelector(".command-line-bar")).toHaveClass(
			"hardware-mode",
		);
		expect(screen.getByText("BLACKOUT")).toBeVisible();
		expect(screen.getByRole("alert")).toHaveTextContent("Command rejected");
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
		fireEvent.click(screen.getByRole("button", { name: "Reuse" }));
		expect(reused).toHaveBeenCalledWith("GROUP 1 AT FULL");
		fireEvent.keyDown(window, { key: "Escape" });
		expect(
			screen.queryByRole("dialog", { name: "Command line history" }),
		).not.toBeInTheDocument();
	});
});
