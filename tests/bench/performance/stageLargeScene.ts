import type { PatchFixturesOutcome } from "../../../apps/light-desktop/src/api/generated/light-wire";
import { createLargeStageDynamicsPlan } from "../../../tools/stage-dynamics-scene.mjs";
import {
	countFixtureInstances,
	createDeterministicLargeStageInputs,
	LARGE_STAGE_DYNAMIC_INSTANCES,
	LARGE_STAGE_FIXTURE_INSTANCES,
	LARGE_STAGE_FIXTURE_RECORDS,
} from "../../../tools/stage-large-scene.mjs";
import { readPatchSnapshot } from "../../support/operator/patch";
import type { ApiDriver } from "../core/api";

export {
	countFixtureInstances,
	LARGE_STAGE_FIXTURE_INSTANCES,
	LARGE_STAGE_FIXTURE_RECORDS,
};

export interface LargeStageScene {
	fixtureRecords: number;
	fixtureInstances: number;
	addedFixtureRecords: number;
	addedMultipatchInstances: number;
	dynamicInstances: number;
	dynamicTargets: number;
	staticControlInstances: number;
	occupiedSlots: number;
	universes: number;
}

/**
 * Replaces Default Stage with the deterministic 1,000-instance capacity rig.
 */
export async function installDeterministicLargeStage(
	api: ApiDriver,
): Promise<LargeStageScene> {
	const before = await readPatchSnapshot(api);
	const fixtureLibrary = await api.fixtureProfilesSnapshot();
	const largeScene = createDeterministicLargeStageInputs(
		before.fixtures,
		fixtureLibrary.profiles as Parameters<
			typeof createDeterministicLargeStageInputs
		>[1],
		before.fixtures[0]?.layer_id ?? "default",
	);
	await api.request<PatchFixturesOutcome>(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: largeScene.fixtures,
			remove_fixture_ids: before.fixtures.map((fixture) => fixture.fixture_id),
			placements: [],
		},
		true,
		before.patch_revision,
		{ showId: before.show_id },
	);

	const after = await readPatchSnapshot(api, before.show_id);
	const fixtureInstances = countFixtureInstances(after.fixtures);
	if (after.fixtures.length !== LARGE_STAGE_FIXTURE_RECORDS)
		throw new Error(
			`Large Stage has ${after.fixtures.length} fixture records; expected ${LARGE_STAGE_FIXTURE_RECORDS}`,
		);
	if (fixtureInstances !== LARGE_STAGE_FIXTURE_INSTANCES)
		throw new Error(
			`Large Stage has ${fixtureInstances} instances; expected ${LARGE_STAGE_FIXTURE_INSTANCES}`,
		);
	const plan = createLargeStageDynamicsPlan(after, largeScene);
	for (const definition of plan.definitions) {
		const created = await api.request<{ object: { id: string } }>(
			"POST",
			"/api/v2/dynamics/create",
			{ request_id: crypto.randomUUID(), definition },
			true,
			undefined,
			{ showId: before.show_id },
		);
		await api.request(
			"POST",
			`/api/v2/dynamics/${encodeURIComponent(created.object.id)}/start`,
			{
				targets: [],
				overrides: {
					size: 1,
					speed_multiplier: { numerator: 1, denominator: 1 },
					phase_offset_degrees: 0,
				},
				timing: {},
				undo_group: "stage-capacity-dynamics",
			},
			true,
			undefined,
			{ showId: before.show_id },
		);
	}
	await setStaticControls(api, before.show_id, plan.staticControlFixtureIds);
	const runtime = await api.request<{ instances: unknown[] }>(
		"GET",
		"/api/v2/dynamics/runtime",
		undefined,
		true,
		undefined,
		{ showId: before.show_id },
	);
	if (runtime.instances.length !== LARGE_STAGE_DYNAMIC_INSTANCES)
		throw new Error(
			`Large Stage has ${runtime.instances.length} active Dynamics; expected ${LARGE_STAGE_DYNAMIC_INSTANCES}`,
		);

	return {
		fixtureRecords: after.fixtures.length,
		fixtureInstances,
		addedFixtureRecords: largeScene.addedFixtureRecords,
		addedMultipatchInstances: largeScene.addedMultipatchInstances,
		dynamicInstances: runtime.instances.length,
		dynamicTargets: plan.dynamicTargetCount,
		staticControlInstances:
			plan.staticControlFixtureIds.length + largeScene.addedMultipatchInstances,
		occupiedSlots: largeScene.patch.occupiedSlots,
		universes: largeScene.patch.universeCount,
	};
}

async function setStaticControls(
	api: ApiDriver,
	showId: string,
	fixtureIds: string[],
): Promise<void> {
	if (!api.session) throw new Error("Large Stage API session is unavailable");
	const userId = api.session.user.id;
	const [values, capture] = await Promise.all([
		api.request<{ projection: { revision: number } }>(
			"GET",
			`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/snapshot`,
			undefined,
			true,
			undefined,
			{ showId },
		),
		api.request<{ projection: { revision: number } }>(
			"GET",
			`/api/v2/users/${encodeURIComponent(userId)}/programmer-capture-mode/snapshot`,
			undefined,
			true,
			undefined,
			{ showId },
		),
	]);
	await api.request(
		"POST",
		`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/actions`,
		{
			request_id: crypto.randomUUID(),
			expected_revision: values.projection.revision,
			expected_capture_mode_revision: capture.projection.revision,
			action: {
				type: "batch",
				mutations: fixtureIds.map((fixtureId) => ({
					type: "set_fixture",
					fixture_id: fixtureId,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.35 },
					timing: {
						fade: false,
						fade_millis: null,
						delay_millis: null,
					},
				})),
			},
		},
		true,
		undefined,
		{ showId },
	);
}
