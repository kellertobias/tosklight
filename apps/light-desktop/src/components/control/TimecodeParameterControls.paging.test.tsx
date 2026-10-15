import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
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
		<ModalProvider>
			<VisibleEncoderCountProvider count={count}>
				<TimecodeParameterControls hardwareConnected={hardware} deck={deck} />
			</VisibleEncoderCountProvider>
		</ModalProvider>,
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
		expect(inDelay).toHaveBeenCalledWith(1);
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

describe("Timecode absolute entry uses each displayed unit", () => {
	afterEach(cleanup);
	it("enters frame 77 through Timeline encoder three's actual keypad", () => {
		const scrub = vi.fn();
		const playhead = {
			...slot("timecode-playhead", scrub),
			label: "Playhead",
			maximum: 1000,
			value: 0,
		};
		show(
			{
				timeline: [
					slot("timecode-lane-navigation"),
					slot("timecode-keyframe-navigation"),
					playhead,
				],
				keyframe: [],
			},
			6,
		);
		fireEvent.click(screen.getByRole("button", { name: "Timecode Timeline" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 3 · Playhead value" }),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Enc 3 · Playhead value",
		});
		const keys = within(dialog).getByLabelText("Number input keypad");
		fireEvent.click(within(keys).getByRole("button", { name: "7" }));
		fireEvent.click(within(keys).getByRole("button", { name: "7" }));
		fireEvent.click(within(keys).getByRole("button", { name: "ENTER" }));
		expect(scrub).toHaveBeenCalledExactlyOnceWith(77);
	});

	const cases = [
		{
			id: "timecode-playhead",
			label: "Playhead",
			input: "77",
			expected: 77,
			maximum: 1000,
		},
		{
			id: "timecode-marker-frame",
			label: "Marker position",
			input: "154",
			expected: 154,
			maximum: 1000,
		},
		{
			id: "timecode-keyframe-frame",
			label: "Keyframe position",
			input: "110",
			expected: 110,
			maximum: 1000,
		},
		{
			id: "timecode-lane-navigation",
			label: "Lane",
			input: "2",
			expected: 2,
			maximum: 5,
		},
		{
			id: "timecode-keyframe-value",
			label: "Speed BPM",
			input: "120",
			expected: 120,
			maximum: 300,
		},
		{
			id: "timecode-keyframe-value",
			label: "Audio percent",
			input: "50",
			expected: 50,
			maximum: 100,
		},
		{
			id: "timecode-cue-in-fade",
			label: "In fade",
			input: "12.5",
			expected: 12500,
			maximum: 600000,
		},
		{
			id: "timecode-cue-in-delay",
			label: "In delay",
			input: "1.5",
			expected: 1500,
			maximum: 600000,
		},
		{
			id: "timecode-cue-out-fade",
			label: "Out fade",
			input: "2.5",
			expected: 2500,
			maximum: 600000,
		},
		{
			id: "timecode-cue-out-delay",
			label: "Out delay",
			input: "3.5",
			expected: 3500,
			maximum: 600000,
		},
		{
			id: "timecode-timeline-zone",
			label: "Timeline zone",
			input: "250",
			expected: 2.5,
			maximum: 10,
		},
		{
			id: "timecode-timeline-scroll",
			label: "Timeline scroll",
			input: "50",
			expected: 1000,
			maximum: 2000,
		},
	];
	it.each(cases)("$label: literal keypad $input commits the displayed unit", ({
		id,
		label,
		input,
		expected,
		maximum,
	}) => {
		const set = vi.fn();
		const encoder = { ...slot(id, set), label, maximum, value: expected };
		show({ timeline: [], keyframe: [encoder] }, 6);
		fireEvent.click(
			screen.getByRole("button", { name: `Set Enc 1 · ${label} value` }),
		);
		const dialog = screen.getByRole("dialog", {
			name: `Enc 1 · ${label} value`,
		});
		expect(within(dialog).getByRole("textbox")).toHaveTextContent(input);
		const keys = within(dialog).getByLabelText("Number input keypad");
		for (const digit of input)
			fireEvent.click(within(keys).getByRole("button", { name: digit }));
		fireEvent.click(within(keys).getByRole("button", { name: "ENTER" }));
		expect(set).toHaveBeenCalledOnce();
		expect(set).toHaveBeenCalledWith(expected);
	});
});
