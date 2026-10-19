import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaybackCardViewModel } from "./PlaybackCards";
import { PlaybackTileView } from "./PlaybackTileView";

const model: PlaybackCardViewModel = {
	page: 3, slot: 402, row: 1, rowUnits: 1,
	name: "Opening Look", assigned: true, kind: "cue-list", selected: true,
	hasFader: false, faderValue: 100, faderLabel: "Master", faderDisplay: "100%",
	actions: [], summary: { label: "Cue 2", detail: "Blue and Amber" },
	artwork: { image: "cue.png", imageSource: "cue-preview" },
};
afterEach(cleanup);

describe("PlaybackTileView", () => {
	it("makes title, number, artwork and cue summary part of the same action", () => {
		const press = vi.fn();
		const activate = vi.fn();
		const { container } = render(<PlaybackTileView model={model} action={{ id: "go", label: "GO +", onClick: press }} callbacks={{ onActivate: activate }} />);
		const button = screen.getByRole("button", { name: "Playback 3.402: Opening Look" });
		expect(screen.getAllByRole("button")).toHaveLength(1);
		expect(screen.queryByText("GO +")).toBeNull();
		const image = container.querySelector("img")!;
		expect(image).toHaveAttribute("src", "cue.png");
		for (const target of [screen.getByText("Opening Look"), screen.getByText("3.402"), image, screen.getByText("Cue 2"), screen.getByText("Blue and Amber")]) {
			expect(button).toContainElement(target);
			fireEvent.click(target);
		}
		expect(press).toHaveBeenCalledTimes(5);
		expect(activate).not.toHaveBeenCalled();
		expect(button.querySelector("button")).toBeNull();
	});

	it("preserves pointer and keyboard held-action callbacks and wrapper interception", () => {
		const down = vi.fn(); const up = vi.fn(); const cancel = vi.fn();
		const keyDown = vi.fn(); const keyUp = vi.fn(); const intercept = vi.fn();
		render(<PlaybackTileView model={model} action={{ id: "flash", label: "Flash", onPointerDown: down, onPointerUp: up, onPointerCancel: cancel, onKeyDown: keyDown, onKeyUp: keyUp }} callbacks={{ onPointerDownCapture: intercept }} />);
		const button = screen.getByRole("button");
		fireEvent.pointerDown(button); fireEvent.pointerUp(button); fireEvent.pointerCancel(button);
		fireEvent.keyDown(button, { key: " " }); fireEvent.keyUp(button, { key: " " });
		for (const callback of [down, up, cancel, keyDown, keyUp, intercept]) expect(callback).toHaveBeenCalledOnce();
	});

	it("lets command capture stop the normal action", () => {
		const press = vi.fn(); const record = vi.fn();
		render(<PlaybackTileView model={model} action={{ id: "go", label: "GO", onClick: press }} callbacks={{ onClickCapture: event => { event.stopPropagation(); record(); } }} />);
		fireEvent.click(screen.getByText("Opening Look"));
		expect(record).toHaveBeenCalledOnce(); expect(press).not.toHaveBeenCalled();
	});

	it("keeps essential warnings, preview notices, identity and state visible", () => {
		const { container } = render(<PlaybackTileView model={{ ...model, className: "running playback-runtime-error", color: "#cf4aa8", summary: { label: "Running", warning: "2 missing targets" }, artwork: { notice: "Preview loading" } }} action={{ id: "go", label: "GO" }} />);
		expect(screen.getByText("2 missing targets")).toBeVisible();
		expect(screen.getByRole("status")).toHaveTextContent("Preview loading");
		const card = container.querySelector("article")!;
		expect(card).toHaveClass("running", "selected", "playback-runtime-error", "playback-tile-card");
		expect(card).toHaveAttribute("data-page", "3"); expect(card).toHaveAttribute("data-playback-slot", "402");
		expect(card).toHaveAttribute("data-playback-row", "1"); expect(card).toHaveAttribute("data-has-fader", "false");
		expect(card.style.getPropertyValue("--playback-color")).toBe("#cf4aa8");
	});

	it.each(["model", "action", "missing"])("disables an unavailable %s action", reason => {
		const press = vi.fn();
		render(<PlaybackTileView model={{ ...model, disabled: reason === "model" }} action={reason === "missing" ? undefined : { id: "go", label: "GO", disabled: reason === "action", onClick: press }} />);
		const button = screen.getByRole("button"); expect(button).toBeDisabled();
		fireEvent.click(button); expect(press).not.toHaveBeenCalled();
	});

	it("keeps explicit transient overlays outside the action", () => {
		render(<PlaybackTileView model={model} action={{ id: "go", label: "GO" }} slots={{ overlays: <button type="button">Configure target</button> }} />);
		expect(screen.getByRole("button", { name: /Playback 3.402/ })).not.toContainElement(screen.getByRole("button", { name: "Configure target" }));
	});
});
