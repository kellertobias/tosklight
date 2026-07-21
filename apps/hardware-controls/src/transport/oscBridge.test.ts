import { describe, expect, it, vi } from "vitest";
import {
	OSC_TEST_CONTROL,
	type ControllableOscWindow,
} from "./controllableOscBridge";
import { createOscBridge, type OscBridge, tauriOscBridge } from "./oscBridge";

describe("OSC bridge selection", () => {
	it("uses Tauri when no controllable browser port was installed", () => {
		expect(createOscBridge({} as ControllableOscWindow)).toBe(tauriOscBridge);
	});

	it("uses only the explicitly installed controllable port", async () => {
		const bridge: OscBridge = {
			connect: vi.fn().mockResolvedValue(undefined),
			send: vi.fn().mockResolvedValue(undefined),
			listenFeedback: vi.fn().mockResolvedValue(() => undefined),
		};
		const runtime = { [OSC_TEST_CONTROL]: bridge } as ControllableOscWindow;
		const selected = createOscBridge(runtime);
		expect(selected).toBe(bridge);
		await selected.send("programmer/record", [true]);
		expect(bridge.send).toHaveBeenCalledWith("programmer/record", [true]);
	});

	it("refuses an incomplete injected authority", () => {
		const runtime = {
			[OSC_TEST_CONTROL]: { send: vi.fn() },
		} as unknown as ControllableOscWindow;
		expect(createOscBridge(runtime)).toBe(tauriOscBridge);
	});
});
