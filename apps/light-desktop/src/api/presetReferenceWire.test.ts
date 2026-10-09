import { describe, expect, it } from "vitest";
import { decodePresetReference } from "./presetReferenceWire";
import { decodeCueListBody } from "./showObjectCueWire";

const reference = { preset_instance_id: "fa9bd190-3d12-486f-947c-915fa62503ab", source_owner: { type: "group", group_id: "odd" }, source_attribute: "color", sample_rank: [1, 3] };
describe("live Preset reference decoding", () => {
	it("retains exact immutable identity and source address alongside literal fallback", () => {
		const body = { id: "list", name: "", mode: "sequence", priority: 0, looped: false,
			cues: [{ id: "cue", number: "1", name: "", fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, changes: [],
				group_changes: [{ group_id: "odd", attribute: "color", value: null, preset_reference: reference }] }] };
		expect(decodeCueListBody(body, "$").cues[0].group_changes?.[0]).toMatchObject({ value: null, preset_reference: reference });
	});
	it.each([
		{ ...reference, preset_instance_id: "not-an-instance" },
		{ ...reference, source_attribute: "position" },
		{ ...reference, sample_rank: [3, 3] },
		{ ...reference, sample_rank: [0, 0] },
		{ ...reference, source_owner: { type: "group" } },
	])("rejects an invalid reference before installing the object", (ref) => {
		expect(() => decodePresetReference(ref, "$.reference", "color")).toThrow();
	});
});
