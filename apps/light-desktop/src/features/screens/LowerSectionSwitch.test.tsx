import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LowerSectionSwitch } from "./LowerSectionSwitch";

const settings = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock("../../components/control/PlaybackSectionSettingsHost", () => ({
	usePlaybackSectionSettings: () => settings,
}));
beforeEach(() => { vi.useFakeTimers(); settings.open.mockClear(); });
afterEach(() => { cleanup(); vi.useRealTimers(); });
function mount() {
	const change = vi.fn();
	const view = render(<LowerSectionSwitch view="encoders" onView={change} />);
	return { ...view, change, button: screen.getByRole("button", { name: "Lower section" }) };
}
it("opens settings at 650ms without the trailing mode toggle", () => {
	const { button, change } = mount();
	fireEvent.pointerDown(button);
	act(() => vi.advanceTimersByTime(649));
	expect(settings.open).not.toHaveBeenCalled();
	act(() => vi.advanceTimersByTime(1));
	fireEvent.pointerUp(button);
	fireEvent.click(button, { detail: 1 });
	expect(settings.open).toHaveBeenCalledOnce();
	expect(change).not.toHaveBeenCalled();
});
it("retains short pointer and keyboard clicks as ordinary section switches", () => {
	const { button, change } = mount();
	fireEvent.pointerDown(button);
	act(() => vi.advanceTimersByTime(100));
	fireEvent.pointerUp(button);
	fireEvent.click(button, { detail: 1 });
	fireEvent.click(button, { detail: 0 });
	expect(change).toHaveBeenCalledTimes(2);
	expect(settings.open).not.toHaveBeenCalled();
});
it.each(["pointerCancel", "pointerLeave"] as const)("cancels a pending hold on %s without switching", (event) => {
	const { button, change } = mount();
	fireEvent.pointerDown(button);
	fireEvent[event](button);
	act(() => vi.advanceTimersByTime(1000));
	fireEvent.click(button, { detail: 1 });
	expect(settings.open).not.toHaveBeenCalled();
	expect(change).not.toHaveBeenCalled();
});
it("clears a pending hold when the section unmounts", () => {
	const { button, unmount } = mount();
	fireEvent.pointerDown(button);
	unmount();
	act(() => vi.advanceTimersByTime(1000));
	expect(settings.open).not.toHaveBeenCalled();
});

it("does not swallow a keyboard click after a canceled pointer gesture", () => {
	const { button, change } = mount();
	fireEvent.pointerDown(button);
	fireEvent.pointerCancel(button);
	fireEvent.click(button, { detail: 0 });
	expect(change).toHaveBeenCalledExactlyOnceWith("playbacks");
});
