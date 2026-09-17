import { describe, expect, it, vi } from "vitest";
import type { ScreenAttachment } from "../../api/client/screenAttachment";
import type { ScreenConfiguration } from "../../api/types";
import type { DesktopBridge } from "../../platform/desktop";
import { reconcileScreenWindows } from "./ScreenWindowManager";

function screen(
	overrides: Partial<ScreenConfiguration> = {},
): ScreenConfiguration {
	return {
		id: "stage",
		name: "Stage",
		desired_open: true,
		display_id: "display-1",
		bounds: null,
		fullscreen: true,
		...overrides,
	} as ScreenConfiguration;
}

function desktop() {
	return {
		listDisplays: vi
			.fn()
			.mockResolvedValue([{ id: "display-1", name: "Display" }]),
		openConsoleScreen: vi.fn().mockResolvedValue(undefined),
		hideConsoleScreen: vi.fn().mockResolvedValue(undefined),
		closeConsoleScreen: vi.fn().mockResolvedValue(undefined),
	} as unknown as DesktopBridge;
}

describe("screen window reconciliation", () => {
	it("does not reapply unchanged geometry during display polling", async () => {
		const bridge = desktop();
		const state = new Map<string, string>();
		await reconcileScreenWindows(bridge, [screen()], state, () => false);
		await reconcileScreenWindows(bridge, [screen()], state, () => false);
		expect(bridge.openConsoleScreen).toHaveBeenCalledOnce();
	});

	it("hides for a missing display and reopens when it returns", async () => {
		const bridge = desktop();
		const state = new Map<string, string>();
		vi.mocked(bridge.listDisplays).mockResolvedValueOnce([]);
		await reconcileScreenWindows(bridge, [screen()], state, () => false);
		expect(bridge.hideConsoleScreen).toHaveBeenCalledWith("stage");
		await reconcileScreenWindows(bridge, [screen()], state, () => false);
		expect(bridge.openConsoleScreen).toHaveBeenCalledOnce();
	});

	it("applies changed configuration and closes removed screens", async () => {
		const bridge = desktop();
		const state = new Map<string, string>();
		await reconcileScreenWindows(bridge, [screen()], state, () => false);
		await reconcileScreenWindows(
			bridge,
			[screen({ fullscreen: false, name: "Audience Stage" })],
			state,
			() => false,
		);
		expect(bridge.openConsoleScreen).toHaveBeenCalledTimes(2);
		await reconcileScreenWindows(bridge, [], state, () => false);
		expect(bridge.closeConsoleScreen).toHaveBeenCalledWith("stage");
	});

	it("opens screens on the desk's own server and session and re-hands a new session", async () => {
		const bridge = desktop();
		const state = new Map<string, string>();
		const attachment = {
			server_url: "http://127.0.0.1:5471",
			session: { session_id: "s1", token: "t1" },
			desk_token: null,
		} as unknown as ScreenAttachment;
		await reconcileScreenWindows(
			bridge,
			[screen()],
			state,
			() => false,
			attachment,
		);
		expect(bridge.openConsoleScreen).toHaveBeenLastCalledWith(
			expect.objectContaining({ screenId: "stage", attachment }),
		);
		await reconcileScreenWindows(
			bridge,
			[screen()],
			state,
			() => false,
			attachment,
		);
		expect(bridge.openConsoleScreen).toHaveBeenCalledOnce();
		const reconnected = {
			...attachment,
			session: { session_id: "s2", token: "t2" },
		} as unknown as ScreenAttachment;
		await reconcileScreenWindows(
			bridge,
			[screen()],
			state,
			() => false,
			reconnected,
		);
		expect(bridge.openConsoleScreen).toHaveBeenCalledTimes(2);
		expect(bridge.openConsoleScreen).toHaveBeenLastCalledWith(
			expect.objectContaining({ attachment: reconnected }),
		);
	});
});
