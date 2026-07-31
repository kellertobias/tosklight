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
	"Beam Show Odd": 1,
	"Beam Show Even": 2,
	"LED Show": 3,
	"Wash Show": 4,
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
	for (const [row, family, label] of [
		[0, "profile", "Beam"],
		[1, "wash", "Wash"],
		[2, "led", "LED"],
	] as const) {
		const first = row * 7 + 1;
		const show = showNumbers(family);
		specs.push(
			group(first, `${label} Stage`, plannedDemoFamilyNumbers(family, "stage")),
			group(
				first + 1,
				`${label} Audience`,
				plannedDemoFamilyNumbers(family, "audience"),
			),
			group(
				first + 2,
				`${label} Auxiliary`,
				plannedDemoFamilyNumbers(family, "aux"),
			),
			group(first + 3, `${label} Show`, show),
			group(
				first + 4,
				`${label} Auxiliary Show`,
				plannedDemoFamilyNumbers(family, "aux"),
			),
			group(
				first + 5,
				`${label} Show Odd`,
				show.filter((_, index) => index % 2 === 0),
			),
			group(
				first + 6,
				`${label} Show Even`,
				show.filter((_, index) => index % 2 === 1),
			),
		);
	}
	return [
		...specs,
		group(22, "ACL1", plannedDemoRoleNumbers("ACL 1")),
		group(23, "ACL2", plannedDemoRoleNumbers("ACL 2")),
		group(24, "ACL3", plannedDemoRoleNumbers("ACL 3")),
		group(25, "ACL4", plannedDemoRoleNumbers("ACL 4")),
		group(26, "Blinders", plannedDemoRoleNumbers("Blinders")),
		group(27, "Sunstrip", plannedDemoRoleNumbers("Sunstrips")),
		group(28, "Strobe", []),
		group(29, "Front Lights", plannedDemoRoleNumbers("Front Lights")),
		group(30, "Floor Spots", []),
		group(31, "Hazer", plannedDemoRoleNumbers("Hazers")),
		group(32, "All ACLs", plannedDemoRoleNumbers("All ACLs")),
		group(33, "Front Profiles", [11, 12, 13, 14, 15]),
		group(34, "House Lights", plannedDemoRoleNumbers("House Lights")),
		group(35, "Follow Spots", plannedDemoRoleNumbers("Follow Spots")),
	];
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
	const existing = new Map(
		(await api.showObjects<any>(showId, "group")).map(
			(item) => [item.id, item] as const,
		),
	);
	for (const spec of specs) {
		const targets = spec.fixtureNumbers.flatMap((fixtureNumber) => {
			const found = targetsByNumber.get(fixtureNumber);
			if (!found)
				throw new Error(
					`Group ${spec.name} references missing fixture ${fixtureNumber}`,
				);
			return found;
		});
		const current = existing.get(spec.id);
		if (
			current &&
			JSON.stringify(current.body.fixtures) !== JSON.stringify(targets)
		) {
			throw new Error(
				`Visible Group ${spec.id} does not match canonical ${spec.name} membership`,
			);
		}
		await putPlannedDemoObject(api, showId, "group", spec.id, {
			...(current?.body ?? {}),
			id: spec.id,
			name: spec.name,
			fixtures: targets,
			color: current?.body.color ?? null,
			icon: current?.body.icon ?? null,
			derived_from: current?.body.derived_from ?? null,
			frozen_from: current?.body.frozen_from ?? null,
			programming: current?.body.programming ?? {},
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
