import { cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TouchPlaybackCardView, type PlaybackCardViewModel } from "./PlaybackCards";

import type { ReactElement } from "react";
import { ModalProvider } from "../modals/ModalStack";
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });
afterEach(cleanup);
const model: PlaybackCardViewModel = {
	page: 2, slot: 301, row: 0, rowUnits: 2, name: "Bump", assigned: true,
	presentation: "push_encoder", hasFader: true, faderValue: 50,
	faderLabel: "Master", faderDisplay: "50%", actions: [],
};
describe("Playback encoder center", () => {
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
