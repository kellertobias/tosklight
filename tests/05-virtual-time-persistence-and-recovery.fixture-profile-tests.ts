import { expect, test } from "../apps/control-ui/e2e/bench/fixtures";
import {
	fixtureProfileMigrationSnapshot,
	fixtureProfiles,
	fixtureProfileWarnings,
	fixtureWarningSnapshot,
	insertLegacyFixtureRows,
	type LegacyFixtureRow,
	legacyDimmerDefinition,
	legacyFixtureRow,
	transferableProfileSnapshot,
} from "./05-virtual-time-persistence-and-recovery.fixture-profile-helpers";
import {
	readSql,
	runSql,
	sqlString,
} from "./05-virtual-time-persistence-and-recovery.show-helpers";

export function registerFreshProfileStartupTest(): void {
	test("FIXTURE-001 @restart › supplemental fresh startup installs transferable profiles once with stable IDs", async ({
		api,
		bench,
	}) => {
		const initial = transferableProfileSnapshot(await fixtureProfiles(api));
		expect(initial.length).toBeGreaterThan(0);
		expect(new Set(initial.map((profile) => profile.id)).size).toBe(
			initial.length,
		);
		expect(
			initial.every(
				(profile) => profile.revision === 1 && profile.reservedSource === null,
			),
		).toBe(true);

		await bench.stopServerGracefully(api.session!.token);
		await bench.startServer();
		await api.login();

		expect(transferableProfileSnapshot(await fixtureProfiles(api))).toEqual(
			initial,
		);
		const database = `${bench.dataDir}/fixtures.sqlite`;
		expect(
			Number(
				await readSql(
					database,
					"SELECT COUNT(DISTINCT id) FROM fixture_profiles",
				),
			),
		).toBe(initial.length);
		expect(
			Number(
				await readSql(
					database,
					"SELECT COUNT(*) FROM fixture_profile_migration_failures",
				),
			),
		).toBe(0);
	});
}

export function registerCompatibleProfileMigrationTest(): void {
	test("FIXTURE-001 @restart › supplemental compatible schema-v1 modes migrate on real startup and retain exact sources idempotently", async ({
		api,
		bench,
	}) => {
		const dimmerModes = [
			await legacyDimmerDefinition(api, 1),
			await legacyDimmerDefinition(api, 2),
		];
		const family = `Legacy startup ${crypto.randomUUID()}`;
		const rows: LegacyFixtureRow[] = dimmerModes.map((definition, index) => ({
			definition: {
				...definition,
				id: crypto.randomUUID(),
				revision: 1,
				schema_version: 1,
				manufacturer: "E2E Legacy",
				name: family,
				model: family,
				mode: index === 0 ? "Coarse" : "Fine",
				profile_id: null,
				mode_id: null,
				profile_snapshot: null,
			},
			source: Buffer.from(`retained-compatible-gdtf-${index}`),
		}));
		const expectedProfileId = rows[0].definition.id;
		const database = `${bench.dataDir}/fixtures.sqlite`;

		await bench.stopServerGracefully(api.session!.token);
		await insertLegacyFixtureRows(database, rows);
		await bench.startServer();
		await api.login();

		expect(
			await api.request<any>("GET", "/api/v1/readiness", undefined, false),
		).toMatchObject({ status: "ready", recovery_mode: false });
		const migrated = (await fixtureProfiles(api)).find(
			(profile) =>
				profile.manufacturer === "E2E Legacy" && profile.name === family,
		);
		expect(migrated).toMatchObject({
			id: expectedProfileId,
			revision: 1,
			schema_version: 2,
			reserved_source: null,
		});
		expect(migrated.modes.map((mode: any) => mode.name)).toEqual([
			"Coarse",
			"Fine",
		]);

		await bench.stopServerGracefully(api.session!.token);
		for (const row of rows) {
			expect(await legacyFixtureRow(database, row.definition.id)).toEqual({
				json: JSON.stringify(row.definition),
				sourceHex: row.source.toString("hex").toUpperCase(),
			});
		}
		expect(
			Number(
				await readSql(
					database,
					`SELECT COUNT(*) FROM fixture_profile_legacy_map WHERE profile_id=${sqlString(expectedProfileId)} AND profile_revision=1`,
				),
			),
		).toBe(2);
		const firstSnapshot = await fixtureProfileMigrationSnapshot(
			database,
			expectedProfileId,
		);

		await bench.startServer();
		await api.login();
		const reopened = (await fixtureProfiles(api)).find(
			(profile) => profile.id === expectedProfileId,
		);
		expect(reopened).toEqual(migrated);
		await bench.stopServerGracefully(api.session!.token);
		expect(
			await fixtureProfileMigrationSnapshot(database, expectedProfileId),
		).toBe(firstSnapshot);
		await bench.startServer();
		await api.login();
	});
}

