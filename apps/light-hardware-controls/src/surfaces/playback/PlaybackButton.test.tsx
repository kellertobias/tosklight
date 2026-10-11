// @vitest-environment jsdom
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { afterEach, it, expect, vi } from "vitest";
import { PlaybackButton } from "./PlaybackButton";
afterEach(cleanup);
it("releases the original explicit address after page changes, exactly once", () => {
	HTMLElement.prototype.setPointerCapture = vi.fn();
	const send = vi.fn();
	const { rerender } = render(
		<PlaybackButton page={2} position={401} send={send} />,
	);
	const button = screen.getByRole("button", { name: "1" });
	fireEvent.pointerDown(button, { pointerId: 1 });
	rerender(<PlaybackButton page={99} position={401} send={send} />);
	fireEvent.pointerUp(button, { pointerId: 1 });
	fireEvent.pointerCancel(button, { pointerId: 1 });
	expect(send.mock.calls).toEqual([
		["playback/2/401/button/1", [true, 3209]],
		["playback/2/401/button/1", [false, 3209]],
	]);
});
