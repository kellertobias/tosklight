import type { Locator, Page } from "@playwright/test";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";
import { clearProgrammerValues } from "./bench/programmer/programmerValues";
import { command, loadCanonicalCopy, objects } from "./support/catalog";
import { doProgrammerStep } from "./support/operator";

type Option = "Smart" | "Merge" | "Add Existing" | "Add Cue";

interface CueChange {
	fixture_id: string;
	value: { value?: number } | null;
}

interface StoredCue {
	number: string;
	changes: CueChange[];
}

const CUELIST = 101;

test.describe("docs/testing/24-record-and-update-options.md", () => {
	test("RECUPD-001 @ui › RECORD RECORD records a one-off option, stores a default plain RECORD uses, and resets to Smart", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await loadCanonicalCopy(api, bench, "recupd-001");
		await desk.open(api.baseUrl);
		const clear = () =>
			clearProgrammerValues(api, { surface: "api", showId: show.id });

		await command(api, "FIXTURE 1 AT 50");
		await command(api, `RECORD CUELIST ${CUELIST}`);
		await clear();
		const [first] = await cues(api);
		expect(first.changes).toHaveLength(1);
		const fixtureOne = first.changes[0].fixture_id;

		// Add Existing, once: only the new fixture is added; Fixture 1 keeps 50%.
		await command(api, "FIXTURE 1 AT 80");
		await command(api, "FIXTURE 2 AT 40");
		let dialog = await openChoice(page, "Record");
		await expect(
			dialog.getByRole("radiogroup", { name: "Record mode" }).getByRole("radio"),
		).toHaveText(["Smart", "Merge", "Add Existing", "Add Cue"]);
		await expect(
			dialog.getByRole("switch", { name: "Set as default" }),
		).not.toBeChecked();
		await expect(dialog.locator(".record-update-current-default")).toContainText(
			"Current default: Smart",
		);
		await confirm(dialog, "Record", "Add Existing");
		await expectLine(api, "RECORD ADD EXISTING");
		await finishOnKeypad(page, api);
		let stored = await cues(api);
		expect(stored).toHaveLength(1);
		expect(stored[0].changes).toHaveLength(2);
		expect(level(stored[0], fixtureOne)).toBeCloseTo(0.5);
		expect(await defaults(api)).toMatchObject({ record_default: "smart" });

		// Add Cue as the default: the line stays plain and plain RECORD never replaces a Cue.
		dialog = await openChoice(page, "Record");
		await confirm(dialog, "Record", "Add Cue", true);
		await expect.poll(async () => (await defaults(api)).record_default).toBe(
			"add_cue",
		);
		await expectLine(api, "RECORD");
		await finishOnKeypad(page, api);
		expect(await cues(api)).toHaveLength(2);
		const refused = await api.executeCommandLineRaw(
			`RECORD CUELIST ${CUELIST} CUE 1`,
		);
		expect(refused.outcome).toBe("rejected");
		expect(level((await cues(api))[0], fixtureOne)).toBeCloseTo(0.5);
		await clear();

		// Merge, once, while Add Cue stays the default: programmer values win in Cue 1.
		await command(api, "FIXTURE 1 AT 90");
		dialog = await openChoice(page, "Record");
		await expect(dialog.locator(".record-update-current-default")).toContainText(
			"Current default: Add Cue",
		);
		await confirm(dialog, "Record", "Merge");
		await expectLine(api, "RECORD MERGE");
		await finishOnKeypad(page, api, true);
		stored = await cues(api);
		expect(stored).toHaveLength(2);
		expect(stored[0].changes).toHaveLength(2);
		expect(level(stored[0], fixtureOne)).toBeCloseTo(0.9);
		expect(await defaults(api)).toMatchObject({ record_default: "add_cue" });
		await clear();

		// Reset to Smart: an explicit Cue number overwrites that Cue again.
		await command(api, "FIXTURE 1 AT 20");
		dialog = await openChoice(page, "Record");
		await confirm(dialog, "Record", "Smart", true);
		await expect.poll(async () => (await defaults(api)).record_default).toBe(
			"smart",
		);
		await expectLine(api, "RECORD");
		await finishOnKeypad(page, api, true);
		stored = await cues(api);
		expect(stored).toHaveLength(2);
		expect(stored[0].changes).toHaveLength(1);
		expect(level(stored[0], fixtureOne)).toBeCloseTo(0.2);
		await clear();
	});

	test("RECUPD-002 @ui › UPDATE UPDATE uses the same layout and a stored Update default", async ({
		api,
		bench,
		desk,
		page,
	}) => {
		const show = await loadCanonicalCopy(api, bench, "recupd-002");
		await desk.open(api.baseUrl);
		await command(api, "FIXTURE 1 AT 50");
		await command(api, `RECORD CUELIST ${CUELIST}`);
		await clearProgrammerValues(api, { surface: "api", showId: show.id });

		await command(api, "FIXTURE 2 AT 40");
		let dialog = await openChoice(page, "Update");
		await expect(
			dialog.getByRole("radiogroup", { name: "Update mode" }).getByRole("radio"),
		).toHaveText(["Smart", "Merge", "Add Existing", "Add Cue"]);
		await expect(
			dialog.getByRole("switch", { name: "Set as default" }),
		).not.toBeChecked();
		await confirm(dialog, "Update", "Merge", true);
		await expect.poll(async () => (await defaults(api)).update_default).toBe(
			"merge",
		);
		await expectLine(api, "UPDATE");

		// Merge updates like UPDATE ALL: a fixture new to the Cuelist enters Cue 1.
		await finishOnKeypad(page, api, true);
		await expect.poll(async () => (await cues(api))[0].changes.length).toBe(2);
		await clearProgrammerValues(api, { surface: "api", showId: show.id });

		// Back to Smart: plain UPDATE ignores an address the Cue does not store.
		await command(api, "FIXTURE 3 AT 30");
		dialog = await openChoice(page, "Update");
		await expect(dialog.locator(".record-update-current-default")).toContainText(
			"Current default: Merge",
		);
		await confirm(dialog, "Update", "Smart", true);
		await expect.poll(async () => (await defaults(api)).update_default).toBe(
			"smart",
		);
		await api.executeCommandLineRaw(`UPDATE CUELIST ${CUELIST} CUE 1`);
		expect((await cues(api))[0].changes).toHaveLength(2);
	});
});

