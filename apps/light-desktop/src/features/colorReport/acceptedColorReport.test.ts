import { describe, expect, it } from "vitest";
import type {
	ColorIntentHeadReport,
	ColorIntentReport,
} from "../../api/client/attributeConfiguration";
import {
	acceptedHeads,
	fixtureColorNotices,
	headColorDetail,
	isExpectedLimitation,
	uvText,
} from "./acceptedColorReport";

function head(overrides: Partial<ColorIntentHeadReport>): ColorIntentHeadReport {
	return {
		fixture_id: "a",
		fixture_number: 1,
		fixture_name: "Wash",
		owner_id: "a",
		head_name: "",
		has_target: true,
		quality: "exact",
		engine: null,
		delta_uv: null,
		calibration_revision: null,
		...overrides,
	};
}

const accepted = (heads: ColorIntentHeadReport[]): ColorIntentReport => ({
	color_model: "intent",
	heads,
	accepted_frame: { state: "accepted", frame: null },
});

describe("accepted-frame colour notices", () => {
	it("ignores the legacy report and a frame that is not yet available", () => {
		const limited = head({ quality: "out_of_gamut" });
		expect(acceptedHeads({ color_model: "intent", heads: [limited] })).toBeNull();
		expect(
			fixtureColorNotices({
				color_model: "intent",
				heads: [limited],
				accepted_frame: { state: "not_yet_available", frame: null },
			}).size,
		).toBe(0);
		expect(fixtureColorNotices({ color_model: "intent", heads: [limited] }).size).toBe(0);
	});

	it("marks expected limitations only: invisible approximation and exact results stay quiet", () => {
		expect(isExpectedLimitation(head({ quality: "exact" }))).toBe(false);
		expect(isExpectedLimitation(head({ quality: "approximate", delta_uv: 0.003 }))).toBe(false);
		expect(isExpectedLimitation(head({ quality: "approximate", delta_uv: 0.02 }))).toBe(true);
		expect(isExpectedLimitation(head({ quality: "uncalibrated" }))).toBe(true);
		expect(isExpectedLimitation(head({ uv: { status: "unsupported", clipped: false } }))).toBe(true);
		expect(isExpectedLimitation(head({ uv: { status: "applied", clipped: false } }))).toBe(false);
	});

	it("describes UV separately from the visible match", () => {
		expect(uvText(head({}))).toBeNull();
		expect(uvText(head({ uv: { status: "not_requested", clipped: false } }))).toBeNull();
		expect(uvText(head({ uv: { status: "unsupported", clipped: false } }))).toBe(
			"UV unavailable on this fixture",
		);
		const notices = fixtureColorNotices(
			accepted([head({ quality: "exact", uv: { status: "unsupported", clipped: false } })]),
		);
		expect(notices.get("a")?.label).toBe("Color details: Shows this colour exactly; UV unavailable on this fixture");
	});

	it("carries the report's note: parked controls or why there is no colour model", () => {
		expect(headColorDetail(head({})).note).toBeNull();
		const parked = head({ quality: "uncalibrated", note: "parked at neutral: CTC" });
		expect(headColorDetail(parked).note).toBe("parked at neutral: CTC");
		const none = head({ quality: "unsupported", note: "No colour model: layered engines" });
		expect(headColorDetail(none)).toMatchObject({
			limited: true,
			note: "No colour model: layered engines",
		});
	});

	it("keys one notice per fixture row and per logical head row", () => {
		const notices = fixtureColorNotices(
			accepted([
				head({ fixture_id: "p", owner_id: "p-1", quality: "wheel_limited" }),
				head({ fixture_id: "p", owner_id: "p-2", quality: "exact" }),
				head({ fixture_id: "q", owner_id: "q", quality: "exact" }),
			]),
		);
		expect([...notices.keys()].sort()).toEqual(["p", "p-1"]);
		expect(notices.get("p")?.heads).toHaveLength(1);
	});
});
