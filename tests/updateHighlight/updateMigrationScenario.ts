import {
	type BenchUiContext,
	expect,
	test,
} from "../../apps/control-ui/e2e/bench/fixtures";
import { setProgrammerFixtureValue } from "../../apps/control-ui/e2e/bench/programmerValues";
import { replaceProgrammingSelection } from "../../apps/control-ui/e2e/bench/programmingSelection";
import {
	loadCanonicalCopy,
	object,
	objects,
	programmer,
	putObject,
} from "../support/catalog";
import { groupBody, openGroups } from "../support/updateHighlight/highlight";
import { objectRows, readSql, runSql } from "../support/updateHighlight/system";

type UpdateMigrationContext = Pick<
	BenchUiContext,
	"api" | "bench" | "desk" | "page"
>;

test("UPDATE-002 @restart › pre-Update desk settings migrate once and Cue, Preset, and ordered Group updates remain undoable", async ({
	api,
	bench,
	desk,
	page,
}) => {
	test.setTimeout(90_000);
	const context = { api, bench, desk, page };
	const setup = await prepareLegacyUpdateScenario(context);
	const migration = await loadMigratedUpdateState(context, setup);
	const state = { ...setup, ...migration };
	await exerciseCueUpdate(context, state);
	await exercisePresetUpdate(context, state);
	await exerciseGroupUpdate(context, state);
	await verifyUpdateAfterRestart(context, state);
});

async function prepareLegacyUpdateScenario({
	api,
	bench,
}: UpdateMigrationContext) {
	const show = await loadCanonicalCopy(api, bench, "update-002-legacy");
	const showEntry = (
		await api.request<any[]>("GET", "/api/v1/shows", undefined, false)
	).find((entry) => entry.id === show.id);
	expect(showEntry).toBeDefined();
	const fixtures = (await objects<any>(api, "patched_fixture")).slice(0, 4);
	expect(fixtures).toHaveLength(4);
	const [first, second, third, fourth] = fixtures.map(
		(fixture) => fixture.body.fixture_id as string,
	);

	const cueListId = crypto.randomUUID();
	const cueId = crypto.randomUUID();
	const cueBaseline = {
		id: cueListId,
		name: "Legacy Update Cue",
		priority: 0,
		mode: "sequence",
		looped: false,
		chaser_step_millis: 1_000,
		speed_group: null,
		intensity_priority_mode: "htp",
		wrap_mode: "off",
		restart_mode: "first_cue",
		force_cue_timing: false,
		disable_cue_timing: false,
		chaser_xfade_millis: 0,
		speed_multiplier: 1,
		cues: [
			{
				id: cueId,
				number: 1,
				name: "Legacy cue",
				changes: [
					{
						fixture_id: first,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.2 },
						automatic_restore: false,
					},
				],
				group_changes: [],
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				phasers: [],
			},
		],
	};
	const presetId = "0.1";
	const presetBaseline = {
		name: "Legacy Update Preset",
		family: "Mixed",
		values: { [first]: { intensity: { kind: "normalized", value: 0.1 } } },
		group_values: {},
	};
	const groupId = "39";
	const groupBaseline = groupBody("Legacy ordered Group", [first, second]);
	await putObject(api, "cue_list", cueListId, cueBaseline);
	await putObject(api, "preset", presetId, presetBaseline);
	await putObject(api, "group", groupId, groupBaseline);

	const configuration = await api.request<any>(
		"GET",
		"/api/v1/configuration",
		undefined,
		false,
	);
	await api.request(
		"PUT",
		"/api/v1/configuration",
		configuration.configuration,
	);
	await bench.stopServerGracefully(api.session!.token);
	await runSql(
		`${bench.dataDir}/desk.sqlite`,
		"UPDATE settings SET value=json_remove(value,'$.update_settings_by_desk') WHERE key='server_configuration'; UPDATE schema_info SET version=6;",
	);
	expect(await readSql(showEntry.path, "SELECT version FROM schema_info")).toBe(
		"4",
	);
	expect(
		await readSql(
			showEntry.path,
			"SELECT count(*) FROM metadata WHERE key LIKE 'update_%'",
		),
	).toBe("0");
	return {
		show,
		showEntry,
		first,
		second,
		third,
		fourth,
		cueListId,
		cueId,
		presetId,
		groupId,
	};
}

type LegacyUpdateSetup = Awaited<
	ReturnType<typeof prepareLegacyUpdateScenario>
>;

