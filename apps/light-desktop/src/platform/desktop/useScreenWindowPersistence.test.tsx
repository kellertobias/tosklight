import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { browserDesktopBridge } from "./browserDesktopBridge";
import { DesktopProvider } from "./DesktopContext";
import type { DesktopBridge } from "./types";
import { useScreenWindowPersistence } from "./useScreenWindowPersistence";

const screen: ScreenConfiguration = {
	id: "stage",
	name: "Stage",
	desired_open: true,
	display_id: null,
	bounds: null,
	fullscreen: false,
	layout: { desks: [], activeDeskId: "desk" },
	show_dock: true,
	show_playbacks: true,
	show_page_controls: true,
	page_mode: "follow_main",
	first_playback_slot: 1,
	playback_count: 10,
	playback_rows: 1,
	playback_layout: null,
};

function Harness({ save }: { save: (value: ScreenConfiguration) => Promise<void> }) {
	useScreenWindowPersistence(screen, save);
	return null;
}

describe("screen window persistence", () => {
	afterEach(() => vi.useRealTimers());

	it("persists native geometry and closes through the desktop owner", async () => {
		vi.useFakeTimers();
		let moved: (() => void) | undefined;
		let close: (() => void | Promise<void>) | undefined;
		const destroy = vi.fn().mockResolvedValue(undefined);
		const bridge: DesktopBridge = {
			...browserDesktopBridge,
			available: true,
			currentWindowState: vi.fn().mockResolvedValue({
				displayId: "display-1",
				bounds: { x: 10, y: 20, width: 800, height: 600 },
				fullscreen: true,
			}),
			destroyCurrentWindow: destroy,
			onCurrentWindowMoved: async (handler) => {
				moved = handler;
				return () => undefined;
			},
			onCurrentWindowCloseRequested: async (handler) => {
				close = handler;
				return () => undefined;
			},
		};
		const save = vi.fn().mockResolvedValue(undefined);
		render(
			<DesktopProvider bridge={bridge}>
				<Harness save={save} />
			</DesktopProvider>,
		);
		await act(async () => {
			await Promise.resolve();
			await Promise.resolve();
		});
		expect(moved).toBeTypeOf("function");
		await act(async () => {
			moved?.();
			await vi.advanceTimersByTimeAsync(300);
		});
		expect(save).toHaveBeenCalledWith({
			...screen,
			display_id: "display-1",
			bounds: { x: 10, y: 20, width: 800, height: 600 },
			fullscreen: true,
		});
		await act(async () => close?.());
		expect(save).toHaveBeenLastCalledWith({ ...screen, desired_open: false });
		expect(destroy).toHaveBeenCalledOnce();
	});
});
