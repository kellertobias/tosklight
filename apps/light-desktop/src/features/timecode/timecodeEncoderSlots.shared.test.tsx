import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { publishTimecodeEncoderDeck } from "./timecodeEncoderBridge";
import { useTimecodeEncoderSlots } from "./timecodeEncoderSlots";

vi.mock("./timecodeEncoderBridge", () => ({
	publishTimecodeEncoderDeck: vi.fn(),
	clearTimecodeEncoderDeck: vi.fn(),
}));

/** Run the hook's body once and return the deck it published. */
function deckFor(overrides: Record<string, unknown>) {
	const published = publishTimecodeEncoderDeck as unknown as ReturnType<
		typeof vi.fn
	>;
	published.mockClear();
	const options = {
		definition: { markers: [], lanes: [] },
		items: [],
		keyframeItems: [],
		selection: null,
		laneIndex: 0,
		keyframeIndex: 0,
		duration: 6000,
		frame: 0,
		fps: 25,
		zoom: 1,
		maximumZoom: 20,
		scrollLeft: 0,
		viewportWidth: 800,
		timelineWidth: 800,
		encoderOwner: Symbol("test"),
		setZoom: vi.fn(),
		setScrollLeft: vi.fn(),
		setSelection: vi.fn(),
		setSelectedLaneId: vi.fn(),
		onScrub: vi.fn(),
		onCommit: vi.fn(),
		...overrides,
	};
	renderHook(() =>
		(useTimecodeEncoderSlots as unknown as (o: unknown) => void)(options),
	);
	return published.mock.calls.at(-1)?.[1];
}

const slot = (deck: any, id: string) =>
	deck.timeline.find((candidate: { id: string }) => candidate.id === id);

describe("The shared Timecode encoders", () => {
	it("moves the playhead less per detent the further an operator has zoomed", () => {
		const wide = slot(deckFor({ zoom: 1 }), "timecode-playhead");
		const close = slot(deckFor({ zoom: 10 }), "timecode-playhead");
		// A second per detent at full view; a tenth of that ten times in.
		expect(wide.coarseStep).toBe(25);
		expect(close.coarseStep).toBe(3);
		// A frame is as fine as the timeline goes, so the fine step cannot shrink further.
		expect(close.fineStep).toBe(1);
	});

	it("never lets a detent become smaller than a frame", () => {
		const veryClose = slot(deckFor({ zoom: 100 }), "timecode-playhead");
		expect(veryClose.coarseStep).toBeGreaterThanOrEqual(1);
	});

	it("offers a scroll encoder beside the zoom encoder", () => {
		const deck = deckFor({ timelineWidth: 4000, viewportWidth: 800 });
		const scroll = slot(deck, "timecode-timeline-scroll");
		expect(scroll).toBeTruthy();
		// It cannot scroll past the end of the timeline.
		expect(scroll.maximum).toBe(3200);
		// A detent moves a tenth of what is on screen, so it follows the zoom on its own.
		expect(scroll.fineStep).toBe(80);
	});

	it("disables the scroll encoder when the whole timeline already fits", () => {
		const scroll = slot(
			deckFor({ timelineWidth: 800, viewportWidth: 800 }),
			"timecode-timeline-scroll",
		);
		expect(scroll.disabled).toBe(true);
		expect(scroll.display).toBe("Whole timeline");
	});
});