async function loadMigratedUpdateState(
	{ api, bench }: UpdateMigrationContext,
	setup: LegacyUpdateSetup,
) {
	const { showEntry, cueListId, presetId, groupId } = setup;
	await bench.startServer();
	await api.login();
	const migratedDefaults = {
		cue_mode: "add_to_current_cue",
		preset_mode: "update_existing",
		group_mode: "update_existing",
		other_target_modes: {},
		show_update_modal_on_touch: true,
	};
	expect(await api.request<any>("GET", "/api/v1/update/settings")).toEqual(
		migratedDefaults,
	);
	expect(
		await readSql(
			`${bench.dataDir}/desk.sqlite`,
			"SELECT version FROM schema_info",
		),
	).toBe("9");
	const authoritativeCueBaseline = (
		await object<any>(api, "cue_list", cueListId)
	).body;
	const authoritativePresetBaseline = (
		await object<any>(api, "preset", presetId)
	).body;
	const authoritativeGroupBaseline = (await object<any>(api, "group", groupId))
		.body;
	return {
		migratedDefaults,
		authoritativeCueBaseline,
		authoritativePresetBaseline,
		authoritativeGroupBaseline,
	};
}

type MigratedUpdateState = LegacyUpdateSetup &
	Awaited<ReturnType<typeof loadMigratedUpdateState>>;

async function exerciseCueUpdate(
	{ api }: UpdateMigrationContext,
	state: MigratedUpdateState,
): Promise<void> {
	const {
		show,
		showEntry,
		first,
		second,
		cueListId,
		cueId,
		migratedDefaults,
		authoritativeCueBaseline,
	} = state;
	await replaceProgrammingSelection(api, {
		surface: "api",
		showId: show.id,
		fixtures: [first, second],
	});
	await setProgrammerFixtureValue(api, {
		surface: "api",
		showId: show.id,
		fixtureId: first,
		attribute: "intensity",
		value: { kind: "normalized", value: 0.8 },
		timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
	});
	await setProgrammerFixtureValue(api, {
		surface: "api",
		showId: show.id,
		fixtureId: second,
		attribute: "intensity",
		value: { kind: "normalized", value: 0.7 },
		timing: { fade: true, fadeMillis: 3_000, delayMillis: null },
	});
	const unrelatedBeforeCue = await objectRows(
		showEntry.path,
		"cue_list",
		cueListId,
	);
	const cueResult = await api.request<any>("POST", "/api/v1/update/apply", {
		target: {
			family: { type: "cue" },
			object_id: cueListId,
			cue_id: cueId,
			cue_number: 1,
		},
		mode: { target_type: "cue", mode: migratedDefaults.cue_mode },
		expected_revision: 1,
	});
	expect(cueResult.revision_after).toBe(2);
	const updatedCue = await object<any>(api, "cue_list", cueListId);
	expect(updatedCue.body.cues[0].changes).toHaveLength(1);
	expect(updatedCue.body.cues[0].changes[0]).toMatchObject({
		fixture_id: first,
		attribute: "intensity",
		value: { kind: "normalized" },
		automatic_restore: false,
	});
	expect(updatedCue.body.cues[0].changes[0].value.value).toBeCloseTo(0.8, 5);
	expect((await programmer(api)).values).toEqual(
		expect.arrayContaining([
			expect.objectContaining({ fixture_id: first, attribute: "intensity" }),
			expect.objectContaining({ fixture_id: second, attribute: "intensity" }),
		]),
	);
	expect(await objectRows(showEntry.path, "cue_list", cueListId)).toEqual(
		unrelatedBeforeCue,
	);
	await api.request(
		"POST",
		`/api/v1/shows/${show.id}/objects/cue_list/${cueListId}/undo`,
		undefined,
		true,
		updatedCue.revision,
	);
	expect((await object<any>(api, "cue_list", cueListId)).body).toEqual(
		authoritativeCueBaseline,
	);
}

async function exercisePresetUpdate(
	{ api }: UpdateMigrationContext,
	state: MigratedUpdateState,
): Promise<void> {
	const {
		show,
		showEntry,
		first,
		presetId,
		migratedDefaults,
		authoritativePresetBaseline,
	} = state;
	const unrelatedBeforePreset = await objectRows(
		showEntry.path,
		"preset",
		presetId,
	);
	const preset = await object<any>(api, "preset", presetId);
	const presetResult = await api.request<any>("POST", "/api/v1/update/apply", {
		target: { family: { type: "preset" }, object_id: presetId },
		mode: {
			target_type: "existing_content",
			mode: migratedDefaults.preset_mode,
		},
		expected_revision: preset.revision,
	});
	expect(presetResult.revision_after).toBe(preset.revision + 1);
	const updatedPreset = await object<any>(api, "preset", presetId);
	expect(Object.keys(updatedPreset.body.values)).toEqual([first]);
	expect(updatedPreset.body.values[first].intensity).toMatchObject({
		kind: "normalized",
	});
	expect(updatedPreset.body.values[first].intensity.value).toBeCloseTo(0.8, 5);
	expect(await objectRows(showEntry.path, "preset", presetId)).toEqual(
		unrelatedBeforePreset,
	);
	await api.request(
		"POST",
		`/api/v1/shows/${show.id}/objects/preset/${presetId}/undo`,
		undefined,
		true,
		updatedPreset.revision,
	);
	expect((await object<any>(api, "preset", presetId)).body).toEqual(
		authoritativePresetBaseline,
	);
}

