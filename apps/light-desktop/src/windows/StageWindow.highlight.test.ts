import { describe, expect, it } from "vitest";
import type { HighlightState } from "../api/types";
import { stageHighlightFixtureIds } from "./StageWindow";

function highlight(overrides: Partial<HighlightState> = {}): HighlightState {
	return {
		active: true,
		mode: "selection",
		output_enabled: true,
		capture_only: false,
		remembered: [{ fixture_id: "fixture-1" }, { fixture_id: "fixture-2" }],
		active_index: null,
		active_fixture: null,
		can_previous: false,
		can_next: false,
		...overrides,
	};
}

describe("Stage Highlight projection", () => {
	it("shows every remembered fixture while selection Highlight is active", () => {
		expect(stageHighlightFixtureIds(highlight())).toEqual([
			"fixture-1",
			"fixture-2",
		]);
	});

	it("shows only the active stepped fixture", () => {
		expect(
			stageHighlightFixtureIds(
				highlight({
					mode: "step",
					active_fixture: { fixture_id: "fixture-2" },
				}),
			),
		).toEqual(["fixture-2"]);
	});

	it("does not fake output when Highlight is inactive or capture-only", () => {
		expect(stageHighlightFixtureIds(highlight({ active: false }))).toEqual([]);
		expect(
			stageHighlightFixtureIds(highlight({ output_enabled: false })),
		).toEqual([]);
	});
});
