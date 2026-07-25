import { describe, expect, it } from "vitest";
import { browserDesktopBridge } from "./browserDesktopBridge";
import {
	DESKTOP_TEST_CONTROL,
	type ControllableDesktopPort,
} from "./controllableBrowserDesktopBridge";
import { createDesktopBridge } from "./index";
import { tauriDesktopBridge } from "./tauriDesktopBridge";

const port: ControllableDesktopPort = {
	perform: () => undefined,
	listDisplays: () => [],
	currentWindowState: () => ({
		displayId: null,
		bounds: { x: 0, y: 0, width: 0, height: 0 },
		fullscreen: false,
	}),
	subscribe: () => () => undefined,
};

describe("desktop bridge selection", () => {
	it("uses the inert browser adapter without desktop authority", () => {
		expect(createDesktopBridge({} as Window)).toBe(browserDesktopBridge);
	});

	it("prefers an explicitly installed browser-test adapter", () => {
		const runtime = {
			__TAURI_INTERNALS__: {},
			[DESKTOP_TEST_CONTROL]: port,
		} as unknown as Window;
		const bridge = createDesktopBridge(runtime);
		expect(bridge.available).toBe(true);
		expect(bridge).not.toBe(tauriDesktopBridge);
	});

	it("uses Tauri only when the native runtime marker exists", () => {
		expect(
			createDesktopBridge({ __TAURI_INTERNALS__: {} } as unknown as Window),
		).toBe(tauriDesktopBridge);
	});
});