async function exerciseGroupUpdate(
	{ api, bench, desk, page }: UpdateMigrationContext,
	state: MigratedUpdateState,
): Promise<void> {
	const {
		show,
		first,
		second,
		third,
		fourth,
		groupId,
		migratedDefaults,
		authoritativeGroupBaseline,
	} = state;
	await replaceProgrammingSelection(api, {
		surface: "api",
		showId: show.id,
		fixtures: [second, third, first, fourth],
	});
	const group = await object<any>(api, "group", groupId);
	const defaultPreview = await api.request<any>(
		"POST",
		"/api/v1/update/preview",
		{
			target: { family: { type: "group" }, object_id: groupId },
			mode: {
				target_type: "existing_content",
				mode: migratedDefaults.group_mode,
			},
			expected_revision: group.revision,
		},
	);
	expect(defaultPreview.mode).toEqual({
		target_type: "existing_content",
		mode: "update_existing",
	});
	expect(
		defaultPreview.items.filter(
			(item: any) => item.outcome.outcome === "unchanged",
		),
	).toHaveLength(2);
	expect(
		defaultPreview.items.filter(
			(item: any) => item.outcome.outcome === "ignored",
		),
	).toHaveLength(2);

	await desk.open(bench.baseUrl);
	await page.keyboard.press("Shift+End");
	await expect(
		page.getByText(/UPDATE armed · touch a recordable target/i),
	).toBeVisible();
	await openGroups(page);
	await page
		.locator(".group-pool-window .group-card")
		.filter({ hasText: "Legacy ordered Group" })
		.click();
	const updateDialog = page.getByRole("dialog", {
		name: /Update Legacy ordered Group/i,
	});
	await expect(updateDialog).toBeVisible();
	await expect(
		updateDialog.getByRole("button", { name: "Update Existing", exact: true }),
	).toHaveClass(/active/);
	await updateDialog
		.getByRole("button", { name: "Add New", exact: true })
		.click();
	await updateDialog
		.getByRole("button", { name: "Update Group", exact: true })
		.click();
	await expect(
		page.getByRole("dialog", { name: "Update complete" }),
	).toBeVisible();
	const updatedGroup = await object<any>(api, "group", groupId);
	expect(updatedGroup.body.fixtures).toEqual([first, second, third, fourth]);
	expect((await programmer(api)).selected).toEqual([
		second,
		third,
		first,
		fourth,
	]);
	await api.request(
		"POST",
		`/api/v1/shows/${show.id}/objects/group/${groupId}/undo`,
		undefined,
		true,
		updatedGroup.revision,
	);
	expect((await object<any>(api, "group", groupId)).body).toEqual(
		authoritativeGroupBaseline,
	);
}

async function verifyUpdateAfterRestart(
	{ api, bench }: UpdateMigrationContext,
	state: MigratedUpdateState,
): Promise<void> {
	const { showEntry, first, presetId, migratedDefaults } = state;
	await bench.stopServerGracefully(api.session!.token);
	await bench.startServer();
	await api.login();
	expect(await api.request<any>("GET", "/api/v1/update/settings")).toEqual(
		migratedDefaults,
	);
	expect(await readSql(showEntry.path, "SELECT version FROM schema_info")).toBe(
		"4",
	);
	const reopenedPreset = await object<any>(api, "preset", presetId);
	const repeated = await api.request<any>("POST", "/api/v1/update/apply", {
		target: { family: { type: "preset" }, object_id: presetId },
		mode: {
			target_type: "existing_content",
			mode: migratedDefaults.preset_mode,
		},
		expected_revision: reopenedPreset.revision,
	});
	expect(repeated.revision_after).toBe(reopenedPreset.revision + 1);
	const repeatedPreset = await object<any>(api, "preset", presetId);
	expect(Object.keys(repeatedPreset.body.values)).toEqual([first]);
	expect(repeatedPreset.body.values[first].intensity.value).toBeCloseTo(0.8, 5);
}
