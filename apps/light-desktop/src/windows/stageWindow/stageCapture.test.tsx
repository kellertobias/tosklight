import { capturePicture } from "./stageCaptureTestFixture";
// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ModalFrame, ModalProvider } from "@tosklight/ui/modals";
import { DesktopProvider } from "../../platform/desktop";
import { browserDesktopBridge } from "../../platform/desktop/browserDesktopBridge";
import type { DesktopBridge, StageStillEvent, StageStillPhase, StageStillStatus } from "../../platform/desktop/types";
import { NativeStageSurface } from "./NativeStageSurface";
import { StageCaptureControls } from "./StageCaptureControls";
import { useStageCapture } from "./useStageCapture";

const listeners = new Set<(event: StageStillEvent) => void>();
const start = vi.fn(); const choose = vi.fn(); const cancel = vi.fn(); const open = vi.fn(); const stop = vi.fn();
const bridge: DesktopBridge = { ...browserDesktopBridge, available: true,
	startStagePaneCapture: start, chooseStagePaneCapture: choose, cancelStagePaneCapture: cancel, openStagePaneCapture: open,
	onStageStill: async (listener) => { listeners.add(listener); return () => { listeners.delete(listener); stop(); }; },
};
function Harness({ id = "live", preload = false, enabled = true }: { id?: string; preload?: boolean; enabled?: boolean }) {
	const capture = useStageCapture(id, enabled, capturePicture({ followPreload: preload }));
	return <section data-testid={id}><NativeStageSurface pane={{ id, active: enabled, ref: () => {}, trouble: null, renderer: "test", send: vi.fn() }} onCapture={() => { void capture.start(); }} />
		<StageCaptureControls capture={capture} paneId={id} followPreload={preload} /></section>;
}
function mount(children = <Harness />, desktop = bridge) {
	return render(<DesktopProvider bridge={desktop}><ModalProvider>{children}</ModalProvider></DesktopProvider>);
}
function request() { return start.mock.calls.at(-1) as [string, string]; }
function status(phase: StageStillPhase, overrides: Partial<StageStillStatus> = {}): StageStillStatus {
	const [paneId, requestId] = request();
	return { paneId, requestId, jobId: `job-${requestId}`, followPreload: paneId === "preload", sceneRevision: 10, valuesFrame: 20,
		timeSeconds: 1.5, width: 3840, height: 2160, framing: "camera", engine: null, notice: null, phase, ...overrides };
}
function push(next: StageStillStatus | null, overrides: Partial<StageStillEvent> = {}) {
	const [paneId, requestId] = request();
	act(() => { for (const listener of listeners) listener({ paneId, requestId, status: next, error: null, ...overrides }); });
}
const choice = { needsChoice: { blenderVersion: "Blender test", ultimateReasons: [],
	ultraExplanation: "Ultra uses the regular ToskLight renderer.", ultimateExplanation: "Ultimate renders with Blender/Cycles." } } satisfies StageStillPhase;
beforeEach(() => { listeners.clear(); vi.clearAllMocks(); start.mockResolvedValue(undefined); choose.mockResolvedValue(undefined); cancel.mockResolvedValue(undefined); open.mockResolvedValue(undefined); });
afterEach(cleanup);

it("owns exact focused Stage shortcut without taking Save, text, repeats, composition or extra modifiers", async () => {
	const view = mount();
	await waitFor(() => expect(screen.getByRole("button", { name: /Render 4K Live/ })).toBeEnabled());
	const surface = view.container.querySelector<HTMLElement>("[data-native-stage]")!;
	const keys = { key: "r", ctrlKey: true, shiftKey: true };
	fireEvent.keyDown(surface, keys); expect(start).not.toHaveBeenCalled();
	surface.focus();
	for (const event of [{ ...keys, repeat: true }, { ...keys, isComposing: true }, { ...keys, altKey: true },
		{ ...keys, metaKey: true }, { key: "s", metaKey: true }]) fireEvent.keyDown(surface, event);
	expect(start).not.toHaveBeenCalled();
	const input = document.createElement("input"); surface.append(input); input.focus();
	fireEvent.keyDown(input, keys); expect(start).not.toHaveBeenCalled(); input.remove();
	surface.focus();
	fireEvent.keyDown(surface, keys); fireEvent.keyDown(surface, keys);
	expect(start).toHaveBeenCalledTimes(1); expect(request()[0]).toBe("live");
});

it("accepts the same shortcut on the focused own Render button and blocks it while another modal owns input", async () => {
	const view = mount();
	const button = await screen.findByRole("button", { name: /Render 4K Live/ });
	await waitFor(() => expect(button).toBeEnabled()); button.focus();
	fireEvent.keyDown(button, { key: "R", metaKey: true, shiftKey: true });
	expect(start).toHaveBeenCalledTimes(1);
	view.unmount(); start.mockClear();
	const blocked = mount(<><Harness /><ModalFrame title="Other modal" ariaLabel="Other modal" onClose={() => {}}><button type="button">Modal action</button></ModalFrame></>);
	await waitFor(() => expect(screen.getByRole("button", { name: /Render 4K Live/ })).toBeEnabled());
	const surface = blocked.container.querySelector<HTMLElement>("[data-native-stage]")!; surface.focus();
	fireEvent.keyDown(surface, { key: "r", ctrlKey: true, shiftKey: true });
	expect(start).not.toHaveBeenCalled();
});

