import type {
	PatchFixtureInput,
	PatchFixtureProjection,
	PatchFixturesOutcome,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import { readPatchSnapshot } from "../../support/operator/patch";
import type { ApiDriver } from "../core/api";

export const LARGE_STAGE_FIXTURE_RECORDS = 470;
export const LARGE_STAGE_FIXTURE_INSTANCES = 500;

export interface LargeStageScene {
	fixtureRecords: number;
	fixtureInstances: number;
	addedFixtureRecords: number;
	addedMultipatchInstances: number;
}

/**
 * Extends Default Stage with deterministic, unpatched fixture copies.
 *
 * The original profiles remain representative and the added instances affect
 * Stage projection/rendering without adding output traffic or consuming DMX
 * addresses.
 */
export async function installDeterministicLargeStage(
	api: ApiDriver,
): Promise<LargeStageScene> {
	const before = await readPatchSnapshot(api);
	if (before.fixtures.length > LARGE_STAGE_FIXTURE_RECORDS) {
		throw new Error(
			`Large Stage seed already has ${before.fixtures.length} fixture records; expected at most ${LARGE_STAGE_FIXTURE_RECORDS}`,
		);
	}
	if (before.fixtures.length === 0)
		throw new Error("Large Stage requires at least one source fixture");

	const addedFixtureRecords =
		LARGE_STAGE_FIXTURE_RECORDS - before.fixtures.length;
	const existingInstances = countFixtureInstances(before.fixtures);
	const addedMultipatchInstances =
		LARGE_STAGE_FIXTURE_INSTANCES - existingInstances - addedFixtureRecords;
	if (
		addedMultipatchInstances < 0 ||
		addedMultipatchInstances > addedFixtureRecords
	) {
		throw new Error(
			`Cannot reach ${LARGE_STAGE_FIXTURE_INSTANCES} instances from ${before.fixtures.length} records and ${existingInstances} existing instances`,
		);
	}

	if (addedFixtureRecords > 0) {
		const fixtures = Array.from({ length: addedFixtureRecords }, (_, index) =>
			largeFixtureInput(
				before.fixtures[index % before.fixtures.length],
				index,
				index < addedMultipatchInstances,
			),
		);
		await api.request<PatchFixturesOutcome>(
			"POST",
			"/api/v2/patch/fixtures",
			{
				request_id: deterministicUuid("3", 1),
				fixtures,
				remove_fixture_ids: [],
				placements: [],
			},
			true,
			before.patch_revision,
			{ showId: before.show_id },
		);
	}

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

	return {
		fixtureRecords: after.fixtures.length,
		fixtureInstances,
		addedFixtureRecords,
		addedMultipatchInstances,
	};
}

export function countFixtureInstances(
	fixtures: readonly PatchFixtureProjection[],
): number {
	return fixtures.reduce(
		(total, fixture) => total + 1 + fixture.multipatch.length,
		0,
	);
}

function largeFixtureInput(
	source: PatchFixtureProjection,
	index: number,
	withMultipatch: boolean,
): PatchFixtureInput {
	const column = index % 24;
	const row = Math.floor(index / 24);
	const location = {
		x: (column * 2 - 23) * 375,
		y: 2_000 + (row % 4) * 600,
		z: -Math.floor(row / 4) * 850,
	};
	const fixtureId = deterministicUuid("1", index + 1);
	return {
		fixture_id: fixtureId,
		fixture_number: 10_000 + index,
		virtual_fixture_number: null,
		name: `Stage baseline ${String(index + 1).padStart(3, "0")} · ${source.name}`,
		profile_id: source.profile_id,
		profile_revision: source.profile_revision,
		mode_id: source.mode_id,
		split_patches: clearAssignments(source.split_patches),
		layer_id: source.layer_id,
		direct_control: null,
		location,
		rotation: { ...source.rotation },
		multipatch: withMultipatch
			? [
					{
						id: deterministicUuid("2", index + 1),
						name: `Stage baseline multipatch ${String(index + 1).padStart(2, "0")}`,
						split_patches: clearAssignments(source.split_patches),
						location: { ...location, x: location.x + 300 },
						rotation: { ...source.rotation },
					},
				]
			: [],
		move_in_black_enabled: source.move_in_black_enabled,
		move_in_black_delay_millis: source.move_in_black_delay_millis,
		highlight_overrides: source.highlight_overrides.map((override) => ({
			...override,
		})),
	};
}

function clearAssignments(
	assignments: PatchFixtureProjection["split_patches"],
): PatchFixtureInput["split_patches"] {
	return assignments.map(({ split }) => ({
		split,
		universe: null,
		address: null,
	}));
}

function deterministicUuid(namespace: string, value: number): string {
	return `${namespace}0000000-0000-4000-8000-${value
		.toString(16)
		.padStart(12, "0")}`;
}
