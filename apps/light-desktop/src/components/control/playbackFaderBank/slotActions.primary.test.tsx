import { PlaybackTileView, type PlaybackCardViewModel } from "@tosklight/ui/playback";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AssignedPlayback } from "./types";
import type { PlaybackBankController } from "./controller";
import { buildPlaybackActions, createSlotInterceptors } from "./slotActions";
import { playbackRequest } from "../../../features/server/playbackActionMapping";

const commands = vi.hoisted(() => ({
	assignDynamicPlayback: vi.fn(), assignGroupPlayback: vi.fn(),
	openPlaybackConfiguration: vi.fn(), recordPlayback: vi.fn(),
	requestPlaybackUpdate: vi.fn(), selectPlayback: vi.fn(),
}));
vi.mock("./actions", () => ({ ...commands,
	isPlaybackControlTarget: () => false,
	isPlaybackSetClickArmed: (controller: PlaybackBankController) => controller.state.playbackSetArmed,
}));
afterEach(cleanup);
beforeEach(() => vi.clearAllMocks());

function setup(changes: Partial<AssignedPlayback> = {}) {
	const playback = {
		address: { page: 3, position: 402 }, name: "Opening",
		target: { type: "cue_list", cue_list_id: "list-1" },
		buttons: ["go_minus", "go", "flash"], button_count: 3,
		...changes,
	} as AssignedPlayback;
	const controller = {
		state: { storeArmed: false, updateArmed: false, shiftArmed: false, playbackSetArmed: false },
		heldActions: { press: vi.fn(), releaseGesture: vi.fn() },
		runtimeActions: { playbackAction: vi.fn() }, runtimeProjections: new Map(),
		controlSourceId: "software:tile", assignmentPending: false, hardware: false,
	} as unknown as PlaybackBankController;
	const build = () => buildPlaybackActions({ controller, playback, active: undefined, selected: false, slot: 402, currentCue: null, buttonCount: 1 });
	const model: PlaybackCardViewModel = {
		page: 3, slot: 402, row: 0, rowUnits: 1, name: "Opening", assigned: true,
		hasFader: false, faderValue: 0, faderLabel: "Master", faderDisplay: "0%", actions: [],
		artwork: { image: "cue.png" },
	};
	const show = () => {
        const { interceptPointer, interceptClick } = createSlotInterceptors(controller, playback, null, 402, null, 1);
        return render(<PlaybackTileView model={model} action={build().primaryAction} callbacks={{ onPointerDownCapture: interceptPointer, onClickCapture: interceptClick }} />);
    };
	return { playback, controller, build, show };
}

