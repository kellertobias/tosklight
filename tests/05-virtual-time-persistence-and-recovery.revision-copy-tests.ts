import fs from "node:fs/promises";
import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import type { DeskDriver } from "../apps/control-ui/e2e/bench/desk";
import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import type { LightBench } from "../apps/control-ui/e2e/bench/lightBench";
import { pairedScenario } from "../apps/control-ui/e2e/bench/pairedScenario";
import type {
	Locator,
	Page,
} from "../apps/control-ui/node_modules/@playwright/test/index.js";
import {
	escapeRegex,
	readSql,
	showEntry,
	showObject,
} from "./05-virtual-time-persistence-and-recovery.show-helpers";
import { loadCanonicalCopy } from "./support/catalog";

type RevisionCopyState = {
	sourceId: string;
	sourceName: string;
	savedRevision: number;
	copyId?: string;
	copyProvenance?: RevisionCopySource;
	expectedSourceName?: string;
	expectedCopyName?: string;
	expectedCopyRevisions?: string[];
	destinationId: string;
	destinationName: string;
	destinationRevision: number;
};

type ApiBenchContext = {
	api: ApiDriver;
	bench: LightBench;
};

type UiContext = ApiBenchContext & {
	desk: DeskDriver;
	page: Page;
};

type RevisionCopySource = {
	show_id: string;
	show_name: string;
	revision: number;
	revision_name: string;
	copied_at: string;
};

type RevisionShow = {
	id: string;
	name: string;
	revision_copy: RevisionCopySource;
};

type RevisionBootstrap = {
	active_show: RevisionShow;
};

type NamedRevision = {
	revision: number;
	name: string;
};

async function arrangeRevisionCopy(
	{ api, bench }: ApiBenchContext,
	surface: string,
): Promise<RevisionCopyState> {
	const source = await loadCanonicalCopy(api, bench, `show-005-${surface}`);
	const sourceEntry = await showEntry(api, source.id);
	const named = await showObject(api, source.id, "group", "4");
	await api.request(
		"PUT",
		`/api/v1/shows/${source.id}/objects/group/4`,
		{
			...named.body,
			name: "Named revision state",
		},
		true,
		named.revision,
	);
	const saved = await api.request<{ revision: number }>(
		"POST",
		`/api/v1/shows/${source.id}/revisions`,
		{ name: "Approved focus" },
	);
	const latest = await showObject(api, source.id, "group", "4");
	await api.request(
		"PUT",
		`/api/v1/shows/${source.id}/objects/group/4`,
		{
			...latest.body,
			name: "Newer autosave state",
		},
		true,
		latest.revision,
	);
	const destination = await loadCanonicalCopy(
		api,
		bench,
		`show-005-destination-${surface}`,
	);
	const destinationEntry = await showEntry(api, destination.id);
	const destinationGroup = await showObject(api, destination.id, "group", "4");
	await api.request(
		"PUT",
		`/api/v1/shows/${destination.id}/objects/group/4`,
		{
			...destinationGroup.body,
			name: "destination-before-overwrite",
		},
		true,
		destinationGroup.revision,
	);
	const destinationRevision = await api.request<{ revision: number }>(
		"POST",
		`/api/v1/shows/${destination.id}/revisions`,
		{ name: "Destination checkpoint" },
	);
	await api.request("POST", `/api/v1/shows/${source.id}/open`, {
		transition: "hold_current",
	});
	return {
		sourceId: source.id,
		sourceName: sourceEntry.name,
		savedRevision: saved.revision,
		destinationId: destination.id,
		destinationName: destinationEntry.name,
		destinationRevision: destinationRevision.revision,
	};
}

