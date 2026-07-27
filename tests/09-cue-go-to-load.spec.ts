import { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { pairedScenario } from "./bench/core/pairedScenario";
import {
	fixtureIdsByNumber,
	loadCanonicalCopy,
	object,
	putObject,
} from "./support/catalog";

test.describe("docs/testing/02-cues-tracking-and-arbitration.md", () => {
	pairedScenario<{ completed: boolean }>({
		id: "CUE-014",
		title:
			"Cue Go To and Load preserve desk-local selection and authoritative output controls",
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
			await api.playbackNumberAction(2, "select", {});
			await api.executeCommandLine("CUE SET 1 CUE 3");
			await api.executeCommandLine("CUE CUE SET 1 . 2 CUE 2");
			expect(await runtime(api, 1)).toMatchObject({
				current_cue_number: 3,
				master: 1,
				enabled: true,
			});
			expect(await runtime(api, 2)).toMatchObject({
				enabled: false,
				loaded_cue_number: 2,
				effective_next_is_loaded: true,
			});

			const before = await playbackState(api);
			await expect(api.executeCommandLine("CUE 99")).rejects.toThrow(
				/cue does not exist/i,
			);
			await expect(api.executeCommandLine("CUE SET 99 CUE 1")).rejects.toThrow(
				/playback 99 does not exist/i,
			);
			await expect(
				api.cueListPlaybackAction(cueListId, "go", {}),
			).rejects.toThrow(/multiple playbacks/i);
			expect(await playbackState(api)).toMatchObject({
				selected_playback: before.selected_playback,
				active: before.active,
			});

			const otherDesk = new ApiDriver(api.baseUrl);
			await otherDesk.login("Operator");
			expect((await playbackState(otherDesk)).selected_playback).toBeNull();
			await expect(otherDesk.executeCommandLine("CUE 2")).rejects.toThrow(
				/no playback is selected/i,
			);
			await otherDesk.playbackNumberAction(1, "select", {});
			expect((await playbackState(otherDesk)).selected_playback).toBe(1);
			expect((await playbackState(api)).selected_playback).toBe(2);

			const sameDesk = new ApiDriver(api.baseUrl);
			sameDesk.session = await sameDesk.request(
				"POST",
				"/api/v2/sessions",
				{ username: "Operator", desk_id: api.session!.desk.id },
				false,
			);
			expect((await playbackState(sameDesk)).selected_playback).toBe(2);

			await api.request(
				"POST",
				"/api/v2/output-runtime/global-master/actions",
				{ grand_master: 0.5, blackout: false },
			);
			await api.executeCommandLine("CUE SET 1 CUE 3");
			const masterFrame = await bench.tick(3_000);
			const visualization = await api.request<any>(
				"GET",
				"/api/v2/output/visualization",
			);
			expect(visualization).toMatchObject({
				grand_master: 0.5,
				blackout: false,
			});
			expect(
				visualization.values.some(
					(item: any) =>
						item.attribute === "intensity" &&
						Math.abs(item.value?.value - 0.8) < 0.001,
				),
			).toBe(true);
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

			await api.executeCommandLine("CUE CUE SET 1 CUE 2");
			expect((await runtime(api, 1)).loaded_cue_number).toBe(2);
			await api.executeCommandLine("CUE CUE SET 1 CUE 1");
			expect(await runtime(api, 1)).toMatchObject({
				loaded_cue_number: 1,
				effective_next_is_loaded: true,
			});
			await api.executeCommandLine("CUE CUE SET 1 CUE 2");

			const beforeRenumber = await object<any>(api, "cue_list", cueListId);
			const loadedCueId = beforeRenumber.body.cues.find(
				(cue: any) => cue.number === 2,
			).id;
			await putObject(
				api,
				"cue_list",
				cueListId,
				{
					...beforeRenumber.body,
					cues: beforeRenumber.body.cues.map((cue: any) => ({
						...cue,
						number: cue.number * 10,
					})),
				},
				beforeRenumber.revision,
			);
			await expect
				.poll(async () => runtime(api, 1))
				.toMatchObject({
					loaded_cue_id: loadedCueId,
					loaded_cue_number: 20,
					effective_next_cue_number: 20,
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
				.poll(async () => runtime(api, 1))
				.toMatchObject({ effective_next_is_loaded: false });
			expect((await runtime(api, 1)).loaded_cue_id).toBeUndefined();
			expect((await runtime(api, 1)).loaded_cue_number).toBeUndefined();

			await api.executeCommandLine("CUE CUE SET 1 CUE 10");
			expect((await runtime(api, 1)).effective_next_is_loaded).toBe(true);
			await api.openShow(show.id, { transition: "hold_current" });
			expect((await playbackState(api)).active).toHaveLength(0);
			state.completed = true;
		},
		assert: async (_context, state) => expect(state.completed).toBe(true),
	});

	test("CUE-014 @osc › OSC selection, Cue keys, and loaded-next feedback use the same desk runtime", async ({
		api,
		bench,
	}) => {
		await loadCanonicalCopy(api, bench, "cue-go-to-load-osc", "compact-rig");
		await installTwinPlaybacks(api);
		const hardware = await bench.osc();
		const alias = api.session!.desk.osc_alias;
		try {
			await hardware.subscribe(`cue-load-${crypto.randomUUID()}`, alias);
			await hardware.send(`/light/${alias}/page-playback/2/select`, [true]);
			await expect
				.poll(async () => (await playbackState(api)).selected_playback)
				.toBe(2);
			for (const key of ["cue", "digit-3", "enter"])
				await hardware.send(`/light/${alias}/programmer/${key}`, [true]);
			await expect
				.poll(async () => runtime(api, 2))
				.toMatchObject({ current_cue_number: 3, enabled: true });
			for (const key of ["cue", "cue", "digit-2", "enter"])
				await hardware.send(`/light/${alias}/programmer/${key}`, [true]);
			await expect
				.poll(async () => runtime(api, 2))
				.toMatchObject({
					current_cue_number: 3,
					effective_next_cue_number: 2,
					effective_next_is_loaded: true,
				});
			const mark = hardware.mark();
			await bench.tick(0);
			await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/page-playback/2/effective-next-cue`,
			);
			await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/page-playback/2/loaded-next`,
			);
			await hardware.expectAfter(
				mark,
				`/light/${alias}/feedback/page-playback/2/selected`,
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
		await putObject(api, "playback", String(number), {
			number,
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
	await putObject(api, "playback_page", "1", {
		number: 1,
		name: "Main",
		slots: { "1": 1, "2": 2 },
	});
	return cueListId;
}

function cue(number: number, id: string, fixture: string, level: number) {
	return {
		id,
		number,
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

async function runtime(api: ApiDriver, playback: number): Promise<any> {
	return (await playbackState(api)).active.find(
		(item: any) => item.playback_number === playback,
	);
}
