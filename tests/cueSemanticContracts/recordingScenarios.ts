import { closeWebSocket } from "../../apps/control-ui/e2e/bench/api";
import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import {
	enterProgrammerPreload,
	releaseProgrammerPreload,
} from "../../apps/control-ui/e2e/bench/programmerPreloadLifecycle";
import {
	clearProgrammerValues,
	setProgrammerGroupValue,
} from "../../apps/control-ui/e2e/bench/programmerValues";
import {
	loadCanonicalCopy,
	object,
	objects,
	putObject,
} from "../support/catalog";
import { cue001Ui } from "./recordingUiSupport";
import {
	currentProgrammer,
	emptyPlaybackPage,
	eventIdentity,
	expectNewRecordedCuelist,
	groupCue,
	groupValues,
	installCompactGroups,
	installPlaybackSequence,
	logicalSlots,
	openEventStream,
	playbackAtSlot,
	playbackState,
	registerPairedCueScenario,
	runtime,
	setSequenceMasterFade,
	showObjectEventAfter,
} from "./support";

const PROGRAMMER_TIMING = {
	fade: true,
	fadeMillis: 0,
	delayMillis: null,
} as const;

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "CUE-008",
	title:
		"blind Preload records the same Cue without activating playback or output",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`cue-008-preload-record-${surface}`,
			"compact-rig",
		);
		await installCompactGroups(api);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.executeCommandLine("GROUP 1 AT 100");
		const pending = await currentProgrammer(api);
		expect(pending.preload_group_pending["1"].intensity.value).toMatchObject({
			kind: "normalized",
			value: 1,
		});
		const installed = await installPlaybackSequence(api, 1, [
			groupCue(1, [["1", "intensity", 1]]),
		]);
		const stored = await object<any>(api, "cue_list", installed.id);
		expect(stored.body.cues[0].group_changes).toMatchObject([
			{
				group_id: "1",
				attribute: "intensity",
				value: { kind: "normalized", value: 1 },
			},
		]);
		expect(
			(await playbackState(api)).active.some(
				(item: any) => item.playback_number === 1 && item.enabled,
			),
		).toBe(false);
		expect(logicalSlots(await bench.tick(0), 4)).toEqual(Array(4).fill(0));
		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		expect(logicalSlots(await bench.tick(3_000), 4)).toEqual(
			Array(4).fill(255),
		);
		state.completed = true;
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await emptyPlaybackPage(api);
		const beforeCuelists = new Set(
			(await objects(api, "cue_list")).map((item) => item.id),
		);
		await enterProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.executeCommandLine("GROUP 1 AT 100");

		await desk.open(bench.baseUrl);
		await page.locator(".mode-toggle").click();
		await page.getByRole("button", { name: "REC", exact: true }).click();
		await page
			.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			})
			.click();

		const stored = await expectNewRecordedCuelist(api, beforeCuelists, 1);
		const playbackNumber = await playbackAtSlot(api, 1);
		expect(stored.body.cues[0].group_changes).toMatchObject([
			{
				group_id: "1",
				attribute: "intensity",
				value: { kind: "normalized", value: 1 },
			},
		]);
		expect(
			(await playbackState(api)).active.some(
				(item: any) => item.playback_number === playbackNumber && item.enabled,
			),
		).toBe(false);
		expect(logicalSlots(await bench.tick(0), 4)).toEqual(Array(4).fill(0));

		await releaseProgrammerPreload(api, {
			surface: "api",
			showId: state.showId,
		});
		await api.request("POST", `/api/v1/cuelists/${playbackNumber}/go`, {});
		await expect
			.poll(async () => runtime(api, playbackNumber))
			.toMatchObject({ current_cue_number: 1, enabled: true });
		expect(logicalSlots(await bench.tick(3_000), 4)).toEqual(
			Array(4).fill(255),
		);
		state.completed = true;
	},
	assert: async (_context, state) => expect(state.completed).toBe(true),
});

