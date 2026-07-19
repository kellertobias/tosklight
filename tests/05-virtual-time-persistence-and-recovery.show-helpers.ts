import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { promisify } from "node:util";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import type { LightBench } from "../apps/control-ui/e2e/bench/lightBench";
import type { Page } from "../apps/control-ui/node_modules/@playwright/test/index.js";
import {
	installSequence,
	playback,
} from "./05-virtual-time-persistence-and-recovery.playback-helpers";
import {
	fixtureRow,
	groupCard,
	openCuelistPool,
	openFixtures,
	openGroups,
	setDimmerByTouch,
	slot,
} from "./05-virtual-time-persistence-and-recovery.time-helpers";
import {
	activeShowId,
	fixtureIdsByNumber,
	loadCanonicalCopy,
	normalized,
	object,
	programmer,
	putObject,
} from "./support/catalog";

const sqlite = promisify(execFile);

export const SHOW_004_CASES = [
	"fixture-number",
	"group-defaults",
	"playback-defaults",
	"route-defaults",
	"virtual-dimmer-metadata",
	"cue-defaults",
] as const;
export type Show004Case = (typeof SHOW_004_CASES)[number];

export async function programShow001ThroughUi(
	api: ApiDriver,
	page: Page,
	revisionName: string,
): Promise<void> {
	await openFixtures(page);
	await fixtureRow(page, 5).click();
	await fixtureRow(page, 6).click();
	await openGroups(page);
	await page.locator(".global-store-button").click();
	await groupCard(page, 3).click();
	const mode = page.locator(".record-mode-dialog");
	await expect(mode).toBeVisible();
	await mode.getByRole("button", { name: "Merge", exact: true }).click();
	await expect(mode).toBeHidden();
	const fixtures = await fixtureIdsByNumber(api);
	await expect
		.poll(async () => (await object<any>(api, "group", "3")).body.fixtures)
		.toEqual([1, 2, 3, 4, 5, 6].map((number) => fixtures[number]));

	await groupCard(page, 3).click();
	await setDimmerByTouch(page, 40);
	await expect
		.poll(async () =>
			normalized((await programmer(api)).group_values["3"]?.intensity?.value),
		)
		.toBe(0.4);
	await openCuelistPool(page);
	await page.locator(".global-store-button").click();
	await expect(page.locator(".global-store-button")).toHaveText("REC ARMED");
	const cuelist = page.locator(".cuelist-card").first();
	await cuelist.click();
	await expect
		.poll(async () =>
			(await api.request<any>("GET", "/api/v1/playbacks")).pool.some(
				(playback: any) => playback.number === 1,
			),
		)
		.toBe(true);

	await page.getByRole("button", { name: "SET", exact: true }).click();
	await cuelist.click();
	await page.locator(".mode-toggle").click();
	await page
		.getByRole("button", { name: "Assign Cuelist 1 to page 1 playback 1" })
		.click();
	await expect
		.poll(async () => {
			const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
			return playbacks.pages.find((candidate: any) => candidate.number === 1)
				?.slots?.["1"];
		})
		.toBe(1);
	await page.locator(".mode-toggle").click();
	await page.getByRole("button", { name: "CLR", exact: true }).click();
	await page.getByRole("button", { name: "CLR", exact: true }).click();
	await cuelist.click();
	await expect(page.locator(".cue-table")).toBeVisible();
	await page.locator(".mode-toggle").click();
	await page
		.locator(
			'.playback-fader-bank article[data-page="1"][data-playback-slot="1"]',
		)
		.getByRole("button", { name: "GO +", exact: true })
		.click();
	await page.locator(".mode-toggle").click();

	await openFixtures(page);
	await fixtureRow(page, 12).click();
	await setDimmerByTouch(page, 65);
	await expect
		.poll(async () => {
			const fixtureId = (await fixtureIdsByNumber(api))[12];
			return normalized(
				(await programmer(api)).values.find(
					(value: any) =>
						value.fixture_id === fixtureId && value.attribute === "intensity",
				)?.value,
			);
		})
		.toBe(0.65);
	await saveNamedRevisionThroughUi(page, revisionName);
}

