import type { Page } from "@playwright/test";
import type { StoredGroup } from "../../src/api/types";
import type { ApiDriver } from "./api";
import type { DeskDriver } from "./desk";
import { inclusiveSelectionNumbers } from "./selectionContract";

interface StoredGroupObject {
	id: string;
	body: StoredGroup;
}

export class VisibleGroupPool {
	constructor(
		private readonly page: Page,
		private readonly desk: DeskDriver,
		private readonly api: ApiDriver,
	) {}

	item(number: number): Promise<void> {
		return this.clickNumbers([number], false);
	}

	items(...numbers: number[]): Promise<void> {
		if (numbers.length === 0)
			throw new Error("Visible Group items require at least one Group number");
		return this.clickNumbers(numbers, false);
	}

	async range(first: number, last: number): Promise<void> {
		assertPositiveInteger(first, "First Group number");
		assertPositiveInteger(last, "Last Group number");
		const groups = await this.groups();
		const present = inclusiveSelectionNumbers(first, last).filter((number) =>
			groups.has(String(number)),
		);
		await this.clickResolvedGroups(present, groups, false);
	}

	dereferencedItem(number: number): Promise<void> {
		return this.clickNumbers([number], true);
	}

	private async clickNumbers(
		numbers: readonly number[],
		dereference: boolean,
	): Promise<void> {
		for (const number of numbers) assertPositiveInteger(number, "Group number");
		const groups = await this.groups();
		for (const number of numbers) {
			if (!groups.has(String(number)))
				throw new Error(`Group ${number} is not present in the active Show`);
		}
		await this.clickResolvedGroups(numbers, groups, dereference);
	}

	private async clickResolvedGroups(
		numbers: readonly number[],
		groups: ReadonlyMap<string, StoredGroupObject>,
		dereference: boolean,
	): Promise<void> {
		const targets = numbers.map((number) =>
			this.page.locator(".group-pool-window .group-card").nth(number - 1),
		);
		await Promise.all(
			targets.map(async (target) => {
				if ((await target.count()) !== 1 || !(await target.isVisible()))
					throw new Error(
						"The Groups pool cannot visibly represent the requested Group target",
					);
			}),
		);
		void groups;
		if (numbers.length === 0) return;
		await this.desk.recordStep(
			"GROUP POOL SELECTION",
			`${dereference ? "Double-click" : "Click"} Group ${numbers.join(", ")} through the visible Groups pool.`,
		);
		for (const target of targets) {
			if (dereference) await target.dblclick();
			else await this.desk.click(target);
		}
	}

	private async groups(): Promise<Map<string, StoredGroupObject>> {
		const bootstrap = await this.api.request<{
			active_show: { id: string } | null;
		}>("GET", "/api/v2/bootstrap");
		if (!bootstrap.active_show) throw new Error("No active Show");
		const groups = await this.api.showObjects<StoredGroup>(
			bootstrap.active_show.id,
			"group",
		);
		return new Map(groups.map((group) => [group.id, group]));
	}
}

function assertPositiveInteger(value: number, label: string): void {
	if (!Number.isSafeInteger(value) || value < 1)
		throw new Error(`${label} must be a positive safe integer`);
}
