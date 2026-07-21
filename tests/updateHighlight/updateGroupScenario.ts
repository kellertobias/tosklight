import { expect } from "../../apps/control-ui/e2e/bench/fixtures";
import { pairedScenario } from "../../apps/control-ui/e2e/bench/pairedScenario";
import { replaceProgrammingSelection } from "../../apps/control-ui/e2e/bench/programmingSelection";
import {
	loadCanonicalCopy,
	object,
	objects,
	programmer,
} from "../support/catalog";
import { escapeRegex, openGroups } from "../support/updateHighlight/highlight";

interface UpdateGroupState {
	showId: string;
	groupId: string;
	groupName: string;
	revision: number;
	original: string[];
	added: string;
}

pairedScenario<UpdateGroupState>({
	id: "UPDATE-001",
	title:
		"Update Add New appends ordered Group membership through the authoritative workflow",
	arrange: async ({ api, bench }, surface) => {
		const show = await loadCanonicalCopy(api, bench, `update-001-${surface}`);
		const groups = await objects<any>(api, "group");
		const fixtures = (await objects<any>(api, "patched_fixture")).map(
			(entry) => entry.body.fixture_id as string,
		);
		const group = groups.find(
			(entry) =>
				!entry.body.derived_from &&
				!entry.body.frozen_from &&
				entry.body.fixtures.length > 0,
		);
		expect(group).toBeDefined();
		const added = fixtures.find(
			(fixture) => !group!.body.fixtures.includes(fixture),
		);
		expect(added).toBeDefined();
		await replaceProgrammingSelection(api, {
			surface: "api",
			showId: show.id,
			fixtures: [added!],
		});
		return {
			showId: show.id,
			groupId: group!.id,
			groupName: group!.body.name || `Group ${group!.id}`,
			revision: group!.revision,
			original: [...group!.body.fixtures],
			added: added!,
		};
	},
	api: async ({ api }, state) => {
		await api.request("POST", "/api/v1/update/apply", {
			target: { family: { type: "group" }, object_id: state.groupId },
			mode: { target_type: "existing_content", mode: "add_new" },
			expected_revision: state.revision,
		});
	},
	ui: async ({ api, bench, desk, page }, state) => {
		await desk.open(bench.baseUrl);
		await expect
			.poll(async () => (await programmer(api)).selected)
			.toEqual([state.added]);
		await page.keyboard.press("Shift+End");
		await expect(
			page.getByText(/UPDATE armed · touch a recordable target/i),
		).toBeVisible();
		await openGroups(page);
		const target = page
			.locator(".group-pool-window .group-card")
			.filter({ hasText: state.groupName })
			.first();
		await expect(target).toBeVisible();
		await target.click();
		const dialog = page.getByRole("dialog", {
			name: new RegExp(`Update ${escapeRegex(state.groupName)}`, "i"),
		});
		await expect(dialog).toBeVisible();
		await dialog.getByRole("button", { name: "Add New", exact: true }).click();
		await expect(dialog.getByText(/Changed 1/)).toBeVisible();
		await dialog
			.getByRole("button", { name: "Update Group", exact: true })
			.click();
		await expect(
			page.getByRole("dialog", { name: "Update complete" }),
		).toBeVisible();
	},
	assert: async ({ api }, state) => {
		const stored = await object<any>(api, "group", state.groupId);
		expect(stored.revision).toBe(state.revision + 1);
		expect(stored.body.fixtures).toEqual([...state.original, state.added]);
		expect((await programmer(api)).selected).toEqual([state.added]);
	},
});
