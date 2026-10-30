import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Cue } from "../../api/types/playback";
import { TrackSheetGrid } from "./TrackSheetGrid";
import { buildTrackSheet } from "./trackSheetModel";

const cues: Cue[] = Array.from({ length: 300 }, (_, index) => ({
	id: `c${index}`,
	number: String(index + 1),
	name: "",
	fade_millis: 0,
	delay_millis: 0,
	trigger: { type: "manual" },
	changes: index % 10 === 0
		? Array.from({ length: 500 }, (_, fixture) => ({ fixture_id: `f${fixture}`, attribute: "intensity", value: { kind: "normalized" as const, value: (index % 3) / 2 } }))
		: [],
}));

describe("TrackSheetGrid", () => {
	it("renders only the visible window of a 500 fixture × 300 Cue sheet and reports cell clicks", () => {
		const sheet = buildTrackSheet({ cues }, { groupMembers: new Map(), attributeFamily: () => "intensity" });
		const onSelect = vi.fn();
		render(
			<TrackSheetGrid
				rows={sheet.rows}
				columns={cues.map((_, index) => index)}
				cues={cues}
				presets={new Map()}
				rowLabel={(row) => ({ fixture: row.fixtureId, attribute: "Dimmer" })}
				selection={null}
				onSelect={onSelect}
			/>,
		);
		const cells = screen.getAllByRole("gridcell");
		expect(cells.length).toBeGreaterThan(0);
		expect(cells.length).toBeLessThan(1500);
		expect(screen.getByRole("grid", { name: "Track sheet" })).toHaveAttribute("aria-rowcount", "501");
		const first = screen.getByRole("gridcell", { name: "f0 Dimmer, Cue 1: New value 0%" });
		expect(screen.getByRole("gridcell", { name: "f0 Dimmer, Cue 2: Tracked 0%" })).toHaveAttribute("data-state", "tracked");
		fireEvent.click(first);
		expect(onSelect).toHaveBeenCalledWith({ rowKey: sheet.rows[0].key, cue: 0 });
	});
});
