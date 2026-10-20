import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { browserDesktopBridge } from "../../platform/desktop/browserDesktopBridge";
import { DesktopProvider } from "../../platform/desktop/DesktopContext";
import type { DesktopBridge } from "../../platform/desktop/types";
import { NativeDragStrip } from "./NativeDragStrip";

afterEach(cleanup);

function nativeShell(overrides: Partial<DesktopBridge>): DesktopBridge {
	return { ...browserDesktopBridge, available: true, ...overrides };
}

describe("native window controls", () => {
	it("renders no desktop chrome in an ordinary browser", () => {
		render(<NativeDragStrip />);

		expect(screen.queryByRole("button")).not.toBeInTheDocument();
		expect(
			document.querySelector(".native-drag-strip"),
		).not.toBeInTheDocument();
	});

	it("provides close, fullscreen, and drag controls in the native shell", () => {
		render(
			<DesktopProvider bridge={nativeShell({})}>
				<NativeDragStrip />
			</DesktopProvider>,
		);
		expect(
			screen.getByRole("button", { name: "Close window" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Enter fullscreen" }),
		).toHaveAttribute("aria-pressed", "false");
		expect(screen.getByRole("button", { name: "Move window" })).toHaveAttribute(
			"data-tauri-drag-region",
		);
	});

	it("enters fullscreen through the distinct desktop action", async () => {
		const setCurrentWindowFullscreen = vi.fn().mockResolvedValue(undefined);
		render(
			<DesktopProvider
				bridge={nativeShell({
					currentWindowFullscreen: vi.fn().mockResolvedValue(false),
					setCurrentWindowFullscreen,
				})}
			>
				<NativeDragStrip />
			</DesktopProvider>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Enter fullscreen" }));
		await waitFor(() =>
			expect(setCurrentWindowFullscreen).toHaveBeenCalledWith(true),
		);
		expect(
			await screen.findByRole("button", { name: "Exit fullscreen" }),
		).toBeInTheDocument();
	});

	it("shows only Exit fullscreen and never closes the app while fullscreen", async () => {
		const closeCurrentWindow = vi.fn().mockResolvedValue(undefined);
		const exitApplication = vi.fn().mockResolvedValue(undefined);
		const setCurrentWindowFullscreen = vi.fn().mockResolvedValue(undefined);
		render(
			<DesktopProvider
				bridge={nativeShell({
					currentWindowFullscreen: vi.fn().mockResolvedValue(true),
					setCurrentWindowFullscreen,
					closeCurrentWindow,
					exitApplication,
				})}
			>
				<NativeDragStrip />
			</DesktopProvider>,
		);

		const exit = await screen.findByRole("button", { name: "Exit fullscreen" });
		expect(
			screen.queryByRole("button", { name: "Close window" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Move window" }),
		).not.toBeInTheDocument();
		fireEvent.click(exit);
		await waitFor(() =>
			expect(setCurrentWindowFullscreen).toHaveBeenCalledWith(false),
		);
		expect(closeCurrentWindow).not.toHaveBeenCalled();
		expect(exitApplication).not.toHaveBeenCalled();
		expect(
			await screen.findByRole("button", { name: "Enter fullscreen" }),
		).toBeInTheDocument();
	});

	it("keeps compact Screen fullscreen exit accessible without replacing normal drag controls", async () => {
		const setCurrentWindowFullscreen = vi.fn().mockResolvedValue(undefined);
		render(
			<DesktopProvider bridge={nativeShell({
				currentWindowFullscreen: vi.fn().mockResolvedValue(true),
				setCurrentWindowFullscreen,
			})}>
				<NativeDragStrip compactFullscreen />
			</DesktopProvider>,
		);
		const exit = await screen.findByRole("button", { name: "Exit fullscreen" });
		expect(exit).toHaveAttribute("title", "Exit fullscreen");
		expect(exit).not.toHaveTextContent("Exit fullscreen");
		expect(exit.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
		expect(exit.parentElement).toHaveClass("native-drag-strip-compact-fullscreen");
		fireEvent.click(exit);
		await waitFor(() => expect(setCurrentWindowFullscreen).toHaveBeenCalledWith(false));
		expect(await screen.findByRole("button", { name: "Move window" })).toHaveAttribute("data-tauri-drag-region");
	});

	it("tracks delayed native fullscreen transitions and releases its resize listener", async () => {
		let fullscreen = false;
		let resized: (() => void) | undefined;
		const stop = vi.fn();
		const bridge = nativeShell({
			currentWindowFullscreen: vi.fn(async () => fullscreen),
			onCurrentWindowResized: vi.fn(async (handler) => { resized = handler; return stop; }),
		});
		const { unmount } = render(<DesktopProvider bridge={bridge}><NativeDragStrip compactFullscreen /></DesktopProvider>);
		await waitFor(() => expect(bridge.onCurrentWindowResized).toHaveBeenCalledOnce());
		await screen.findByRole("button", { name: "Enter fullscreen" });
		// Native transition completes after initial mounting; Exit must replace stale controls.
		fullscreen = true; resized?.();
		await screen.findByRole("button", { name: "Exit fullscreen" });
		fullscreen = false; resized?.();
		await screen.findByRole("button", { name: "Move window" });
		unmount();
		expect(stop).toHaveBeenCalledOnce();
	});

	it("unsubscribes a native listener that arrives after the screen closes", async () => {
		let finish: ((stop: () => void) => void) | undefined;
		const stop = vi.fn();
		const bridge = nativeShell({ onCurrentWindowResized: () => new Promise(resolve => { finish = resolve; }) });
		const { unmount } = render(<DesktopProvider bridge={bridge}><NativeDragStrip /></DesktopProvider>);
		unmount(); finish?.(stop);
		await waitFor(() => expect(stop).toHaveBeenCalledOnce());
	});

	it("closes only its own window on a screen", () => {
		const closeCurrentWindow = vi.fn().mockResolvedValue(undefined);
		const exitApplication = vi.fn().mockResolvedValue(undefined);
		render(
			<DesktopProvider
				bridge={nativeShell({ closeCurrentWindow, exitApplication })}
			>
				<NativeDragStrip />
			</DesktopProvider>,
		);
		screen.getByRole("button", { name: "Close window" }).click();
		expect(closeCurrentWindow).toHaveBeenCalledOnce();
		expect(exitApplication).not.toHaveBeenCalled();
	});

	it("quits the desk from the main window", () => {
		const closeCurrentWindow = vi.fn().mockResolvedValue(undefined);
		const exitApplication = vi.fn().mockResolvedValue(undefined);
		render(
			<DesktopProvider
				bridge={nativeShell({ closeCurrentWindow, exitApplication })}
			>
				<NativeDragStrip closes="application" />
			</DesktopProvider>,
		);
		screen.getByRole("button", { name: "Quit ToskLight" }).click();
		expect(exitApplication).toHaveBeenCalledOnce();
		expect(closeCurrentWindow).not.toHaveBeenCalled();
	});
});