it("routes Follow Preload and ignores other panes, stale requests and foreign jobs", async () => {
	mount(<><Harness /><Harness id="preload" preload /></>);
	const button = screen.getByRole("button", { name: /Render 4K Follow Preload/ });
	await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
	expect(request()[0]).toBe("preload");
	push(status(choice), { paneId: "live" });
	push(status(choice), { requestId: "stale" });
	expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	push(status(choice));
	const dialog = screen.getByRole("dialog", { name: "Render 4K quality" });
	expect(within(dialog).getByText(/Follow Preload scene frozen/)).toBeInTheDocument();
	expect(within(dialog).getByText(/Live quality remains unchanged/)).toBeInTheDocument();
	expect(within(dialog).getByText("camera")).toBeInTheDocument();
	push(status({ saved: { path: "/wrong.png" } }, { jobId: "foreign" }));
	expect(screen.queryByText("/wrong.png")).not.toBeInTheDocument();
});

it("explains unavailable Ultimate honestly, chooses Ultra and cancels the owned native job", async () => {
	mount();
	const button = screen.getByRole("button", { name: /Render 4K Live/ });
	await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
	const unsupported = { needsChoice: { ...choice.needsChoice,
		ultimateReasons: ["Frozen media cannot be reproduced"] } } as StageStillPhase;
	push(status(unsupported));
	const dialog = screen.getByRole("dialog");
	expect(within(dialog).getByText("Frozen media cannot be reproduced")).toBeInTheDocument();
	expect(within(dialog).getByRole("button", { name: "Ultimate · Blender" })).toBeDisabled();
	fireEvent.click(within(dialog).getByRole("button", { name: "Ultra · ToskLight" }));
	expect(choose).toHaveBeenCalledWith("live", `job-${request()[1]}`, "ultra");
	push(status({ rendering: { detail: "Rendering full-quality frozen image" } }, { engine: "ultra", notice: "Blender unavailable; using Ultra" }));
	expect(screen.getByText("Blender unavailable; using Ultra")).toBeInTheDocument();
	fireEvent.click(screen.getByRole("button", { name: "Cancel render" }));
	expect(cancel).toHaveBeenCalledWith("live", `job-${request()[1]}`);
	push(status("cancelled"));
	expect(screen.getByText("4K render cancelled")).toBeInTheDocument();
});

it("shows saved path and owned Open/Reveal actions, then rejects late terminal observations", async () => {
	mount(); const button = screen.getByRole("button", { name: /Render 4K Live/ });
	await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
	push(status({ saved: { path: "/captures/frozen-4k.png" } }, { engine: "ultra" }));
	expect(screen.getByText("/captures/frozen-4k.png")).toBeInTheDocument();
	expect(screen.getByText("Ultra · ToskLight · 3840 × 2160")).toBeInTheDocument();
	expect(screen.getByText("camera")).toBeInTheDocument();
	fireEvent.click(screen.getByRole("button", { name: "Open image" })); fireEvent.click(screen.getByRole("button", { name: "Reveal image" }));
	expect(open.mock.calls).toEqual([["live", false], ["live", true]]);
	push(status({ failed: { detail: "late failure" } }));
	expect(screen.queryByText("late failure")).not.toBeInTheDocument();
	const oldRequest = request()[1]; fireEvent.click(button);
	push(status({ saved: { path: "/stale.png" } }, { requestId: oldRequest }), { requestId: oldRequest });
	expect(screen.queryByText("/stale.png")).not.toBeInTheDocument();
	push(null, { error: "Capture interrupted: renderer closed" });
	expect(screen.getByRole("alert")).toHaveTextContent("Capture interrupted");
	expect(button).toBeEnabled();
});

it("leaves browser capture explicitly unavailable and unsubscribes without polling", async () => {
	const view = mount(<Harness />, browserDesktopBridge);
	expect(screen.getByRole("button", { name: /Render 4K Live/ })).toBeDisabled();
	expect(screen.getByText("4K rendering requires native Stage.")).toBeInTheDocument();
	view.unmount();
	const native = mount(); await waitFor(() => expect(listeners.size).toBe(1));
	native.unmount(); expect(stop).toHaveBeenCalledOnce(); expect(listeners.size).toBe(0);
});

it("selects usable Ultimate through the same frozen in-app request", async () => {
	mount(); const button = screen.getByRole("button", { name: /Render 4K Live/ });
	await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
	push(status(choice));
	const dialog = screen.getByRole("dialog");
	expect(within(dialog).getByText("Ultimate renders with Blender/Cycles.")).toBeInTheDocument();
	fireEvent.click(within(dialog).getByRole("button", { name: "Ultimate · Blender" }));
	expect(choose).toHaveBeenCalledWith("live", `job-${request()[1]}`, "ultimate");
	expect(start).toHaveBeenCalledTimes(1);
});

it("does not enable start before event subscription and disposes late subscription completion", async () => {
	let resolve: ((stop: () => void) => void) | undefined;
	const lateStop = vi.fn();
	const desktop = { ...bridge, onStageStill: () => new Promise<() => void>((done) => { resolve = done; }) };
	const view = mount(<Harness />, desktop);
	expect(screen.getByRole("button", { name: /Render 4K Live/ })).toBeDisabled();
	view.unmount();
	await act(async () => { resolve?.(lateStop); });
	expect(lateStop).toHaveBeenCalledOnce(); expect(start).not.toHaveBeenCalled();
});
