import type { Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { clearProgrammerValues } from "./bench/programmer/programmerValues";
import { command, loadCanonicalCopy, objects } from "./support/catalog";

interface StoredChange {
	fixture_id: string;
	attribute: string;
	value: { kind: string; value?: number } | null;
	automatic_restore?: boolean;
}

interface StoredCueList {
	id: string;
	pool_number?: number | null;
	cues: Array<{ id: string; number: string; changes: StoredChange[] }>;
}

const POOL = 145;
const GROUP = "858";

async function stored(api: ApiDriver, showId: string, kind: string) {
	return api.showObjects<Record<string, unknown>>(showId, kind);
}

async function cueList(api: ApiDriver, showId: string) {
	const lists = await api.showObjects<StoredCueList>(showId, "cue_list");
	const list = lists.find((item) => item.body.pool_number === POOL);
	expect(list).toBeDefined();
	return list!;
}

const level = (list: StoredCueList, cue: number, fixtureId: string) =>
	list.cues[cue].changes.find(
		(change) => change.fixture_id === fixtureId && change.attribute === "intensity" && !change.automatic_restore,
	)?.value?.value;

/** Rewrites one intensity in a stored Cuelist, as offline programming would. */
function withLevel(list: StoredCueList, cue: number, fixtureId: string, value: number): StoredCueList {
	const next = structuredClone(list);
	const changes = next.cues[cue].changes;
	const existing = changes.find((change) => change.fixture_id === fixtureId && change.attribute === "intensity" && !change.automatic_restore);
	if (existing?.value) existing.value.value = value;
	else {
		const template = structuredClone(changes.find((change) => change.attribute === "intensity" && !change.automatic_restore)!);
		changes.push({ ...template, fixture_id: fixtureId, value: { ...template.value!, value } });
	}
	return next;
}

async function openShowMenu(page: Page) {
	const menu = page.getByRole("dialog", { name: "Show", exact: true });
	if (!(await menu.isVisible())) await page.getByRole("button", { name: /Open show menu/ }).click();
	await expect(menu).toBeVisible();
	return menu;
}

test("TL-858 @ui › an offline copy merges back: separate edits merge, a conflict is decided, a shared Group number is kept twice, and Undo reverts it", async ({ api, bench, desk, page }) => {
	test.setTimeout(150_000);
	const show = await loadCanonicalCopy(api, bench, "show-merge");
	const clear = () => clearProgrammerValues(api, { surface: "api", showId: show.id });
	await command(api, "FIXTURE 1 AT 50");
	await command(api, `RECORD CUELIST ${POOL}`);
	await clear();
	await command(api, "FIXTURE 2 AT 40");
	await command(api, `RECORD CUELIST ${POOL}`);
	await clear();
	const fixtures = await objects<{ fixture_id: string; fixture_number?: number }>(api, "patched_fixture");
	const fixture = (number: number) => fixtures.find((item) => item.body.fixture_number === number)!.body.fixture_id;
	expect(await api.showObject(show.id, "group", GROUP)).toBeNull();

	await page.setViewportSize({ width: 1400, height: 900 });
	await desk.open(api.baseUrl);

	// Take the copy from the Show menu.
	let menu = await openShowMenu(page);
	await menu.getByRole("button", { name: "Take Offline Copy" }).click();
	const take = page.getByRole("dialog", { name: "Take offline copy" });
	await take.getByRole("textbox", { name: "Offline copy name" }).fill("Programmer B");
	await take.getByRole("button", { name: "Take Offline Copy", exact: true }).click();
	await expect(take.getByRole("status")).toContainText("Saved Programmer B");
	await take.getByRole("button", { name: "Done", exact: true }).click();
	const copy = (await api.shows<{ id: string; name: string }>()).find((item) => item.name === "Programmer B");
	expect(copy).toBeDefined();
	expect((await api.showRevisions<{ name: string }>(show.id)).map((revision) => revision.name)).toContain("Offline copy: Programmer B");

	// The copy starts as the show was; it is programmed elsewhere: Cue 1 Fixture 1 changes, Cue 2
	// gains Fixture 3, Group 858 is new.
	const forked = await cueList(api, show.id);
	await api.seedShowObject(copy!.id, "cue_list", forked.id, withLevel(withLevel(forked.body, 0, fixture(1), 70), 1, fixture(3), 100), forked.revision);
	await api.seedShowObject(copy!.id, "group", GROUP, { id: GROUP, name: "Copy group", fixtures: [fixture(3)] });
	// Meanwhile the desk changes the same Cue 1 value and stores its own Group 858.
	await api.seedShowObject(show.id, "cue_list", forked.id, withLevel(forked.body, 0, fixture(1), 30), forked.revision);
	await api.seedShowObject(show.id, "group", GROUP, { id: GROUP, name: "Desk group", fixtures: [fixture(2)] });

	menu = await openShowMenu(page);
	await menu.getByRole("button", { name: "Merge Offline Copy" }).click();
	const merge = page.getByRole("dialog", { name: "Merge offline copy" });
	await merge.getByRole("button", { name: /Offline copy/ }).click();
	await page.getByRole("option", { name: "Programmer B", exact: true }).click();
	await expect(merge.getByRole("table", { name: "Merge summary" })).toBeVisible();
	const renumbered = merge.getByRole("region", { name: "Renumbered objects" });
	await expect(renumbered).toContainText(`Group ${GROUP} “Copy group” from the copy becomes ${Number(GROUP) + 1}`);
	const conflicts = merge.getByRole("region", { name: "Conflicts" }).getByRole("listitem");
	await expect(conflicts).toHaveCount(1);
	await expect(conflicts.first()).toContainText(`Cuelist ${POOL} › Cue 1 › Fixture 1 intensity › value`);
	const accept = merge.getByRole("button", { name: /^Decide 1 Conflict$/ });
	await expect(accept).toBeDisabled();
	await conflicts.first().getByRole("button", { name: /Take Theirs/ }).click();
	await merge.getByRole("button", { name: /^Merge \d+ Changes?$/ }).click();
	await expect(merge.getByRole("status")).toContainText("Merged");

	const merged = await cueList(api, show.id);
	expect(level(merged.body, 0, fixture(1))).toBe(70);
	expect(level(merged.body, 1, fixture(2))).toBe(level(forked.body, 1, fixture(2)));
	expect(level(merged.body, 1, fixture(3))).toBe(100);
	const groups = await stored(api, show.id, "group");
	expect(groups.find((group) => group.id === GROUP)?.body.name).toBe("Desk group");
	expect(groups.find((group) => group.id === String(Number(GROUP) + 1))?.body.name).toBe("Copy group");
	expect((await api.showRevisions<{ name: string }>(show.id)).map((revision) => revision.name)).toContain("Before merging Programmer B");

	// The whole merge is one Undo step.
	await merge.getByRole("button", { name: "Done", exact: true }).click();
	await page.keyboard.press("Escape");
	await api.sendCommandKey("UND");
	await expect.poll(async () => level((await cueList(api, show.id)).body, 0, fixture(1))).toBe(30);
	expect(await api.showObject(show.id, "group", String(Number(GROUP) + 1))).toBeNull();
});
