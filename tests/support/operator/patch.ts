import type { ApiDriver } from "../../../apps/control-ui/e2e/bench/api";
import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type {
	PatchFixtureInput,
	PatchFixtureProjection,
	PatchFixturesOutcome,
	PatchSnapshot,
} from "../../../apps/control-ui/src/api/generated/light-wire";
import type {
	Locator,
	Page,
} from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import { activeShowId } from "../catalog";

export interface SoftwareFixtureAddressRequest {
	page: Page;
	addressCell: Locator;
	address: string | null;
}

export interface PatchFixtureCopy {
	fixtureId: string;
	fixtureNumber: number;
	name: string;
	address: string;
}

/** Edits one visible fixture address through the current operator address screen. */
export async function setFixtureAddressThroughSoftware(
	request: SoftwareFixtureAddressRequest,
): Promise<void> {
	await request.page.getByRole("button", { name: "SET", exact: true }).click();
	await request.addressCell.click();
	const dialog = request.page.getByRole("dialog", { name: "Fixture Address" });
	await expect(dialog).toBeVisible();
	if (request.address === null) await clearEveryVisibleSplit(dialog);
	else await enterAddress(dialog, request.address);
	await dialog.getByRole("button", { name: "Set Address", exact: true }).click();
}

/** Makes the complete fixture projection output-less in one authoritative Patch transaction. */
export async function unpatchFixture(
	api: ApiDriver,
	fixtureId: string,
): Promise<PatchFixturesOutcome> {
	const showId = await activeShowId(api);
	const snapshot = await readPatchSnapshot(api, showId);
	const fixture = snapshot.fixtures.find(
		(candidate) => candidate.fixture_id === fixtureId,
	);
	if (!fixture) throw new Error(`Patch fixture ${fixtureId} does not exist`);
	return applyPatchFixtures(
		api,
		showId,
		snapshot.patch_revision,
		[unpatchedFixtureInput(fixture)],
	);
}

/** Creates independently-addressable fixture copies in one Patch transaction. */
export async function duplicatePatchedFixtures(
	api: ApiDriver,
	sourceFixtureId: string,
	copies: readonly PatchFixtureCopy[],
): Promise<PatchFixturesOutcome> {
	const showId = await activeShowId(api);
	const snapshot = await readPatchSnapshot(api, showId);
	const source = snapshot.fixtures.find(
		(candidate) => candidate.fixture_id === sourceFixtureId,
	);
	if (!source) throw new Error(`Patch fixture ${sourceFixtureId} does not exist`);
	return applyPatchFixtures(
		api,
		showId,
		snapshot.patch_revision,
		copies.map((copy) => copiedFixtureInput(source, copy)),
	);
}

/** Edits one fixture address through the authoritative Patch API surface. */
export async function setFixtureAddressThroughApi(
	api: ApiDriver,
	fixtureId: string,
	address: string | null,
): Promise<PatchFixturesOutcome> {
	const showId = await activeShowId(api);
	const snapshot = await readPatchSnapshot(api, showId);
	const fixture = snapshot.fixtures.find(
		(candidate) => candidate.fixture_id === fixtureId,
	);
	if (!fixture) throw new Error(`Patch fixture ${fixtureId} does not exist`);
	return applyPatchFixtures(
		api,
		showId,
		snapshot.patch_revision,
		[fixtureInputAt(fixture, address)],
	);
}

export async function expectFixtureUnpatched(
	api: ApiDriver,
	fixtureId: string,
): Promise<void> {
	const showId = await activeShowId(api);
	await expect
		.poll(async () => {
			const fixture = (await readPatchSnapshot(api, showId)).fixtures.find(
				(candidate) => candidate.fixture_id === fixtureId,
			);
			return fixture !== undefined && allAssignmentsAreClear(fixture);
		})
		.toBe(true);
}

export async function readPatchSnapshot(
	api: ApiDriver,
	showId?: string,
): Promise<PatchSnapshot> {
	const resolvedShowId = showId ?? (await activeShowId(api));
	return api.request<PatchSnapshot>(
		"GET",
		`/api/v2/shows/${resolvedShowId}/patch`,
	);
}

async function applyPatchFixtures(
	api: ApiDriver,
	showId: string,
	expectedPatchRevision: number,
	fixtures: PatchFixtureInput[],
): Promise<PatchFixturesOutcome> {
	return api.request<PatchFixturesOutcome>(
		"POST",
		`/api/v2/shows/${showId}/patch/fixtures`,
		{
			request_id: crypto.randomUUID(),
			fixtures,
			remove_fixture_ids: [],
		},
		true,
		expectedPatchRevision,
	);
}

