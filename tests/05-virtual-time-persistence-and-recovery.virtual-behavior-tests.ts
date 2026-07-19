import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import {
	installTimeCuelists,
	playbackRuntime,
	restartPlaybackRun,
} from "./05-virtual-time-persistence-and-recovery.playback-helpers";
import {
	setSpeedGroups,
	visualizationLevel,
} from "./05-virtual-time-persistence-and-recovery.time-helpers";
import {
	activeShowId,
	fixtureIdsByNumber,
	loadCanonicalCopy,
} from "./support/catalog";

export function registerVirtualBehaviorTest(): void {
	test("TIME-003 @wire › chaser and phaser phase use virtual timestamps across incremental, speed, pause, and week jumps", async ({
		api,
		bench,
	}) => {
		await loadCanonicalCopy(api, bench, "time-003");
		const fixtures = await fixtureIdsByNumber(api);
		const showId = await activeShowId(api);
		const chaserId = await installTimeCuelists(api, fixtures[1], fixtures[2]);
		await setSpeedGroups(api, [120, 90, 60, 30, 15]);

		await restartPlaybackRun(api, bench, showId, [1]);
		await bench.tick(1_000);
		const direct = await playbackRuntime(api, 1);
		await restartPlaybackRun(api, bench, showId, [1]);
		for (let index = 0; index < 4; index += 1) await bench.tick(250);
		const incremental = await playbackRuntime(api, 1);
		expect(incremental.current_cue_number).toBe(direct.current_cue_number);
		expect(incremental.activated_at).toBe(direct.activated_at);

		await restartPlaybackRun(api, bench, showId, [1]);
		await bench.tick(250);
		await setSpeedGroups(api, [60, 90, 60, 30, 15]);
		await bench.tick(499);
		expect((await playbackRuntime(api, 1)).current_cue_number).toBe(1);
		await bench.tick(1);
		expect((await playbackRuntime(api, 1)).current_cue_number).toBe(2);

		await setSpeedGroups(api, [120, 90, 60, 30, 15]);
		await restartPlaybackRun(api, bench, showId, [1]);
		await bench.tick(250);
		await api.request("POST", `/api/v1/playbacks/${chaserId}/pause`, {});
		const paused = await playbackRuntime(api, 1);
		await bench.tick(3_000);
		expect(await playbackRuntime(api, 1)).toMatchObject({
			current_cue_number: paused.current_cue_number,
			paused: true,
		});
		await api.request("POST", `/api/v1/playbacks/${chaserId}/go`, {});
		await bench.tick(249);
		expect((await playbackRuntime(api, 1)).current_cue_number).toBe(1);
		await bench.tick(1);
		expect((await playbackRuntime(api, 1)).current_cue_number).toBe(2);

		await restartPlaybackRun(api, bench, showId, [1]);
		const week = await bench.tick(604_800_000);
		expect(week.now).toBe("2020-01-08T00:00:00Z");
		expect((await playbackRuntime(api, 1)).current_cue_number).toBe(1);

		await restartPlaybackRun(api, bench, showId, [2]);
		await bench.tick(250);
		const phaserDirect = await visualizationLevel(
			api,
			fixtures[2],
			"intensity",
		);
		await restartPlaybackRun(api, bench, showId, [2]);
		for (let index = 0; index < 10; index += 1) await bench.tick(25);
		expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(
			phaserDirect,
			6,
		);
		expect(phaserDirect).toBeCloseTo(0.5, 6);

		await api.request("POST", "/api/v1/cuelists/3/button", {
			button: 3,
			pressed: true,
		});
		await bench.tick(1_000);
		expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(
			phaserDirect,
			6,
		);
		await api.request("POST", "/api/v1/cuelists/3/button", {
			button: 3,
			pressed: true,
		});
		await bench.tick(250);
		expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(
			1,
			6,
		);

		await restartPlaybackRun(api, bench, showId, [2]);
		await bench.tick(604_800_000);
		expect(await visualizationLevel(api, fixtures[2], "intensity")).toBeCloseTo(
			0,
			6,
		);
	});
}
