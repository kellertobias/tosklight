import type { ApiDriver } from "../bench/core/api";
import {
	type DemoFamily,
	plannedDemoFamilyNumbers,
	plannedDemoRoleNumbers,
} from "./plannedDemoManifest";
import { putPlannedDemoObject } from "./plannedDemoObjects";

interface PatchedTargetFixture {
	fixture_id: string;
	fixture_number: number | null;
	logical_heads?: Array<{ fixture_id: string }>;
}

const GROUP_MASTER_PLAYBACKS: Readonly<Record<string, number>> = {
	"Show Profile Odd": 1,
	"Show Profile Even": 2,
	"Show LED": 3,
	"Show Wash": 4,
	"All ACLs": 5,
	Blinders: 6,
};

export interface PlannedDemoGroupSpec {
	id: string;
	name: string;
	fixtureNumbers: readonly number[];
}

export function plannedDemoGroupSpecs(): PlannedDemoGroupSpec[] {
	const specs: PlannedDemoGroupSpec[] = [];
	let number = 1;
	for (const [family, label] of [
		["profile", "Profile"],
		["wash", "Wash"],
		["led", "LED"],
	] as const) {
		specs.push(
			group(number++, `${label} All`, plannedDemoFamilyNumbers(family)),
			group(
				number++,
				`${label} Stage`,
				plannedDemoFamilyNumbers(family, "stage"),
			),
			group(
				number++,
				`${label} Audience`,
				plannedDemoFamilyNumbers(family, "audience"),
			),
			group(number++, `${label} Aux`, plannedDemoFamilyNumbers(family, "aux")),
		);
	}
	const showFamilies = [
		["profile", "Profile"],
		["wash", "Wash"],
		["led", "LED"],
	] as const;
	for (const [family, label] of showFamilies) {
		specs.push(
			group(number++, `Show ${label}`, showNumbers(family)),
			group(
				number++,
				`Aux Show ${label}`,
				plannedDemoFamilyNumbers(family, "aux"),
			),
		);
	}
	const show = showFamilies.flatMap(([family]) => showNumbers(family));
	const auxShow = showFamilies.flatMap(([family]) =>
		plannedDemoFamilyNumbers(family, "aux"),
	);
	specs.push(
		group(number++, "Show", show),
		group(number++, "Aux Show", auxShow),
	);
	for (const [family, label] of showFamilies) {
		const members = showNumbers(family);
		specs.push(
			group(
				number++,
				`Show ${label} Odd`,
				members.filter((_, index) => index % 2 === 0),
			),
			group(
				number++,
				`Show ${label} Even`,
				members.filter((_, index) => index % 2 === 1),
			),
		);
	}
	specs.push(
		group(31, "ACL 1", plannedDemoRoleNumbers("ACL 1")),
		group(32, "ACL 2", plannedDemoRoleNumbers("ACL 2")),
		group(33, "ACL 3", plannedDemoRoleNumbers("ACL 3")),
		group(34, "ACL 4", plannedDemoRoleNumbers("ACL 4")),
		group(35, "All ACLs", plannedDemoRoleNumbers("All ACLs")),
		group(36, "Blinders", plannedDemoRoleNumbers("Blinders")),
		group(37, "Front Lights", plannedDemoRoleNumbers("Front Lights")),
		group(38, "Front Center", [11, 12, 13]),
		group(39, "Follow Spots", plannedDemoRoleNumbers("Follow Spots")),
		group(40, "Sunstrips", plannedDemoRoleNumbers("Sunstrips")),
		group(41, "House Lights", plannedDemoRoleNumbers("House Lights")),
		group(42, "Hazers", plannedDemoRoleNumbers("Hazers")),
	);
	return specs;
}

export async function installPlannedDemoGroups(
	api: ApiDriver,
	showId: string,
	fixtures: readonly PatchedTargetFixture[],
) {
	const targetsByNumber = new Map(
		fixtures.flatMap((fixture) =>
			fixture.fixture_number == null
				? []
				: [[fixture.fixture_number, targetIds(fixture)] as const],
		),
	);
	const specs = plannedDemoGroupSpecs();
	for (const spec of specs) {
		const targets = spec.fixtureNumbers.flatMap((fixtureNumber) => {
			const found = targetsByNumber.get(fixtureNumber);
			if (!found)
				throw new Error(
					`Group ${spec.name} references missing fixture ${fixtureNumber}`,
				);
			return found;
		});
		await putPlannedDemoObject(api, showId, "group", spec.id, {
			id: spec.id,
			name: spec.name,
			fixtures: targets,
			color: null,
			icon: null,
			derived_from: null,
			frozen_from: null,
			programming: {},
			master: 1,
			playback_fader: GROUP_MASTER_PLAYBACKS[spec.name] ?? null,
		});
	}
	return specs;
}

function showNumbers(family: DemoFamily) {
	return [
		...plannedDemoFamilyNumbers(family, "stage"),
		...plannedDemoFamilyNumbers(family, "audience"),
	];
}

function group(
	id: number,
	name: string,
	fixtureNumbers: readonly number[],
): PlannedDemoGroupSpec {
	return { id: String(id), name, fixtureNumbers };
}

function targetIds(fixture: PatchedTargetFixture) {
	return fixture.logical_heads?.length
		? fixture.logical_heads.map((head) => head.fixture_id)
		: [fixture.fixture_id];
}
