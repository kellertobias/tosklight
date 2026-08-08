import type { Locator, Page } from "@playwright/test";
import type { PatchedFixture } from "../../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { inclusiveSelectionNumbers } from "./selectionContract";
import { VisibleGroupPool } from "./selectionVisibleGroupScenario";

/**
 * What the Fixture Sheet reports back about the selection after a gesture.
 *
 * Declared here rather than imported: this used to be shared with a Stage surface that drove the
 * same gestures against the desk's own drawing of the rig. The renderer draws every Stage now, in
 * another process, so there are no fixture elements in the page to click and the Fixture Sheet is
 * the only visible surface a browser test can drive.
 */
export interface SelectionObservation {
	targets: ReadonlyArray<{ number: number }>;
	expression: import("../../../apps/light-desktop/src/features/programmingInteraction/contracts").SelectionExpression | null;
}

type FixtureSurface = "fixtureSheet";

interface ResolvedVisibleFixture {
	number: number;
	fixtureIds: string[];
}

class FixtureClickRoute {
	constructor(private readonly surface: VisibleFixtureSurface) {}

	item(number: number, head?: number): Promise<void> {
		return this.surface.item(number, head);
	}

	items(...numbers: number[]): Promise<void> {
		return this.surface.items(...numbers);
	}

	range(first: number, last: number, head?: number): Promise<void> {
		return this.surface.range(first, last, head);
	}
}

class FixtureTouchRoute {
	constructor(private readonly surface: VisibleFixtureSurface) {}

	item(number: number, head?: number): Promise<void> {
		return this.surface.touchItem(number, head);
	}

	items(...numbers: number[]): Promise<void> {
		return this.surface.touchItems(...numbers);
	}

	range(first: number, last: number, head?: number): Promise<void> {
		return this.surface.touchRange(first, last, head);
	}
}

export class VisibleFixtureSurface {
	readonly via: {
		click: FixtureClickRoute;
		touch: FixtureTouchRoute;
	};

	constructor(
		private readonly surface: FixtureSurface,
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly api: ApiDriver,
		private readonly observeSelection: () => Promise<SelectionObservation>,
	) {
		this.via = {
			click: new FixtureClickRoute(this),
			touch: new FixtureTouchRoute(this),
		};
	}

	async item(number: number, head?: number): Promise<void> {
		const fixtures = await this.resolveItems([{ number, head }], "item");
		await this.clickResolved(fixtures, "plain");
	}

	async items(...numbers: number[]): Promise<void> {
		if (numbers.length === 0)
			throw new Error(
				"Visible fixture items require at least one Fixture number",
			);
		const fixtures = await this.resolveItems(
			numbers.map((number) => ({ number })),
			"item",
		);
		await this.clickResolved(fixtures, "plain");
	}

	async range(first: number, last: number, head?: number): Promise<void> {
		const fixtures = await this.resolveItems(
			inclusiveSelectionNumbers(first, last).map((number) => ({
				number,
				head,
			})),
			"range",
		);
		await this.clickResolved(fixtures, "plain");
	}

	async touchItem(number: number, head?: number): Promise<void> {
		const fixtures = await this.resolveItems([{ number, head }], "item");
		await this.touchResolved(fixtures);
	}

	async touchItems(...numbers: number[]): Promise<void> {
		if (numbers.length === 0)
			throw new Error(
				"Visible fixture touch items require at least one Fixture number",
			);
		const fixtures = await this.resolveItems(
			numbers.map((number) => ({ number })),
			"item",
		);
		await this.touchResolved(fixtures);
	}

	async touchRange(first: number, last: number, head?: number): Promise<void> {
		const fixtures = await this.resolveItems(
			inclusiveSelectionNumbers(first, last).map((number) => ({
				number,
				head,
			})),
			"range",
		);
		await this.touchResolved(fixtures);
	}