export async function assertShow001State(
	api: ApiDriver,
	bench: LightBench,
	state: {
		copyId: string;
		fixtureIds: Record<number, string>;
		revisionName: string;
	},
): Promise<void> {
	expect((await object<any>(api, "group", "3")).body.fixtures).toEqual(
		[1, 2, 3, 4, 5, 6].map((number) => state.fixtureIds[number]),
	);
	const playbacks = await api.request<any>("GET", "/api/v1/playbacks");
	const definition = playbacks.pool.find(
		(playback: any) => playback.number === 1,
	);
	expect(definition?.target.type).toBe("cue_list");
	const cueList = playbacks.cue_lists.find(
		(candidate: any) => candidate.id === definition.target.cue_list_id,
	);
	expect(cueList?.cues).toHaveLength(1);
	const groupChange = cueList.cues[0].group_changes[0];
	expect(groupChange).toMatchObject({
		group_id: "3",
		attribute: "intensity",
		value: { kind: "normalized" },
	});
	expect(groupChange.value.value).toBeCloseTo(0.4, 6);
	expect(
		playbacks.active.find((runtime: any) => runtime.playback_number === 1),
	).toMatchObject({ current_cue_number: 1, enabled: true });
	const durable = await programmer(api);
	expect(
		durable.values.find(
			(value: any) =>
				value.fixture_id === state.fixtureIds[12] &&
				value.attribute === "intensity",
		)?.value,
	).toMatchObject({ value: 0.65 });
	const revisions = await api.request<any[]>(
		"GET",
		`/api/v1/shows/${state.copyId}/revisions`,
	);
	expect(
		revisions.some((revision) => revision.name === state.revisionName),
	).toBe(true);
	const frame = await bench.tick(3_000);
	expect(
		frame.universes
			.find((universe) => universe.universe === 1)
			?.slots.slice(0, 6),
	).toEqual(Array(6).fill(102));
	expect(slot(frame, 12)).toBe(166);
}

export async function arrangeMalformedRecovery(
	api: ApiDriver,
	bench: LightBench,
	surface: string,
) {
	const damaged = await loadCanonicalCopy(api, bench, `show-003-${surface}`);
	const entry = await showEntry(api, damaged.id);
	const response = await fetch(
		`${api.baseUrl}/api/v1/shows/${damaged.id}/download`,
		{
			headers: { authorization: `Bearer ${api.session?.token}` },
		},
	);
	expect(response.ok).toBe(true);
	const recoveryShowName = `show-003-valid-${surface}-${crypto.randomUUID()}`;
	const recovery = await api.request<{ id: string }>("POST", "/api/v1/shows", {
		name: recoveryShowName,
		data_base64: Buffer.from(await response.arrayBuffer()).toString("base64"),
		overwrite: false,
	});
	await bench.stopServerGracefully(api.session!.token);
	await fs.writeFile(entry.path, Buffer.from("not a ToskLight SQLite show\n"));
	const damagedHash = await fileHash(entry.path);
	await bench.startServer();
	await api.login("Operator");
	const readiness = await api.request<any>(
		"GET",
		"/api/v1/readiness",
		undefined,
		false,
	);
	expect(readiness).toMatchObject({ status: "ready", recovery_mode: true });
	return {
		damagedPath: entry.path,
		damagedHash,
		damagedShowId: damaged.id,
		recoveryShowId: recovery.id,
		recoveryShowName,
	};
}

