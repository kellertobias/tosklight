import fs from "node:fs/promises";
import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import { mapExistingPlaybackToSlot } from "../apps/control-ui/e2e/bench/mapExistingPlaybackToSlot";
import { setOutputRuntime } from "../apps/control-ui/e2e/bench/outputRuntime";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import {
	installGroupCue,
	playbackRuntime,
} from "./05-virtual-time-persistence-and-recovery.playback-helpers";
import {
	assertShow001State,
	fileHash,
	programShow001ThroughUi,
	showEntry,
} from "./05-virtual-time-persistence-and-recovery.show-helpers";
import {
	setProgrammerFade,
	slot,
} from "./05-virtual-time-persistence-and-recovery.time-helpers";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	programmer,
	putObject,
} from "./support/catalog";

export function registerShow001PairedScenario(): void {
	pairedScenario<{
		copyId: string;
		fixtureIds: Record<number, string>;
		revisionName: string;
	}>({
		id: "SHOW-001",
		title:
			"operator programming and a named revision produce the durable restart state",
		arrange: async ({ api, bench }, surface) => {
			const copy = await loadCanonicalCopy(api, bench, `show-001-${surface}`);
			await setProgrammerFade(api, 0, 3_000);
			return {
				copyId: copy.id,
				fixtureIds: await fixtureIdsByNumber(api),
				revisionName: "SHOW-001 before restart",
			};
		},
		api: async ({ api }, state) => {
			await api.command("selection.set", {
				fixtures: [state.fixtureIds[5], state.fixtureIds[6]],
			});
			await api.executeCommandLine("RECORD + GROUP 3");
			await api.executeCommandLine("GROUP 3 AT 40");
			await api.executeCommandLine("RECORD SET 1");
			await mapExistingPlaybackToSlot(api, {
				surface: "api",
				showId: state.copyId,
				page: 1,
				slot: 1,
				playbackNumber: 1,
			});
			await api.command("programmer.clear", {});
			await api.command("programmer.clear", {});
			await api.request("POST", "/api/v1/cuelists/1/go", {});
			await api.executeCommandLine("FIXTURE 12 AT 65");
			await api.request("POST", `/api/v1/shows/${state.copyId}/revisions`, {
				name: state.revisionName,
			});
		},
		ui: async ({ api, bench, desk, page }, state) => {
			await desk.open(bench.baseUrl);
			await programShow001ThroughUi(api, page, state.revisionName);
		},
		assert: async ({ api, bench }, state) =>
			assertShow001State(api, bench, state),
	});
}

export function registerEmptyShowRestartTest(): void {
	test("SHOW-001 @restart › an unnamed empty show survives an abrupt restart and Save As renames its identity", async ({
		api,
		bench,
	}) => {
		const provisional = await api.request<any>("POST", "/api/v1/shows", {
			name: "New Empty Show",
			data_base64: null,
			overwrite: false,
		});
		await api.request("POST", `/api/v1/shows/${provisional.id}/open`, {
			transition: "hold_current",
		});
		await api.request(
			"PUT",
			`/api/v1/shows/${provisional.id}/objects/user_layout/empty-show-durability`,
			{ marker: "programmed before naming" },
			true,
			0,
		);
		const provisionalPath = provisional.path as string;
		expect(
			(await api.request<any>("GET", "/api/v1/bootstrap", undefined, false))
				.active_show,
		).toMatchObject({
			id: provisional.id,
			name: "New Empty Show",
		});

		await bench.restart();
		await api.login("Operator");
		expect(
			(await api.request<any>("GET", "/api/v1/bootstrap", undefined, false))
				.active_show,
		).toMatchObject({
			id: provisional.id,
			name: "New Empty Show",
		});
		expect(
			(await object<any>(api, "user_layout", "empty-show-durability")).body,
		).toEqual({
			marker: "programmed before naming",
		});

		const renamed = await api.request<any>(
			"PUT",
			`/api/v1/shows/${provisional.id}/rename`,
			{
				name: "Opening Night",
			},
		);
		expect(renamed).toMatchObject({
			id: provisional.id,
			name: "Opening Night",
		});
		expect(renamed.path).toContain("Opening Night.show");
		await expect(fs.access(provisionalPath)).rejects.toThrow();
		expect(
			(await api.request<any[]>("GET", "/api/v1/shows")).filter(
				(show) => show.id === provisional.id,
			),
		).toEqual([
			expect.objectContaining({ id: provisional.id, name: "Opening Night" }),
		]);

		await bench.restart();
		await api.login("Operator");
		expect(
			(await api.request<any>("GET", "/api/v1/bootstrap", undefined, false))
				.active_show,
		).toMatchObject({
			id: provisional.id,
			name: "Opening Night",
		});
		expect(
			(await object<any>(api, "user_layout", "empty-show-durability")).body,
		).toEqual({
			marker: "programmed before naming",
		});
	});
}

