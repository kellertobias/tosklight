import { describe, expect, it } from "vitest";
import type { CueList } from "../../api/types";
import { cuelistPoolCatalog } from "../../features/playbackTopology/cuelistPoolCatalog";

const list = (id: string): CueList => ({
	id,
	name: id,
	cues: [],
	mode: "sequence",
	priority: 0,
	looped: false,
});
const assignment = (
	number: number,
	id: string,
): import("../../components/control/playbackFaderBank/types").AssignedPlayback => ({
	address: { page: 4, position: number },
	name: `Playback ${number}`,
	target: { type: "cue_list", cue_list_id: id },
	buttons: ["go", "none", "none"],
	fader: "master",
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
});

describe("independent Cuelist pool catalog", () => {
	it("retains every legacy alias but uses the lowest as canonical", () => {
		const entries = cuelistPoolCatalog(
			[{ ...list("a"), pool_number: 101, legacy_pool_aliases: [190] }],
			[assignment(190, "a"), assignment(101, "a")],
		);
		expect(
			entries.map(({ number, canonicalNumber, legacyAlias }) => [
				number,
				canonicalNumber,
				legacyAlias,
			]),
		).toEqual([
			[101, 101, false],
			[190, 101, true],
		]);
	});
	it("rejects ambiguous legacy addresses", () => {
		expect(() =>
			cuelistPoolCatalog(
				[
					{ ...list("a"), pool_number: 101 },
					{ ...list("b"), pool_number: 101 },
				],
				[assignment(101, "a"), assignment(101, "b")],
			),
		).toThrow("ambiguous");
	});
	it("preserves stable Cuelist numbering independent of absent or empty pages", () => {
		const lists = [list("b"), list("a")];
		expect(cuelistPoolCatalog(lists, [], true)[0].cueList.id).toBe("a");
		expect(cuelistPoolCatalog(lists, [], false)[0].cueList.id).toBe("a");
	});
	it("never borrows a reserved SpeedGroup presentation for Cuelist101", () => {
		const special: import("../../components/control/playbackFaderBank/types").AssignedPlayback =
			{
				...assignment(101, "a"),
				name: "Speed A",
				target: { type: "speed_group", group: "A" },
			};
		const entries = cuelistPoolCatalog(
			[{ ...list("a"), pool_number: 101 }],
			[special],
		);
		expect(entries[0].cueList.name).toBe("a");
		expect(entries[0].assignment).toBeUndefined();
		expect(entries[0].number).toBe(101);
	});
	it("uses explicit pool metadata independently of moved physical assignments", () => {
		const cue = { ...list("a"), pool_number: 101, legacy_pool_aliases: [190] };
		const entries = cuelistPoolCatalog([cue], [assignment(500, "a")]);
		expect(entries.map((entry) => entry.number)).toEqual([101, 190]);
		expect(entries[0].assignment?.address.position).toBe(500);
	});
	it("preserves pending legacy aliases until the atomic migration event arrives", () => {
		const entries = cuelistPoolCatalog(
			[
				{ ...list("a"), pool_number: 101 },
				{ ...list("b"), pool_number: 202, legacy_pool_aliases: [250] },
			],
			[assignment(101, "a"), assignment(202, "b"), assignment(250, "b")],
		);
		expect(entries.map(({ number, cueList }) => [number, cueList.id])).toEqual([
			[101, "a"],
			[202, "b"],
			[250, "b"],
		]);
	});
	it("rejects a transient explicit address colliding with another legacy alias", () => {
		expect(() =>
			cuelistPoolCatalog(
				[
					{ ...list("a"), pool_number: 250 },
					{ ...list("b"), pool_number: 202, legacy_pool_aliases: [250] },
				],
				[assignment(202, "b"), assignment(250, "b")],
			),
		).toThrow("ambiguous");
	});
});
