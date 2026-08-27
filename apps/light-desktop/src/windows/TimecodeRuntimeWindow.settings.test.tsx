// @vitest-environment jsdom
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../api/types/timecode";
import { parseFrame, TimecodeSettings } from "./TimecodeRuntimeWindow";

const pickerProps = vi.hoisted(
	() => new Map<string, Record<string, unknown>>(),
);
vi.mock("../components/files/RootConfinedFilePickerButton", () => ({
	RootConfinedFilePickerButton: (props: Record<string, unknown>) => {
		pickerProps.set(String(props.label), props);
		return <button type="button">{String(props.label)}</button>;
	},
}));

describe("Timecode Settings", () => {
	it("uses duration fields and managed audio and CSV choices", async () => {
		const draft = definition();
		const setDraft = vi.fn();
		const importAudio = vi.fn(async () => undefined);
		const importCsv = vi.fn(async () => undefined);
		const setMarkersLocked = vi.fn();
		render(
			<TimecodeSettings
				draft={draft}
				setDraft={setDraft}
				duration={draft.duration_frame ?? 0}
				busy={false}
				audioImporting={false}
				importAudio={importAudio}
				csvMode="append"
				setCsvMode={vi.fn()}
				csvError={null}
				importCsv={importCsv}
				markersLocked={false}
				setMarkersLocked={setMarkersLocked}
			/>,
		);

		const tab = (name: string) => screen.getByRole("tab", { name });
		// Generic opens first and carries what the Timecode is.
		expect(screen.queryByLabelText("Number")).toBeNull();
		expect(screen.queryByLabelText("Frames")).toBeNull();
		expect(screen.getByLabelText("Duration")).toHaveValue("00:00:10.00");
		fireEvent.change(screen.getByLabelText("Duration"), {
			target: { value: "00:00:20.00" },
		});
		expect(setDraft).toHaveBeenCalledWith({ ...draft, duration_frame: 880 });
		expect(pickerProps.get("Select File")?.allowedExtensions).toEqual([
			"wav",
			"mp3",
		]);
		const audio = new File(["audio"], "track.mp3");
		await act(async () => {
			await (
				pickerProps.get("Select File")?.onFiles as (
					files: File[],
				) => Promise<void>
			)([audio]);
		});
		expect(importAudio).toHaveBeenCalledWith(audio);

		// Sync carries how it follows an external clock.
		fireEvent.click(tab("Sync"));
		expect(screen.getByLabelText("Transport offset")).toHaveValue(
			"00:00:00.00",
		);

		// Markers carries what its markers do.
		fireEvent.click(tab("Markers"));
		expect(screen.queryByRole("textbox", { name: "Marker CSV" })).toBeNull();
		expect(screen.getByLabelText("Lock markers")).not.toBeChecked();
		fireEvent.click(screen.getByLabelText("Lock markers"));
		expect(setMarkersLocked).toHaveBeenCalledWith(true);
		expect(pickerProps.get("Choose marker CSV")?.allowedExtensions).toEqual([
			"csv",
		]);
		const csv = new File(["position,name\n00:00:01:00,Intro"], "markers.csv");
		await act(async () => {
			await (
				pickerProps.get("Choose marker CSV")?.onFiles as (
					files: File[],
				) => Promise<void>
			)([csv]);
		});
		expect(importCsv).toHaveBeenCalledWith(csv);
	});

	it("parses desk duration values and rejects malformed frames", () => {
		expect(parseFrame("01:02:03:04")).toBe(163_816);
		expect(parseFrame("01:02:03.04")).toBe(163_816);
		expect(parseFrame("00:00:00:44")).toBeNull();
		expect(parseFrame("62 frames")).toBeNull();
	});
});

function definition(): TimecodeDefinition {
	return {
		id: "00000000-0000-4000-8000-000000000162",
		number: 1,
		name: "Timecode 1",
		duration_frame: 440,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [],
	};
}
