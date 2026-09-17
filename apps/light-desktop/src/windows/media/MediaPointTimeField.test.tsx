import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MediaPointTimeField } from "./MediaPointTimeField";
import type { MediaPointTimeControl } from "./mediaPaneModel";

function type(text: string, clear: number) {
	for (let index = 0; index < clear; index += 1)
		fireEvent.keyDown(window, { key: "Backspace" });
	for (const key of text) fireEvent.keyDown(window, { key });
	fireEvent.keyDown(window, { key: "Enter" });
}

const inPoint: MediaPointTimeControl = {
	id: "media.in_point",
	label: "In point",
	kind: "point-time",
	value: 1234,
	reference: "start",
	framesPerSecond: 25,
	display: "00:49.09",
};

describe("Media In/Out point entry", () => {
	it("enters mm:ss.ff and hands the desk the frame count", () => {
		const onChange = vi.fn();
		render(
			<MediaPointTimeField
				control={inPoint}
				disabled={false}
				onChange={onChange}
			/>,
		);
		expect(screen.queryByRole("slider")).toBeNull();
		fireEvent.click(screen.getByRole("button", { name: /In point: 00:49.09/ }));
		const modal = screen.getByRole("dialog", {
			name: "In point (mm:ss.ff)",
		});
		expect(within(modal).getByLabelText("Full text keyboard")).toBeTruthy();

		type("01:02.30", 8);
		expect(
			within(modal).getByText("Frames must be below 25 at 25 fps"),
		).toBeTruthy();
		expect(onChange).not.toHaveBeenCalled();

		type("01:02.03", 8);
		expect(onChange).toHaveBeenCalledWith("media.in_point", 1553);
		expect(screen.queryByRole("dialog")).toBeNull();
	});

	it("takes a frame count and offers a retry while the rate is unknown", () => {
		const onChange = vi.fn();
		const retry = vi.fn();
		render(
			<MediaPointTimeField
				control={{
					...inPoint,
					id: "media.out_point",
					label: "Out point",
					reference: "end",
					value: 0,
					framesPerSecond: null,
					display: "End of clip",
					rateNotice: "Frame rate unknown: update the Media Server.",
					onRetryFrameRate: retry,
				}}
				disabled={false}
				onChange={onChange}
			/>,
		);
		expect(screen.getByRole("status")).toHaveTextContent(
			"Frame rate unknown: update the Media Server.",
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Check frame rate again" }),
		);
		expect(retry).toHaveBeenCalledTimes(1);

		fireEvent.click(
			screen.getByRole("button", { name: /Out point: End of clip/ }),
		);
		screen.getByRole("dialog", { name: "Out point (frames before end)" });
		type("48", 1);
		expect(onChange).toHaveBeenCalledWith("media.out_point", 48);
	});
});
