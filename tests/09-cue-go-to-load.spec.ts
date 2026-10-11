import { assignPlayback, runtime as addressedRuntime } from "./cueSemanticContracts/playbackFixtures";
import type { PlaybackAddress } from "../apps/light-desktop/src/features/playbackTopology/contracts";
import { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import type { PairedScenario } from "./bench/core/pairedScenario";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	putObject,
} from "./support/catalog";

test.describe("docs/testing/02-cues-tracking-and-arbitration.md", () => {
	const cue014Contract: Omit<PairedScenario<{completed:boolean}>,"ui"> = {
		id: "CUE-014",
		title:
			"Cue Go To and Load preserve shared desk selection and authoritative output controls",
		surfaces: ["api"],
		arrange: () => ({ completed: false }),
		api: async ({ api, bench }, state) => {
			const show = await loadCanonicalCopy(
				api,
				bench,
				"cue-go-to-load-wire",
				"compact-rig",
			);
			const cueListId = await installTwinPlaybacks(api);
			await api.playbackAction({ page: 1, position: 2 }, "select", {});
			const inactiveFrame = await bench.tick(0);
			expect(
				inactiveFrame.universes.find((item: any) => item.universe === 1)?.slots[0],
			).toBe(0);
			await api.executeCommandLine("LOAD PBK 1 . 2 CUE 2");
			expect(await runtime(api, { page: 1, position: 2 })).toMatchObject({
				enabled: false,
				loaded_cue_number: "2",
				effective_next_is_loaded: true,
			});
			expect((await bench.tick(3_000)).universes).toEqual(inactiveFrame.universes);
			await api.executeCommandLine("GO TO PBK 1 CUE 3");
			const beforeLoad = await runtime(api, { page: 1, position: 2 });
			await api.executeCommandLine("LOAD PBK 1 . 2 CUE 2");
			expect(await runtime(api,{page:1,position:1})).toMatchObject({
				current_cue_number: "3",
				master: 1,
				enabled: true,
			});
			expect(await runtime(api,{page:1,position:2})).toMatchObject({
				enabled: beforeLoad.enabled,
				current_cue_id: beforeLoad.current_cue_id,
				current_cue_number: "3",
				master: beforeLoad.master,
				loaded_cue_number: "2",
				effective_next_is_loaded: true,
			});

			const before = await playbackState(api);
			await expect(api.executeCommandLine("GO TO PBK 2 CUE 99")).rejects.toThrow(
				/cue does not exist/i,
			);
			await expect(api.executeCommandLine("GO TO PBK 99 CUE 1")).rejects.toThrow(
				/playback is not assigned/i,
			);
			expect(await playbackState(api)).toMatchObject({
				selected_playback: before.selected_playback,
				active: before.active,
			});
			await api.cueListPlaybackAction(cueListId, "go", {});
			for (const position of [1, 2]) {
				expect(await runtime(api, { page: 1, position })).toMatchObject({
					current_cue_number: "2",
					loaded_cue_number: null,
					effective_next_is_loaded: false,
					enabled: true,
				});
			}
			expect((await playbackState(api)).selected_playback).toEqual(
				before.selected_playback,
			);

			const otherWindow = new ApiDriver(api.baseUrl);
			otherWindow.session = await otherWindow.request(
				"POST",
				"/api/v2/sessions",
				{ client_id: crypto.randomUUID() },
				false,
			);
			expect(otherWindow.session!.desk.id).toBe(api.session!.desk.id);
			expect((await playbackState(otherWindow)).selected_playback).toEqual({
				page: 1,
				position: 2,
			});
			await otherWindow.executeCommandLine("PBK 1");
			expect((await playbackState(api)).selected_playback).toEqual({
				page: 1,
				position: 1,
			});
			expect(await runtime(otherWindow, { page: 1, position: 2 })).toMatchObject({
				current_cue_number: "2",
				enabled: true,
			});
			await api.playbackAction({ page: 1, position: 2 }, "select", {});

			const sameWindow = new ApiDriver(api.baseUrl);
			sameWindow.session = await sameWindow.request(
				"POST",
				"/api/v2/sessions",
				{ client_id: api.session!.client_id },
				false,
			);
			expect((await playbackState(sameWindow)).selected_playback).toEqual({
				page: 1,
				position: 2,
			});

			await api.request(
				"POST",
				"/api/v2/output-runtime/global-master/actions",
				{ grand_master: 0.5, blackout: false },
			);
			await api.executeCommandLine("GO TO PBK 1 CUE 3");
			const masterFrame = await bench.tick(3_000);
			const visualization = await api.request<any>(
				"GET",
				"/api/v2/output/visualization",
			);
			expect(visualization).toMatchObject({
				grand_master: 0.5,
				blackout: false,
			});
			const fixtureId = (await fixtureIdsByNumber(api))[1];
			const storedCue = (await object<any>(api, "cue_list", cueListId)).body.cues.find(
				(item: any) => item.number === "3",
			);
			const storedIntensity = storedCue.changes.find(
				(item: any) => item.fixture_id === fixtureId,
			).value;
			expect(storedIntensity.kind).toBe("normalized");
			expect(storedIntensity.value).toBeCloseTo(0.8, 6);
			const visibleIntensity = visualization.values.find(
				(item: any) => item.fixture_id === fixtureId && item.attribute === "intensity",
			);
			expect(visibleIntensity).toBeDefined();
			expect(visibleIntensity.value.value).toBeCloseTo(0.4, 3);
			expect(
				masterFrame.universes.find((universe: any) => universe.universe === 1)
					?.slots[0],
			).toBe(102);
			await api.request(
				"POST",
				"/api/v2/output-runtime/global-master/actions",
				{ blackout: true },
			);
			expect(
				(await bench.tick(0)).universes.find(
					(universe: any) => universe.universe === 1,
				)?.slots[0],
			).toBe(0);
			await api.request(
				"POST",
				"/api/v2/output-runtime/global-master/actions",
				{ grand_master: 1, blackout: false },
			);

			await api.executeCommandLine("LOAD PBK 1 CUE 2");
			expect((await runtime(api,{page:1,position:1})).loaded_cue_number).toBe("2");
			await api.executeCommandLine("LOAD PBK 1 CUE 1");
			expect(await runtime(api,{page:1,position:1})).toMatchObject({
				loaded_cue_number: "1",
				effective_next_is_loaded: true,
			});
			await api.executeCommandLine("LOAD PBK 1 CUE 2");

			const beforeRenumber = await object<any>(api, "cue_list", cueListId);
			const loadedCueId = beforeRenumber.body.cues.find(
				(cue: any) => cue.number === "2",
			).id;
			await putObject(
				api,
				"cue_list",
				cueListId,
				{
					...beforeRenumber.body,
					cues: beforeRenumber.body.cues.map((cue: any) => ({
						...cue,
						number: String(Number(cue.number) * 10),
					})),
				},
				beforeRenumber.revision,
			);
			await expect
				.poll(async () => runtime(api,{page:1,position:1}))
				.toMatchObject({
					loaded_cue_id: loadedCueId,
					loaded_cue_number: "20",
					effective_next_cue_number: "20",
					effective_next_is_loaded: true,
				});

			const beforeDelete = await object<any>(api, "cue_list", cueListId);
			await putObject(
				api,
				"cue_list",
				cueListId,
				{
					...beforeDelete.body,
					cues: beforeDelete.body.cues.filter(
						(cue: any) => cue.id !== loadedCueId,
					),
				},
				beforeDelete.revision,
			);
			await expect
				.poll(async () => runtime(api,{page:1,position:1}))
				.toMatchObject({ effective_next_is_loaded: false });
			expect((await runtime(api,{page:1,position:1})).loaded_cue_id).toBeNull();
			expect((await runtime(api,{page:1,position:1})).loaded_cue_number).toBeNull();

			await api.executeCommandLine("LOAD PBK 1 CUE 10");
			expect((await runtime(api,{page:1,position:1})).effective_next_is_loaded).toBe(true);
			await api.openShow(show.id, { transition: "hold_current" });
			expect((await playbackState(api)).active).toHaveLength(0);
			state.completed = true;
		},
		assert: async (_context, state) => expect(state.completed).toBe(true),
	};
 test("CUE-014 @api › Go To and Load preserve shared desk selection and output authority",async({api,bench,show,request,desk,page})=>{const context={api,bench,show,request,desk,page};const state=await cue014Contract.arrange(context,"api"); await cue014Contract.api(context,state); await cue014Contract.assert(context,state,"api");});

	test("CUE-014 @osc › OSC selection, Cue keys, and loaded-next feedback use the same desk runtime", async ({
		api,
		bench,
	}) => {
		await loadCanonicalCopy(api, bench, "cue-go-to-load-osc", "compact-rig");
		await installTwinPlaybacks(api);
 const assignmentPage=await object<any>(api,"playback_page","1");
 assignmentPage.body.assignments[2].buttons[2]="select";
 await putObject(api,"playback_page","1",assignmentPage.body,assignmentPage.revision);
		const hardware = await bench.osc();
		const alias = "desk";
		try {
			await hardware.subscribe(`cue-load-${crypto.randomUUID()}`, alias);
			await hardware.send(`/light/${alias}/playback/1/2/button/3`, [true]);
			await expect
				.poll(async () => (await playbackState(api)).selected_playback)
				.toEqual({page:1,position:2});
			await hardware.send(`/light/${alias}/programmer/shift`, [true]);
			await hardware.send(`/light/${alias}/programmer/div`, [true]);
			await hardware.send(`/light/${alias}/programmer/shift`, [false]);
			for (const key of ["playback", "digit-2", "cue", "digit-3", "enter"])
				await hardware.send(`/light/${alias}/programmer/${key}`, [true]);
			await expect
				.poll(async () => runtime(api,{page:1,position:2}))
				.toMatchObject({ current_cue_number: "3", enabled: true });
			await hardware.send(`/light/${alias}/programmer/shift`, [true]);
			await hardware.send(`/light/${alias}/programmer/div`, [true]);
			await hardware.send(`/light/${alias}/programmer/div`, [true]);
			await hardware.send(`/light/${alias}/programmer/shift`, [false]);
			for (const key of ["playback", "digit-2", "cue", "digit-2", "enter"])
				await hardware.send(`/light/${alias}/programmer/${key}`, [true]);
			await expect
				.poll(async () => runtime(api,{page:1,position:2}))
				.toMatchObject({
					current_cue_number: "3",
					effective_next_cue_number: "2",
					effective_next_is_loaded: true,
				});
			const mark = hardware.mark();
			await bench.tick(0);
			await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/playback/1/2/effective-next-cue`,
			);
			await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/playback/1/2/loaded-next`,
			);
			await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/playback/1/2/button/3ed`,
			);
		} finally {
			await hardware.close();
		}
	});
});

async function installTwinPlaybacks(api: ApiDriver): Promise<string> {
	const fixture = (await fixtureIdsByNumber(api))[1];
	const cueListId = crypto.randomUUID();
	const cues = [
		cue(1, crypto.randomUUID(), fixture, 0.2),
		cue(2, crypto.randomUUID(), fixture, 0.5),
		cue(3, crypto.randomUUID(), fixture, 0.8),
	];
	await putObject(api, "cue_list", cueListId, {
		id: cueListId,
		name: "Twin Cuelist",
		priority: 0,
		mode: "sequence",
		looped: false,
		wrap_mode: "off",
		restart_mode: "first_cue",
		cues,
	});
	for (const [number, name] of [
		[1, "Twin A"],
		[2, "Twin B"],
	] as const) {
		await assignPlayback(api,{page:1,position:number}, {
			name,
			target: { type: "cue_list", cue_list_id: cueListId },
			buttons: ["go_minus", "go", "flash"],
			fader: "master",
			go_activates: true,
			auto_off: false,
			xfade_millis: 0,
			color: "#20c997",
			flash_release: "release_all",
			protect_from_swap: false,
		});
	}

	return cueListId;
}

function cue(number: number, id: string, fixture: string, level: number) {
	return {
		id,
		number: String(number),
		name: `Cue ${number}`,
		fade_millis: 3_000,
		delay_millis: 0,
		trigger: { type: "manual" },
		group_changes: [],
		changes: [
			{
				fixture_id: fixture,
				attribute: "intensity",
				value: { kind: "normalized", value: level },
				automatic_restore: false,
			},
		],
	};
}

async function playbackState(api: ApiDriver): Promise<any> {
	return api.request("GET", "/api/v2/playback-overview");
}

async function runtime(api:ApiDriver,address:PlaybackAddress):Promise<any> { return addressedRuntime(api,address); }
