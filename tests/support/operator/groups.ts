import { expect } from "../../../apps/control-ui/e2e/bench/fixtures";
import type { Page } from "../../../apps/control-ui/node_modules/@playwright/test/index.js";
import {
	executeProgrammerCommand,
	type ProgrammerSurface,
} from "./programmer";

export type GroupRecordOperation = "overwrite" | "merge" | "subtract";

export type StoreGroupRequest =
	| {
			via: "pool";
			page: Page;
			group: number;
			mode?: "Merge" | "Overwrite";
	  }
	| {
			via: "programmer";
			surface: ProgrammerSurface;
			group: number;
			operation?: GroupRecordOperation;
	  };

/** Stores a Group through the explicitly requested operator workflow. */
export async function storeGroup(request: StoreGroupRequest): Promise<void> {
	assertGroupNumber(request.group);
	if (request.via === "pool") {
		await storeGroupFromPool(request.page, request.group, request.mode);
		return;
	}
	if (request.surface.via === "command-line") {
		await request.surface.api.executeLegacyCommandLine(
			groupRecordCommand(request.group, request.operation ?? "overwrite"),
		);
		return;
	}
	await executeProgrammerCommand(
		request.surface,
		groupRecordCommand(request.group, request.operation ?? "overwrite"),
		{ reset: false },
	);
}

async function storeGroupFromPool(
	page: Page,
	group: number,
	mode?: "Merge" | "Overwrite",
): Promise<void> {
	await ensureGroupPool(page);
	const record = page.locator(".global-store-button");
	const card = groupCard(page, group);
	if (mode) await expect(card.locator("small")).toContainText("fixtures · ordered");
	const requiresMode =
		(await card.getByText(/^(Empty|⚠ Group is empty)$/).count()) === 0;
	if (requiresMode && !mode)
		throw new Error(
			`Group ${group} already exists; pool storage requires an explicit Merge or Overwrite mode`,
		);
	await record.click();
	await expect(record).toHaveText("REC ARMED");
	await card.click();
	const dialog = page.locator(".record-mode-dialog");
	if (requiresMode) {
		await expect(dialog).toBeVisible();
		await dialog.getByRole("button", { name: mode!, exact: true }).click();
		await expect(dialog).toBeHidden();
	} else {
		await expect(dialog).toHaveCount(0);
	}
	await expect(record).toHaveText("REC");
}

async function ensureGroupPool(page: Page): Promise<void> {
	if (await page.locator(".group-pool-window").isVisible()) return;
	await page.getByRole("button", { name: "SHIFT", exact: true }).click();
	await page.getByRole("button", { name: "1", exact: true }).click();
	await expect(page.locator(".group-pool-window")).toBeVisible();
}

function groupCard(page: Page, group: number) {
	return page.locator(".group-pool-window .group-card").nth(group - 1);
}

function groupRecordCommand(
	group: number,
	operation: GroupRecordOperation,
): string {
	const modifier = operation === "merge" ? " +" : operation === "subtract" ? " -" : "";
	return `RECORD${modifier} GROUP ${group}`;
}

function assertGroupNumber(group: number): void {
	if (!Number.isSafeInteger(group) || group < 1)
		throw new Error(`Group number must be a positive integer, received ${group}`);
}
