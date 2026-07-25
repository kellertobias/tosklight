import { describe, expect, it, vi } from "vitest";
import type { PatchedFixture, StoredGroup } from "../../../src/api/types";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { BrowserVisibleSelection } from "./selectionVisibleScenario";

describe("visible selection scenario routes", () => {
	it("clicks complete Fixture Sheet items in semantic master/head order", async () => {
		const harness = visibleHarness();

		await harness.selection.fixtures.via.fixtureSheet.item(101);

		expect(harness.clickedNames()).toEqual([
			"fixture-sheet:master-101",
			"fixture-sheet:head-101-1",
			"fixture-sheet:head-101-2",
		]);
	});

	it("uses child-only range semantics in Fixture Sheet order", async () => {
		const harness = visibleHarness();

		await harness.selection.fixtures.via.fixtureSheet.range(101, 102);

		expect(harness.clickedNames()).toEqual([
			"fixture-sheet:head-101-1",
			"fixture-sheet:head-101-2",
			"fixture-sheet:master-102",
		]);
	});

	it("uses the browser touch action for the explicit touch route", async () => {
		const harness = visibleHarness();

		await harness.selection.fixtures.via.fixtureSheet.via.touch.item(102);

		expect(
			harness.locator("fixture-sheet:master-102").tap,
		).toHaveBeenCalledOnce();
		expect(harness.click).not.toHaveBeenCalled();
	});

	it("performs a real Stage click then Shift-click gesture", async () => {
		const harness = visibleHarness();

		await harness.selection.fixtures.via.stage.via.click.item(102);
		const result =
			await harness.selection.fixtures.via.stage.via.shiftClick?.item(103);

		expect(harness.clickedNames()).toEqual(["stage:master-102"]);
		expect(harness.locator("stage:master-102").click).toHaveBeenCalledWith({
			modifiers: ["Meta"],
		});
		expect(harness.locator("stage:master-103").click).toHaveBeenCalledWith({
			modifiers: ["Shift"],
		});
		expect(result).toEqual({
			order: "stage-visible",
			anchor: 102,
			target: 103,
			selection: [102, 103],
			expression: null,
		});
	});

	it("rejects numeric Stage Shift-click ranges before mutation", async () => {
		const harness = visibleHarness();

		expect(() =>
			harness.selection.fixtures.via.stage.via.shiftClick?.range(102, 103),
		).toThrow(/visible Stage order/);
		expect(harness.click).not.toHaveBeenCalled();
	});

	it("rejects unsupported Stage head targets before clicking", async () => {
		const harness = visibleHarness();

		await expect(
			harness.selection.fixtures.via.stage.item(101, 1),
		).rejects.toThrow(/cannot visibly address an individual fixture head/);
		expect(harness.click).not.toHaveBeenCalled();
	});

	it("rejects a hidden visible target before the first click", async () => {
		const harness = visibleHarness({ hidden: ["fixture-sheet:master-102"] });

		await expect(
			harness.selection.fixtures.via.fixtureSheet.items(102, 103),
		).rejects.toThrow(/cannot visibly represent/);
		expect(harness.click).not.toHaveBeenCalled();
	});

	it("clicks stored empty Groups while skipping absent IDs in a range", async () => {
		const harness = visibleHarness();

		await harness.selection.groups.via.pool.range(1, 4);

		expect(harness.clickedNames()).toEqual([
			"group-pool:1",
			"group-pool:3",
			"group-pool:4",
		]);
	});

	it("rejects an explicitly absent Group before mutation", async () => {
		const harness = visibleHarness();

		await expect(harness.selection.groups.via.pool.items(1, 2)).rejects.toThrow(
			/Group 2 is not present/,
		);
		expect(harness.click).not.toHaveBeenCalled();
	});

	it("uses the pool's real double-click dereference gesture", async () => {
		const harness = visibleHarness();

		await harness.selection.groups.via.pool.dereferencedItem(3);

		expect(harness.locator("group-pool:3").dblclick).toHaveBeenCalledOnce();
		expect(harness.click).not.toHaveBeenCalled();
	});
});

interface HarnessOptions {
	hidden?: string[];
}

