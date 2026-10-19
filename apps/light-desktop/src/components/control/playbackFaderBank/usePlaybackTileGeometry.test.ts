import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { usePlaybackTileGeometry } from "./usePlaybackTileGeometry";

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

it("follows viewport resizes with exactly the authored rows and disconnects outside tile mode", () => {
	let resized: ResizeObserverCallback = () => {};
	const observe = vi.fn();
	const disconnect = vi.fn();
	vi.stubGlobal(
		"ResizeObserver",
		class {
			constructor(callback: ResizeObserverCallback) {
				resized = callback;
			}
			observe = observe;
			disconnect = disconnect;
		},
	);
	const viewport = document.createElement("div");
	const bank = document.createElement("div");
	bank.style.columnGap = "5px";
	bank.style.rowGap = "5px";
	const size = (width: number, height: number) => {
		Object.defineProperty(viewport, "clientWidth", {
			configurable: true,
			value: width,
		});
		Object.defineProperty(viewport, "clientHeight", {
			configurable: true,
			value: height,
		});
	};
	size(1000, 250);
	const { result, rerender } = renderHook(
		({ enabled }) => usePlaybackTileGeometry(enabled, viewport, bank, 6, 3),
		{ initialProps: { enabled: true } },
	);
	expect(result.current).toMatchObject({
		gridTemplateColumns: "repeat(6, 162.5px)",
		gridTemplateRows: "repeat(3, 80px)",
		width: 1000,
		height: 250,
	});
	expect(observe).toHaveBeenCalledWith(viewport);
	expect(observe).toHaveBeenCalledWith(bank);
	act(() => {
		size(325, 550);
		resized([], {} as ResizeObserver);
	});
	expect(result.current).toMatchObject({
		gridTemplateColumns: "repeat(6, 50px)",
		gridTemplateRows: "repeat(3, 180px)",
		width: 325,
		height: 550,
	});
	rerender({ enabled: false });
	expect(result.current).toBeNull();
	expect(disconnect).toHaveBeenCalledOnce();
});
