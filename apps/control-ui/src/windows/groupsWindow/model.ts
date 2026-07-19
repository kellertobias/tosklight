import { useMemo } from "react";
import type { useServer } from "../../api/ServerContext";
import type { StoredGroup, VersionedObject } from "../../api/types";
import { groups as fallbackGroups } from "../../data/mockData";
import { useGroups } from "../../features/server/useShowObjectsState";

export type GroupsServer = ReturnType<typeof useServer>;
export type Group = VersionedObject<StoredGroup>;

export interface FixtureMetadata {
	capabilities: Map<string, Set<string>>;
	fixtureNames: Map<string, string>;
	knownFixtureIds: Set<string>;
}

function fallbackGroupPool(): Group[] {
	return fallbackGroups.map((group) => ({
		kind: "group",
		id: String(group.id),
		revision: 0,
		updated_at: "",
		body: {
			name: group.name,
			fixtures: Array.from({ length: group.fixtures }, (_, index) =>
				String(index),
			),
			master: 1,
			playback_fader: group.id <= 8 ? group.id : null,
			programming: {},
			derived_from: null,
			frozen_from: null,
		},
	}));
}

function groupCards(groups: readonly Group[]) {
	return Array.from(
		{ length: 40 },
		(_, index) =>
			groups.find((group) => group.id === String(index + 1)) ?? null,
	);
}

function fixtureMetadata(
	fixtures: NonNullable<GroupsServer["patch"]>["fixtures"],
): FixtureMetadata {
	const knownFixtureIds = new Set<string>();
	const fixtureNames = new Map<string, string>();
	const capabilities = new Map<string, Set<string>>();
	for (const fixture of fixtures) {
		knownFixtureIds.add(fixture.fixture_id);
		for (const head of fixture.logical_heads)
			knownFixtureIds.add(head.fixture_id);
		const label =
			fixture.fixture_number != null
				? `Fixture ${fixture.fixture_number}`
				: fixture.name || fixture.definition.name || fixture.fixture_id;
		fixtureNames.set(
			fixture.fixture_id,
			`${label} · ${fixture.definition.manufacturer} ${fixture.definition.model}`,
		);
		for (const head of fixture.definition.heads ?? []) {
			const owner = head.shared
				? fixture.fixture_id
				: fixture.logical_heads.find(
						(candidate) => candidate.head_index === head.index,
					)?.fixture_id;
			if (!owner) continue;
			fixtureNames.set(
				owner,
				head.shared
					? (fixtureNames.get(fixture.fixture_id) ?? fixture.fixture_id)
					: `${fixtureNames.get(fixture.fixture_id)} · head ${head.index}`,
			);
			capabilities.set(
				owner,
				new Set(head.parameters.map((parameter) => parameter.attribute)),
			);
		}
	}
	return { capabilities, fixtureNames, knownFixtureIds };
}

export function useGroupPoolModel(server: GroupsServer) {
	const storedGroups = useGroups(server.playbacks);
	const groups = useMemo(() => {
		if (!server.bootstrap) return fallbackGroupPool();
		return server.bootstrap.active_show ? storedGroups : [];
	}, [server.bootstrap, storedGroups]);
	const cards = useMemo(() => groupCards(groups), [groups]);
	const fixtures = server.patch?.fixtures ?? [];
	const metadata = useMemo(() => fixtureMetadata(fixtures), [fixtures]);
	return { cards, groups, ...metadata };
}