export function registerProfileRecoveryTests(): void {
	test("FIXTURE-001 @restart › supplemental malformed and conflicting schema-v1 rows keep startup ready with retained evidence and stable warnings", async ({
		api,
		bench,
	}) => {
		const base = await legacyDimmerDefinition(api, 1);
		const family = `Conflicting startup ${crypto.randomUUID()}`;
		const conflictingRows: LegacyFixtureRow[] = [0, 1].map((index) => ({
			definition: {
				...base,
				id: crypto.randomUUID(),
				revision: 1,
				schema_version: 1,
				manufacturer: "E2E Recovery",
				name: family,
				model: family,
				mode: index === 0 ? "Narrow" : "Wide",
				physical: {
					...base.physical,
					width_millimetres: index === 0 ? 250 : 500,
				},
				profile_id: null,
				mode_id: null,
				profile_snapshot: null,
			},
			source: Buffer.from(`retained-conflict-gdtf-${index}`),
		}));
		const malformedId = crypto.randomUUID();
		const malformedJson = "{";
		const malformedSource = Buffer.from("retained-malformed-gdtf");
		const database = `${bench.dataDir}/fixtures.sqlite`;

		await bench.stopServerGracefully(api.session!.token);
		await insertLegacyFixtureRows(database, conflictingRows);
		await runSql(
			database,
			`INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(${sqlString(malformedId)},1,'Broken','Broken','Broken',${sqlString(malformedJson)},X'${malformedSource.toString("hex")}')`,
		);
		await bench.startServer();
		await api.login();

		expect(
			await api.request<any>("GET", "/api/v1/readiness", undefined, false),
		).toMatchObject({ status: "ready", recovery_mode: false });
		const warnings = await fixtureProfileWarnings(api);
		expect(
			warnings.some(
				(warning) =>
					warning.includes(malformedId) &&
					warning.includes("could not be migrated") &&
					warning.includes("original definition and GDTF source were retained"),
			),
		).toBe(true);
		expect(
			warnings.some(
				(warning) =>
					warning.includes("E2E Recovery") &&
					warning.includes(family) &&
					warning.includes("conflicting fixture-level metadata") &&
					warning.includes("retained as separate profiles"),
			),
		).toBe(true);
		const recoveryProfiles = (await fixtureProfiles(api)).filter(
			(profile) =>
				profile.manufacturer === "E2E Recovery" && profile.name === family,
		);
		expect(recoveryProfiles).toHaveLength(2);
		expect(bench.recentLog()).toContain(
			"fixture library migration requires operator attention",
		);
		expect(bench.recentLog()).toContain(malformedId);

		await bench.stopServerGracefully(api.session!.token);
		expect(await legacyFixtureRow(database, malformedId)).toEqual({
			json: malformedJson,
			sourceHex: malformedSource.toString("hex").toUpperCase(),
		});
		for (const row of conflictingRows) {
			expect(await legacyFixtureRow(database, row.definition.id)).toEqual({
				json: JSON.stringify(row.definition),
				sourceHex: row.source.toString("hex").toUpperCase(),
			});
		}
		const failure = await readSql(
			database,
			`SELECT hex(error) FROM fixture_profile_migration_failures WHERE legacy_id=${sqlString(malformedId)} AND legacy_revision=1`,
		);
		expect(failure).not.toBe("");
		const warningSnapshot = await fixtureWarningSnapshot(
			database,
			family,
			malformedId,
		);

		await bench.startServer();
		await api.login();
		expect(await fixtureProfileWarnings(api)).toEqual(warnings);
		expect(
			(await fixtureProfiles(api)).filter(
				(profile) =>
					profile.manufacturer === "E2E Recovery" && profile.name === family,
			),
		).toEqual(recoveryProfiles);
		await bench.stopServerGracefully(api.session!.token);
		expect(await fixtureWarningSnapshot(database, family, malformedId)).toBe(
			warningSnapshot,
		);
		expect(
			await readSql(
				database,
				`SELECT hex(error) FROM fixture_profile_migration_failures WHERE legacy_id=${sqlString(malformedId)} AND legacy_revision=1`,
			),
		).toBe(failure);
		await bench.startServer();
		await api.login();
	});
}
