// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type {
	TimecodeDefinition,
	TimecodeLane,
} from "../../api/types/timecode";
import { TimecodeTimelineEditor } from "./TimecodeTimelineEditor";
import { displayedTimecodeLanes } from "./timecodeEditorShared";

const speed = (id: string, group: string, name: string): TimecodeLane => ({
	id,
	name,
	content: {
		kind: "speed_group",
		group,
		keyframes: [{ id: `${id}-point`, frame: 44, bpm: 120, phase: 0 }],
	},
});
const definition: TimecodeDefinition = {
	id: "timecode",
	number: 1,
	name: "Opening",
	duration_frame: 880,
	transport_offset_frame: 0,
	auto_start: false,
	markers: [],
	audio: { asset_id: "audio-file", asset_revision: 1 },
	lanes: [
		{
			id: "looks",
			name: "Stage Looks",
			content: { kind: "cue_list", cue_list_id: "list", clips: [] },
		},
		speed("movement", "A", "Movement"),
		speed("chase", "B", "Chase"),
		speed("colour", "C", "Colour"),
		{
			id: "audio",
			name: "Main audio",
			content: { kind: "audio_volume", keyframes: [] },
		},
	],
};
afterEach(cleanup);

function mount() {
	const onCommit = vi.fn(),
		onPreview = vi.fn();
	const result = render(
		<TimecodeTimelineEditor
			definition={definition}
			frame={0}
			fps={44}
			cueLists={[]}
			audioPlayers={[]}
			onScrub={vi.fn()}
			onCommit={onCommit}
			onPreview={onPreview}
			onBeginGesture={vi.fn()}
			onEndGesture={vi.fn()}
		/>,
	);
	return { ...result, onCommit, onPreview };
}

it("folds all Speed Groups into one named preview row and expands only the selected group", () => {
	const { container, onCommit, onPreview } = mount();
	const toggle = screen.getByRole("button", {
		name: "Speed Groups",
	});
	expect(toggle).toHaveAttribute("aria-expanded", "true");
	fireEvent.click(toggle);
	expect(toggle).toHaveAttribute("aria-expanded", "false");
	expect(container.querySelectorAll(".timecode-speed-summary")).toHaveLength(1);
	expect(
		container.querySelectorAll(".timecode-speed-summary [data-lane-id]"),
	).toHaveLength(3);
	expect(container.querySelector(".lane-speed_group")).toBeNull();
	fireEvent.click(screen.getByRole("button", { name: "Expand Speed Groups" }));
	expect(toggle).toHaveAttribute("aria-expanded", "true");
	fireEvent.click(
		screen.getByRole("button", { name: /Movement.*Drag to reorder lane/ }),
	);
	expect(container.querySelectorAll(".lane-expanded")).toHaveLength(1);
	expect(container.querySelector(".lane-expanded")).toHaveAttribute(
		"data-lane-id",
		"movement",
	);
	fireEvent.click(
		screen.getByRole("button", { name: /Chase.*Drag to reorder lane/ }),
	);
	expect(container.querySelector(".lane-expanded")).toHaveAttribute(
		"data-lane-id",
		"chase",
	);
	fireEvent.click(
		screen.getByRole("button", { name: "Close speed group editor" }),
	);
	expect(container.querySelector(".lane-expanded")).toBeNull();
	expect(
		screen.queryByRole("complementary", { name: "Selection settings" }),
	).toBeNull();
	expect(onCommit).not.toHaveBeenCalled();
	expect(onPreview).not.toHaveBeenCalled();
});

it("puts audio before Speed Groups before Cuelists without changing stored order", () => {
	const before = structuredClone(definition);
	expect(displayedTimecodeLanes(definition).map((lane) => lane.id)).toEqual([
		"audio",
		"movement",
		"chase",
		"colour",
		"looks",
	]);
	const { container } = mount();
	const audio = container.querySelector(".main-audio")!;
	const speeds = container.querySelector(".timecode-speed-section")!;
	const cues = container.querySelector(".timecode-cuelist-section")!;
	expect(
		audio.compareDocumentPosition(speeds) & Node.DOCUMENT_POSITION_FOLLOWING,
	).toBeTruthy();
	expect(
		speeds.compareDocumentPosition(cues) & Node.DOCUMENT_POSITION_FOLLOWING,
	).toBeTruthy();
	expect(definition).toEqual(before);
});

it("folds Cuelists independently and retains section collapse after closing the speed inspector", () => {
	const { container } = mount();
	fireEvent.click(screen.getByRole("button", { name: "Cuelists" }));
	expect(container.querySelector(".lane-cue_list")).toBeNull();
	fireEvent.click(
		screen.getByRole("button", { name: /Movement.*Drag to reorder lane/ }),
	);
	fireEvent.click(screen.getByRole("button", { name: "Speed Groups" }));
	expect(
		screen.queryByRole("complementary", { name: "Selection settings" }),
	).toBeNull();
	expect(screen.getByRole("button", { name: "Cuelists" })).toHaveAttribute(
		"aria-expanded",
		"false",
	);
	fireEvent.click(screen.getByRole("button", { name: "Expand Cuelists" }));
	expect(container.querySelector(".lane-cue_list")).not.toBeNull();
	expect(screen.getByRole("button", { name: "Speed Groups" })).toHaveAttribute(
		"aria-expanded",
		"false",
	);
});
