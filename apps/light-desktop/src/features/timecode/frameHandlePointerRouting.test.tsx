import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { FrameHandle } from "./TimecodeCueControls";
afterEach(cleanup);
it("routes overlapping Cue and end targets by true boundary and retains the chosen writer", () => {
	const select = vi.fn();
	const cue = vi.fn(),
		next = vi.fn(),
		end = vi.fn();
	render(
		<div className="timecode-cue-clip-contents" data-pixels-per-frame="2">
			<FrameHandle
				label="Cue"
				frame={395}
				minimum={1}
				maximum={396}
				left={790}
				top={100}
				className="cue-divider-grip"
				onCommit={cue}
				onPointerStart={() => select("Cue")}
			/>
			<FrameHandle
				label="Next"
				frame={396}
				minimum={396}
				maximum={399}
				left={792}
				top={100}
				className="cue-divider-grip"
				onCommit={next}
				onPointerStart={() => select("Next")}
			/>
			<FrameHandle
				label="End"
				frame={400}
				minimum={397}
				maximum={500}
				left={778}
				anchorLeft={800}
				top={100}
				className="cue-divider-grip"
				onCommit={end}
				onPointerStart={() => select("End")}
			/>
		</div>,
	);
	for (const name of ["Cue", "Next", "End"]) {
		const node = screen.getByRole("slider", { name });
		const left = Number.parseFloat(node.style.left) - 22;
		vi.spyOn(node, "getBoundingClientRect").mockReturnValue({
			left,
			right: left + 44,
			top: 78,
			bottom: 122,
			width: 44,
			height: 44,
		} as DOMRect);
	}
	const target = screen.getByRole("slider", { name: "Cue" });
	target.focus();
	select("Previously selected other Cue");
	fireEvent.pointerDown(screen.getByRole("slider", { name: "Next" }), {
		pointerId: 1,
		clientX: 790,
		clientY: 100,
	});
	expect(document.activeElement).toBe(target);
	expect(select).toHaveBeenLastCalledWith("Cue");
	fireEvent.pointerMove(target, { pointerId: 1, clientX: 90, clientY: 100 });
	expect(cue).not.toHaveBeenCalled();
	fireEvent.pointerUp(target, { pointerId: 1, clientX: 90, clientY: 100 });
	expect(cue).toHaveBeenCalledExactlyOnceWith(45);
	expect(next).not.toHaveBeenCalled();
	expect(end).not.toHaveBeenCalled();
	// Even the inset End touch region routes to the actual last Cue at its visible boundary.
	fireEvent.pointerDown(screen.getByRole("slider", { name: "End" }), {
		pointerId: 2,
		clientX: 792,
		clientY: 100,
	});
	const nextTarget = screen.getByRole("slider", { name: "Next" });
	expect(document.activeElement).toBe(nextTarget);
	fireEvent.pointerCancel(nextTarget, { pointerId: 2 });
	expect(next).not.toHaveBeenCalled();
	fireEvent.pointerDown(nextTarget, {
		pointerId: 3,
		clientX: 799.5,
		clientY: 100,
	});
	const endTarget = screen.getByRole("slider", { name: "End" });
	expect(document.activeElement).toBe(endTarget);
	expect(select).toHaveBeenLastCalledWith("End");
	fireEvent.pointerUp(endTarget, {
		pointerId: 3,
		clientX: 807.5,
		clientY: 100,
	});
	expect(end).toHaveBeenCalledExactlyOnceWith(404);
});

it("never redirects a derived disabled trigger to its editable neighbour", () => {
	const commit = vi.fn(), parent = vi.fn();
	render(
		<div className="timecode-timeline-item item-clip" onPointerDown={parent}>
			<FrameHandle
				label="Manual"
				frame={100}
				minimum={0}
				maximum={200}
				left={100}
				top={100}
				className="cue-divider-grip"
				onCommit={commit}
			/>
			<FrameHandle
				label="Derived"
				frame={102}
				minimum={0}
				maximum={200}
				left={102}
				top={100}
				className="cue-divider-grip"
				disabled
				onCommit={commit}
			/>
		</div>,
	);
	for (const name of ["Manual", "Derived"]) {
		const node = screen.getByRole("slider", { name });
		const left = Number.parseFloat(node.style.left) - 22;
		vi.spyOn(node, "getBoundingClientRect").mockReturnValue({
			left,
			right: left + 44,
			top: 78,
			bottom: 122,
			width: 44,
			height: 44,
		} as DOMRect);
	}
	const manual = screen.getByRole("slider", { name: "Manual" });
	fireEvent.pointerDown(manual, { pointerId: 4, clientX: 102, clientY: 100 });
	fireEvent.pointerUp(manual, { pointerId: 4, clientX: 120, clientY: 100 });
	expect(commit).not.toHaveBeenCalled();
	expect(parent).not.toHaveBeenCalled();
	fireEvent.pointerDown(screen.getByRole("slider", {name:"Derived"}), {pointerId:5,clientX:100,clientY:100});
	fireEvent.pointerUp(manual, {pointerId:5,clientX:120,clientY:100});
	expect(commit).toHaveBeenCalledExactlyOnceWith(120);
	expect(parent).not.toHaveBeenCalled();
});