function visibleHarness(options: HarnessOptions = {}) {
	const locators = new Map<string, FakeLocator>();
	const fixtureSelector = /data-fixture-id="([^"]+)"/;
	const groupRoot = new FakeGroupRoot(locators, new Set(options.hidden));
	const page = {
		waitForTimeout: vi.fn(async () => undefined),
		locator: vi.fn((selector: string) => {
			if (selector === ".group-pool-window .group-card") return groupRoot;
			const fixtureId = fixtureSelector.exec(selector)?.[1];
			if (!fixtureId) throw new Error(`Unexpected locator ${selector}`);
			const surface = selector.startsWith(".fixture-window")
				? "fixture-sheet"
				: "stage";
			return getLocator(
				locators,
				`${surface}:${fixtureId}`,
				new Set(options.hidden),
			);
		}),
	};
	const click = vi.fn(async (target: FakeLocator) => {
		target.plainClicks += 1;
	});
	const desk = {
		click,
		recordStep: vi.fn(async () => undefined),
	} as unknown as DeskDriver;
	const api = {
		patch: vi.fn(async () => patch()),
		request: vi.fn(async () => ({ active_show: { id: "show-a" } })),
		showObjects: vi.fn(async () => groups()),
	} as unknown as ApiDriver;
	return {
		selection: new BrowserVisibleSelection(page as never, desk, api, async () => ({
			targets: [{ number: 102 }, { number: 103 }],
			expression: null,
		})),
		click,
		locator: (name: string) => {
			const locator = locators.get(name);
			if (!locator) throw new Error(`No locator ${name}`);
			return locator;
		},
		clickedNames: () =>
			click.mock.calls.map(([locator]) => (locator as FakeLocator).name),
	};
}

class FakeLocator {
	plainClicks = 0;
	readonly click = vi.fn(async (_options?: unknown) => undefined);
	readonly tap = vi.fn(async () => undefined);
	readonly dblclick = vi.fn(async () => undefined);

	constructor(
		readonly name: string,
		private readonly visible: boolean,
	) {}

	async count(): Promise<number> {
		return 1;
	}

	async isVisible(): Promise<boolean> {
		return this.visible;
	}
}

class FakeGroupRoot {
	constructor(
		private readonly locators: Map<string, FakeLocator>,
		private readonly hidden: Set<string>,
	) {}

	nth(index: number): FakeLocator {
		return getLocator(this.locators, `group-pool:${index + 1}`, this.hidden);
	}
}

function getLocator(
	locators: Map<string, FakeLocator>,
	name: string,
	hidden: ReadonlySet<string>,
): FakeLocator {
	let locator = locators.get(name);
	if (!locator) {
		locator = new FakeLocator(name, !hidden.has(name));
		locators.set(name, locator);
	}
	return locator;
}

function patch(): {
	revision: number;
	fixtures: PatchedFixture[];
	routes: [];
} {
	return {
		revision: 1,
		routes: [],
		fixtures: [
			patchedFixture(101, "master-101", [
				{ fixture_id: "head-101-1", head_index: 1 },
				{ fixture_id: "head-101-2", head_index: 2 },
			]),
			patchedFixture(102, "master-102"),
			patchedFixture(103, "master-103"),
		],
	};
}

function patchedFixture(
	number: number,
	id: string,
	logicalHeads: PatchedFixture["logical_heads"] = [],
): PatchedFixture {
	return {
		fixture_id: id,
		fixture_number: number,
		universe: 1,
		address: number,
		definition: {} as PatchedFixture["definition"],
		logical_heads: logicalHeads,
	};
}

function groups(): Array<{ id: string; body: StoredGroup }> {
	return [
		storedGroup("1", ["master-102"]),
		storedGroup("3", []),
		storedGroup("4", ["master-103"]),
	];
}

function storedGroup(id: string, fixtures: string[]) {
	return {
		id,
		body: {
			name: `Group ${id}`,
			fixtures,
			master: 1,
			playback_fader: null,
			programming: {},
			derived_from: null,
			frozen_from: null,
		} satisfies StoredGroup,
	};
}
