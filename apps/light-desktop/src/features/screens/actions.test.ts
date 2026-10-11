import { describe, expect, it, vi } from "vitest";
import type { LightApi } from "../../api/client/api";
import type { ScreenConfiguration, ScreenSnapshot } from "../../api/types";
import { createScreenActions } from "./actions";

function harness() {
	const putScreen = vi.fn();
	const screens = vi.fn();
	const setError = vi.fn();
	const setScreens = vi.fn();
	const api = { playback: { putScreen, screens } } as unknown as LightApi;
	return {
		putScreen,
		screens,
		setError,
		setScreens,
		actions: createScreenActions({ api, setError, setScreens }),
	};
}

describe("screen save outcomes", () => {
	it("keeps background screen saves report-only without unhandled rejection", async () => {
		const h = harness();
		h.putScreen.mockRejectedValue(new Error("Background save failed"));
		await expect(
			h.actions.saveScreen({ id: "saved" } as ScreenConfiguration),
		).resolves.toBeUndefined();
		expect(h.setError).toHaveBeenCalledWith(
			expect.stringContaining("Background save failed"),
		);
		expect(h.setScreens).not.toHaveBeenCalled();
	});
	it("reports and propagates refusal so authored creation cannot close as success", async () => {
		const h = harness();
		const refusal = new Error("Unknown Playback surface screen:new");
		h.putScreen.mockRejectedValue(refusal);
		await expect(
			h.actions.saveScreen({ id: "new" } as ScreenConfiguration, {
				throwOnError: true,
			}),
		).rejects.toBe(refusal);
		expect(h.setError).toHaveBeenCalledWith(
			expect.stringContaining("Unknown Playback surface"),
		);
		expect(h.screens).not.toHaveBeenCalled();
		expect(h.setScreens).not.toHaveBeenCalled();
	});
	it("publishes the authoritative screen snapshot only after a successful save", async () => {
		const h = harness();
		const saved = { id: "new" } as ScreenConfiguration;
		const snapshot = { screens: [saved] } as ScreenSnapshot;
		h.putScreen.mockResolvedValue(saved);
		h.screens.mockResolvedValue(snapshot);
		await h.actions.saveScreen(saved);
		expect(h.putScreen).toHaveBeenCalledWith(saved);
		expect(h.setScreens).toHaveBeenCalledWith(snapshot);
		expect(h.setError).toHaveBeenCalledWith(null);
	});
});