registerPairedCueScenario<{ completed: boolean; showId: string }>({
	id: "CUE-001",
	title:
		"Record targets playbacks while decimal insertion and Record operations preserve tracking",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(
			api,
			bench,
			`cue-001-record-and-replay-${surface}`,
			"compact-rig",
		);
		await installCompactGroups(api);
		return { completed: false, showId: show.id };
	},
	api: async ({ api, bench }, state) => {
		await setSequenceMasterFade(api, 0);
		const installed = await installPlaybackSequence(api, 1, [
			groupCue(1, [["1", "intensity", 1]]),
			groupCue(2, [
				["2", "intensity", 1],
				["2", "red", 0.2],
			]),
		]);

		await setProgrammerGroupValue(api, {
			surface: "api",
			showId: state.showId,
			groupId: "3",
			attribute: "intensity",
			value: { kind: "normalized", value: 1 },
			timing: PROGRAMMER_TIMING,
		});
		await api.executeCommandLine("RECORD SET 1 CUE 1.5");
		let stored = await object<any>(api, "cue_list", installed.id);
		expect(stored.body.cues.map((cue: any) => cue.number)).toEqual([1, 1.5, 2]);
		expect(groupValues(stored.body.cues[0])).toEqual({ "1:intensity": 1 });
		expect(groupValues(stored.body.cues[1])).toEqual({ "3:intensity": 1 });
		expect(groupValues(stored.body.cues[2])).toEqual({
			"2:intensity": 1,
			"2:red": 0.2,
		});

		const pageAddressed = await installPlaybackSequence(api, 2, [
			groupCue(1, [["1", "intensity", 1]]),
			groupCue(2, [
				["2", "intensity", 1],
				["2", "red", 0.2],
			]),
		]);
		await api.executeCommandLine("RECORD SET 1 . 2 CUE 1.5");
		const pageStored = await object<any>(api, "cue_list", pageAddressed.id);
		const cueSemantics = (body: any) =>
			body.cues.map((cue: any) => ({
				number: cue.number,
				values: groupValues(cue),
			}));
		expect(cueSemantics(pageStored.body)).toEqual(cueSemantics(stored.body));

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		const trackedSequence = [
			[1, 0, 0],
			[1, 0, 1],
			[1, 1, 1],
		];
		for (const groups of trackedSequence) {
			await api.request("POST", "/api/v1/cuelists/1/go", {});
			expect(logicalSlots(await bench.tick(0), 12)).toEqual(
				groups.flatMap((value) => Array(4).fill(value * 255)),
			);
		}
		await api.request("POST", "/api/v1/cuelists/1/off", {});

		await setProgrammerGroupValue(api, {
			surface: "api",
			showId: state.showId,
			groupId: "2",
			attribute: "intensity",
			value: { kind: "normalized", value: 0.8 },
			timing: PROGRAMMER_TIMING,
		});
		await api.executeCommandLine("RECORD + SET 1 CUE 2");
		stored = await object<any>(api, "cue_list", installed.id);
		expect(
			groupValues(stored.body.cues.find((cue: any) => cue.number === 2)),
		).toEqual({ "2:intensity": 0.8, "2:red": 0.2 });

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		await setProgrammerGroupValue(api, {
			surface: "api",
			showId: state.showId,
			groupId: "2",
			attribute: "red",
			value: { kind: "normalized", value: 0.9 },
			timing: PROGRAMMER_TIMING,
		});
		await api.executeCommandLine("RECORD - SET 1 CUE 2");
		stored = await object<any>(api, "cue_list", installed.id);
		expect(
			groupValues(stored.body.cues.find((cue: any) => cue.number === 2)),
		).toEqual({ "2:intensity": 0.8 });

		await clearProgrammerValues(api, {
			surface: "api",
			showId: state.showId,
		});
		const beforeDelete = stored.body;
		const stream = await openEventStream(api);
		try {
			let mark = stream.events.length;
			await api.executeCommandLine("RECORD - SET 1 CUE 2");
			const recordMinusEvent = await showObjectEventAfter(
				stream.events,
				mark,
				installed.id,
			);
			const afterRecordMinus = await object<any>(api, "cue_list", installed.id);
			const recordMinusRuntime = await playbackState(api);
			const recordMinusFrame = logicalSlots(await bench.tick(0), 12);
			expect(afterRecordMinus.body.cues.map((cue: any) => cue.number)).toEqual([
				1, 1.5,
			]);

			await putObject(
				api,
				"cue_list",
				installed.id,
				beforeDelete,
				afterRecordMinus.revision,
			);
			mark = stream.events.length;
			await api.executeCompatibilityProgrammerCommand({
				family: "cue_delete",
				command: "DELETE SET 1 CUE 2",
			});
			const deleteEvent = await showObjectEventAfter(
				stream.events,
				mark,
				installed.id,
			);
			stored = await object<any>(api, "cue_list", installed.id);
			expect(stored.body).toEqual(afterRecordMinus.body);
			expect(eventIdentity(deleteEvent)).toEqual(
				eventIdentity(recordMinusEvent),
			);
			expect(await playbackState(api)).toMatchObject({
				active: recordMinusRuntime.active,
			});
			expect(logicalSlots(await bench.tick(0), 12)).toEqual(recordMinusFrame);
		} finally {
			await closeWebSocket(stream.socket, "CUE-001 event stream");
		}

		await api.request("POST", "/api/v1/cuelists/1/go", {});
		expect(logicalSlots(await bench.tick(0), 12)).toEqual([
			...Array(4).fill(255),
			...Array(8).fill(0),
		]);
		await api.request("POST", "/api/v1/cuelists/1/go", {});
		expect(logicalSlots(await bench.tick(0), 12)).toEqual([
			...Array(4).fill(255),
			...Array(4).fill(0),
			...Array(4).fill(255),
		]);
		state.completed = true;
	},
	ui: cue001Ui,
	assert: async (_context, state) => expect(state.completed).toBe(true),
});