describe("primary Playback tile actions", () => {
	it("does not engage a held action while Shift configures the tile from the keyboard", () => {
		const { controller, show } = setup({ button_count: 1, buttons: ["flash", "none", "none"] });
		show();
		fireEvent.keyDown(screen.getByRole("button"), { key: "Enter", shiftKey: true });
		fireEvent.click(screen.getByRole("button"), { shiftKey: true });
		expect(controller.heldActions.press).not.toHaveBeenCalled();
		expect(controller.runtimeActions!.playbackAction).not.toHaveBeenCalled();
		expect(commands.openPlaybackConfiguration).toHaveBeenCalledTimes(1);
	});
	it("chooses configured GO with its original second-button identity even in a one-button row", () => {
		const { controller, build, show } = setup();
		expect(build().faderActions[0].label).toBe("GO −");
		expect(build().primaryAction.label).toBe("GO +");
		show(); fireEvent.click(screen.getByText("Opening"));
		expect(controller.runtimeActions!.playbackAction).toHaveBeenCalledExactlyOnceWith(
			{ page: 3, position: 402 }, "button", { button: 2, pressed: true, control_source: { surface_id: "software:tile", control_id: 2 } },
		);
	});

	it.each(["macro", "timecode"] as const)("uses GO for a %s tile", type => {
		const { controller, show } = setup({ target: { type } as AssignedPlayback["target"], buttons: ["pause", "go", "off"] });
		show(); fireEvent.click(screen.getByRole("button"));
		expect(controller.runtimeActions!.playbackAction).toHaveBeenCalledWith(expect.anything(), "button", expect.objectContaining({ button: 2 }));
	});

	it("chooses a configured Dynamic toggle without changing its button identity", () => {
		const { controller, show } = setup({ target: { type: "dynamic" } as AssignedPlayback["target"], buttons: ["off", "flash", "toggle"] });
		show(); fireEvent.click(screen.getByRole("button"));
		expect(controller.runtimeActions!.playbackAction).toHaveBeenCalledWith(expect.anything(), "button", expect.objectContaining({ button: 3, control_source: { surface_id: "software:tile", control_id: 3 } }));
	});

	it.each(["cue_list", "dynamic"] as const)("uses a typed direct action when %s has no configured preferred button", type => {
		const { controller, show } = setup({ target: { type } as AssignedPlayback["target"], buttons: ["off", "pause", "flash"] });
		show(); fireEvent.click(screen.getByRole("button"));
		const action = type === "dynamic" ? "toggle" : "go";
		expect(controller.runtimeActions!.playbackAction).toHaveBeenCalledWith(expect.anything(), action, expect.objectContaining({ pressed: true }));
		const [address, sent, input] = vi.mocked(controller.runtimeActions!.playbackAction).mock.calls[0];
		expect(playbackRequest(address, sent, input).action).toEqual({ type: action, pressed: true });
	});

	it.each(["flash", "swap"] as const)("honors an explicitly configured single %s including pointer/key release", action => {
		const { controller, build, show } = setup({ button_count: 1, buttons: [action, "go", "off"] });
		expect(build().primaryAction.label).toBe(action.toUpperCase());
		show(); const button = screen.getByRole("button");
		fireEvent.pointerDown(button, { pointerId: 7 }); fireEvent.pointerUp(button, { pointerId: 7 });
		fireEvent.keyDown(button, { key: "Enter" }); fireEvent.keyUp(button, { key: "Enter" });
		expect(controller.heldActions.press).toHaveBeenCalledTimes(2);
		expect(controller.heldActions.press).toHaveBeenLastCalledWith("software:tile", { page: 3, position: 402 }, 1, action, { key: "Enter" });
		expect(controller.heldActions.releaseGesture).toHaveBeenCalledTimes(2);
		expect(controller.runtimeActions!.playbackAction).not.toHaveBeenCalled();
	});

	it("Shift capture configures a held tile without pressing Flash", () => {
		const { controller, show } = setup({ button_count: 1, buttons: ["flash", "go", "off"] });
		controller.state.shiftArmed = true;
		show(); fireEvent.pointerDown(screen.getByRole("button"), { pointerId: 7 });
		expect(commands.openPlaybackConfiguration).toHaveBeenCalledOnce();
		expect(controller.heldActions.press).not.toHaveBeenCalled();
	});

	it("keeps all-none and deliberately disabled single-button assignments disabled", () => {
		const all = setup({ buttons: ["none", "none", "none"] });
		expect(all.build().primaryAction.disabled).toBe(true);
		const single = setup({ button_count: 1, buttons: ["none", "go", "flash"] });
		expect(single.build().primaryAction.disabled).toBe(true);
	});

	it.each(["record", "update", "set", "select"])("preserves %s capture instead of firing the tile", mode => {
		const { controller, show } = setup();
		if (mode === "record") controller.state.storeArmed = true;
		if (mode === "update") controller.state.updateArmed = true;
		if (mode === "set") controller.state.playbackSetArmed = true;
		if (mode === "select") controller.selectionPending = true;
		show(); fireEvent.click(screen.getByText("Opening"));
		const command = { record: commands.recordPlayback, update: commands.requestPlaybackUpdate, set: commands.openPlaybackConfiguration, select: commands.selectPlayback }[mode];
		expect(command).toHaveBeenCalledOnce();
		expect(controller.runtimeActions!.playbackAction).not.toHaveBeenCalled();
	});

	it("Shift configures the primary tile even when GO belongs to button two", () => {
		const { controller, show } = setup();
		show(); fireEvent.click(screen.getByRole("button"), { shiftKey: true });
		expect(commands.openPlaybackConfiguration).toHaveBeenCalledOnce();
		expect(controller.runtimeActions!.playbackAction).not.toHaveBeenCalled();
	});
});