export async function prepareMigrationCase(
	api: ApiDriver,
	bench: LightBench,
	migration: Show004Case,
) {
	const copy = await loadCanonicalCopy(api, bench, `show-004-${migration}`);
	let cueListId: string | undefined;
	if (migration === "playback-defaults" || migration === "cue-defaults") {
		const fixtures = await fixtureIdsByNumber(api);
		cueListId = await installSequence(api, fixtures[1]);
		if (migration === "playback-defaults")
			await putObject(
				api,
				"playback",
				"1",
				playback(1, cueListId, "Legacy playback"),
			);
	}
	return { entry: await showEntry(api, copy.id), cueListId };
}

export async function stageLegacyMigration(
	file: string,
	migration: Show004Case,
	cueListId?: string,
): Promise<void> {
	if (migration === "fixture-number") {
		await runSql(
			file,
			"UPDATE objects SET body_json=json_remove(body_json, '$.fixture_number') WHERE kind='patched_fixture'",
		);
	} else if (migration === "group-defaults") {
		await runSql(
			file,
			"UPDATE objects SET body_json=json_remove(body_json, '$.color', '$.icon', '$.derived_from', '$.frozen_from', '$.programming', '$.master', '$.playback_fader') WHERE kind='group' AND id='3'",
		);
	} else if (migration === "playback-defaults") {
		await runSql(
			file,
			"UPDATE objects SET body_json=json_remove(body_json, '$.buttons', '$.button_count', '$.fader', '$.has_fader', '$.go_activates', '$.auto_off', '$.xfade_millis', '$.color', '$.flash_release', '$.protect_from_swap', '$.presentation_icon', '$.presentation_image') WHERE kind='playback' AND id='1'",
		);
	} else if (migration === "route-defaults") {
		await runSql(
			file,
			"UPDATE objects SET body_json=json_remove(body_json, '$.destination', '$.delivery_mode') WHERE kind='route'",
		);
	} else if (migration === "virtual-dimmer-metadata") {
		await runSql(
			file,
			"UPDATE objects SET body_json=json_remove(body_json, '$.definition.heads[0].parameters[0].metadata', '$.definition.heads[0].parameters[0].capabilities') WHERE kind='patched_fixture' AND json_extract(body_json, '$.fixture_number')=21",
		);
	} else {
		if (!cueListId) throw new Error("cue-defaults migration needs a Cuelist");
		await runSql(
			file,
			`UPDATE objects SET body_json=json_remove(body_json, '$.cues[0].id', '$.cues[1].id', '$.intensity_priority_mode', '$.wrap_mode', '$.restart_mode', '$.force_cue_timing', '$.disable_cue_timing', '$.chaser_xfade_millis', '$.chaser_xfade_percent', '$.speed_multiplier') WHERE kind='cue_list' AND id='${cueListId}'`,
		);
	}
}

export async function migrationSnapshot(
	api: ApiDriver,
	migration: Show004Case,
	cueListId?: string,
): Promise<any> {
	if (migration === "fixture-number") {
		const fixtures = await api.request<any[]>(
			"GET",
			`/api/v1/shows/${await activeShowId(api)}/objects/patched_fixture`,
			undefined,
			false,
		);
		return fixtures
			.map((fixture) => ({
				id: fixture.id,
				revision: fixture.revision,
				name: fixture.body.name,
				fixture_number: fixture.body.fixture_number,
			}))
			.sort((left, right) => left.fixture_number - right.fixture_number);
	}
	if (migration === "group-defaults") return object<any>(api, "group", "3");
	if (migration === "playback-defaults")
		return object<any>(api, "playback", "1");
	if (migration === "route-defaults")
		return object<any>(api, "route", "artnet");
	if (migration === "virtual-dimmer-metadata") {
		const fixtures = await api.request<any[]>(
			"GET",
			`/api/v1/shows/${await activeShowId(api)}/objects/patched_fixture`,
			undefined,
			false,
		);
		return fixtures.find((fixture) => fixture.body.fixture_number === 21);
	}
	if (!cueListId) throw new Error("cue-defaults migration needs a Cuelist");
	return object<any>(api, "cue_list", cueListId);
}

