import fs from "node:fs/promises";
import type { FixtureProfile } from "../../apps/light-desktop/src/api/types";
import type { ApiDriver } from "../bench/core/api";
import { PLANNED_DEMO_FIXTURES } from "./plannedDemoManifest";

const PLANNED_DEMO_PACKAGES = [
	...new Map(
		PLANNED_DEMO_FIXTURES.map((fixture) => [
			fixture.profile.archive,
			fixture.profile,
		]),
	).values(),
];

export async function ensurePlannedDemoFixtureLibrary(api: ApiDriver) {
	let profiles = (await api.fixtureLibrarySnapshot())
		.profiles as FixtureProfile[];
	for (const fixtureProfile of PLANNED_DEMO_PACKAGES) {
		const installed = profiles.some(
			(candidate) =>
				candidate.manufacturer === fixtureProfile.manufacturer &&
				candidate.name === fixtureProfile.name,
		);
		if (installed) continue;
		const bytes = await fs.readFile(
			new URL(
				`../../assets/fixture-library/${fixtureProfile.archive}`,
				import.meta.url,
			),
		);
		await api.importFixturePackage(bytes);
		profiles = (await api.fixtureLibrarySnapshot())
			.profiles as FixtureProfile[];
	}
}
