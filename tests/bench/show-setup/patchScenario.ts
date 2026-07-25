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
	readonly via = { ui: new PatchActionSurface(this) };

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

function validFixtureNumber(number: number): number {
	if (!Number.isSafeInteger(number) || number < 1)
		throw new Error("Fixture numbers start at 1");
	return number;
}
