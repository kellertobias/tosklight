import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, it, expect, vi } from "vitest";
import {
	PlaybackLayoutModal,
	addPlaybackRow,
	playbackLayoutInvalid,
} from "./PlaybackLayoutModal";
import type { PlaybackSurfaceLayout } from "../../api/types";
afterEach(cleanup);
const layout: PlaybackSurfaceLayout = {
	rows: [
		{
			first_position: 201,
			count: 50,
			presentation: "buttons",
			button_count: 1,
		},
		{
			first_position: 301,
			count: 5,
			presentation: "push_encoder",
			button_count: 1,
		},
	],
};
it("accepts mixed row widths and presentation across the full1000 positions", () => {
	expect(playbackLayoutInvalid(layout)).toBe(false);
	expect(
		playbackLayoutInvalid({
			rows: [{ ...layout.rows[0], first_position: 999, count: 2 }],
		}),
	).toBe(false);
	expect(
		playbackLayoutInvalid({
			rows: [{ ...layout.rows[0], first_position: 1000, count: 2 }],
		}),
	).toBe(true);
	expect(
		playbackLayoutInvalid({ rows: [{ ...layout.rows[0], count: 1.5 }] }),
	).toBe(true);
});
it("adds a distinct row and never clamps into a duplicate terminal range", () => {
	expect(addPlaybackRow(layout).rows.at(-1)).toEqual({
		...layout.rows[1],
		first_position: 306,
	});
	const terminal = {
		rows: [{ ...layout.rows[0], first_position: 951, count: 50 }],
	};
	expect(addPlaybackRow(terminal)).toBe(terminal);
});
it("keeps invalid input visible without saving and saves corrected exact rows", () => {
	const save = vi.fn();
	render(
		<PlaybackLayoutModal
			initialLayout={layout}
			pageSource={{ type: "own", page: 99 }}
			onSave={save}
			onClose={() => {}}
		/>,
	);
	const counts = screen.getAllByLabelText("Playbacks in row");
	fireEvent.change(counts[0], { target: { value: "1000" } });
	expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
	expect(counts[0]).toHaveValue("1000");
	fireEvent.change(counts[0], { target: { value: "50" } });
	fireEvent.click(screen.getByRole("button", { name: "Save" }));
	expect(save).toHaveBeenCalledWith(layout, { type: "own", page: 99 });
});
it("reorders through pointer dragging while preserving each row address and presentation", () => {
	const save = vi.fn();
	render(
		<PlaybackLayoutModal
			initialLayout={layout}
			pageSource={{ type: "pinned", page: 5 }}
			onSave={save}
			onClose={() => {}}
		/>,
	);
	const handle = screen.getByRole("button", { name: "Reorder playback row 2" });
	handle.setPointerCapture = vi.fn();
	const target = screen
		.getByRole("button", { name: "Reorder playback row 1" })
		.closest("article");
	const original = document.elementFromPoint;
	document.elementFromPoint = vi.fn(() => target);
	fireEvent.pointerDown(handle, { pointerId: 1 });
	fireEvent.pointerMove(handle, { pointerId: 1 });
	document.elementFromPoint = original;
	fireEvent.click(screen.getByRole("button", { name: "Save" }));
	expect(save).toHaveBeenCalledWith(
		{ rows: [layout.rows[1], layout.rows[0]] },
		{ type: "pinned", page: 5 },
	);
});

it("labels page source and follow controls and saves the operator's Own page choice", () => {
	const save = vi.fn();
	render(
		<PlaybackLayoutModal
			initialLayout={layout}
			pageSource={{ type: "follow", surface_id: "main" }}
			surfaceOptions={[{ id: "main", name: "Main" }]}
			onSave={save}
			onClose={() => {}}
		/>,
	);
	expect(
		screen.getByLabelText("Follow surface", { exact: true }),
	).toHaveTextContent("Main");
	fireEvent.click(screen.getByLabelText("Page source", { exact: true }));
	fireEvent.click(
		screen.getByRole("option", { name: "Own page" }),
	);
	expect(screen.getByLabelText("Page", { exact: true })).toHaveValue("1");
	fireEvent.click(screen.getByRole("button", { name: "Save" }));
	expect(save).toHaveBeenCalledWith(layout, { type: "own", page: 1 });
});
