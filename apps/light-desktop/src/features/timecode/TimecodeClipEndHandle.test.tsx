import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { ClipEndHandle } from "./TimecodeClipEndHandle";
afterEach(cleanup);
const clip: TimecodeCueListClip = {
	id: "clip",
	start_frame: 0,
	end_frame: 100,
	start_cue_id: "one",
	end_cue_id: "one",
	start_behavior: "state",
	end_behavior: "hold",
	cue_starts: [],
	in_fade_frames: 0,
	out_fade_frames: 0,
};
it("shows the real ending in the expanded envelope and commits only duration drag release", () => {
	const commit = vi.fn(),
		preview = vi.fn();
	const view = render(
		<div data-pixels-per-frame="2">
			<ClipEndHandle
				clip={clip}
				pixelsPerFrame={2}
				expanded
				minimum={50}
				maximum={120}
				onCommit={commit}
				onPreview={preview}
			/>
		</div>,
	);
	const handle = screen.getByRole("slider", { name: "Extend last Cue" });
	expect(handle).toHaveStyle({ left: "178px", top: "134px" });
	expect(handle).toHaveAttribute("data-time-anchor-x", "200");
	// Its centered 44px touch target ends exactly at the true clip boundary.
	expect(Number.parseFloat(handle.style.left) + 22).toBe(200);
	// The hit area aligns with the Cue-name strip and other divider grips.
	expect(Number.parseFloat(handle.style.top) - 22).toBe(112);
	expect(Number.parseFloat(handle.style.top) + 22).toBe(156);
	expect(handle.querySelector(".timecode-cue-point-leader")).not.toBeNull();
	fireEvent.pointerDown(handle, { pointerId: 1, clientX: 100 });
	fireEvent.pointerMove(handle, { pointerId: 1, clientX: 188 });
	expect(preview).toHaveBeenLastCalledWith(120);
	expect(commit).not.toHaveBeenCalled();
	fireEvent.pointerUp(handle, { pointerId: 1, clientX: 188 });
	expect(commit).toHaveBeenCalledExactlyOnceWith(120);
	view.rerender(
		<ClipEndHandle clip={clip} pixelsPerFrame={2} expanded onCommit={commit} />,
	);
	expect(screen.getByRole("slider", { name: "Extend last Cue" })).toHaveStyle({
		left: "178px",
		top: "134px",
	});
});

it("leaves compact Cue names unobstructed even for a four-frame clip", () => {
	render(
		<ClipEndHandle
			clip={{ ...clip, end_frame: 4 }}
			pixelsPerFrame={3}
			expanded={false}
			onCommit={vi.fn()}
		/>,
	);
	expect(screen.queryByRole("slider", { name: "Extend last Cue" })).toBeNull();
});

it("selects the final Cue when its narrow expanded name is covered without storing on tap", () => {
	const select = vi.fn(),
		commit = vi.fn();
	render(
		<ClipEndHandle
			clip={{ ...clip, end_frame: 4, end_cue_id: "final" }}
			pixelsPerFrame={3}
			expanded
			onSelectCue={select}
			onCommit={commit}
		/>,
	);
	const handle = screen.getByRole("slider", { name: "Extend last Cue" });
	fireEvent.pointerDown(handle, { pointerId: 2, clientX: 8 });
	expect(select).toHaveBeenCalledExactlyOnceWith("final");
	fireEvent.pointerUp(handle, { pointerId: 2, clientX: 8 });
	expect(commit).not.toHaveBeenCalled();
});
