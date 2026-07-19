import type { ApiDriver } from "../apps/control-ui/e2e/bench/api";
import { expect } from "../apps/control-ui/e2e/bench/fixtures";
import {
	readSql,
	runSql,
	sqlString,
} from "./05-virtual-time-persistence-and-recovery.show-helpers";

export type LegacyFixtureRow = {
	definition: Record<string, any>;
	source: Buffer;
};

export async function fixtureProfiles(api: ApiDriver): Promise<any[]> {
	return api.request<any[]>(
		"GET",
		"/api/v1/fixture-profiles",
		undefined,
		false,
	);
}

export async function fixtureProfileWarnings(
	api: ApiDriver,
): Promise<string[]> {
	return api.request<string[]>(
		"GET",
		"/api/v1/fixture-profiles/warnings",
		undefined,
		false,
	);
}

export function transferableProfileSnapshot(profiles: any[]): any[] {
	return profiles
		.map((profile) => ({
			id: profile.id,
			revision: profile.revision,
			manufacturer: profile.manufacturer,
			name: profile.name,
			reservedSource: profile.reserved_source,
			modes: profile.modes.map((mode: any) => ({
				id: mode.id,
				name: mode.name,
			})),
		}))
		.sort((left, right) => left.id.localeCompare(right.id));
}

export async function legacyDimmerDefinition(
	api: ApiDriver,
	footprint: number,
): Promise<Record<string, any>> {
	const bootstrap = await api.request<any>(
		"GET",
		"/api/v1/bootstrap",
		undefined,
		false,
	);
	const fixtures = await api.request<any[]>(
		"GET",
		`/api/v1/shows/${bootstrap.active_show.id}/objects/patched_fixture`,
		undefined,
		false,
	);
	const source = fixtures.find(
		(entry) => entry.body?.definition?.heads?.[0]?.parameters?.[0],
	);
	expect(source).toBeDefined();
	const definition = structuredClone(source.body.definition);
	const parameter = definition.heads[0].parameters[0];
	return {
		...definition,
		schema_version: 1,
		footprint,
		heads: [
			{
				...definition.heads[0],
				parameters: [
					{
						...parameter,
						components: Array.from({ length: footprint }, (_, offset) => ({
							...parameter.components[0],
							offset,
						})),
					},
				],
			},
		],
		profile_id: null,
		mode_id: null,
		profile_snapshot: null,
	};
}

export async function insertLegacyFixtureRows(
	database: string,
	rows: LegacyFixtureRow[],
): Promise<void> {
	await runSql(
		database,
		rows
			.map((row) => {
				const definition = row.definition;
				return `INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(${sqlString(definition.id)},${Number(definition.revision)},${sqlString(definition.manufacturer)},${sqlString(definition.model)},${sqlString(definition.mode)},${sqlString(JSON.stringify(definition))},X'${row.source.toString("hex")}')`;
			})
			.join(";"),
	);
}

export async function legacyFixtureRow(
	database: string,
	id: string,
): Promise<{ json: string; sourceHex: string }> {
	const encoded = await readSql(
		database,
		`SELECT hex(definition_json)||'|'||COALESCE(hex(source_gdtf),'') FROM fixture_definitions WHERE id=${sqlString(id)} AND revision=1`,
	);
	const [jsonHex, sourceHex] = encoded.split("|");
	return { json: Buffer.from(jsonHex, "hex").toString("utf8"), sourceHex };
}

export async function fixtureProfileMigrationSnapshot(
	database: string,
	profileId: string,
): Promise<string> {
	return readSql(
		database,
		`SELECT hex(profile_json)||':'||(SELECT COUNT(*) FROM fixture_profile_legacy_map WHERE profile_id=p.id AND profile_revision=p.revision)||':'||(SELECT COUNT(*) FROM fixture_profile_legacy_sources WHERE profile_id=p.id AND profile_revision=p.revision) FROM fixture_profiles p WHERE p.id=${sqlString(profileId)} AND p.revision=1`,
	);
}

export async function fixtureWarningSnapshot(
	database: string,
	family: string,
	malformedId: string,
): Promise<string> {
	return readSql(
		database,
		`SELECT group_concat(hex(message),'|') FROM (SELECT message FROM fixture_library_warnings WHERE message LIKE ${sqlString(`%${family}%`)} OR message LIKE ${sqlString(`%${malformedId}%`)} ORDER BY message)`,
	);
}
