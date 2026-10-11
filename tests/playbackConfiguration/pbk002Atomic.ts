import { decodePlaybackBody } from "../../apps/light-desktop/src/api/showObjectPlaybackWire";
import type { LightBench } from "../bench/core/lightBench";
import { assertLegacyPlaybackRefusalAndRecovery } from "../support/playbackCompatibility";
import { runSql } from "../05-virtual-time-persistence-and-recovery.show-helpers";
import type { ApiDriver } from "../bench/core/api";
import {
	type BenchUiContext,
	expect,
} from "../bench/core/fixtures";
import {
	playbackSlotAuthority,
	postPlaybackTopologyAction,
} from "../support/playbackTopology";
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
import type {
	PlaybackTarget,
	PreparedShow,
} from "../bench/playbacks/playback-configuration/models";

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
			definition(`Function ${assignment.slot}`, assignment.target, {
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
	const firstDefinition = decodePlaybackBody(first.body, "playback_page.1.assignments.1");
	const firstPage = await pageObject(api, 1);
	const authority = await playbackSlotAuthority(api, 1, 1);
	const staleResponse = await postPlaybackTopologyAction(
		api,
		authority.showId,
		authority.showRevision,
		{
			type: "update_playback",
			address: { page: 1, position: 1 },
			definition: { ...firstDefinition, name: "Must not land" },
			expectedPageRevision: firstPage.revision - 1,
			expectedPageObjectId: firstPage.id,
		},
	);
	expect(staleResponse.status).toBe(409);
	expect((await playbackAt(api, 1, 1)).body.name).toBe("Function 1");
	expect((await pageObject(api, 1)).body).toEqual(firstPage.body);

	const invalidResponse = await postPlaybackTopologyAction(
		api,
		authority.showId,
		authority.showRevision,
		{
			type: "update_playback",
			address: { page: 1, position: 1 },
			definition: {
				...firstDefinition,
				target: { type: "group", group_id: "1" },
				buttons: ["go", "go_minus", "flash"],
			},
			expectedPageRevision: firstPage.revision,
			expectedPageObjectId: firstPage.id,
		},
	);
	expect(invalidResponse.status).toBe(400);
	expect((await playbackAt(api, 1, 1)).body).toEqual(first.body);
}

async function verifyLegacyRefusalAndRecovery(
	api: ApiDriver,
	bench: LightBench,
	prepared: PreparedShow,
	assignments: FunctionAssignment[],
): Promise<void> {
	const pageBefore = await pageObject(api, 1);
	const cueBefore = await object<any>(api, "cue_list", prepared.cueListId);
	await assertLegacyPlaybackRefusalAndRecovery(
		api,
		bench,
		prepared.showId,
		(file) =>
			runSql(
				file,
				"UPDATE objects SET body_json=json_set(body_json, '$.slots', json('{\"11\":700}')) WHERE kind='playback_page' AND id='1'",
			),
	);
	expect((await pageObject(api, 1)).body).toEqual(pageBefore.body);
	expect(await object<any>(api, "cue_list", prepared.cueListId)).toEqual(
		cueBefore,
	);
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
	const before = await pageObject(api, 1);
	await poolAction(api, 1, "on");
	await clearSlot(api, 1, 1);
	const remaining = { ...before.body.assignments };
	delete remaining["1"];
	expect((await pageObject(api, 1)).body.assignments).toEqual(remaining);
	expect((await playbackSnapshot(api)).active.some((item: any) => item.playback_address?.page === 1 && item.playback_address.position === 1)).toBe(false);
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
	await verifyLegacyRefusalAndRecovery(api, bench, prepared, assignments);
	await verifyClearPreservesSource(api, prepared);
}
