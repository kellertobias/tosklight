// @vitest-environment jsdom
import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	TimecodeDefinition,
	TimecodeTransportSnapshot,
} from "../api/types/timecode";
import { TimecodeEditor } from "./TimecodeRuntimeWindow";

vi.mock("../components/files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: ({ label }: { label: string }) => (
		<button type="button">{label}</button>
	),
}));
afterEach(cleanup);

const definition: TimecodeDefinition = {
	id: "00000000-0000-4000-8000-000000000162",
	number: 1,
	name: "Opening",
	duration_frame: 440,
	transport_offset_frame: 0,
	auto_start: false,
	markers: [],
	lanes: [],
};
const item = { revision: 4, definition };
function snapshot(frame: number, revision: number): TimecodeTransportSnapshot {
	return {
		timecode_id: definition.id,
		frame,
		revision,
		state: "playing",
		duration_frame: 440,
		audio_linked: false,
		cue_list_clips: [],
	};
}
function setup() {
	return {
		update: vi.fn(),
		transportAction: vi.fn(),
		objects: vi.fn(),
		create: vi.fn(),
		waveform: vi.fn(),
	};
}

describe("Timecode transport and save feedback", () => {
	it("holds a scrubbed playhead while live frames advance and explicitly seeks to the visible target", async () => {
		const api = setup();
		api.transportAction.mockResolvedValue(snapshot(88, 3));
		const editor = (frame: number, revision: number) => (
			<TimecodeEditor
				showId="show"
				item={item}
				api={api as never}
				cueLists={[]}
				audioPlayers={[]}
				snapshot={snapshot(frame, revision)}
				onClose={vi.fn()}
			/>
		);
		const view = render(editor(44, 1));
		await screen.findByText("Saved");
		const viewport = screen.getByLabelText("Timecode timeline viewport");
		vi.spyOn(viewport, "getBoundingClientRect").mockReturnValue({
			x: 0,
			y: 0,
			left: 0,
			top: 0,
			right: 720,
			bottom: 400,
			width: 720,
			height: 400,
			toJSON: () => ({}),
		});
		const canvas = viewport.querySelector<HTMLElement>(
			".timecode-timeline-canvas",
		)!;
		const playhead = screen.getByRole("button", {
			name: "Drag playhead to seek",
		});
		playhead.setPointerCapture = vi.fn();
		fireEvent.pointerDown(playhead, {
			pointerId: 1,
			clientX: 160 + 88 * Number(canvas.dataset.pixelsPerFrame),
		});
		fireEvent.pointerUp(playhead, { pointerId: 1 });
		expect(screen.queryByLabelText("Follow transport")).toBeNull();
		expect(api.transportAction).not.toHaveBeenCalled();
		view.rerender(editor(132, 2));
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:02.00");
		expect(
			screen.getByRole("button", { name: "Timecode position" }),
		).toHaveTextContent("00:00:03.00");
		fireEvent.click(screen.getByRole("button", { name: "Timecode position" }));
		await waitFor(() =>
			expect(api.transportAction).toHaveBeenCalledWith("show", definition.id, {
				type: "seek",
				frame: 88,
			}),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Timecode position" }),
			).toHaveTextContent("00:00:02.00"),
		);
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:02.00");
		view.rerender(editor(176, 4));
		expect(
			screen.getByRole("button", { name: "Timecode position" }),
		).toHaveTextContent("00:00:04.00");
	});

	it.each([
		"Pause",
		"Timecode position",
	])("preserves a newer scrub when a pending %s response arrives", async (actionLabel) => {
		const api = setup();
		let resolveAction!: (value: TimecodeTransportSnapshot) => void;
		api.transportAction.mockImplementation(
			() =>
				new Promise((resolve) => {
					resolveAction = resolve;
				}),
		);
		render(
			<TimecodeEditor
				showId="show"
				item={item}
				api={api as never}
				cueLists={[]}
				audioPlayers={[]}
				snapshot={snapshot(44, 1)}
				onClose={vi.fn()}
			/>,
		);
		await screen.findByText("Saved");
		fireEvent.click(screen.getByRole("button", { name: actionLabel }));
		await waitFor(() => expect(api.transportAction).toHaveBeenCalledTimes(1));
		const canvas = screen
			.getByLabelText("Timecode timeline viewport")
			.querySelector<HTMLElement>(".timecode-timeline-canvas")!;
		const playhead = screen.getByRole("button", {
			name: "Drag playhead to seek",
		});
		playhead.setPointerCapture = vi.fn();
		fireEvent.pointerDown(playhead, {
			pointerId: 1,
			clientX: 160 + 88 * Number(canvas.dataset.pixelsPerFrame),
		});
		fireEvent.pointerUp(playhead, { pointerId: 1 });
		expect(screen.queryByLabelText("Follow transport")).toBeNull();
		await act(async () =>
			resolveAction({ ...snapshot(132, 2), state: "paused" }),
		);
		expect(screen.queryByLabelText("Follow transport")).toBeNull();
		expect(
			screen.getByRole("button", { name: "Drag playhead to seek" }),
		).toHaveTextContent("00:00:02.00");
		expect(
			screen.getByRole("button", { name: "Timecode position" }),
		).toHaveTextContent("00:00:03.00");
	});

	it("shows pending and failed saves, retries the draft, and still permits Stop after failure", async () => {
		const api = setup();
		let rejectSave!: (reason: Error) => void;
		api.update.mockImplementationOnce(
			() =>
				new Promise((_resolve, reject) => {
					rejectSave = reject;
				}),
		);
		api.update.mockResolvedValue({ object: { revision: 5 } });
		api.transportAction.mockResolvedValue({
			...snapshot(0, 1),
			state: "stopped",
		});
		render(
			<TimecodeEditor
				showId="show"
				item={item}
				api={api as never}
				cueLists={[]}
				audioPlayers={[]}
				onClose={vi.fn()}
			/>,
		);
		await screen.findByText("Saved");
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		fireEvent.change(screen.getByLabelText("Name"), {
			target: { value: "Opening revised" },
		});
		await screen.findByText("Saving…");
		expect(screen.queryByText("Saved")).toBeNull();
		await act(async () => rejectSave(new Error("Connection lost")));
		await screen.findByText("Not saved");
		expect(screen.getByRole("alert")).toHaveTextContent("Connection lost");
		expect(screen.getByRole("button", { name: /^Play$/ })).toBeDisabled();
		fireEvent.click(screen.getByRole("button", { name: /^Stop$/ }));
		await waitFor(() =>
			expect(api.transportAction).toHaveBeenCalledWith("show", definition.id, {
				type: "stop",
			}),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Retry autosave" }),
			).toBeEnabled(),
		);
		fireEvent.click(screen.getByRole("button", { name: "Retry autosave" }));
		await screen.findByText("Saved");
		expect(api.update).toHaveBeenLastCalledWith("show", definition.id, 4, {
			name: "Opening revised",
		});
		expect(screen.queryByRole("alert")).toBeNull();
	});
});
