import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Cue } from "../../api/types";
import { CueTable } from "./CueTable";

const cue: Cue = {
	id: "cue-1",
	number: 1,
	name: "Opening",
	fade_millis: 2_000,
	delay_millis: 500,
	out_fade_millis: 3_000,
	out_delay_millis: 750,
	trigger: { type: "wait", delay_millis: 4_000 },
	changes: [],
};

afterEach(cleanup);

describe("CueTable timing progress", () => {
	it("renders optional normalized timing progress without deriving runtime state", () => {
		render(
			<CueTable
				cues={[cue]}
				active={undefined}
				selectedCue={0}
				settingsOpen={false}
				thumbnails={{}}
				emptyState={{ title: "Empty", description: "Empty", icon: "◎" }}
				onSelectCue={vi.fn()}
				timingProgressByRow={{
					0: { triggerTime: 0.25, inFade: 0.5, outFade: 2 },
				}}
			/>,
		);

		expect(screen.getByRole("button", { name: "Trigger Time" })).toHaveStyle({
			"--cue-timing-progress": "25%",
		});
		expect(
			screen.getByRole("progressbar", { name: "In Fade progress" }),
		).toHaveAttribute("aria-valuetext", "2 s, 50% complete");
		expect(screen.getByRole("button", { name: "Out Fade" })).toHaveStyle({
			"--cue-timing-progress": "100%",
		});
		expect(
			screen.getByRole("progressbar", { name: "Trigger Time progress" }),
		).toHaveAttribute("aria-valuenow", "25");
		expect(
			screen.getByRole("progressbar", { name: "Out Fade progress" }),
		).toHaveAttribute("aria-valuenow", "100");
		expect(
			screen.queryByRole("progressbar", { name: "In Delay progress" }),
		).not.toBeInTheDocument();
	});

	it("opens the exact property from pointer/touch-capable cell controls", () => {
		const onEditCueProperty = vi.fn();
		render(
			<CueTable
				cues={[cue]}
				active={undefined}
				selectedCue={0}
				settingsOpen={false}
				thumbnails={{}}
				emptyState={{ title: "Empty", description: "Empty", icon: "◎" }}
				onSelectCue={vi.fn()}
				onEditCueProperty={onEditCueProperty}
			/>,
		);

		for (const [name, property] of [
			["Trigger", "trigger"],
			["Trigger Time", "triggerTime"],
			["In Delay", "inDelay"],
			["In Fade", "inFade"],
			["Out Delay", "outDelay"],
			["Out Fade", "outFade"],
		] as const) {
			const cell = screen.getByRole("button", { name });
			fireEvent.pointerDown(cell, { pointerType: "touch" });
			fireEvent.pointerUp(cell, { pointerType: "touch" });
			fireEvent.click(cell);
			expect(onEditCueProperty).toHaveBeenLastCalledWith(0, property);
		}
	});
});

describe("CueTable command targets", () => {
	const secondCue = { ...cue, id: "cue-2", number: 2, name: "Second" };
	const emptyState = { title: "Empty", description: "Empty", icon: "◎" };

	function renderTable(commandText: string) {
		const command = {
			text: commandText,
			replace: vi.fn(async () => true),
			execute: vi.fn(async () => true),
		};
		const onSelectCue = vi.fn();
		const view = render(
			<CueTable
				cues={[cue, secondCue]}
				active={undefined}
				selectedCue={0}
				settingsOpen={false}
				thumbnails={{}}
				emptyState={emptyState}
				onSelectCue={onSelectCue}
				playbackNumber={7}
				command={command}
			/>,
		);
		return { ...view, command, onSelectCue };
	}

	it.each([
		"COPY",
		"MOVE",
	] as const)("uses the complete visible Cue row for %s source and destination", (operation) => {
		const source = renderTable(operation);
		const rows = source.container.querySelectorAll<HTMLTableRowElement>(
			".cue-table tbody tr",
		);
		const targetClass = `${operation.toLowerCase()}-target`;
		expect(rows[0]).toHaveClass(targetClass, "cue-command-target");
		expect(rows[0]).toHaveTextContent(
			`${operation[0]}${operation.slice(1).toLowerCase()}`,
		);
		const nestedCell = rows[0].querySelector("td:nth-child(3)");
		expect(nestedCell).not.toBeNull();
		if (nestedCell) fireEvent.click(nestedCell);
		expect(source.command.replace).toHaveBeenCalledWith(
			`${operation} SET 7 CUE 1 AT`,
		);
		expect(source.onSelectCue).not.toHaveBeenCalled();

		source.rerender(
			<CueTable
				cues={[cue, secondCue]}
				active={undefined}
				selectedCue={0}
				settingsOpen={false}
				thumbnails={{}}
				emptyState={emptyState}
				onSelectCue={source.onSelectCue}
				playbackNumber={7}
				command={{ ...source.command, text: `${operation} SET 7 CUE 1 AT` }}
			/>,
		);
		const destination = source.container.querySelectorAll<HTMLTableRowElement>(
			".cue-table tbody tr",
		)[1];
		expect(destination).toHaveClass(targetClass);
		fireEvent.click(destination);
		expect(source.command.execute).toHaveBeenCalledWith(
			`${operation} SET 7 CUE 1 AT SET 7 CUE 2`,
		);
	});

	it("executes Delete from the row and leaves unsupported Set as navigation", () => {
		const deletion = renderTable("DELETE");
		const row = deletion.container.querySelector<HTMLTableRowElement>(
			".cue-table tbody tr",
		);
		expect(row).not.toBeNull();
		if (!row) return;
		expect(row).toHaveClass("delete-target");
		expect(row).toHaveTextContent("Delete");
		fireEvent.click(row);
		expect(deletion.command.execute).toHaveBeenCalledWith("DELETE SET 7 CUE 1");
		deletion.unmount();

		const unsupported = renderTable("SET");
		const ordinaryRow =
			unsupported.container.querySelector<HTMLTableRowElement>(
				".cue-table tbody tr",
			);
		expect(ordinaryRow).not.toBeNull();
		if (!ordinaryRow) return;
		expect(ordinaryRow).not.toHaveClass("cue-command-target");
		fireEvent.click(ordinaryRow);
		expect(unsupported.onSelectCue).toHaveBeenCalledWith(0);
		expect(unsupported.command.execute).not.toHaveBeenCalled();
		expect(unsupported.command.replace).not.toHaveBeenCalled();
	});
});
