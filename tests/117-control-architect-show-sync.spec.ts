import fs from "node:fs/promises";
import path from "node:path";
import { ArchitectHarness } from "./bench/architect/architectHarness";
import type { ApiDriver } from "./bench/core/api";
import { expect, test } from "./bench/core/fixtures";

/**
 * docs/testing/31-control-architect-show-sync.md, end to end: a real desk and a headless Architect
 * running the Architect's own sync engine. The Architect steps go through `viz-sync-harness`, whose
 * every edit is one gesture, exactly as an Architect command is.
 */

const POLL = { timeout: 15_000, intervals: [50, 100, 250] };

async function deskObjects<T = Record<string, unknown>>(api: ApiDriver, showId: string, kind: string) {
	return api.request<{ show_revision: number; objects: Array<{ id: string; revision: number; body: T }> }>(
		"GET",
		`/api/v2/objects/${kind}`,
		undefined,
		true,
		undefined,
		{ showId },
	);
}

async function deskRevision(api: ApiDriver, showId: string): Promise<number> {
	return (await deskObjects(api, showId, "cad_annotation")).show_revision;
}

async function deskPatch(api: ApiDriver) {
	return api.request<{
		patch_revision: number;
		fixtures: Array<Record<string, any>>;
	}>("GET", "/api/v2/patch");
}

async function saveDeskLayer(api: ApiDriver, id: string, name: string, order: number) {
	const existing = (await deskObjects(api, (await activeShow(api))!, "patch_layer")).objects.find(
		(layer) => layer.id === id,
	);
	await api.request("POST", `/api/v2/patch/layers/${id}/update`, {
		request_id: crypto.randomUUID(),
		action: { type: "save", expected_revision: existing?.revision ?? 0, layer: { name, order } },
	});
}

async function architectFollowing(api: ApiDriver): Promise<boolean> {
	const readiness = await api.request<{ architect_sync_active: boolean }>(
		"GET",
		"/api/v2/readiness",
		undefined,
		false,
	);
	return readiness.architect_sync_active;
}

async function activeShow(api: ApiDriver): Promise<string | null> {
	const readiness = await api.request<{ active_show: string | null }>(
		"GET",
		"/api/v2/readiness",
		undefined,
		false,
	);
	return readiness.active_show;
}

function annotation(id: string, text: string) {
	return { put: { kind: "cad_annotation", id, body: { id, text, x: 0, y: 0 } } };
}

async function boundArchitect(bench: { baseUrl: string }, showId: string) {
	const architect = await ArchitectHarness.start();
	const document = path.join(architect.dataDir, "Tour.show");
	await architect.openFromDesk(bench.baseUrl, showId, document);
	await expect.poll(() => architect.phase(), POLL).toBe("synced");
	return { architect, document };
}

/** Counts whole-show reloads on the desk's own event stream while `body` runs. */
async function countShowReloads(baseUrl: string, token: string, body: () => Promise<void>): Promise<number> {
	const socket = new WebSocket(`${baseUrl.replace("http", "ws")}/api/v2/events`, [
		"light.events.v2",
		"light.v2",
		`light.token.${token}`,
	]);
	let reloads = 0;
	await new Promise<void>((resolve, reject) => {
		socket.onerror = () => reject(new Error("the desk event socket failed"));
		socket.onopen = () => {
			socket.send(JSON.stringify({ type: "subscribe", filter: {}, capacity: 1024, rate_limits: [] }));
		};
		socket.onmessage = (message) => {
			const text = String(message.data);
			if (text.includes('"type":"ready"')) resolve();
			if (/show_opened|show_overwritten|show_rolled_back/u.test(text)) reloads += 1;
		};
	});
	try {
		await body();
	} finally {
		socket.close();
	}
	return reloads;
}

