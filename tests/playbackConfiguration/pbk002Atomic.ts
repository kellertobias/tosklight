import type { ApiDriver } from "../../apps/control-ui/e2e/bench/api";
import {
	type BenchUiContext,
	expect,
} from "../../apps/control-ui/e2e/bench/fixtures";
import {
	clearSlot,
	definition,
	object,
	objects,
	pageObject,
	playbackAt,
	playbackSnapshot,
	poolAction,
	prepareShow,
	putObject,
	saveSlot,
	writePage,
} from "./helpers";
import type { PlaybackTarget, PreparedShow } from "./models";

interface FunctionAssignment {
	slot: number;
	target: PlaybackTarget;
	buttons: [string, string, string];
	count: number;
	fader: string;
	hasFader: boolean;
}

function functionAssignments(cueListId: string): FunctionAssignment[] {
	return [
		{
			slot: 1,
			target: { type: "cue_list", cue_list_id: cueListId },
			buttons: ["go_minus", "go", "flash"],
			count: 1,
			fader: "master",
			hasFader: false,
		},
		{
			slot: 2,
			target: { type: "group", group_id: "1" },
			buttons: ["select", "select_dereferenced", "flash"],
			count: 2,
			fader: "master",
			hasFader: true,
		},
		...["A", "B", "C", "D", "E"].map((group, index) => ({
			slot: index + 3,
			target: { type: "speed_group", group } as PlaybackTarget,
			buttons: ["double", "half", "learn"] as [string, string, string],
			count: 3,
			fader: "learned_percentage",
			hasFader: true,
		})),
		{
			slot: 8,
			target: { type: "programmer_fade" },
			buttons: ["double", "half", "off"],
			count: 0,
			fader: "master",
			hasFader: true,
		},
		{
			slot: 9,
			target: { type: "cue_fade" },
			buttons: ["double", "half", "off"],
			count: 3,
			fader: "master",
			hasFader: true,
		},
		{
			slot: 10,
			target: { type: "grand_master" },
			buttons: ["blackout", "pause_dynamics", "flash"],
			count: 3,
			fader: "master",
			hasFader: true,
		},
	];
}

async function installFunctionAssignments(
	api: ApiDriver,
	assignments: FunctionAssignment[],
): Promise<void> {
	for (const assignment of assignments) {
		const buttons = assignment.buttons.map((action, index) =>
			index < assignment.count ? action : "none",
		);
		const result = await saveSlot(
			api,
			1,
			assignment.slot,
			definition(0, `Function ${assignment.slot}`, assignment.target, {
				buttons: buttons as [string, string, string],
				button_count: assignment.count,
				fader: assignment.fader,
				has_fader: assignment.hasFader,
				color: "#8b5cf6",
			}),
		);
		expect(result.playback).toMatchObject({
			target: assignment.target,
			buttons,
			button_count: assignment.count,
			fader: assignment.fader,
			has_fader: assignment.hasFader,
			color: "#8b5cf6",
		});
	}
}

async function verifyAtomicRejections(api: ApiDriver): Promise<void> {
	const first = await playbackAt(api, 1, 1);
	const firstPage = await pageObject(api, 1);
	const headers = {
		authorization: `Bearer ${api.session!.token}`,
		"content-type": "application/json",
	};
	const staleResponse = await fetch(
		`${api.baseUrl}/api/v1/playback-pages/1/slots/1`,
		{
			method: "PUT",
			headers,
			body: JSON.stringify({
				playback: { ...first.body, name: "Must not land" },
				expected_playback_revision: first.revision - 1,
				expected_page_revision: firstPage.revision - 1,
			}),
		},
	);
	expect(staleResponse.status).toBe(409);
	expect((await playbackAt(api, 1, 1)).body.name).toBe("Function 1");
	expect((await pageObject(api, 1)).body).toEqual(firstPage.body);

	const invalidResponse = await fetch(
		`${api.baseUrl}/api/v1/playback-pages/1/slots/1`,
		{
			method: "PUT",
			headers,
			body: JSON.stringify({
				playback: {
					...first.body,
					target: { type: "group", group_id: "1" },
					buttons: ["go", "go_minus", "flash"],
				},
				expected_playback_revision: first.revision,
				expected_page_revision: firstPage.revision,
			}),
		},
	);
	expect(invalidResponse.status).toBe(400);
	expect((await playbackAt(api, 1, 1)).body).toEqual(first.body);
}

async function verifyLegacyMigration(
	api: ApiDriver,
	prepared: PreparedShow,
	assignments: FunctionAssignment[],
): Promise<void> {
	await putObject(api, "playback", "700", {
		number: 700,
		name: "Legacy Playback",
		target: { type: "cue_list", cue_list_id: prepared.cueListId },
		buttons: ["go_minus", "go", "flash"],
		fader: "master",
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
	});
	const pageWithLegacy = await pageObject(api, 1);
	await putObject(
		api,
		"playback_page",
		"1",
		{
			...pageWithLegacy.body,
			slots: { ...pageWithLegacy.body.slots, "11": 700 },
		},
		pageWithLegacy.revision,
	);
	const migrated = (await playbackSnapshot(api)).pool.find(
		(item: any) => item.number === 700,
	);
	expect(migrated).toMatchObject({
		button_count: 3,
		has_fader: true,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	});
	await saveSlot(api, 1, 11, migrated);
	expect((await playbackAt(api, 1, 11)).body).toMatchObject({
		button_count: 3,
		has_fader: true,
		color: "#20c997",
	});

	await api.request("POST", `/api/v1/shows/${prepared.showId}/open`, {
		transition: "hold_current",
	});
	for (const assignment of assignments) {
		expect((await playbackAt(api, 1, assignment.slot)).body).toMatchObject({
			target: assignment.target,
			color: "#8b5cf6",
		});
	}
}

async function verifyClearPreservesSource(
	api: ApiDriver,
	prepared: PreparedShow,
): Promise<void> {
	const sourceBefore = await object<any>(api, "cue_list", prepared.cueListId);
	const assigned = await playbackAt(api, 1, 1);
	await poolAction(api, assigned.body.number, "on");
	await clearSlot(api, 1, 1);
	expect((await pageObject(api, 1)).body.slots["1"]).toBeUndefined();
	expect(
		(await objects(api, "playback")).some(
			(item) => item.id === String(assigned.body.number),
		),
	).toBe(false);
	expect(
		(await playbackSnapshot(api)).active.some(
			(item: any) => item.playback_number === assigned.body.number,
		),
	).toBe(false);
	expect(await object<any>(api, "cue_list", prepared.cueListId)).toEqual(
		sourceBefore,
	);
}

export async function runPbk002AtomicConfigurationScenario({
	api,
	bench,
}: BenchUiContext): Promise<void> {
	const prepared = await prepareShow(
		api,
		bench,
		"pbk-002-functions",
		"default-stage",
	);
	await writePage(api, 1, {});
	const assignments = functionAssignments(prepared.cueListId);
	await installFunctionAssignments(api, assignments);
	await verifyAtomicRejections(api);
	await verifyLegacyMigration(api, prepared, assignments);
	await verifyClearPreservesSource(api, prepared);
}