async function exerciseRevisionCopyApi(
	{ api, bench }: ApiBenchContext,
	state: RevisionCopyState,
): Promise<void> {
	const copy = await api.request<RevisionShow>(
		"POST",
		`/api/v1/shows/${state.sourceId}/revisions/${state.savedRevision}/open`,
		{ transition: "hold_current" },
	);
	expect(copy.id).not.toBe(state.sourceId);
	expect(copy.name).toMatch(
		new RegExp(
			`^${escapeRegex(state.sourceName)}-rev-${state.savedRevision}-\\d{4}-\\d{2}-\\d{2}`,
		),
	);
	expect(copy.revision_copy).toMatchObject({
		show_id: state.sourceId,
		show_name: state.sourceName,
		revision: state.savedRevision,
		revision_name: "Approved focus",
	});

	const collision = await api.request<RevisionShow>(
		"POST",
		`/api/v1/shows/${state.sourceId}/revisions/${state.savedRevision}/open`,
		{ transition: "hold_current" },
	);
	expect(collision.id).not.toBe(copy.id);
	expect(collision.name).not.toBe(copy.name);
	await api.request("POST", `/api/v1/shows/${copy.id}/open`, {
		transition: "hold_current",
	});

	const copyGroup = await showObject(api, copy.id, "group", "4");
	await api.request(
		"PUT",
		`/api/v1/shows/${copy.id}/objects/group/4`,
		{
			...copyGroup.body,
			name: "Copy-only edit",
		},
		true,
		copyGroup.revision,
	);
	await api.request("POST", `/api/v1/shows/${copy.id}/revisions`, {
		name: "Copy checkpoint",
	});
	await bench.stopServerGracefully(api.session!.token);
	await bench.startServer();
	await api.login();

	state.copyId = copy.id;
	state.copyProvenance = copy.revision_copy;
	state.expectedSourceName = "Newer autosave state";
	state.expectedCopyName = "Copy-only edit";
	state.expectedCopyRevisions = ["Copy checkpoint"];
}