test("ARCHITECT-SYNC-01 @api edits travel both ways without Save and survive restarting both applications", async ({
	api,
	bench,
	show,
}) => {
	test.setTimeout(60_000);
	const { architect, document } = await boundArchitect(bench, show.id);
	let restarted: ArchitectHarness | undefined;
	try {
		const fixture = show.fixtureIds[0];
		// The desk shows that an Architect is following its show.
		await expect.poll(() => architectFollowing(api), POLL).toBe(true);
		const before = await deskRevision(api, show.id);
		const originalX = (await deskPatch(api)).fixtures.find((candidate) => candidate.fixture_id === fixture)!
			.location.x;

		// One gesture: an annotation and a 0.5 m drag, as one transaction.
		const reloads = await countShowReloads(bench.baseUrl, show.session.token, async () => {
			await architect.gesture(annotation("note-1", "Check truss"), {
				move_fixture: { fixture_id: fixture, x: 500, y: 0, z: 0 },
			});

			await expect
				.poll(async () => (await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id), POLL)
				.toEqual(["note-1"]);
		});
		expect(reloads, "an Architect edit never reloads the desk's show").toBe(0);
		expect(await deskRevision(api, show.id), "one gesture is one desk commit").toBe(before + 1);
		const moved = (await deskPatch(api)).fixtures.find((candidate) => candidate.fixture_id === fixture)!;
		expect(moved.location.x).toBe(originalX + 500);
		await expect.poll(() => architect.phase(), POLL).toBe("synced");

		// Desk edits reach the Architect without reopening the document.
		await saveDeskLayer(api, "front", "Front Truss", 2);
		const patch = await deskPatch(api);
		const input = { ...patch.fixtures.find((candidate) => candidate.fixture_id === show.fixtureIds[1])! };
		input.split_patches = [{ split: 1, universe: 1, address: 101 }];
		await api.request(
			"POST",
			"/api/v2/patch/fixtures",
			{ request_id: crypto.randomUUID(), fixtures: [patchInput(input)], remove_fixture_ids: [] },
			true,
			patch.patch_revision,
		);
		await expect
			.poll(async () => (await architect.objects<{ name: string }>("patch_layer")).map((layer) => layer.body.name), POLL)
			.toContain("Front Truss");
		await expect
			.poll(async () => (await architect.fixtures()).find((candidate) => candidate.fixture_id === show.fixtureIds[1])
				?.split_patches[0]?.address, POLL)
			.toBe(101);
		expect((await architect.status()).snapshot_reads, "desk edits arrive as commits, not reloads").toBe(0);

		// Restart both applications: everything is still there, on both sides.
		await architect.kill();
		await expect.poll(() => architectFollowing(api), POLL).toBe(false);
		await bench.restart();
		await api.login();
		restarted = await ArchitectHarness.start(architect.dataDir);
		expect((await restarted.open(document)).bound).toBe(true);
		await expect.poll(() => restarted!.phase(), POLL).toBe("synced");
		expect((await restarted.objects("cad_annotation")).map((object) => object.id)).toEqual(["note-1"]);
		expect((await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id)).toEqual(["note-1"]);
		expect(
			(await restarted.fixtures()).find((candidate) => candidate.fixture_id === fixture)!.location.x,
		).toBe(originalX + 500);
	} finally {
		await architect.kill();
		await restarted?.kill();
	}
});

test("ARCHITECT-SYNC-02 @api independent edits both survive and a same-field conflict keeps both drafts", async ({
	api,
	bench,
	show,
}) => {
	await saveDeskLayer(api, "truss", "Truss", 1);
	const { architect } = await boundArchitect(bench, show.id);
	try {
		await architect.setOnline(false);
		await expect.poll(() => architect.phase(), POLL).toBe("offline");
		const [layer] = (await architect.objects<Record<string, unknown>>("patch_layer")).filter(
			(candidate) => candidate.id === "truss",
		);
		await architect.gesture({
			put: { kind: "patch_layer", id: "truss", body: { ...layer.body, name: "Back Truss", order: 4 } },
		});
		const offline = await architect.status();
		expect(offline.status?.state).toBe("offline");
		expect(offline.status?.savedToControl).toBe(false);
		expect(offline.status?.pending).toBe(1);
		await saveDeskLayer(api, "truss", "Front Truss", 1);

		await architect.setOnline(true);
		await expect.poll(async () => (await architect.conflicts()).length, POLL).toBe(1);
		const [conflict] = await architect.conflicts();
		expect(conflict).toMatchObject({ kind: "patch_layer", id: "truss", path: "/name", mine: "Back Truss", theirs: "Front Truss" });
		expect((await architect.status()).status?.state).toBe("conflict");
		const desk = (await deskObjects<{ name: string; order: number }>(api, show.id, "patch_layer")).objects.find(
			(candidate) => candidate.id === "truss",
		)!;
		expect(desk.body).toMatchObject({ name: "Front Truss", order: 4 });
		await expect
			.poll(async () => (await architect.objects<{ name: string; order: number }>("patch_layer")).find(
				(candidate) => candidate.id === "truss",
			)?.body, POLL)
			.toMatchObject({ name: "Front Truss", order: 4 });

		await architect.resolve(conflict.entry, "use_mine");
		await expect.poll(() => architect.phase(), POLL).toBe("synced");
		expect(await architect.conflicts()).toEqual([]);
		const resolved = (await deskObjects<{ name: string }>(api, show.id, "patch_layer")).objects.find(
			(candidate) => candidate.id === "truss",
		)!;
		expect(resolved.body.name).toBe("Back Truss");
		await expect
			.poll(async () => (await architect.objects<{ name: string }>("patch_layer")).find((candidate) => candidate.id === "truss")
				?.body.name, POLL)
			.toBe("Back Truss");
	} finally {
		await architect.kill();
	}
});