	private async resolveItems(
		requests: Array<{ number: number; head?: number }>,
		semantics: "item" | "range",
	): Promise<ResolvedVisibleFixture[]> {
		for (const request of requests) {
			assertPositiveInteger(request.number, "Fixture number");
			if (request.head !== undefined)
				assertNonNegativeInteger(request.head, "Fixture head");
		}
		const patch = await this.api.patch();
		const byNumber = new Map(
			patch.fixtures
				.filter(
					(fixture): fixture is PatchedFixture & { fixture_number: number } =>
						fixture.fixture_number != null,
				)
				.map((fixture) => [fixture.fixture_number, fixture]),
		);
		return requests.flatMap(({ number, head }) => {
			const fixture = byNumber.get(number);
			if (!fixture)
				throw new Error(`Fixture ${number} is not present in the active patch`);
			const fixtureIds = resolveFixtureIds(fixture, head, semantics);
			return [{ number, fixtureIds }];
		});
	}

	private async clickResolved(
		fixtures: readonly ResolvedVisibleFixture[],
		gesture: "plain",
	): Promise<void> {
		const targets = await this.visibleTargets(fixtures);
		await this.record(gesture === "plain" ? "Click" : gesture, fixtures);
		for (const target of targets) await this.desk.click(target);
	}

	private async touchResolved(
		fixtures: readonly ResolvedVisibleFixture[],
	): Promise<void> {
		const targets = await this.visibleTargets(fixtures);
		await this.record("Tap", fixtures);
		for (const target of targets) await target.tap();
	}

	private async visibleTargets(
		fixtures: readonly ResolvedVisibleFixture[],
	): Promise<Locator[]> {
		const targets = fixtures.flatMap((fixture) =>
			fixture.fixtureIds.map((fixtureId) => this.locator(fixtureId)),
		);
		await Promise.all(
			targets.map(async (target) => {
				if ((await target.count()) !== 1 || !(await target.isVisible()))
					throw new Error(
						`The ${surfaceLabel(this.surface)} cannot visibly represent the requested Fixture target`,
					);
			}),
		);
		return targets;
	}

	private locator(fixtureId: string): Locator {
		const escaped = cssAttributeValue(fixtureId);
		return this.page.locator(
			`.fixture-window .ui-data-table-row[data-fixture-id="${escaped}"]`,
		);
	}

	private async record(
		gesture: string,
		fixtures: readonly ResolvedVisibleFixture[],
	): Promise<void> {
		await this.desk.recordStep(
			`${surfaceLabel(this.surface).toUpperCase()} SELECTION`,
			`${gesture} Fixture ${fixtures.map((fixture) => fixture.number).join(", ")} through the visible ${surfaceLabel(this.surface)}.`,
		);
	}

}

export class BrowserVisibleSelection {
	readonly fixtures: {
		via: {
			fixtureSheet: VisibleFixtureSurface;
		};
	};
	readonly groups: {
		via: {
			pool: VisibleGroupPool;
		};
	};

	constructor(
		page: Page,
		desk: DeskDriver,
		api: ApiDriver,
		observeSelection: () => Promise<SelectionObservation>,
	) {
		this.fixtures = {
			via: {
				fixtureSheet: new VisibleFixtureSurface(
					"fixtureSheet",
					page,
					desk,
					api,
					observeSelection,
				),
			},
		};
		this.groups = {
			via: {
				pool: new VisibleGroupPool(page, desk, api),
			},
		};
	}
}

function resolveFixtureIds(
	fixture: PatchedFixture,
	head: number | undefined,
	semantics: "item" | "range",
): string[] {
	if (head === 0) return [fixture.fixture_id];
	if (head !== undefined) {
		const logicalHead = fixture.logical_heads.find(
			(candidate) => candidate.head_index === head,
		);
		if (!logicalHead)
			throw new Error(
				`Fixture ${fixture.fixture_number ?? fixture.fixture_id} has no head ${head}`,
			);
	
		return [logicalHead.fixture_id];
	}
	if (semantics === "range" && fixture.logical_heads.length > 0)
		return fixture.logical_heads.map((candidate) => candidate.fixture_id);
	return [
		fixture.fixture_id,
		...fixture.logical_heads.map((candidate) => candidate.fixture_id),
	];
}

function surfaceLabel(_surface: FixtureSurface): string {
	return "Fixture Sheet";
}

function cssAttributeValue(value: string): string {
	return value.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

function assertPositiveInteger(value: number, label: string): void {
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error(`${label} must be a positive safe integer`);
}

function assertNonNegativeInteger(value: number, label: string): void {
	if (!Number.isSafeInteger(value) || value < 0)
		throw new Error(`${label} must be a non-negative safe integer`);
}