async function clearEveryVisibleSplit(dialog: Locator): Promise<void> {
	const splits = dialog.getByRole("button", { name: /^Split \d+/ });
	const count = await splits.count();
	if (count === 0) {
		await clearAddress(dialog);
		return;
	}
	for (let index = 0; index < count; index += 1) {
		await splits.nth(index).click();
		await clearAddress(dialog);
	}
}

async function enterAddress(dialog: Locator, address: string): Promise<void> {
	if (!/^\d+\.\d+$/.test(address))
		throw new Error(`Fixture address must use universe.address, received ${address}`);
	await clearAddress(dialog);
	for (const character of address) {
		const name = character === "." ? "Universe separator" : `Address ${character}`;
		await dialog.getByRole("button", { name, exact: true }).click();
	}
}

async function clearAddress(dialog: Locator): Promise<void> {
	await dialog
		.getByRole("button", { name: "Clear address · Unpatch", exact: true })
		.click();
	await expect(
		dialog
			.locator(".fixture-address-summary span")
			.filter({ hasText: "Pending" })
			.locator("b"),
	).toHaveText("Unpatched");
}

function unpatchedFixtureInput(fixture: PatchFixtureProjection): PatchFixtureInput {
	return {
		...fixtureInput(fixture),
		split_patches: clearAssignments(fixture.split_patches),
		multipatch: fixture.multipatch.map((instance) => ({
			...instance,
			split_patches: clearAssignments(instance.split_patches),
		})),
	};
}

function copiedFixtureInput(
	source: PatchFixtureProjection,
	copy: PatchFixtureCopy,
): PatchFixtureInput {
	return {
		...fixtureInputAt(source, copy.address),
		fixture_id: copy.fixtureId,
		fixture_number: copy.fixtureNumber,
		name: copy.name,
		multipatch: [],
	};
}

function fixtureInputAt(
	fixture: PatchFixtureProjection,
	address: string | null,
): PatchFixtureInput {
	const assignments = fixture.split_patches.map((assignment) => ({
		...assignment,
	}));
	if (assignments.length === 0)
		throw new Error(`Patch fixture ${fixture.fixture_id} has no addressable split`);
	const parsed = parseAddress(address);
	assignments[0] = { ...assignments[0], ...parsed };
	return { ...fixtureInput(fixture), split_patches: assignments };
}

function fixtureInput(fixture: PatchFixtureProjection): PatchFixtureInput {
	return {
		fixture_id: fixture.fixture_id,
		fixture_number: fixture.fixture_number,
		virtual_fixture_number: fixture.virtual_fixture_number,
		name: fixture.name,
		profile_id: fixture.profile_id,
		profile_revision: fixture.profile_revision,
		mode_id: fixture.mode_id,
		split_patches: fixture.split_patches.map((assignment) => ({ ...assignment })),
		layer_id: fixture.layer_id,
		direct_control: fixture.direct_control,
		location: fixture.location,
		rotation: fixture.rotation,
		multipatch: fixture.multipatch.map((instance) => ({
			...instance,
			split_patches: instance.split_patches.map((assignment) => ({
				...assignment,
			})),
		})),
		move_in_black_enabled: fixture.move_in_black_enabled,
		move_in_black_delay_millis: fixture.move_in_black_delay_millis,
		highlight_overrides: fixture.highlight_overrides,
	};
}

function parseAddress(address: string | null): {
	universe: number | null;
	address: number | null;
} {
	if (address === null) return { universe: null, address: null };
	const match = /^(\d+)\.(\d+)$/.exec(address);
	if (!match)
		throw new Error(`Fixture address must use universe.address, received ${address}`);
	return { universe: Number(match[1]), address: Number(match[2]) };
}

function clearAssignments(
	assignments: PatchFixtureProjection["split_patches"],
): PatchFixtureInput["split_patches"] {
	return assignments.map(({ split }) => ({ split, universe: null, address: null }));
}

function allAssignmentsAreClear(fixture: PatchFixtureProjection): boolean {
	return [fixture.split_patches, ...fixture.multipatch.map((item) => item.split_patches)]
		.flat()
		.every((assignment) => assignment.universe === null && assignment.address === null);
}