async function openChoice(page: Page, verb: "Record" | "Update") {
	const rec = page.locator(".global-store-button");
	const modifiers = verb === "Update" ? (["Shift"] as const) : ([] as const);
	await rec.click({ modifiers: [...modifiers] });
	await expect(rec).toContainText(verb === "Update" ? "UPDATE" : "ARMED");
	await rec.click({ modifiers: [...modifiers] });
	const dialog = page.getByRole("dialog", { name: verb, exact: true });
	await expect(dialog).toBeVisible();
	await expect(
		dialog
			.locator("header.ui-modal-titlebar")
			.getByRole("button", { name: verb, exact: true }),
	).toBeEnabled();
	return dialog;
}

async function confirm(
	dialog: Locator,
	verb: "Record" | "Update",
	option: Option,
	setAsDefault = false,
) {
	await dialog.getByRole("radio", { name: option, exact: true }).click();
	await expect(
		dialog.getByRole("radio", { name: option, exact: true }),
	).toHaveAttribute("aria-checked", "true");
	if (setAsDefault)
		await dialog
			.getByRole("switch", { name: "Set as default" })
			.locator("..")
			.locator(".ui-switch-track")
			.click();
	await dialog
		.locator("header.ui-modal-titlebar")
		.getByRole("button", { name: verb, exact: true })
		.click();
	await expect(dialog).toBeHidden();
}

async function expectLine(api: ApiDriver, text: string) {
	await expect
		.poll(async () => (await api.getCommandLine()).commandLine.text.trim())
		.toBe(text);
}

/** Completes the armed line on the on-screen keypad: Cuelist 101, optionally Cue 1, then ENT. */
async function finishOnKeypad(page: Page, api: ApiDriver, cueOne = false) {
	await doProgrammerStep({ via: "software", page }, [
		"CUE",
		"CUE",
		"1",
		"0",
		"1",
		...(cueOne ? (["CUE", "1"] as const) : []),
	]);
	await expect
		.poll(async () => (await api.getCommandLine()).commandLine.text)
		.toMatch(cueOne ? /CUELIST 101 CUE 1\s*$/ : /CUELIST 101\s*$/);
	await doProgrammerStep({ via: "software", page }, ["ENT"]);
	await expect
		.poll(async () => (await api.getCommandLine()).commandLine.text)
		.not.toMatch(/^\s*(RECORD|UPDATE)\b/);
}

async function defaults(api: ApiDriver) {
	const response = await api.request<{
		settings: { record_default: string; update_default: string };
	}>("GET", "/api/v2/programming-update/settings");
	return response.settings;
}

async function cues(api: ApiDriver): Promise<StoredCue[]> {
	const playback = (await objects<any>(api, "playback")).find(
		(candidate) => candidate.body.number === CUELIST,
	);
	expect(playback).toBeDefined();
	const cueListId = playback?.body.target.cue_list_id;
	const cueList = (await objects<any>(api, "cue_list")).find(
		(candidate) => candidate.body.id === cueListId,
	);
	expect(cueList).toBeDefined();
	return cueList?.body.cues ?? [];
}

function level(cue: StoredCue, fixtureId: string) {
	return cue.changes.find((change) => change.fixture_id === fixtureId)?.value
		?.value;
}
