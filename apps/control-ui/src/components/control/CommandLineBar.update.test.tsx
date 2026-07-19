import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Button } from "../common";
import { ProgrammingInteractionViewProvider } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { ProgrammingInteractionStore } from "../../features/programmingInteraction/store";
import {
	commandChange,
	commandLine,
	DESK_ID,
	FakeProgrammingTransport,
	programmingSnapshot,
	settleSession,
	SHOW_ID,
} from "../../features/programmingInteraction/testFixtures";
import { CommandLineBar } from "./CommandLineBar";
import { UPDATE_SETTINGS_EVENT, UPDATE_TARGET_MENU_EVENT } from "./updateWorkflow";

const state = {
  midiProfile: false,
  controlMode: "programmer",
  preload: "idle",
  preloadActive: false,
  updateArmed: false,
  storeArmed: false,
  shiftArmed: false,
  cueListSetArmed: false,
  playbackSetArmed: false,
  presetSetArmed: false,
  regularNumberShortcuts: true,
  playbackPage: 0,
  playbackPageNames: ["Main"],
  blackout: false,
  builtIn: null as string | null,
  patchSetArmed: false,
};
const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
  if (action.type === "SET_UPDATE_ARMED") state.updateArmed = Boolean(action.value);
  if (action.type === "SET_STORE_ARMED") state.storeArmed = Boolean(action.value);
  if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
});
const server = {
  bootstrap: { hardware_connected: false, active_programmers: [], frame_rate_hz: 60, active_timecode: null as string | null },
  session: { session_id: "session-a" },
  selectedFixtures: [],
  playbacks: null,
  commandLine: "FIXTURE",
  commandTargetMode: "FIXTURE",
  commandLinePristine: true,
  commandHistory: [
    { id: "entry-2", desk_id: "desk-a", session_id: "session-a", command: "FIXTURE 2 AT FULL", status: "rejected", feedback: "Fixture 2 is not patched", source: "software", at: "2026-07-17T20:00:02Z" },
    { id: "entry-1", desk_id: "desk-a", session_id: "session-a", command: "FIXTURE 1 AT FULL", status: "accepted", feedback: "Applied to 1 target(s)", source: "osc", at: "2026-07-17T20:00:01Z" },
  ] as const,
  error: null,
  status: "connected",
  poolPlaybackAction: vi.fn(),
  setPlaybackPage: vi.fn(),
  preloadAction: vi.fn(),
  executeCommandLine: vi.fn().mockResolvedValue(true),
  setCommandLine: vi.fn((value: string) => { server.commandLine = value; }),
  resetCommandLine: vi.fn(),
  dismissError: vi.fn(),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

beforeEach(() => {
  vi.useFakeTimers();
  state.updateArmed = false;
  state.storeArmed = false;
  state.shiftArmed = false;
  state.builtIn = null;
  state.patchSetArmed = false;
  state.midiProfile = false;
  state.regularNumberShortcuts = true;
  server.commandLine = "FIXTURE";
  server.bootstrap.active_timecode = null;
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

describe("scoped command-line integration", () => {
	it("renders optimistic edits and waits for the revisioned write before Enter", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		const pendingWrite = deferred<ReturnType<typeof commandLine>>();
		const replaceCommandLine = vi.fn(() => pendingWrite.promise);
		const loadSnapshot = vi
			.fn()
			.mockResolvedValueOnce(programmingSnapshot())
			.mockResolvedValueOnce(
				programmingSnapshot({ sequence: 12, command: commandLine(3) }),
			);
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
				replaceCommandLine={replaceCommandLine}
			>
				<CommandLineBar />
			</ProgrammingInteractionViewProvider>,
		);
		await act(settleSession);
		const input = screen.getByRole("textbox", { name: "Command line" });

		fireEvent.change(input, { target: { value: "FIXTURE 1" } });
		expect(input).toHaveValue("FIXTURE 1");
		expect(replaceCommandLine).toHaveBeenCalledWith(
			DESK_ID,
			"FIXTURE 1",
			1,
		);
		expect(server.setCommandLine).not.toHaveBeenCalled();

		fireEvent.keyDown(input, { key: "Enter" });
		expect(server.executeCommandLine).not.toHaveBeenCalled();
		await act(async () => {
			pendingWrite.resolve(commandLine(2, "FIXTURE 1"));
			await settleSession();
		});

		expect(server.executeCommandLine).toHaveBeenCalledWith("FIXTURE 1", {
			target: "FIXTURE",
			pristine: false,
		});
		expect(input).toHaveValue("FIXTURE");
	});

	it("renders an authoritative OSC command event without a legacy write", async () => {
		const store = new ProgrammingInteractionStore();
		const transport = new FakeProgrammingTransport();
		render(
			<ProgrammingInteractionViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={async () => programmingSnapshot()}
			>
				<CommandLineBar />
			</ProgrammingInteractionViewProvider>,
		);
		await act(settleSession);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 12,
				correlationId: null,
				change: commandChange({ revision: 2, text: "FIXTURE 12" }),
			}),
		);

		expect(screen.getByRole("textbox", { name: "Command line" })).toHaveValue(
			"FIXTURE 12",
		);
		expect(server.setCommandLine).not.toHaveBeenCalled();
	});
});

