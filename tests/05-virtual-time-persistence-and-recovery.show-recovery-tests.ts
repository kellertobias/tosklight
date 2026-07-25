import fs from "node:fs/promises";
import { expect, test } from "./bench/core/fixtures";
import { pairedScenario } from "./bench/core/pairedScenario";
import {
	arrangeMalformedRecovery,
	assertMigrationSnapshot,
	fileHash,
	hash,
	migrationSnapshot,
	prepareMigrationCase,
	runSql,
	SHOW_004_CASES,
	showEntry,
	stageLegacyMigration,
} from "./05-virtual-time-persistence-and-recovery.show-helpers";
import { loadCanonicalCopy, object, putObject } from "./support/catalog";

export function registerAtomicRecoveryTests(): void {
	for (const fault of [
		"before-atomic-replacement",
		"during-temporary-write",
		"after-replacement-before-cleanup",
	] as const) {
		test(`SHOW-002 @api @restart › ${fault} fixture recovers as one complete old or new SQLite revision`, async ({
			api,
			bench,
		}) => {
			const copy = await loadCanonicalCopy(api, bench, `show-002-${fault}`);
			let group = await object<any>(api, "group", "3");
			await putObject(
				api,
				"group",
				"3",
				{ ...group.body, name: "Atomic baseline" },
				group.revision,
			);
			await api.saveShowRevision(copy.id, "SHOW-002 baseline");
			const entry = await showEntry(api, copy.id);
			await bench.stopServerGracefully(api.session!.token);
			const oldBytes = await fs.readFile(entry.path);
			const oldHash = hash(oldBytes);

			await bench.startServer();
			await api.login();
			group = await object<any>(api, "group", "3");
			await putObject(
				api,
				"group",
				"3",
				{ ...group.body, name: "Atomic replacement" },
				group.revision,
			);
			await bench.stopServerGracefully(api.session!.token);
			const newBytes = await fs.readFile(entry.path);
			const newHash = hash(newBytes);
			expect(newHash).not.toBe(oldHash);
			await fs.writeFile(entry.path, oldBytes);

			const temporary = `${entry.path}.storage-fault.tmp`;
			const backup = `${entry.path}.storage-fault.backup`;
			await fs.writeFile(backup, oldBytes);
			if (fault === "during-temporary-write")
				await fs.writeFile(
					temporary,
					newBytes.subarray(0, Math.floor(newBytes.length / 2)),
				);
			if (fault === "after-replacement-before-cleanup")
				await fs.writeFile(entry.path, newBytes);

			await bench.startServer();
			await api.login();
			const recoveredHash = await fileHash(entry.path);
			const expectedNew = fault === "after-replacement-before-cleanup";
			expect(recoveredHash).toBe(expectedNew ? newHash : oldHash);
			expect((await object<any>(api, "group", "3")).body.name).toBe(
				expectedNew ? "Atomic replacement" : "Atomic baseline",
			);
			expect([oldHash, newHash]).toContain(recoveredHash);
			expect(
				(await api.request<any>("GET", "/api/v2/bootstrap", undefined, false))
					.active_show_error,
			).toBeNull();
			if (fault === "during-temporary-write")
				expect((await fs.stat(temporary)).size).toBeLessThan(newBytes.length);
		});
	}
}

export function registerMalformedRecoveryScenario(): void {
	pairedScenario<{
		damagedPath: string;
		damagedHash: string;
		damagedShowId: string;
		recoveryShowId: string;
		recoveryShowName: string;
	}>({
		id: "SHOW-003",
		title:
			"a malformed active show stays intact while the operator opens a valid recovery show",
		surfaces: ["api"],
		arrange: async ({ api, bench }, surface) =>
			arrangeMalformedRecovery(api, bench, surface),
		api: async ({ api }, state) => {
			await api.openShow(state.recoveryShowId, {
				transition: "safe_blackout",
			});
		},
		ui: async ({ bench, desk, page }, state) => {
			await desk.open(bench.baseUrl);
			const recovery = page.getByRole("alertdialog", {
				name: "Show recovery required",
			});
			await expect(recovery).toBeVisible();
			await expect(recovery).toContainText("has not been changed or deleted");
			await recovery
				.getByRole("button", {
					name: `Load Latest Autosave for ${state.recoveryShowName}`,
				})
				.click();
		},
		assert: async ({ api, bench }, state) => {
			await expect
				.poll(
					async () =>
						(
							await api.request<any>(
								"GET",
								"/api/v2/bootstrap",
								undefined,
								false,
							)
						).active_show?.id,
				)
				.toBe(state.recoveryShowId);
			const bootstrap = await api.request<any>(
				"GET",
				"/api/v2/bootstrap",
				undefined,
				false,
			);
			expect(bootstrap.active_show_error).toBeNull();
			expect(await fileHash(state.damagedPath)).toBe(state.damagedHash);
			const frame = await bench.tick(0);
			const universe = frame.universes.find(
				(candidate) => candidate.universe === 1,
			);
			expect(universe).toBeDefined();
			expect(universe!.slots.every((value) => value === 0)).toBe(true);
		},
	});
}

