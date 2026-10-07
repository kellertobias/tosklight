import { describe, expect, it } from "vitest";
import type { OutputReadoutSnapshot } from "../../../../api/familyEncoderModels";
import { familySlotDisplay, positionSelectionState, type ProgrammerValueEntry } from "./familyEncoderDisplay";
import { FIXTURE_A, FIXTURE_B, PAN, TARGET_X, TILT } from "./familyEncoderTestSupport";
import {
	FROM_POINT_LABEL,
	FROM_TARGET_LABEL,
	FROM_XYZ_LABEL,
	positionTargetProvenance,
	selectionPositionEntries,
	withFrameRequests,
} from "./positionReadouts";

const POINT_ID = "33333333-3333-4333-8333-333333333333";
const OTHER_POINT_ID = "44444444-4444-4444-8444-444444444444";

const target = (reference: { kind: "origin" } | { kind: "point"; point_id: string }, x = 0) => ({
	kind: "position" as const,
	value: {
		kind: "target" as const,
		reference,
		offset_metres: [
			{ kind: "value" as const, value: x },
			{ kind: "value" as const, value: 0 },
			{ kind: "value" as const, value: 0 },
		] as [never, never, never],
	},
});
const angles = (pan: number) => ({
	kind: "position" as const,
	value: {
		kind: "angles" as const,
		pan_degrees: { kind: "value" as const, value: pan },
		tilt_degrees: { kind: "value" as const, value: 0 },
	},
});
const entry = (fixtureId: string, value: ProgrammerValueEntry["value"], programmerOrder?: number) =>
	({ fixtureId, attribute: "position", value, programmerOrder }) as ProgrammerValueEntry;

function readouts(
	poses: Array<{ pan: number; tilt: number } | null>,
	requested: Array<ProgrammerValueEntry["value"] | null> = [],
): OutputReadoutSnapshot {
	return {
		lane: "normal",
		lease: 3,
		revision: 1,
		owners: poses.map((pose, index) => ({
			fixture_id: [FIXTURE_A, FIXTURE_B][index],
			requested: requested[index] ?? null,
			position: {
				available: pose !== null,
				commands: pose ? [{ destination: "d", emitter_id: "e", pan_degrees: pose.pan, tilt_degrees: pose.tilt }] : [],
				common: pose ? { pan_degrees: pose.pan, tilt_degrees: pose.tilt } : null,
			},
		})),
	} as unknown as OutputReadoutSnapshot;
}

describe("Position readouts (TL-652)", () => {
	it("projects the selected group's Position onto its members, newest programmer order first", () => {
		const group = [{ groupId: "1", attribute: "position", value: target({ kind: "origin" }, 0.2), programmerOrder: 5 }];
		const projected = selectionPositionEntries({
			fixtureValues: [entry(FIXTURE_A, angles(10), 2)],
			groupValues: group,
			groupId: "1",
			fixtureIds: [FIXTURE_A, FIXTURE_B],
		});
		expect(positionSelectionState(projected, [FIXTURE_A, FIXTURE_B])).toEqual({
			representation: "target",
			reference: { kind: "origin" },
		});
		expect(familySlotDisplay(TARGET_X, { programmerValues: projected, readouts: null }).text).toBe("0.2 m");
		// A newer fixture value wins over the older group value for that fixture.
		const newer = selectionPositionEntries({
			fixtureValues: [entry(FIXTURE_A, angles(10), 9)],
			groupValues: group,
			groupId: "1",
			fixtureIds: [FIXTURE_A, FIXTURE_B],
		});
		expect(positionSelectionState(newer, [FIXTURE_A, FIXTURE_B]).representation).toBeUndefined();
		// Another group's value, or no selected group, projects nothing.
		const none = selectionPositionEntries({ fixtureValues: [], groupValues: group, groupId: "2", fixtureIds: [FIXTURE_A] });
		expect(none).toEqual([]);
	});

	it("reads X/Y/Z from the displayed frame's request when the Programmer holds nothing", () => {
		const frame = readouts([null, null], [target({ kind: "origin" }, 1.5), target({ kind: "origin" }, 1.5)]);
		expect(familySlotDisplay(TARGET_X, { programmerValues: [], readouts: frame })).toMatchObject({ text: "1.5 m", source: "requested" });
		// The Programmer's own value is preferred for a fixture that holds one.
		expect(withFrameRequests([entry(FIXTURE_A, target({ kind: "origin" }, 0.3))], frame, [FIXTURE_A, FIXTURE_B])).toHaveLength(2);
	});

	it("shows differing numbers as their real minimum...maximum instead of Mixed", () => {
		const values = [entry(FIXTURE_A, target({ kind: "origin" }))];
		expect(familySlotDisplay(TILT, { programmerValues: values, readouts: readouts([{ pan: 0, tilt: 88.58 }, { pan: 0, tilt: 12.4 }]) })).toEqual({
			value: null,
			text: "12.4°...88.6°",
			source: "resolved",
			provenance: FROM_XYZ_LABEL,
		});
		// Equal at display resolution: one value, no range.
		expect(familySlotDisplay(TILT, { programmerValues: [], readouts: readouts([{ pan: 0, tilt: 88.58 }, { pan: 0, tilt: 88.61 }]) }).text).toBe("88.6°");
		// Requested Angles that differ read as a range too.
		const spread = [entry(FIXTURE_A, angles(270)), entry(FIXTURE_B, angles(-270))];
		expect(familySlotDisplay(PAN, { programmerValues: spread, readouts: null })).toEqual({ value: null, text: "-270°...270°", source: "requested" });
	});

	it("names the Target the resolved angles come from", () => {
		const fixtures = [FIXTURE_A, FIXTURE_B];
		const both = (left: ProgrammerValueEntry["value"], right: ProgrammerValueEntry["value"]) => [entry(FIXTURE_A, left), entry(FIXTURE_B, right)];
		const point = { kind: "point" as const, point_id: POINT_ID };
		expect(positionTargetProvenance(both(target({ kind: "origin" }), target({ kind: "origin" })), fixtures)).toBe(FROM_XYZ_LABEL);
		expect(positionTargetProvenance(both(target(point), target(point)), fixtures)).toBe(FROM_POINT_LABEL);
		expect(positionTargetProvenance(both(target(point), target({ kind: "point", point_id: OTHER_POINT_ID })), fixtures)).toBe(FROM_TARGET_LABEL);
		expect(positionTargetProvenance(both(target(point), target({ kind: "origin" })), fixtures)).toBe(FROM_TARGET_LABEL);
		expect(positionTargetProvenance(both(target(point), angles(0)), fixtures)).toBeNull();
		expect(positionTargetProvenance([], fixtures)).toBeNull();
		const display = familySlotDisplay(PAN, { programmerValues: both(target(point), target(point)), readouts: readouts([{ pan: 5, tilt: 1 }, { pan: 5, tilt: 1 }]) });
		expect(display).toEqual({ value: 5, text: "5°", source: "resolved", provenance: FROM_POINT_LABEL });
	});
});