describe("Shift+Record Update gestures", () => {
  it("opens bounded desk history without changing or executing the unfinished command", () => {
    server.commandLine = "FIXTURE 7 AT";
    render(<CommandLineBar/>);

    fireEvent.click(screen.getByRole("textbox", { name: "Command line" }));
    const panel = screen.getByRole("dialog", { name: "Command line history" });
    expect(panel).toHaveTextContent("FIXTURE 2 AT FULL");
    expect(panel).toHaveTextContent("Rejected");
    expect(panel).toHaveTextContent("FIXTURE 1 AT FULL");
    expect(panel).toHaveTextContent("Accepted");
    expect(server.commandLine).toBe("FIXTURE 7 AT");
    expect(server.executeCommandLine).not.toHaveBeenCalled();

    fireEvent.click(screen.getAllByRole("button", { name: "Reuse" })[1]);
    expect(server.setCommandLine).toHaveBeenCalledWith("FIXTURE 1 AT FULL", false);
    expect(server.executeCommandLine).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "Command line history" })).not.toBeInTheDocument();
  });

  it("closes history with Escape or outside pointer input without clearing the command", () => {
    server.commandLine = "GROUP 3 AT";
    render(<CommandLineBar/>);
    const input = screen.getByRole("textbox", { name: "Command line" });

    fireEvent.click(input);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Command line history" })).not.toBeInTheDocument();
    expect(server.commandLine).toBe("GROUP 3 AT");

    fireEvent.click(input);
    fireEvent.pointerDown(screen.getByRole("button", { name: /Open running and output controls/ }));
    expect(screen.queryByRole("dialog", { name: "Command line history" })).not.toBeInTheDocument();
    expect(server.commandLine).toBe("GROUP 3 AT");
  });

  it("uses gray for no timecode and blue only for a present timecode", () => {
    const { rerender } = render(<CommandLineBar/>);
    expect(screen.getByText("No Timecode").closest(".timecode-status")).toHaveClass("timecode-idle");

    server.bootstrap.active_timecode = "01:02:03:04";
    rerender(<CommandLineBar/>);
    expect(screen.getByText("01:02:03:04")).toHaveClass("timecode-active");
  });

  it("keeps the command-to-REC/Preload space free of Highlight status panels in both layouts", () => {
    const { container, rerender } = render(<CommandLineBar/>);
    const assertNoHighlightPanel = () => {
      const commandField = container.querySelector(".command-field");
      const recordPreload = container.querySelector(".command-record-preload");
      expect(commandField?.nextElementSibling).toBe(recordPreload);
      expect(container.querySelector('[aria-label="Highlight status"]')).not.toBeInTheDocument();
      expect(container.querySelector(".highlight-feedback,.command-highlight-feedback")).not.toBeInTheDocument();
    };
    assertNoHighlightPanel();

    state.midiProfile = true;
    rerender(<CommandLineBar/>);
    assertNoHighlightPanel();
  });

  it("routes the physical Home-key SET shortcut through the selected Patch control surface", () => {
    state.builtIn = "patch";
    const set = vi.fn();
    render(<><Button data-keypad-key="SET" onClick={set}>SET target</Button><CommandLineBar/></>);

    fireEvent.keyDown(window, { code: "Home", key: "Home" });

    expect(set).toHaveBeenCalledOnce();
    expect(server.setCommandLine).not.toHaveBeenCalled();
  });

  it("disables the complete software keyboard shortcut layer", () => {
    state.builtIn = "patch";
    state.regularNumberShortcuts = false;
    const set = vi.fn();
    render(<><Button data-keypad-key="SET" onClick={set}>SET target</Button><CommandLineBar/></>);

    fireEvent.keyDown(window, { code: "Home", key: "Home" });
    fireEvent.keyDown(window, { code: "Delete", key: "Delete", shiftKey: true });

    expect(set).not.toHaveBeenCalled();
    expect(dispatch).not.toHaveBeenCalledWith({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
    expect(server.setCommandLine).not.toHaveBeenCalled();
  });

  it("keeps single, second-press, and long-press software gestures mutually exclusive", () => {
    const menu = vi.fn();
    const settings = vi.fn();
    window.addEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.addEventListener(UPDATE_SETTINGS_EVENT, settings);
    state.shiftArmed = true;
    render(<CommandLineBar/>);
    const record = screen.getByRole("button", { name: "REC" });

    fireEvent.pointerDown(record);
    fireEvent.pointerUp(record);
    fireEvent.click(record);
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_UPDATE_ARMED", value: true });
    expect(server.setCommandLine).toHaveBeenCalledWith("UPDATE ", false);
    expect(menu).not.toHaveBeenCalled();

    fireEvent.pointerDown(record);
    fireEvent.pointerUp(record);
    fireEvent.click(record);
    expect(menu).toHaveBeenCalledTimes(1);

    state.updateArmed = false;
    fireEvent.pointerDown(record);
    vi.advanceTimersByTime(650);
    fireEvent.pointerUp(record);
    fireEvent.click(record);
    expect(settings).toHaveBeenCalledTimes(1);
    expect(dispatch.mock.calls.filter(([action]) => action.type === "SET_UPDATE_ARMED" && action.value === true)).toHaveLength(1);

    window.removeEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.removeEventListener(UPDATE_SETTINGS_EVENT, settings);
  });

  it("uses the same exclusive gestures for Shift+End on a software-only desk", () => {
    const menu = vi.fn();
    const settings = vi.fn();
    window.addEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.addEventListener(UPDATE_SETTINGS_EVENT, settings);
    render(<CommandLineBar/>);

    fireEvent.keyDown(window, { code: "End", key: "End", shiftKey: true });
    vi.advanceTimersByTime(100);
    fireEvent.keyUp(window, { code: "End", key: "End", shiftKey: true });
    expect(state.updateArmed).toBe(true);

    fireEvent.keyDown(window, { code: "End", key: "End", shiftKey: true });
    fireEvent.keyUp(window, { code: "End", key: "End", shiftKey: true });
    expect(menu).toHaveBeenCalledTimes(1);

    state.updateArmed = false;
    fireEvent.keyDown(window, { code: "End", key: "End", shiftKey: true });
    vi.advanceTimersByTime(650);
    fireEvent.keyUp(window, { code: "End", key: "End", shiftKey: true });
    expect(settings).toHaveBeenCalledTimes(1);
    expect(state.updateArmed).toBe(false);

    window.removeEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.removeEventListener(UPDATE_SETTINGS_EVENT, settings);
  });
});