export function registerShow001ProcessRestartTest(): void {
	test("SHOW-001 @restart › supplemental process check preserves named show state, durable programmer, active playback, PID, and first frame", async ({
		api,
		bench,
	}) => {
		const copy = await loadCanonicalCopy(api, bench, "show-001");
		await setProgrammerFade(api, 0, 0);
		const fixtures = await fixtureIdsByNumber(api);
		const group = await object<any>(api, "group", "3");
		await putObject(
			api,
			"group",
			"3",
			{
				...group.body,
				fixtures: [...group.body.fixtures, fixtures[5], fixtures[6]],
			},
			group.revision,
		);
		const expectedGroup = [1, 2, 3, 4, 5, 6].map((number) => fixtures[number]);
		expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(
			expectedGroup,
		);
		const cueListId = await installGroupCue(api, "3", 0.4);
		expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(
			expectedGroup,
		);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		await api.executeCommandLine("FIXTURE 12 AT 65");
		await bench.tick(0);
		await setOutputRuntime(api, {
			surface: "api",
			showId: copy.id,
			grandMaster: 0.5,
			blackout: false,
		});
		const durableBefore = await programmer(api);
		const revision = await api.request<any>(
			"POST",
			`/api/v1/shows/${copy.id}/revisions`,
			{ name: "SHOW-001 before restart" },
		);
		expect(revision.name).toBe("SHOW-001 before restart");
		const expectedFirstFrame = await bench.tick(0);
		expect(
			expectedFirstFrame.universes
				.find((universe) => universe.universe === 1)
				?.slots.slice(0, 6),
		).toEqual(Array(6).fill(51));
		expect(slot(expectedFirstFrame, 12)).toBe(83);
		const entry = await showEntry(api, copy.id);
		expect(entry.path.startsWith(bench.dataDir)).toBe(true);
		const oldPid = bench.serverPid();
		expect(oldPid).toBeTruthy();
		await bench.stopServerGracefully(api.session!.token);
		const showHash = await fileHash(entry.path);
		const newPid = await bench.startServer();
		expect(newPid).not.toBe(oldPid);
		await api.login("Operator");

		const bootstrap = await api.request<any>(
			"GET",
			"/api/v1/bootstrap",
			undefined,
			false,
		);
		expect(bootstrap.active_show.id).toBe(copy.id);
		expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(
			expectedGroup,
		);
		const restoredGroupChange = (await object<any>(api, "cue_list", cueListId))
			.body.cues[0].group_changes[0];
		expect(restoredGroupChange).toMatchObject({
			group_id: "3",
			attribute: "intensity",
			value: { kind: "normalized" },
		});
		expect(restoredGroupChange.value.value).toBeCloseTo(0.4, 6);
		expect(
			(await object<any>(api, "playback", "1")).body.target.cue_list_id,
		).toBe(cueListId);
		expect(await playbackRuntime(api, 1)).toMatchObject({
			current_cue_number: 1,
			enabled: true,
		});
		const restored = await programmer(api);
		expect(restored.user_id).toBe(durableBefore.user_id);
		expect(
			restored.values.find(
				(value: any) =>
					value.fixture_id === fixtures[12] && value.attribute === "intensity",
			)?.value,
		).toMatchObject({ value: 0.65 });
		expect(await fileHash(entry.path)).toBe(showHash);
		expect(
			await api.request<any>("GET", "/api/v1/visualization"),
		).toMatchObject({ grand_master: 0.5, blackout: false });
		const firstFrame = await bench.tick(0);
		expect(firstFrame.universes).toEqual(expectedFirstFrame.universes);
		expect(
			firstFrame.universes
				.find((universe) => universe.universe === 1)
				?.slots.slice(0, 6),
		).toEqual(Array(6).fill(51));
		expect(slot(firstFrame, 12)).toBe(83);
	});
}
