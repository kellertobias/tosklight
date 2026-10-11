import { cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TouchPlaybackCardView, type PlaybackCardViewModel } from "./PlaybackCards";

import type { ReactElement } from "react";
import { ModalProvider } from "../modals/ModalStack";
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });
afterEach(() => { cleanup(); vi.useRealTimers(); });
const model: PlaybackCardViewModel = {
	page: 2, slot: 301, row: 0, rowUnits: 2, name: "Bump", assigned: true,
	presentation: "push_encoder", hasFader: true, faderValue: 50,
	faderLabel: "Master", faderDisplay: "50%", actions: [],
};
describe("Playback encoder center", () => {
	it("drags the dial level and stops on cancellation without pressing its center action", () => {
		vi.useFakeTimers();
		const change = vi.fn();
		const press = vi.fn();
		render(<TouchPlaybackCardView model={{ ...model, actions: [{ id: "go", label: "GO", onClick: press }] }} callbacks={{ onFaderChange: change }} />);
		const dial = screen.getByRole("group", { name: "Master" });
		fireEvent.pointerDown(dial, { pointerId: 12, button: 0, clientY: 100 });
		fireEvent.pointerMove(dial, { pointerId: 12, clientY: 70 });
		expect(change).toHaveBeenCalled();
		expect(change.mock.calls[0][0]).toBeGreaterThan(50);
		fireEvent.pointerCancel(dial, { pointerId: 12 });
		const count = change.mock.calls.length;
		vi.advanceTimersByTime(800);
		expect(change).toHaveBeenCalledTimes(count);
		expect(press).not.toHaveBeenCalled();
	});
	it("sets an absolute level through the separate numeric control without activating the Playback", () => {
		const change = vi.fn();
		const press = vi.fn();
		render(<TouchPlaybackCardView model={{ ...model, actions: [{ id: "go", label: "GO", onClick: press }] }} callbacks={{ onFaderChange: change }} />);
		fireEvent.click(screen.getByRole("button", { name: "Set Master value" }));
		expect(screen.getByRole("textbox", { name: "Master value" })).toHaveTextContent("50");
		fireEvent.click(screen.getByRole("button", { name: "7" }));
		fireEvent.click(screen.getByRole("button", { name: "5" }));
		fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
		expect(change).toHaveBeenCalledWith(75);
		expect(press).not.toHaveBeenCalled();
	});
	it("displays the normalized current position as a rotary dial and retains mode feedback", () => {
		const { container, rerender } = render(<TouchPlaybackCardView model={{ ...model, faderValue: 99.9, faderDisplay: "300 BPM", actions: [{ id: "go", label: "GO" }] }} />);
		expect(container.querySelector(".playback-rotary-control")).toHaveAttribute("data-position", "99.9");
		expect(screen.getByText("99.9%")).toBeInTheDocument();
		expect(screen.getByText("300 BPM")).toBeInTheDocument();
		rerender(<TouchPlaybackCardView model={{ ...model, faderValue: 25, actions: [{ id: "go", label: "GO" }] }} />);
		expect(container.querySelector(".playback-rotary-control")).toHaveAttribute("data-position", "25");
		expect(screen.getByText("25%")).toBeInTheDocument();
	});
	it("presses the configured button on the actual center while numeric entry and rotation only change the fader", () => {
		const press = vi.fn();
		const change = vi.fn();
		const { container } = render(<TouchPlaybackCardView model={{ ...model, actions: [{ id: "go", label: "GO", onClick: press }] }} callbacks={{ onFaderChange: change }} />);
		fireEvent.click(container.querySelector(".touch-encoder-value")!);
		expect(press).toHaveBeenCalledOnce();
		expect(screen.queryByRole("dialog")).toBeNull();
		fireEvent.keyDown(screen.getByRole("group", { name: "Master" }), { key: "ArrowUp" });
		expect(change).toHaveBeenCalledWith(50.1);
		fireEvent.click(screen.getByRole("button", { name: "Set Master value" }));
		expect(screen.getByRole("dialog")).toBeInTheDocument();
		expect(press).toHaveBeenCalledOnce();
	});
	it.each(["pointerUp", "pointerCancel"] as const)("forwards center hold and %s to the captured address after page change", (finish) => {
		let page = 2;
		let captured: number | undefined;
		const releases: number[] = [];
		const click = vi.fn();
		const action = { id: "flash", label: "FLASH", onClick: click,
			onPointerDown: () => { captured = page; },
			onPointerUp: () => { releases.push(captured!); },
			onPointerCancel: () => { releases.push(captured!); },
		};
		const { container, rerender } = render(<TouchPlaybackCardView model={{ ...model, actions: [action] }} />);
		fireEvent.pointerDown(container.querySelector(".touch-encoder-value")!, { pointerId: 7 });
		page = 4;
		rerender(<TouchPlaybackCardView model={{ ...model, page, actions: [action] }} />);
		fireEvent[finish](container.querySelector(".touch-encoder-value")!, { pointerId: 7 });
		expect(releases).toEqual([2]);
		expect(click).not.toHaveBeenCalled();
	});
	it("forwards keyboard hold/release without opening numeric entry", () => {
		const down = vi.fn(); const up = vi.fn();
		const { container } = render(<TouchPlaybackCardView model={{ ...model, actions: [{ id: "flash", label: "FLASH", onKeyDown: down, onKeyUp: up }] }} />);
		const center = container.querySelector(".touch-encoder-value")!;
		fireEvent.keyDown(center, { key: " ", repeat: false });
		fireEvent.keyUp(center, { key: " " });
		expect(down).toHaveBeenCalledOnce(); expect(up).toHaveBeenCalledOnce();
		expect(screen.queryByRole("dialog")).toBeNull();
	});
});
