import { expect } from "../../bench/core/fixtures";
import type { Page } from "@playwright/test";
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
		await request.surface.api.executeCommandLine(
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
	// Groups is a Shift Built-in. SHIFT sits on the playback face of the programmer surface and
	// the digits on the other, so the old SHIFT + digit shortcut cannot be clicked at all.
	const dock = page.getByRole("button", {
		name: "Desktops / Built-ins",
		exact: true,
	});
	if ((await dock.getAttribute("data-dock-mode")) !== "builtins") await dock.click();
	const shift = page.locator('[data-keypad-key="SHIFT"]:visible').first();
	const flipped = !(await shift.isVisible().catch(() => false));
	if (flipped) await page.locator(".mode-toggle").click();
	await shift.click();
	await page
		.locator("[aria-label='Shift Built-ins']")
		.getByRole("button", { name: "Groups", exact: true })
		.click();
	// Release Shift and leave the surface showing the face it had: an armed Shift renames every
	// key after its shifted meaning, and the next step addresses them by their own names.
	await shift.click();
	if (flipped) await page.locator(".mode-toggle").click();
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
