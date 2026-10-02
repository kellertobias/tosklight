import { describe, expect, it } from "vitest";
import type {
	AttributeEncoderGroup,
	AttributeEncoderPlacement,
} from "../attributeEncoderPages";
import {
	type FamilyLayoutSlot,
	familyEncoderLayout,
	familyLayoutSlots,
} from "./familyEncoderLayout";
import { pagesSnapshot } from "./familyEncoderTestSupport";

function placement(id: string, page: number, slot: number): AttributeEncoderPlacement {
	return {
		id,
		label: id,
		encoder_group: "position",
		encoder_page: page,
		encoder_slot: slot,
	};
}

const positionRegistry: AttributeEncoderGroup = {
	id: "position",
	label: "Position",
	pages: [
		{
			number: 1,
			slots: [
				placement("pan", 1, 1),
				placement("tilt", 1, 2),
				placement("position.movement", 1, 3),
				null,
			],
		},
	],
};

const ids = (page: FamilyLayoutSlot[]) =>
	page.map((slot) =>
		slot === null ? null : slot.kind === "component" ? slot.slot.id : slot.attribute,
	);

const supportsEverything = () => true;

describe("family encoder layout", () => {
	it("keeps the legacy pages when the backend does not report the semantic contract", () => {
		for (const snapshot of [null, pagesSnapshot(false)])
			expect(
				familyEncoderLayout({
					snapshot,
					family: "Position",
					registryGroup: positionRegistry,
					visibleEncoderCount: 4,
					supportsAttribute: supportsEverything,
				}),
			).toBeNull();
	});

	it("keeps non-semantic families on their registry pages", () => {
		expect(
			familyEncoderLayout({
				snapshot: pagesSnapshot(true),
				family: "Beam",
				registryGroup: undefined,
				visibleEncoderCount: 4,
				supportsAttribute: supportsEverything,
			}),
		).toBeNull();
	});

	it("puts Pan/Tilt on page 1, Point/X/Y/Z on page 2 and the unreplaced registry rest after", () => {
		const layout = familyEncoderLayout({
			snapshot: pagesSnapshot(true),
			family: "Position",
			registryGroup: positionRegistry,
			visibleEncoderCount: 4,
			supportsAttribute: supportsEverything,
		});
		expect(layout?.pages.map(ids)).toEqual([
			["position.pan", "position.tilt", null, null],
			["position.target", "position.target.x", "position.target.y", "position.target.z"],
			[null, null, "position.movement", null],
		]);
		const page2 = familyLayoutSlots(layout!, 2);
		expect(page2.encoderSlots).toEqual([null, null, null, null]);
		expect(page2.componentSlots.map((slot) => slot?.slot.id)).toEqual([
			"position.target",
			"position.target.x",
			"position.target.y",
			"position.target.z",
		]);
		expect(familyLayoutSlots(layout!, 3).encoderSlots).toEqual([
			null,
			null,
			"position.movement",
			null,
		]);
	});

	it("fills 6-encoder layouts sequentially with the assigned semantic slots", () => {
		const layout = familyEncoderLayout({
			snapshot: pagesSnapshot(true),
			family: "Position",
			registryGroup: undefined,
			visibleEncoderCount: 6,
			supportsAttribute: supportsEverything,
		});
		expect(layout?.pages.map(ids)).toEqual([
			[
				"position.pan",
				"position.tilt",
				"position.target",
				"position.target.x",
				"position.target.y",
				"position.target.z",
			],
		]);
	});

	it("orders Focus, Zoom, Softness and drops an unsupported registry attribute slot", () => {
		const layout = (supportsSoftness: boolean) =>
			familyEncoderLayout({
				snapshot: pagesSnapshot(true),
				family: "Focus",
				registryGroup: undefined,
				visibleEncoderCount: 4,
				supportsAttribute: (attribute) => supportsSoftness && attribute === "softness",
			});
		expect(layout(true)?.pages.map(ids)).toEqual([["focus", "zoom", "softness", null]]);
		expect(layout(false)?.pages.map(ids)).toEqual([["focus", "zoom", null, null]]);
	});

	it("renders nothing yet for the reserved Direct Color pages", () => {
		const layout = familyEncoderLayout({
			snapshot: pagesSnapshot(true),
			family: "Color",
			registryGroup: undefined,
			visibleEncoderCount: 4,
			supportsAttribute: supportsEverything,
		});
		expect(layout?.pages).toHaveLength(1);
		expect(layout?.group.reserved_pages).toEqual([{ number: 3, reason: "native_color" }]);
	});
});