export function assertMigrationSnapshot(
	migration: Show004Case,
	snapshot: any,
): void {
	if (migration === "fixture-number") {
		expect(snapshot.map((fixture: any) => fixture.fixture_number)).toEqual([
			1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 21, 22, 23, 24,
		]);
	} else if (migration === "group-defaults") {
		expect(snapshot.body).toMatchObject({
			color: null,
			icon: null,
			derived_from: null,
			frozen_from: null,
			programming: {},
			master: 1,
			playback_fader: null,
		});
	} else if (migration === "playback-defaults") {
		expect(snapshot.body).toMatchObject({
			buttons: ["go_minus", "go", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
			color: "#20c997",
			flash_release: "release_all",
			protect_from_swap: false,
		});
	} else if (migration === "route-defaults") {
		expect(snapshot.body).toMatchObject({
			protocol: "art_net",
			logical_universe: 1,
			destination_universe: 1,
			delivery_mode: "broadcast",
			destination: null,
			enabled: true,
		});
	} else if (migration === "virtual-dimmer-metadata") {
		const intensity = snapshot.body.definition.heads[0].parameters.find(
			(parameter: any) => parameter.attribute === "intensity",
		);
		expect(intensity).toMatchObject({
			virtual_dimmer: true,
			capabilities: [],
			metadata: {
				physical_min: 0,
				physical_max: 1,
				unit: null,
				invert: false,
				wrap: false,
				curve: "linear",
			},
		});
	} else {
		expect(snapshot.body).toMatchObject({
			intensity_priority_mode: "htp",
			restart_mode: "first_cue",
			force_cue_timing: false,
			disable_cue_timing: false,
			chaser_xfade_percent: 0,
			speed_multiplier: 1,
		});
		expect(snapshot.body.cues.map((cue: any) => cue.id)).toHaveLength(2);
		expect(
			snapshot.body.cues.every((cue: any) => /^[0-9a-f-]{36}$/.test(cue.id)),
		).toBe(true);
	}
}
export async function saveNamedRevisionThroughUi(
	page: Page,
	name: string,
): Promise<void> {
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page
		.getByRole("button", { name: "Save Named Revision", exact: true })
		.click();
	const dialog = page.getByRole("dialog", { name: "Save named revision" });
	await dialog.getByLabel("Revision name").fill(name);
	await dialog.getByRole("button", { name: /^Save Revision/ }).click();
	await expect(dialog).toBeHidden();
	await expect(page.locator(".show-details")).toContainText(name);
	await page.getByRole("button", { name: "Close Show", exact: true }).click();
}
export async function showEntry(api: ApiDriver, id: string): Promise<any> {
	const entries = await api.request<any[]>("GET", "/api/v1/shows");
	const entry = entries.find((candidate) => candidate.id === id);
	expect(entry).toBeDefined();
	return entry;
}

export async function showObject(
	api: ApiDriver,
	showId: string,
	kind: string,
	id: string,
): Promise<any> {
	const entries = await api.request<any[]>(
		"GET",
		`/api/v1/shows/${showId}/objects/${kind}`,
		undefined,
		false,
	);
	const entry = entries.find((candidate) => candidate.id === id);
	expect(entry).toBeDefined();
	return entry;
}

export function escapeRegex(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
export function sqlString(value: string): string {
	return `'${value.replaceAll("'", "''")}'`;
}

export async function runSql(file: string, sql: string): Promise<void> {
	await sqlite("sqlite3", [file, sql]);
}

export async function readSql(file: string, sql: string): Promise<string> {
	const { stdout } = await sqlite("sqlite3", ["-noheader", file, sql]);
	return stdout.trim();
}

export async function fileHash(file: string): Promise<string> {
	return hash(await fs.readFile(file));
}

export function hash(bytes: Uint8Array): string {
	return createHash("sha256").update(bytes).digest("hex");
}
