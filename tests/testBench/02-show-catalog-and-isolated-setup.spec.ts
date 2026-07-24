import fs from "node:fs/promises";
import { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import { DeskDriver } from "../../apps/control-ui/e2e/bench/desk";
import { expect, test } from "../../apps/control-ui/e2e/bench/fixtures";
import { LightBench } from "../../apps/control-ui/e2e/bench/lightBench";
import { scenario } from "../../apps/control-ui/e2e/bench/scenario";
import {
	BrowserShows,
	defineShow,
	Show,
} from "../../apps/control-ui/e2e/bench/showScenario";

const staleCompactRig = defineShow("stale-compact-rig", (show) => {
	show.from(Show.CompactRig).requires({
		fixtureNumbers: [9999],
		profiles: ["Missing Profile"],
		groups: ["999"],
		desktops: ["Missing Desktop"],
	});
});

scenario(
	"BENCH-SHOW-001",
	"activates every immutable catalog entry as an isolated working copy",
	async (t) => {
		for (const show of [
			Show.Empty,
			Show.TwelveDimmers,
			Show.CompactRig,
			Show.DefaultStage,
		]) {
			await t.show.use(show);
			await t.show.expect.active(show);
			const before = t.show.contractIdentity();
			await t.show.resetWorkingCopy();
			await t.show.expect.active(show);
			const after = t.show.contractIdentity();
			expect(after.canonicalId).toBe(before.canonicalId);
			expect(after.workingId).not.toBe(before.workingId);
			expect(after.workingId).not.toBe(after.canonicalId);
		}
	},
);

scenario(
	"BENCH-SHOW-002",
	"rejects stale recipe prerequisites before an operator action",
	async (t) => {
		await expect(t.show.use(staleCompactRig)).rejects.toThrow(
			/fixture numbers 9999.*Groups 999.*profiles Missing Profile.*Desktops Missing Desktop.*Available entries/,
		);
	},
);

test("BENCH-SHOW-003 @bench › concurrent catalogs isolate data, mutation, reset, and cleanup", async ({
	api,
	bench,
	desk,
	page,
	show,
}, testInfo) => {
	testInfo.setTimeout(90_000);
	const peerBench = new LightBench();
	await peerBench.start(testInfo.workerIndex + 10_000);
	const peerInitial = await peerBench.createTwelveDimmerShow();
	const peerApi = new ApiDriver(peerBench.baseUrl);
	peerApi.session = peerInitial.session;
	const peerDesk = new DeskDriver(
		page,
		testInfo.title,
		peerInitial.session.desk.id,
	);
	const first = new BrowserShows(api, bench, desk, show);
	const second = new BrowserShows(peerApi, peerBench, peerDesk, peerInitial);
	const peerDataDir = peerBench.dataDir;
	try {
		await Promise.all([
			first.use(Show.CompactRig),
			second.use(Show.CompactRig),
		]);
		const firstIdentity = first.contractIdentity();
		const secondIdentity = second.contractIdentity();
		expect(firstIdentity.dataDir).not.toBe(secondIdentity.dataDir);
		expect(firstIdentity.workingId).not.toBe(secondIdentity.workingId);
		await api.seedShowObject(firstIdentity.workingId, "group", "999", {
			id: "999",
			name: "Only first worker",
			fixtures: [],
			derived_from: null,
			frozen_from: null,
			programming: {},
			master: 1,
			playback_fader: 100,
		});
		expect(
			(await peerApi.showObjects(secondIdentity.workingId, "group")).some(
				(group) => group.id === "999",
			),
		).toBe(false);
		await first.resetWorkingCopy();
		expect(
			(await api.showObjects(first.contractIdentity().workingId, "group")).some(
				(group) => group.id === "999",
			),
		).toBe(false);
		await api.openShow(firstIdentity.canonicalId, {
			transition: "hold_current",
		});
		expect(
			(await api.showObjects(firstIdentity.canonicalId, "group")).some(
				(group) => group.id === "999",
			),
		).toBe(false);
	} finally {
		await peerDesk.dispose();
		await peerBench.stop();
	}
	await expect(fs.stat(peerDataDir)).rejects.toMatchObject({ code: "ENOENT" });
});