test("ARCHITECT-SYNC-03 @api offline edits survive a restart, apply once in order, and a lost reply is retried once", async ({
	api,
	bench,
	show,
}) => {
	const { architect, document } = await boundArchitect(bench, show.id);
	let restarted: ArchitectHarness | undefined;
	try {
		const before = await deskRevision(api, show.id);
		await architect.setOnline(false);
		for (const id of ["a", "b", "c"]) await architect.gesture(annotation(`note-${id}`, id));
		await architect.kill();
		await bench.stopServerAbruptly();

		restarted = await ArchitectHarness.start(architect.dataDir);
		expect((await restarted.open(document)).bound).toBe(true);
		await expect.poll(async () => (await restarted!.status()).status, POLL).toMatchObject({
			state: "offline",
			pending: 3,
			savedToControl: false,
		});
		expect((await restarted.objects("cad_annotation")).map((object) => object.id).sort()).toEqual([
			"note-a",
			"note-b",
			"note-c",
		]);

		await bench.startServer();
		await api.login();
		await expect.poll(() => restarted!.phase(), { ...POLL, timeout: 30_000 }).toBe("synced");
		expect((await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id).sort()).toEqual([
			"note-a",
			"note-b",
			"note-c",
		]);
		expect(await deskRevision(api, show.id), "each journaled gesture applied exactly once").toBe(before + 3);

		// The desk commits, the reply never arrives; the retry is answered as already applied.
		const beforeRetry = await deskRevision(api, show.id);
		await restarted.loseNextReply();
		await restarted.gesture(annotation("note-d", "d"));
		await expect.poll(() => restarted!.phase(), POLL).toBe("synced");
		expect(await deskRevision(api, show.id)).toBe(beforeRetry + 1);
	} finally {
		await architect.kill();
		await restarted?.kill();
	}
});

test("ARCHITECT-SYNC-04 @api a show switch holds edits Offline and Save As forks the show", async ({
	api,
	bench,
	show,
}) => {
	const { architect, document } = await boundArchitect(bench, show.id);
	try {
		const other = await api.createShow<{ id: string }>({ name: `Other-${crypto.randomUUID()}` });
		await api.openShow(other.id, { transition: "hold_current" });
		await architect.gesture(annotation("held", "held while another show is open"));
		await expect.poll(async () => (await architect.status()).status, POLL).toMatchObject({
			state: "offline",
			detail: expect.stringContaining("Show not active on Control"),
			savedToControl: false,
		});
		expect((await deskObjects(api, other.id, "cad_annotation")).objects).toEqual([]);

		await api.openShow(show.id, { transition: "hold_current" });
		await expect.poll(() => architect.phase(), { ...POLL, timeout: 30_000 }).toBe("synced");
		expect((await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id)).toEqual(["held"]);

		// Save As makes a different show: a new identity and no binding.
		const copy = path.join(path.dirname(document), "Tour copy.show");
		const original = (await architect.document()).show_id;
		const forked = await architect.saveAs(copy);
		expect(forked.opened.bound).toBe(false);
		expect((await architect.document()).show_id).not.toBe(original);
		await architect.gesture(annotation("copy-only", "never reaches the desk"));
		await new Promise((resolve) => setTimeout(resolve, 500));
		expect((await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id)).toEqual(["held"]);
	} finally {
		await architect.kill();
	}
});