export function registerCorruptActiveShowRecoveryTests(): void {
	for (const corruption of [
		"malformed",
		"schema-invalid",
		"referentially-invalid",
	] as const) {
		test(`SHOW-003 @restart › supplemental ${corruption} active show starts ready in recovery and preserves corrupt evidence`, async ({
			api,
			bench,
		}) => {
			const copy = await loadCanonicalCopy(
				api,
				bench,
				`show-003-${corruption}`,
			);
			if (corruption === "referentially-invalid") {
				await putObject(api, "playback", "999", {
					number: 999,
					name: "Recovery reference",
					target: { type: "group", group_id: "1" },
					buttons: ["select", "select_dereferenced", "flash"],
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
			}
			const entry = await showEntry(api, copy.id);
			await bench.stopServerGracefully(api.session!.token);
			const validBytes = await fs.readFile(entry.path);
			if (corruption === "malformed") {
				await fs.writeFile(
					entry.path,
					Buffer.from("not a ToskLight SQLite show\n"),
				);
			} else if (corruption === "schema-invalid") {
				await runSql(
					entry.path,
					"UPDATE objects SET body_json=json_set(body_json, '$.master', 'not-a-number') WHERE kind='group' AND id='1'",
				);
			} else {
				await runSql(
					entry.path,
					"UPDATE objects SET body_json=json_set(body_json, '$.target', json('{\"type\":\"group\",\"group_id\":\"missing-group\"}')) WHERE kind='playback' AND id='999'",
				);
			}
			const corruptHash = await fileHash(entry.path);
			await bench.startServer();
			await api.login();

			const readinessResponse = await fetch(
				`${bench.baseUrl}/api/v2/readiness`,
			);
			expect(readinessResponse.ok).toBe(true);
			const readiness = (await readinessResponse.json()) as any;
			expect(readiness).toMatchObject({ status: "ready", recovery_mode: true });
			expect(readiness.active_show_error).toBeTruthy();
			const bootstrap = await api.request<any>(
				"GET",
				"/api/v2/bootstrap",
				undefined,
				false,
			);
			expect(bootstrap.active_show.id).toBe(copy.id);
			expect(bootstrap.active_show_error).toBeTruthy();
			const safe = await bench.tick(0);
			expect(
				safe.universes.every((universe) =>
					universe.slots.every((value) => value === 0),
				),
			).toBe(true);
			expect(await fileHash(entry.path)).toBe(corruptHash);

			const recovered = await api.createShow<{ id: string }>({
				name: `show-003-recovered-${corruption}-${crypto.randomUUID()}`,
				data_base64: validBytes.toString("base64"),
				overwrite: false,
			});
			await api.openShow(recovered.id, {
				transition: "safe_blackout",
			});
			expect(
				(await api.request<any>("GET", "/api/v2/bootstrap", undefined, false))
					.active_show_error,
			).toBeNull();
			expect(
				(await bench.tick(0)).universes.find(
					(universe) => universe.universe === 1,
				),
			).toBeDefined();
			expect(await fileHash(entry.path)).toBe(corruptHash);
		});
	}
}

export function registerLegacyMigrationTests(): void {
	for (const migration of SHOW_004_CASES) {
		// D3 (derive-only): the virtual-dimmer intensity parameter's metadata/capabilities are
		// re-derived at load, never written back — this case must leave the show file
		// byte-identical. Every other case normalizes the legacy fields with one write.
		const byteStable = migration === "virtual-dimmer-metadata";
		test(`SHOW-004 @api @restart › ${migration} legacy fields ${byteStable ? "self-heal by re-derivation and stay byte-stable" : "normalize once and stay byte/revision stable"}`, async ({
			api,
			bench,
		}) => {
			const prepared = await prepareMigrationCase(api, bench, migration);
			await bench.stopServerGracefully(api.session!.token);
			await stageLegacyMigration(
				prepared.entry.path,
				migration,
				prepared.cueListId,
			);
			const legacyHash = await fileHash(prepared.entry.path);

			await bench.startServer();
			await api.login();
			const migrated = await migrationSnapshot(
				api,
				migration,
				prepared.cueListId,
			);
			assertMigrationSnapshot(migration, migrated);
			await bench.stopServerGracefully(api.session!.token);
			const migratedHash = await fileHash(prepared.entry.path);
			if (byteStable) expect(migratedHash).toBe(legacyHash);
			else expect(migratedHash).not.toBe(legacyHash);

			await bench.startServer();
			await api.login();
			const reopened = await migrationSnapshot(
				api,
				migration,
				prepared.cueListId,
			);
			expect(reopened).toEqual(migrated);
			await bench.stopServerGracefully(api.session!.token);
			expect(await fileHash(prepared.entry.path)).toBe(migratedHash);
			await bench.startServer();
			await api.login();
		});
	}
}
