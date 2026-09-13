import { expect, type Page } from "@playwright/test";
import {
	openPatch,
	patchFixtureRow,
} from "../../support/foundational/ui";
import {
	duplicatePatchedFixtures,
	expectFixtureUnpatched,
	readPatchSnapshot,
	setFixtureAddressThroughSoftware,
} from "../../support/operator/patch";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";

class PatchActionSurface {
	constructor(private readonly owner: BrowserPatch) {}

	unpatch(number: number): Promise<void> {
		return this.owner.unpatchThroughSoftware(number);
	}

	address(number: number, address: string): Promise<void> {
		return this.owner.setAddressThroughSoftware(number, address);
	}
}

export interface PatchLibraryFixtureIntent {
	readonly number: number;
	readonly name: string;
	readonly manufacturer: string;
	readonly profile: string;
	readonly mode: string;
	readonly address: string;
}

class PatchApiSurface {
	constructor(private readonly owner: BrowserPatch) {}

	add(intent: PatchLibraryFixtureIntent): Promise<void> {
		return this.owner.addLibraryFixtureThroughApi(intent);
	}

	remove(number: number): Promise<void> {
		return this.owner.removeFixtureThroughApi(number);
	}
}

class PatchExpectation {
	constructor(
		private readonly owner: BrowserPatch,
		private readonly number: number,
	) {}

	unpatched(): Promise<void> {
		return this.owner.expectUnpatched(this.number);
	}

	address(address: string): Promise<void> {
		return this.owner.expectAddress(this.number, address);
	}
}

export interface PatchConflictHandle {
	readonly anchor: number;
	readonly candidate: number;
}

/** Public semantic Patch actions used by browser acceptance scenarios. */
export class BrowserPatch {
	readonly via = {
		ui: new PatchActionSurface(this),
		api: new PatchApiSurface(this),
	};

	constructor(
		private readonly api: ApiDriver,
		private readonly page: Page,
		private readonly desk: DeskDriver,
	) {}

	expect(number: number): PatchExpectation {
		return new PatchExpectation(this, validFixtureNumber(number));
	}

	async prepareAddressConflict(): Promise<PatchConflictHandle> {
		const source = (await readPatchSnapshot(this.api)).fixtures.find(
			(fixture) => fixture.fixture_number === 1,
		);
		if (!source) throw new Error("Canonical show is missing Fixture 1");
		await duplicatePatchedFixtures(this.api, source.fixture_id, [
			{
				fixtureId: crypto.randomUUID(),
				fixtureNumber: 901,
				name: "Atomic Anchor",
				address: "2.1",
			},
			{
				fixtureId: crypto.randomUUID(),
				fixtureNumber: 902,
				name: "Atomic Candidate",
				address: "2.2",
			},
		]);
		return { anchor: 901, candidate: 902 };
	}

	/** Adds one shipped library fixture to the show in one authoritative Patch transaction. */
	async addLibraryFixtureThroughApi(
		intent: PatchLibraryFixtureIntent,
	): Promise<void> {
		const number = validFixtureNumber(intent.number);
		const library = await this.api.request<{
			profiles: Array<{
				id: string;
				revision: number;
				manufacturer: string;
				name: string;
				modes: Array<{ id: string; name: string }>;
			}>;
		}>("GET", "/api/v2/fixture-library/profiles");
		const profile = library.profiles.find(
			(candidate) =>
				sameName(candidate.manufacturer, intent.manufacturer) &&
				sameName(candidate.name, intent.profile),
		);
		if (!profile)
			throw new Error(
				`The fixture library has no ${intent.manufacturer} ${intent.profile}`,
			);
		const mode = profile.modes.find((candidate) =>
			sameName(candidate.name, intent.mode),
		);
		if (!mode)
			throw new Error(
				`${intent.manufacturer} ${intent.profile} has no mode ${intent.mode}`,
			);
		const [universe, address] = intent.address.split(".").map(Number);
		await this.desk.recordStep(
			"PATCH",
			`Patch ${intent.manufacturer} ${intent.profile} (${intent.mode}) as Fixture ${number} at ${intent.address}.`,
		);
		await this.applyFixtures({
			fixtures: [
				{
					fixture_id: crypto.randomUUID(),
					fixture_number: number,
					virtual_fixture_number: null,
					name: intent.name,
					profile_id: profile.id,
					profile_revision: profile.revision,
					mode_id: mode.id,
					split_patches: [{ split: 1, universe, address }],
					layer_id: "default",
					direct_control: null,
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					multipatch: [],
					move_in_black_enabled: false,
					move_in_black_delay_millis: 0,
					highlight_overrides: [],
				},
			],
			remove_fixture_ids: [],
		});
		await expect
			.poll(async () =>
				(await this.api.patch()).fixtures.some(
					(candidate) => candidate.fixture_number === number,
				),
			)
			.toBe(true);
	}

