import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimecodeEncoderDeck } from "../../features/timecode/timecodeEncoderBridge";
import { VisibleEncoderCountProvider } from "./parameterControls/VisibleEncoderCount";
import { TimecodeParameterControls } from "./TimecodeParameterControls";

function slot(id: string, set: (value: number) => void = vi.fn()) {
	return {
		id,
		label: id,
		display: id,
		value: 0,
		minimum: 0,
		maximum: 100,
		fineStep: 1,
		coarseStep: 10,
		disabled: false,
		set,
	};
}

/** The Cue deck: four Cue timings, the Cue itself, and the two the timeline always keeps. */
function cueDeck(sets: Record<string, (value: number) => void> = {}) {
	const ids = [
		"in-delay",
		"in-fade",
		"out-delay",
		"out-fade",
		"cue",
		"playhead",
		"zone",
	];
	return {
		timeline: [],
		keyframe: ids.map((id) => slot(id, sets[id])),
		selectionLabel: "Selected Cue",
	} as unknown as TimecodeEncoderDeck;
}

const show = (deck: TimecodeEncoderDeck, count: 4 | 6, hardware = false) =>
	render(
		<VisibleEncoderCountProvider count={count}>
			<TimecodeParameterControls hardwareConnected={hardware} deck={deck} />
		</VisibleEncoderCountProvider>,
	);

describe("A Timecode deck wider than the desk", () => {
	afterEach(cleanup);

	it("pages seven Cue slots onto six encoders rather than drawing all seven", () => {
		show(cueDeck(), 6);
		expect(screen.getAllByRole("button", { name: /^Set / })).toHaveLength(6);
		// The page control says where the operator is.
		expect(screen.getByRole("button", { name: "1/2" })).toBeVisible();
	});

	it("shows the rest on the next page and comes back round", () => {
		show(cueDeck(), 6);
		fireEvent.click(screen.getByRole("button", { name: "1/2" }));
		expect(screen.getAllByRole("button", { name: /^Set / })).toHaveLength(1);
		// The seventh slot is encoder one of page two: the numbering follows the desk, not the deck.
		expect(
			screen.getByRole("button", { name: "Set Enc 1 · zone value" }),
		).toBeVisible();
		fireEvent.click(screen.getByRole("button", { name: "2/2" }));
		expect(screen.getAllByRole("button", { name: /^Set / })).toHaveLength(6);
	});

	it("pages further on a four-encoder desk", () => {
		show(cueDeck(), 4);
		expect(screen.getAllByRole("button", { name: /^Set / })).toHaveLength(4);
		expect(screen.getByRole("button", { name: "1/2" })).toBeVisible();
	});

	it("gives a hardware encoder the slot on the page in front of the operator", () => {
		const zone = vi.fn();
		const inDelay = vi.fn();
		show(cueDeck({ zone, "in-delay": inDelay }), 6, true);

		const turn = () =>
			window.dispatchEvent(
				new CustomEvent("light:encoder-action", {
					detail: { control: "encoder/1", value: "up" },
				}),
			);

		// Encoder one on page one is the first Cue timing.
		turn();
		expect(inDelay).toHaveBeenCalledTimes(1);
		expect(zone).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("button", { name: "1/2" }));

		// The same physical encoder now drives page two's first slot, not the deck's first.
		turn();
		expect(zone).toHaveBeenCalledTimes(1);
		expect(inDelay).toHaveBeenCalledTimes(1);
	});

	it("offers no page control when the deck fits", () => {
		const deck = {
			timeline: [],
			keyframe: [slot("a"), slot("b")],
			selectionLabel: "Selected Keyframe",
		} as unknown as TimecodeEncoderDeck;
		show(deck, 6);
		expect(screen.queryByRole("button", { name: /\d\/\d/ })).toBeNull();
	});
});
