import { describe, expect, it } from "vitest";
import { clampEncoderPage, encoderPages } from "./encoderPaging";

const slots = (count: number) =>
	Array.from({ length: count }, (_, index) => `slot-${index + 1}`);

describe("Fitting encoder slots onto the encoders a desk has", () => {
	it("leaves a deck that fits on one page", () => {
		const pages = encoderPages(slots(4), 6);
		expect(pages).toHaveLength(1);
		expect(pages[0].slots).toHaveLength(4);
		expect(pages[0].total).toBe(1);
	});

	it("pages the Timecode Cue deck across a six-encoder surface", () => {
		// Four Cue timings, the Cue itself, and the two the timeline always keeps.
		const pages = encoderPages(slots(7), 6);
		expect(pages.map((page) => page.slots.length)).toEqual([6, 1]);
		expect(pages[1]).toMatchObject({ number: 2, total: 2 });
	});

	it("pages the same deck further on a four-encoder surface", () => {
		expect(
			encoderPages(slots(7), 4).map((page) => page.slots.length),
		).toEqual([4, 3]);
	});

	it("keeps every slot, in order, however it is paged", () => {
		const pages = encoderPages(slots(7), 4);
		expect(pages.flatMap((page) => page.slots)).toEqual(slots(7));
	});

	it("still offers one page when a view registers nothing", () => {
		expect(encoderPages([], 6)).toEqual([{ slots: [], number: 1, total: 1 }]);
	});

	it("shows the deck rather than nothing when a surface reports no encoders", () => {
		// Dividing by zero here would page forever and draw no encoders at all.
		expect(encoderPages(slots(3), 0).length).toBe(3);
	});

	it("brings a page that no longer exists back into range", () => {
		// Selecting a different object can shorten the deck under the operator.
		expect(clampEncoderPage(2, 1)).toBe(1);
		expect(clampEncoderPage(0, 3)).toBe(1);
		expect(clampEncoderPage(2, 3)).toBe(2);
	});
});
