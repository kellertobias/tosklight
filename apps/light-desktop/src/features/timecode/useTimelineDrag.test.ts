import { act, fireEvent, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { useTimelineDrag } from "./useTimelineDrag";
const selection = { kind: "volume", laneId: "audio", itemId: "point" } as const;
function definition(): TimecodeDefinition {
	return {
		id: "time",
		number: 1,
		name: "Audio",
		duration_frame: 440,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [{ id: "near", frame: 104, name: "Near" }],
		lanes: [
			{
				id: "audio",
				name: "Audio",
				content: {
					kind: "audio_volume",
					keyframes: [
						{
							id: "point",
							frame: 100,
							value: 0.5,
							fade_frames: 0,
							curve: "linear",
						},
					],
				},
			},
		],
	};
}
function setup() {
	const lane = document.createElement("div");
	lane.className = "timecode-editor-lane";
	Object.defineProperty(lane, "clientHeight", { value: 100 });
	const target = document.createElement("button");
	lane.append(target);
	const preview = vi.fn(),
		begin = vi.fn(),
		end = vi.fn(),
		select = vi.fn();
	const hook = renderHook(
		({ value }) =>
			useTimelineDrag({
				definition: value,
				pixelsPerFrame: 2,
				onPreview: preview,
				onBeginGesture: begin,
				onEndGesture: end,
				setSelection: select,
			}),
		{ initialProps: { value: definition() } },
	);
	const down = () =>
		act(() =>
			hook.result.current(
				{
					preventDefault: vi.fn(),
					pointerId: 1,
					clientX: 513,
					clientY: 50,
					currentTarget: target,
				} as never,
				selection,
				100,
				undefined,
				0.5,
			),
		);
	return { hook, preview, begin, end, select, down };
}
it("does not snap, preview, or save a press, zero-motion event, or subthreshold tap", () => {
	const rig = setup();
	rig.down();
	expect(rig.select).not.toHaveBeenCalled();
	fireEvent.pointerMove(window, { pointerId: 1, clientX: 513, clientY: 50 });
	fireEvent.pointerMove(window, { pointerId: 1, clientX: 515, clientY: 51 });
	expect(rig.preview).not.toHaveBeenCalled();
	expect(rig.begin).not.toHaveBeenCalled();
	fireEvent.pointerUp(window, { pointerId: 1 });
	expect(rig.end).not.toHaveBeenCalled();
	expect(rig.select).toHaveBeenCalledExactlyOnceWith(selection);
	rig.hook.unmount();
});
it("preserves horizontal frame during relative vertical movement beside a snap target", () => {
	const rig = setup();
	rig.down();
	fireEvent.pointerMove(window, { pointerId: 2, clientX: 550, clientY: 70 });
	expect(rig.preview).not.toHaveBeenCalled();
	fireEvent.pointerMove(window, { pointerId: 1, clientX: 513, clientY: 69 });
	const next = rig.preview.mock.lastCall![0] as TimecodeDefinition;
	const content = next.lanes[0].content;
	if (content.kind !== "audio_volume") throw Error("Audio expected");
	expect(content.keyframes[0]).toMatchObject({ frame: 100, value: 0.25 });
	expect(rig.begin).toHaveBeenCalledTimes(1);
	fireEvent.pointerUp(window, { pointerId: 1 });
	expect(rig.end).toHaveBeenCalledTimes(1);
	rig.hook.unmount();
});
it("cancel restores only the dragged item and retains unrelated authoritative updates", () => {
	const rig = setup();
	rig.down();
	fireEvent.pointerMove(window, { pointerId: 1, clientX: 553, clientY: 50 });
	const moved = rig.preview.mock.lastCall![0] as TimecodeDefinition;
	moved.markers = [
		...moved.markers,
		{ id: "remote", frame: 300, name: "Remote" },
	];
	rig.hook.rerender({ value: moved });
	fireEvent.pointerCancel(window, { pointerId: 1 });
	const restored = rig.preview.mock.lastCall![0] as TimecodeDefinition;
	const content = restored.lanes[0].content;
	if (content.kind !== "audio_volume") throw Error("Audio expected");
	expect(content.keyframes[0]).toMatchObject({ frame: 100, value: 0.5 });
	expect(restored.markers.at(-1)?.id).toBe("remote");
	expect(rig.end).toHaveBeenCalledTimes(1);
	expect(rig.select).not.toHaveBeenCalled();
	rig.hook.unmount();
});