	/** Removes one fixture from the show, not merely its DMX address. */
	async removeFixtureThroughApi(number: number): Promise<void> {
		number = validFixtureNumber(number);
		const fixture = (await this.api.patch()).fixtures.find(
			(candidate) => candidate.fixture_number === number,
		);
		if (!fixture) throw new Error(`Fixture ${number} is absent`);
		await this.desk.recordStep(
			"PATCH",
			`Remove Fixture ${number} from the show.`,
		);
		await this.applyFixtures({
			fixtures: [],
			remove_fixture_ids: [fixture.fixture_id],
		});
		await expect
			.poll(async () =>
				(await this.api.patch()).fixtures.some(
					(candidate) => candidate.fixture_number === number,
				),
			)
			.toBe(false);
	}

	private async applyFixtures(change: {
		fixtures: unknown[];
		remove_fixture_ids: string[];
	}): Promise<void> {
		const snapshot = await readPatchSnapshot(this.api);
		await this.api.request(
			"POST",
			"/api/v2/patch/fixtures",
			{ request_id: crypto.randomUUID(), ...change },
			true,
			snapshot.patch_revision,
		);
	}

	async unpatchThroughSoftware(number: number): Promise<void> {
		number = validFixtureNumber(number);
		await this.desk.recordStep(
			"PATCH",
			`Unpatch Fixture ${number} through the visible Fixture Address workflow.`,
		);
		await openPatch(this.page);
		const row = patchFixtureRow(this.page, number);
		await setFixtureAddressThroughSoftware({
			page: this.page,
			addressCell: row.locator(".patch-address"),
			address: null,
		});
		await this.expectUnpatched(number);
	}

	async setAddressThroughSoftware(
		number: number,
		address: string,
	): Promise<void> {
		number = validFixtureNumber(number);
		await this.desk.recordStep(
			"PATCH",
			`Set Fixture ${number} to ${address} through the visible Fixture Address workflow.`,
		);
		await openPatch(this.page);
		const row = patchFixtureRow(this.page, number);
		await setFixtureAddressThroughSoftware({
			page: this.page,
			addressCell: row.locator(".patch-address"),
			address,
		});
	}

	async keepOldAddressAfterConflict(): Promise<void> {
		const conflict = this.page.getByRole("dialog", { name: "Patch conflict" });
		await expect(conflict).toBeVisible();
		await this.desk.click(
			conflict.getByRole("button", {
				name: "Keep old patch / mode",
				exact: true,
			}),
		);
		await expect(conflict).toBeHidden();
	}

	async expectUnpatched(number: number): Promise<void> {
		const fixture = (await this.api.patch()).fixtures.find(
			(candidate) => candidate.fixture_number === validFixtureNumber(number),
		);
		if (!fixture)
			throw new Error(`Fixture ${number} is not patched into the show`);
		await expectFixtureUnpatched(this.api, fixture.fixture_id);
	}

	async expectAddress(number: number, address: string): Promise<void> {
		const fixture = (await readPatchSnapshot(this.api)).fixtures.find(
			(candidate) => candidate.fixture_number === validFixtureNumber(number),
		);
		if (!fixture) throw new Error(`Fixture ${number} is absent`);
		const [universe, slot] = address.split(".").map(Number);
		expect(fixture.split_patches).toEqual(
			expect.arrayContaining([
				expect.objectContaining({ universe, address: slot }),
			]),
		);
	}
}

function sameName(actual: string | null | undefined, expected: string) {
	return (actual ?? "").trim().toLowerCase() === expected.trim().toLowerCase();
}

function validFixtureNumber(number: number): number {
	if (!Number.isSafeInteger(number) || number < 1)
		throw new Error("Fixture numbers start at 1");
	return number;
}
