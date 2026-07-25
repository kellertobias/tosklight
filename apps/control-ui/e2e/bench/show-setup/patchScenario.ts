import type { Page } from "@playwright/test";
import {
	expectFixtureUnpatched,
	setFixtureAddressThroughSoftware,
} from "../../../../../tests/support/operator/patch";
import {
	openPatch,
	patchFixtureRow,
} from "../../../../../tests/support/foundational/ui";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";

class PatchActionSurface {
	constructor(private readonly owner: BrowserPatch) {}

	unpatch(number: number): Promise<void> {
		return this.owner.unpatchThroughSoftware(number);
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

	async expectUnpatched(number: number): Promise<void> {
		const fixture = (await this.api.patch()).fixtures.find(
			(candidate) => candidate.fixture_number === validFixtureNumber(number),
		);
		if (!fixture) throw new Error(`Fixture ${number} is not patched into the show`);
		await expectFixtureUnpatched(this.api, fixture.fixture_id);
	}
}

function validFixtureNumber(number: number): number {
	if (!Number.isSafeInteger(number) || number < 1)
		throw new Error("Fixture numbers start at 1");
	return number;
}
