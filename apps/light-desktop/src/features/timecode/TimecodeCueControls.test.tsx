import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ExactFrame, FrameHandle } from "./TimecodeCueControls";

afterEach(cleanup);
it("submits Enter followed by blur once while awaiting authority", () => {
	const commit = vi.fn();
	render(<ExactFrame label="Trigger" frame={44} onCommit={commit} />);
	const input = screen.getByLabelText("Trigger");
	fireEvent.change(input, { target: { value: "00:00:02.00" } });
	fireEvent.keyDown(input, { key: "Enter" });
	fireEvent.blur(input);
	expect(commit).toHaveBeenCalledExactlyOnceWith(88);
});
it("keeps invalid frame values local and allows correction", () => {
	const commit = vi.fn();
	render(<ExactFrame label="Trigger" frame={44} onCommit={commit} />);
	const input = screen.getByLabelText("Trigger");
	fireEvent.change(input, { target: { value: "00:00:02.44" } });
	fireEvent.blur(input);
	expect(input).toHaveAttribute("aria-invalid", "true");
	expect(commit).not.toHaveBeenCalled();
	fireEvent.change(input, { target: { value: "00:00:02.43" } });
	fireEvent.blur(input);
	expect(commit).toHaveBeenCalledExactlyOnceWith(131);
});

it("does not store a trigger just because its handle was tapped or clamped", () => {
	const commit = vi.fn();
	const preview = vi.fn();
	render(
		<div data-pixels-per-frame="2">
			<FrameHandle
				label="Move Cue 2 trigger"
				frame={44}
				minimum={44}
				maximum={100}
				left={88}
				top={32}
				onCommit={commit}
				onPreview={preview}
			/>
		</div>,
	);
	const handle = screen.getByRole("slider", { name: "Move Cue 2 trigger" });
	fireEvent.pointerDown(handle, { pointerId: 1, clientX: 100 });
	fireEvent.pointerUp(handle, { pointerId: 1, clientX: 100 });
	expect(commit).not.toHaveBeenCalled();
	expect(preview).toHaveBeenLastCalledWith(null);
	fireEvent.keyDown(handle, { key: "ArrowLeft" });
	expect(commit).not.toHaveBeenCalled();
	fireEvent.pointerDown(handle, { pointerId: 2, clientX: 100 });
	fireEvent.pointerUp(handle, { pointerId: 2, clientX: 120 });
	expect(commit).toHaveBeenCalledExactlyOnceWith(54);
});
