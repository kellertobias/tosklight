import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../../api/types";
import { fixtureParameterTargets } from "./useParameterProjection";

function fixture(
	id: string,
	heads: Array<{
		index: number;
		shared: boolean;
		attributes: string[];
	}>,
): PatchedFixture {
	return {
		fixture_id: id,
		logical_heads: heads
			.filter((head) => !head.shared)
			.map((head) => ({
				fixture_id: `${id}.${head.index}`,
				head_index: head.index,
			})),
		definition: {
			heads: heads.map((head) => ({
				index: head.index,
				shared: head.shared,
				parameters: head.attributes.map((attribute) => ({ attribute })),
			})),
		},
	} as PatchedFixture;
}

describe("fixtureParameterTargets", () => {
	it("preserves selection order while excluding fixtures and heads without an attribute", () => {
		const fixtures = [
			fixture("wash", [
				{ index: 0, shared: true, attributes: ["pan"] },
				{ index: 1, shared: false, attributes: ["intensity", "color.red"] },
			]),
			fixture("dimmer", [
				{ index: 0, shared: true, attributes: ["intensity"] },
			]),
		];
		const targets = fixtureParameterTargets(
			["dimmer", "wash.1", "wash"],
			fixtures,
		);

		expect(targets.get("intensity")).toEqual(["dimmer", "wash.1"]);
		expect(targets.get("color.red")).toEqual(["wash.1"]);
		expect(targets.get("pan")).toEqual(["wash"]);
	});
});