test("ARCHITECT-SYNC-05 @api damaged journal, mirror and binding index recover without losing the document", async ({
	api,
	bench,
	show,
}) => {
	const { architect, document } = await boundArchitect(bench, show.id);
	const association = (await architect.status()).binding!.association_id;
	const directory = path.join(architect.dataDir, "show-sync", association);
	let reopened: ArchitectHarness | undefined;
	try {
		// A damaged journal: the unconfirmed edit is recovered from the document itself.
		await architect.setOnline(false);
		await architect.gesture(annotation("journal-lost", "survives a damaged journal"));
		await architect.kill();
		await damage(path.join(directory, "journal.sqlite"));
		reopened = await ArchitectHarness.start(architect.dataDir);
		await reopened.open(document);
		await expect.poll(async () => (await reopened!.status()).status?.state, POLL).toBe("error");
		expect((await reopened.status()).status?.detail).toContain("journal was damaged");
		await reopened.dismissError();
		await expect.poll(() => reopened!.phase(), POLL).toBe("synced");
		expect((await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id)).toEqual([
			"journal-lost",
		]);
		const asideFiles = await fs.readdir(directory);
		expect(asideFiles.some((file) => file.startsWith("journal.damaged-")), "the damaged file is kept").toBe(true);

		// A damaged mirror: rebuilt from the desk, and the pending edit still applies.
		await reopened.setOnline(false);
		await reopened.gesture(annotation("mirror-lost", "survives a damaged mirror"));
		await reopened.kill();
		await damage(path.join(directory, "mirror.sqlite"));
		reopened = await ArchitectHarness.start(architect.dataDir);
		await reopened.open(document);
		await expect.poll(async () => (await reopened!.status()).status?.state, POLL).toBe("error");
		await reopened.dismissError();
		await expect.poll(() => reopened!.phase(), POLL).toBe("synced");
		expect((await deskObjects(api, show.id, "cad_annotation")).objects.map((object) => object.id).sort()).toEqual([
			"journal-lost",
			"mirror-lost",
		]);
		expect(await reopened.conflicts()).toEqual([]);

		// A damaged binding index: the document opens standalone and says why; the file stays.
		await reopened.kill();
		const index = path.join(architect.dataDir, "show-sync", "index.json");
		await fs.writeFile(index, "{not json");
		reopened = await ArchitectHarness.start(architect.dataDir);
		const opened = await reopened.open(document);
		expect(opened.bound).toBe(false);
		expect(opened.notice).toContain("damaged");
		expect(await fs.readFile(index, "utf8")).toBe("{not json");
	} finally {
		await architect.kill();
		await reopened?.kill();
	}
});

test("ARCHITECT-SYNC-06 @api a standalone document sends nothing anywhere", async ({ api, show }) => {
	const architect = await ArchitectHarness.start();
	try {
		const document = path.join(architect.dataDir, "Standalone.show");
		expect((await architect.create(document, "Standalone")).bound).toBe(false);
		const before = await deskRevision(api, show.id);
		await architect.gesture(annotation("local", "local only"));
		expect((await architect.objects("cad_annotation")).map((object) => object.id)).toEqual(["local"]);
		expect((await architect.status()).bound).toBe(false);
		expect(await deskRevision(api, show.id)).toBe(before);
	} finally {
		await architect.kill();
	}
});

test("ARCHITECT-SYNC-07 @api a thousand Architect edits a minute keep the desk outputting without reloading", async ({
	api,
	bench,
	show,
}) => {
	test.setTimeout(120_000);
	const { architect } = await boundArchitect(bench, show.id);
	try {
		const diagnostics = () =>
			api.request<{ output: { frames_sent: number; deadline_misses: number } }>("GET", "/api/v2/diagnostics");
		const startOutput = (await diagnostics()).output;
		const running = api.request("POST", "/api/v2/test/clock/free-run", { millis: 20_000 }, false);
		const started = Date.now();
		const reloads = await countShowReloads(bench.baseUrl, show.session.token, async () => {
			for (let index = 0; index < 1_000; index += 1) {
				await architect.gesture(annotation(`burst-${index % 50}`, `edit ${index}`));
			}
			await expect.poll(() => architect.phase(), { ...POLL, timeout: 60_000 }).toBe("synced");
		});
		const elapsed = Date.now() - started;
		await running;
		const endOutput = (await diagnostics()).output;
		expect(elapsed, "1,000 edits reach the desk within a minute").toBeLessThan(60_000);
		expect(reloads, "no edit reloads the desk's show").toBe(0);
		expect((await architect.status()).snapshot_reads, "the Architect never re-reads the whole show").toBe(0);
		expect(endOutput.frames_sent - startOutput.frames_sent, "DMX output kept running").toBeGreaterThan(20 * 30);
		const annotations = (await deskObjects<{ text: string }>(api, show.id, "cad_annotation")).objects;
		expect(annotations).toHaveLength(50);
		expect(annotations.find((object) => object.id === "burst-49")?.body.text).toBe("edit 999");
	} finally {
		await architect.kill();
	}
});

async function damage(file: string): Promise<void> {
	for (const suffix of ["-wal", "-shm"]) await fs.rm(`${file}${suffix}`, { force: true });
	await fs.writeFile(file, "this is not a database");
}

/** The request shape of a projected fixture: the projection without its read-only fields. */
function patchInput(projection: Record<string, any>) {
	const { fixture_revision: _revision, logical_heads: _heads, freeze_targets: _freeze, ...input } = projection;
	return input;
}
