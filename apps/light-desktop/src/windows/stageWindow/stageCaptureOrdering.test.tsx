// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { useLayoutEffect, useReducer, useRef } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ModalProvider } from "@tosklight/ui/modals";
import { DesktopProvider } from "../../platform/desktop";
import { tauriDesktopBridge } from "../../platform/desktop/tauriDesktopBridge";
import { appReducer, initialState } from "../../state/appReducer";
import type { AppState } from "../../types";
import type { StagePanePicture } from "../../platform/desktop/types";
import { DetailedPeopleSetting } from "./DetailedPeopleSetting";
import { StageCaptureControls } from "./StageCaptureControls";
import { useStageCapture } from "./useStageCapture";
import { useStagePanePicture } from "./useStagePanePicture";
import type { NativeStagePane } from "./useNativeStagePane";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), app: {} as Record<string, unknown> }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
vi.mock("../../state/AppContext", () => ({ useApp: () => mocks.app }));
function Harness({ initial, beforeEffect }: { initial: boolean; beforeEffect: boolean }) {
	const [state, dispatch] = useReducer(appReducer, { ...initialState, stageVizDetailedPeople: initial } as AppState);
	mocks.app.state = state; mocks.app.dispatch = dispatch;
	const pane = { id: "ordering", active: true } as NativeStagePane;
	const picture = useStagePanePicture(pane, "lines_3d", true);
	const capture = useStageCapture(pane.id, true, picture);
	const requested = useRef(false);
	useLayoutEffect(() => {
		if (beforeEffect && !requested.current && state.stageVizDetailedPeople !== initial) {
			requested.current = true;
			void capture.start();
		}
	}, [beforeEffect, state.stageVizDetailedPeople, initial, capture]);
	return <><DetailedPeopleSetting /><StageCaptureControls capture={capture} paneId={pane.id} followPreload /></>;
}
afterEach(() => { cleanup(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); });
beforeEach(() => {
	mocks.invoke.mockReset();
	// Cover both mocked module imports and Tauri's lazy dynamic-import entry point.
	Reflect.set(window, "__TAURI_INTERNALS__", { invoke: mocks.invoke });
});

it.each([
	{ initial: false, beforeEffect: false }, { initial: true, beforeEffect: false },
	{ initial: false, beforeEffect: true }, { initial: true, beforeEffect: true },
])("owns the invocation picture before deferred delivery/effects (initial=$initial beforeEffect=$beforeEffect)", async ({ initial, beforeEffect }) => {
	let release!: () => void;
	const blocked = new Promise<void>((resolve) => { release = resolve; });
	const events: Array<{ command: string; detail: boolean; camera: number; selection: string[] }> = [];
	let first = true;
	let nativeLook = initial, camera = 0;
	let selection: string[] = [];
	mocks.invoke.mockImplementation(async (command: string, args?: { picture?: StagePanePicture; x?: number; fixtures?: string[] }) => {
		if (!["set_stage_pane_picture", "start_stage_pane_capture", "stage_pane_input", "set_stage_pane_selection"].includes(command)) return;
		if (command === "set_stage_pane_picture" && first) { first = false; await blocked; }
		// Model the native live look and atomic Picture + capture, while later inputs remain live.
		if (args?.picture) nativeLook = Boolean(args.picture.detailedPeople);
		if (command === "stage_pane_input") camera += args?.x ?? 0;
		if (command === "set_stage_pane_selection") selection = args?.fixtures ?? [];
		events.push({ command, detail: nativeLook, camera, selection: [...selection] });
	});
	await tauriDesktopBridge.openStagePane("ordering", true, {
		x: 0, y: 0, width: 640, height: 480, scale: 1, surfaceWidth: 640, surfaceHeight: 480,
	});
	render(<DesktopProvider bridge={tauriDesktopBridge}><ModalProvider><Harness initial={initial} beforeEffect={beforeEffect} /></ModalProvider></DesktopProvider>);
	const button = screen.getByRole("button", { name: /Render 4K Follow Preload/ });
	await waitFor(() => expect(button).toBeEnabled());
	const priorInput = tauriDesktopBridge.sendStagePaneInput("orbit", 1, 0, "ordering");
	const priorSelection = tauriDesktopBridge.setStagePaneSelection("ordering", ["before"]);
	fireEvent.click(screen.getByRole("switch", { name: "Detailed people" }));
	button.focus();
	if (!beforeEffect) {
		if (initial) fireEvent.keyDown(button, { key: "r", ctrlKey: true, shiftKey: true });
		else fireEvent.click(button);
	}
	// Both explicit and legacy no-pane gestures resolve to the same native owner queue.
	const laterInput = tauriDesktopBridge.sendStagePaneInput("orbit", 10, 0);
	const laterSelection = tauriDesktopBridge.setStagePaneSelection("ordering", ["after"]);
	fireEvent.click(screen.getByRole("switch", { name: "Detailed people" }));
	expect(events).toEqual([]);
	await act(async () => { release(); await Promise.all([priorInput, priorSelection, laterInput, laterSelection]); });
	await waitFor(() => expect(events.at(-1)?.detail).toBe(initial));
	const frozen = events.find((event) => event.command === "start_stage_pane_capture");
	expect(frozen).toEqual({ command: "start_stage_pane_capture", detail: !initial, camera: 1, selection: ["before"] });
	const captureIndex = events.indexOf(frozen!);
	expect(events.findIndex((event) => event.command === "stage_pane_input" && event.camera === 11)).toBeGreaterThan(captureIndex);
	expect(events.findIndex((event) => event.command === "set_stage_pane_selection" && event.selection[0] === "after")).toBeGreaterThan(captureIndex);
	expect(events.at(-1)).toEqual({ command: "set_stage_pane_picture", detail: initial, camera: 11, selection: ["after"] });
	await tauriDesktopBridge.closeStagePane("ordering");
});
