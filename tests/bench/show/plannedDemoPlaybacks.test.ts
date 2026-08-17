import { describe, expect, it } from "vitest";
import { PLANNED_DEMO_FIXTURES } from "../../support/plannedDemoManifest";
import { installPlannedDemoPlaybacks } from "../../support/plannedDemoPlaybacks";

describe("Plan 76 initial Playback topology", () => {
	it("creates Group Masters, individual ACLs, Hazer, Start, and the Speed D chase", async () => {
		const writes: Array<{ kind: string; id: string; body: any }> = [];
		const retainedVirtualPlayback = {
			"1001": { number: 1001, target: { type: "dynamic", dynamic_id: "pwm" } },
		};
		const api = {
			showObjects: async () => [],
			showObject: async (_showId: string, kind: string, id: string) =>
				kind === "playback_page" && id === "1"
					? {
							id,
							revision: 1,
							body: { virtual_playbacks: retainedVirtualPlayback },
						}
					: null,
			seedShowObject: async (
				_showId: string,
				kind: string,
				id: string,
				body: any,
			) => {
				writes.push({ kind, id, body });
			},
		} as any;
		const fixtures = PLANNED_DEMO_FIXTURES.map((fixture) => ({
			fixture_id: `fixture-${fixture.number}`,
			fixture_number: fixture.number,
			logical_heads: [],
		}));
		const result = await installPlannedDemoPlaybacks(api, "show", fixtures);
		expect(result.cuelists).toHaveLength(8);
		expect(result.playbacks).toHaveLength(14);
		expect(
			result.playbacks.filter((item) => item.target.type === "group"),
		).toHaveLength(6);
		const start = result.cuelists.find((item) => item.name === "Start")!;
		const startChanges = start.cues[0].changes as Array<{
			attribute: string;
			value: { value: number };
		}>;
		expect(
			startChanges.filter((change) => change.attribute === "intensity").length,
		).toBeGreaterThan(0);
		expect(
			startChanges
				.filter((change) => change.attribute === "intensity")
				.every((change) => change.value.value === 1),
		).toBe(true);
		for (const attribute of [
			"color.red",
			"color.green",
			"color.blue",
			"pan",
			"tilt",
		]) {
			const values = startChanges
				.filter((change) => change.attribute === attribute)
				.map((change) => change.value.value);
			expect(values.length).toBeGreaterThan(0);
			expect(new Set(values)).toEqual(
				new Set([attribute.startsWith("color.") ? 1 : 0.5]),
			);
		}
		const hazer = result.cuelists.find((item) => item.name === "Hazer")!;
		expect(hazer.cues[0].changes).toHaveLength(2);
		expect(
			hazer.cues[0].changes.every(
				(change: any) =>
					change.attribute === "intensity" && change.value.value === 0.2,
			),
		).toBe(true);
		expect(
			result.playbacks.find((item) => item.name === "Hazer"),
		).toMatchObject({
			fader: "master",
			has_fader: true,
			go_activates: true,
		});
		const front = result.cuelists.find((item) => item.name === "Front Light")!;
		expect(front.cues[0].changes).toHaveLength(12);
		expect(
			result.playbacks.find((item) => item.name === "Front Light")?.number,
		).toBe(2);
		expect(
			result.playbacks
				.filter((item) => item.target.type === "cue_list")
				.map((item) => item.number),
		).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
		const chase = result.cuelists.find((item) => item.name === "ACL Chase")!;
		expect(chase).toMatchObject({
			mode: "chaser",
			wrap_mode: "reset",
			looped: true,
			speed_group: "D",
		});
		expect(chase.cues).toHaveLength(7);
		const activeOrder = [0, 1, 2, 3, 2, 1, 0];
		for (const [index, cue] of chase.cues.entries()) {
			const values = cue.changes.map((change: any) => change.value.value);
			expect(values.filter((value: number) => value === 1)).toHaveLength(1);
			expect(values[activeOrder[index] ?? 0]).toBe(1);
		}
		const page = writes.find((write) => write.kind === "playback_page")?.body;
		expect(page?.name).toBe("Busking");
		expect(page?.slots).toMatchObject({
			1: 101,
			6: 106,
			11: 2,
			12: 1,
			17: 8,
			18: 4,
		});
		expect(page?.virtual_playbacks).toEqual(retainedVirtualPlayback);
	});
});
