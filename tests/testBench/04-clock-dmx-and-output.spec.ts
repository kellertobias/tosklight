import { BrowserClock } from "../../apps/control-ui/e2e/bench/clockScenario";
import {
	fixture,
	fixtureRange,
} from "../../apps/control-ui/e2e/bench/fixtureDmx";
import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	installTimeCuelists,
	restartPlaybackRun,
} from "../05-virtual-time-persistence-and-recovery.playback-helpers";

scenario(
	"BENCH-CLOCK-DMX-001",
	"uses exact clock boundaries and fixture-aware logical DMX without hidden ticks",
	async (t) => {
		const first = await t.clock.advanceStep();
		expect(first.now).toBe("2020-01-01T00:00:00Z");
		expect(first.packets_sent).toBe(2);

		const boundaries: string[] = [];
		await t.clock.at(
			[
				{ name: "before", at: "999ms" },
				{ name: "exact", at: "1s" },
				{ name: "after", at: "1.001s" },
			],
			({ name, frame }) => boundaries.push(`${name}:${frame.now}`),
		);
		expect(boundaries).toEqual([
			"before:2020-01-01T00:00:00.999Z",
			"exact:2020-01-01T00:00:01Z",
			"after:2020-01-01T00:00:01.001Z",
		]);

		await t.expectFixtureDMX(fixture(1), { Intensity: 0 });
		await t.expectFixtureDMX(fixtureRange(1, 3), { Intensity: 0 });
		await expect(
			t.expectFixtureDMX(fixture(1), { "Missing component": 0 }),
		).rejects.toThrow(/Valid channels: Intensity/u);
		await expect(
			t.expectFixtureDMX(fixture(1), { Intensity: 256 }),
		).rejects.toThrow(/integer from 0 through 255/u);
	},
);

test("BENCH-CLOCK-DMX-002 @bench @wire › free run drives changing production-scheduler frames and freezes before the next action", async ({
	api,
	bench,
	desk,
	show,
}) => {
	await installTimeCuelists(api, show.fixtureIds[0], show.fixtureIds[1]);
	await restartPlaybackRun(api, bench, show.id, [2]);
	const clock = new BrowserClock(bench, desk);
	const artnetMark = bench.artnet.mark();
	const sacnMark = bench.sacn.mark();

	const result = await clock.freeRunFor("350ms");
	expect(result.now).toBe("2020-01-01T00:00:00.350Z");
	const artnet = bench.artnet
		.packetsAfter(artnetMark)
		.filter((packet) => packet.protocol === "artnet" && packet.universe === 1);
	const sacn = bench.sacn
		.packetsAfter(sacnMark)
		.filter((packet) => packet.protocol === "sacn" && packet.universe === 101);
	expect(artnet.length).toBeGreaterThan(5);
	expect(sacn.length).toBeGreaterThan(5);
	expect(new Set(artnet.map((packet) => packet.slots[1])).size).toBeGreaterThan(
		3,
	);

	const frozenArtNet = bench.artnet.mark();
	const frozenSacn = bench.sacn.mark();
	await clock.waitWall("100ms");
	expect(bench.artnet.packetsAfter(frozenArtNet)).toEqual([]);
	expect(bench.sacn.packetsAfter(frozenSacn)).toEqual([]);
});