async function openApprovedRevisionCopy(
	{ api, bench, desk, page }: UiContext,
	state: RevisionCopyState,
): Promise<{ copy: RevisionShow; showMenu: Locator }> {
	await desk.open(bench.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Load", exact: true }).click();
	let sourceCard = page
		.locator(".revision-show-library article")
		.filter({ has: page.getByText(state.sourceName, { exact: true }) });
	await sourceCard
		.getByRole("button", { name: "Load Latest Autosave" })
		.click();
	await expect
		.poll(
			async () =>
				(await showObject(api, state.sourceId, "group", "4")).body.name,
		)
		.toBe("Newer autosave state");

	await page.getByRole("button", { name: "Load", exact: true }).click();
	sourceCard = page
		.locator(".revision-show-library article")
		.filter({ has: page.getByText(state.sourceName, { exact: true }) });
	const revisionAction = sourceCard
		.locator(".named-revision-list button")
		.filter({ hasText: "Approved focus" });
	await expect(revisionAction).toContainText("Load Revision as Copy");
	await revisionAction.click();

	await expect(page.locator(".dock-identity b")).toContainText("Revision Copy");
	await expect(
		page.getByRole("dialog", { name: "Load show", exact: true }),
	).toBeHidden();
	const copy = (
		await api.request<RevisionBootstrap>(
			"GET",
			"/api/v1/bootstrap",
			undefined,
			false,
		)
	).active_show;
	expect(copy.id).not.toBe(state.sourceId);
	const displayedCopiedAt = await page.evaluate(
		(copiedAt) => new Date(copiedAt).toLocaleString(),
		copy.revision_copy.copied_at,
	);
	await expect(page.locator(".dock-identity")).toHaveAttribute(
		"title",
		new RegExp(
			`Source: ${escapeRegex(state.sourceName)}, Revision ${state.savedRevision} · Approved focus\\. Created ${escapeRegex(displayedCopiedAt)}`,
		),
	);
	const showMenu = page.getByRole("dialog", { name: "Show", exact: true });
	await expect(showMenu).toContainText(
		`Revision ${state.savedRevision} · Approved focus`,
	);
	await expect(showMenu).toContainText(`Created ${displayedCopiedAt}`);
	await expect(showMenu).toContainText(
		`autosaved to this copy, not to ${state.sourceName}`,
	);

	const copyGroup = await showObject(api, copy.id, "group", "4");
	await api.request(
		"PUT",
		`/api/v1/shows/${copy.id}/objects/group/4`,
		{
			...copyGroup.body,
			name: "Copy-only edit",
		},
		true,
		copyGroup.revision,
	);
	await api.request("POST", `/api/v1/shows/${copy.id}/revisions`, {
		name: "Copy checkpoint",
	});
	return { copy, showMenu };
}

async function restartAndRenameRevisionCopy(
	{ api, bench, desk, page }: UiContext,
	state: RevisionCopyState,
	copy: RevisionShow,
	showMenu: Locator,
): Promise<RevisionShow> {
	await showMenu.getByRole("button", { name: "Load", exact: true }).click();
	const destinationCard = page
		.locator(".revision-show-library article")
		.filter({ has: page.getByText(state.destinationName, { exact: true }) });
	await destinationCard
		.getByRole("button", { name: "Load Latest Autosave" })
		.click();
	await page.getByRole("button", { name: "Load", exact: true }).click();
	const copyCard = page
		.locator(".revision-show-library article")
		.filter({ has: page.getByText(copy.name, { exact: true }) });
	await copyCard.getByRole("button", { name: "Load Latest Autosave" }).click();
	await expect
		.poll(
			async () =>
				(
					await api.request<RevisionBootstrap>(
						"GET",
						"/api/v1/bootstrap",
						undefined,
						false,
					)
				).active_show.id,
		)
		.toBe(copy.id);
	expect((await showObject(api, copy.id, "group", "4")).body.name).toBe(
		"Copy-only edit",
	);

	await bench.stopServerGracefully(api.session!.token);
	await bench.startServer();
	await api.login();
	await desk.open(bench.baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Load", exact: true }).click();
	const restartedCopyCard = page
		.locator(".revision-show-library article")
		.filter({ has: page.getByText(copy.name, { exact: true }) });
	await restartedCopyCard
		.getByRole("button", { name: "Load Latest Autosave" })
		.click();
	await expect
		.poll(
			async () =>
				(
					await api.request<RevisionBootstrap>(
						"GET",
						"/api/v1/bootstrap",
						undefined,
						false,
					)
				).active_show.id,
		)
		.toBe(copy.id);
	expect(
		(
			await api.request<NamedRevision[]>(
				"GET",
				`/api/v1/shows/${copy.id}/revisions`,
			)
		).map((entry) => entry.name),
	).toEqual(["Copy checkpoint"]);

	const restartedShowMenu = page.getByRole("dialog", {
		name: "Show",
		exact: true,
	});
	await restartedShowMenu.getByRole("button", { name: "Save As" }).click();
	const newName = `show-005-independent-${crypto.randomUUID()}`;
	const saveAs = page.getByRole("dialog", { name: "Save show" });
	await saveAs.getByRole("textbox", { name: "Show name" }).fill(newName);
	await saveAs.getByRole("button", { name: "Save as New Show" }).click();
	await expect
		.poll(
			async () =>
				(
					await api.request<RevisionBootstrap>(
						"GET",
						"/api/v1/bootstrap",
						undefined,
						false,
					)
				).active_show.name,
		)
		.toBe(newName);
	const renamedCopy = (
		await api.request<RevisionBootstrap>(
			"GET",
			"/api/v1/bootstrap",
			undefined,
			false,
		)
	).active_show;
	expect(renamedCopy.id).not.toBe(copy.id);
	expect(renamedCopy.revision_copy).toEqual(copy.revision_copy);
	expect((await showObject(api, renamedCopy.id, "group", "4")).body.name).toBe(
		"Copy-only edit",
	);
	return renamedCopy;
}

async function overwriteDestinationFromRevisionCopy(
	{ api, bench, page }: UiContext,
	state: RevisionCopyState,
	copy: RevisionShow,
): Promise<void> {
	await page.getByRole("button", { name: "Save As", exact: true }).click();
	let overwriteDestination = page
		.locator(".overwrite-destination-list article")
		.filter({ has: page.getByText(state.destinationName, { exact: true }) });
	await overwriteDestination
		.getByRole("button", { name: "Choose Destination" })
		.click();
	let confirmation = page.getByRole("alertdialog", {
		name: new RegExp(`Confirm overwrite ${escapeRegex(state.destinationName)}`),
	});
	await expect(confirmation).toContainText(
		`Replace ${state.destinationName} Latest Autosave?`,
	);
	await confirmation.getByRole("button", { name: "Cancel" }).click();
	expect(
		(await showObject(api, state.destinationId, "group", "4")).body.name,
	).toBe("destination-before-overwrite");

	await page.getByRole("button", { name: "Save As", exact: true }).click();
	overwriteDestination = page
		.locator(".overwrite-destination-list article")
		.filter({ has: page.getByText(state.destinationName, { exact: true }) });
	await overwriteDestination
		.getByRole("button", { name: "Choose Destination" })
		.click();
	confirmation = page.getByRole("alertdialog", {
		name: new RegExp(`Confirm overwrite ${escapeRegex(state.destinationName)}`),
	});
	await confirmation
		.getByRole("button", {
			name: new RegExp(
				`Replace ${escapeRegex(state.destinationName)} Latest Autosave`,
			),
		})
		.click();
	await expect
		.poll(
			async () =>
				(await showObject(api, state.destinationId, "group", "4")).body.name,
		)
		.toBe("Copy-only edit");
	const destinationAfter = await showEntry(api, state.destinationId);
	expect(destinationAfter.name).toBe(state.destinationName);
	expect(
		(
			await api.request<NamedRevision[]>(
				"GET",
				`/api/v1/shows/${state.destinationId}/revisions`,
			)
		).map((entry) => [entry.revision, entry.name]),
	).toEqual([[state.destinationRevision, "Destination checkpoint"]]);

	const backupDirectory = `${bench.dataDir}/backups`;
	const backupNames = (await fs.readdir(backupDirectory))
		.filter(
			(name) =>
				name.startsWith(`${state.destinationName}-`) && name.endsWith(".show"),
		)
		.sort();
	expect(backupNames.length).toBeGreaterThan(0);
	const backup = `${backupDirectory}/${backupNames.at(-1)}`;
	expect(
		await readSql(backup, "SELECT value FROM metadata WHERE key='show_id'"),
	).toBe(state.destinationId);
	expect(
		await readSql(backup, "SELECT value FROM metadata WHERE key='name'"),
	).toBe(state.destinationName);
	expect(
		await readSql(
			backup,
			"SELECT json_extract(body_json,'$.name')||'|'||revision FROM objects WHERE kind='group' AND id='4'",
		),
	).toMatch(/^destination-before-overwrite\|\d+$/);
	expect((await showObject(api, state.sourceId, "group", "4")).body.name).toBe(
		"Newer autosave state",
	);
	expect((await showObject(api, copy.id, "group", "4")).body.name).toBe(
		"Copy-only edit",
	);
}

async function overwriteOriginalFromRevisionCopy(
	{ api, page }: UiContext,
	state: RevisionCopyState,
	renamedCopy: RevisionShow,
): Promise<void> {
	await page
		.getByRole("dialog", { name: "Show", exact: true })
		.getByRole("button", { name: "Save", exact: true })
		.click();
	const manualSave = page.getByRole("dialog", { name: "Save revision copy" });
	await expect(
		manualSave.getByRole("button", { name: "Keep as Separate Show" }),
	).toBeVisible();
	await expect(
		manualSave.getByRole("button", { name: "Overwrite Original Show" }),
	).toBeVisible();
	await manualSave
		.getByRole("button", { name: "Overwrite Original Show" })
		.click();
	const confirmation = page.getByRole("alertdialog", {
		name: new RegExp(`Confirm overwrite ${escapeRegex(state.sourceName)}`),
	});
	await expect(confirmation).toContainText(
		"identity and named revisions are preserved",
	);
	await confirmation.getByRole("button", { name: "Cancel" }).click();
	expect((await showObject(api, state.sourceId, "group", "4")).body.name).toBe(
		"Newer autosave state",
	);

	await page
		.getByRole("dialog", { name: "Show", exact: true })
		.getByRole("button", { name: "Save", exact: true })
		.click();
	await page
		.getByRole("dialog", { name: "Save revision copy" })
		.getByRole("button", { name: "Overwrite Original Show" })
		.click();
	await page
		.getByRole("alertdialog")
		.getByRole("button", {
			name: new RegExp(
				`Replace ${escapeRegex(state.sourceName)} Latest Autosave`,
			),
		})
		.click();
	await expect
		.poll(
			async () =>
				(await showObject(api, state.sourceId, "group", "4")).body.name,
		)
		.toBe("Copy-only edit");

	state.copyId = renamedCopy.id;
	state.copyProvenance = renamedCopy.revision_copy;
	state.expectedSourceName = "Copy-only edit";
	state.expectedCopyName = "Copy-only edit";
	state.expectedCopyRevisions = [];
}

async function exerciseRevisionCopyUi(
	context: UiContext,
	state: RevisionCopyState,
): Promise<void> {
	const { copy, showMenu } = await openApprovedRevisionCopy(context, state);
	const renamedCopy = await restartAndRenameRevisionCopy(
		context,
		state,
		copy,
		showMenu,
	);
	await overwriteDestinationFromRevisionCopy(context, state, copy);
	await overwriteOriginalFromRevisionCopy(context, state, renamedCopy);
}

async function assertRevisionCopy(
	{ api }: { api: ApiDriver },
	state: RevisionCopyState,
): Promise<void> {
	expect(state.copyId).toBeTruthy();
	expect(state.copyProvenance).toMatchObject({
		show_id: state.sourceId,
		show_name: state.sourceName,
		revision: state.savedRevision,
		revision_name: "Approved focus",
	});
	expect((await showObject(api, state.sourceId, "group", "4")).body.name).toBe(
		state.expectedSourceName,
	);
	expect((await showObject(api, state.copyId!, "group", "4")).body.name).toBe(
		state.expectedCopyName,
	);
	expect(
		(
			await api.request<NamedRevision[]>(
				"GET",
				`/api/v1/shows/${state.sourceId}/revisions`,
			)
		).map((entry) => entry.name),
	).toEqual(["Approved focus"]);
	expect(
		(
			await api.request<NamedRevision[]>(
				"GET",
				`/api/v1/shows/${state.copyId}/revisions`,
			)
		).map((entry) => entry.name),
	).toEqual(state.expectedCopyRevisions);
	expect(
		(
			await api.request<RevisionBootstrap>(
				"GET",
				"/api/v1/bootstrap",
				undefined,
				false,
			)
		).active_show.id,
	).toBe(state.copyId);
	expect(
		(
			await api.request<Array<{ id: string }>>(
				"GET",
				"/api/v1/shows",
				undefined,
				false,
			)
		).some((entry) => entry.id === state.copyId),
	).toBe(true);
}

export function registerRevisionCopyScenario(): void {
	pairedScenario<RevisionCopyState>({
		id: "SHOW-005",
		title: "named revisions load as durable, visibly independent copies",
		arrange: arrangeRevisionCopy,
		api: exerciseRevisionCopyApi,
		ui: exerciseRevisionCopyUi,
		assert: assertRevisionCopy,
	});
}
