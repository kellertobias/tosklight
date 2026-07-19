import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../../api/ServerContext";
import type { VisualizationSnapshot } from "../../../api/types";
import { useApp } from "../../../state/AppContext";
import { useGroups } from "../../../features/server/useShowObjectsState";
import { usePollingResource } from "../../../hooks/usePollingResource";
import {
	directProgrammerChoices,
	type ParameterFamily,
	type ProgrammerValueEntry,
	parameterFamilies,
} from "./model";

function useVisualization() {
	const server = useServer();
	const [visualization, setVisualization] =
		useState<VisualizationSnapshot | null>(null);
	useEffect(() => {
		if (server.selectedFixtures.length) return;
		setVisualization(null);
	}, [server.selectedFixtures.length]);
	usePollingResource({
		enabled: server.selectedFixtures.length > 0,
		intervalMillis: 400,
		load: server.readVisualization,
		onValue: setVisualization,
	});
	return visualization;
}

function useSupportedAttributes() {
	const server = useServer();
	const groups = useGroups(server.playbacks);
	return useMemo(() => {
		const result = new Set<string>();
		for (const fixture of server.patch?.fixtures ?? []) {
			const selected =
				server.selectedFixtures.includes(fixture.fixture_id) ||
				fixture.logical_heads.some((head) =>
					server.selectedFixtures.includes(head.fixture_id),
				);
			if (!selected) continue;
			for (const head of fixture.definition.heads ?? [])
				for (const parameter of head.parameters)
					result.add(parameter.attribute);
		}
		if (server.selectedGroupId) {
			result.add("intensity");
			const group = groups.find(
				(candidate) => candidate.id === server.selectedGroupId,
			);
			for (const attribute of Object.keys(group?.body.programming ?? {}))
				result.add(attribute);
		}
		return result;
	}, [
		server.patch,
		server.selectedFixtures,
		server.selectedGroupId,
		groups,
	]);
}

function useResolvedValues(
	visualization: VisualizationSnapshot | null,
	selectedFixtures: string[],
) {
	return useMemo(() => {
		const normalized = new Map<string, number>();
		const normalizedByFixture = new Map<string, Map<string, number>>();
		const discrete = new Map<string, string>();
		const discreteByFixture = new Map<string, Map<string, string>>();
		for (const entry of visualization?.values ?? []) {
			if (!selectedFixtures.includes(entry.fixture_id)) continue;
			if (entry.value.kind === "normalized") {
				if (!normalized.has(entry.attribute))
					normalized.set(entry.attribute, entry.value.value);
				const values = normalizedByFixture.get(entry.fixture_id) ?? new Map();
				values.set(entry.attribute, entry.value.value);
				normalizedByFixture.set(entry.fixture_id, values);
			} else if (entry.value.kind === "discrete") {
				if (!discrete.has(entry.attribute))
					discrete.set(entry.attribute, entry.value.value);
				const values = discreteByFixture.get(entry.fixture_id) ?? new Map();
				values.set(entry.attribute, entry.value.value);
				discreteByFixture.set(entry.fixture_id, values);
			}
		}
		return { normalized, normalizedByFixture, discrete, discreteByFixture };
	}, [visualization, selectedFixtures]);
}

export function useParameterProjection(family: ParameterFamily) {
	const server = useServer();
	const { state } = useApp();
	const visualization = useVisualization();
	const supported = useSupportedAttributes();
	const ownProgrammer = server.bootstrap?.active_programmers.find(
		(programmer) => programmer.session_id === server.session?.session_id,
	);
	const programmerValues = (ownProgrammer?.values ??
		[]) as ProgrammerValueEntry[];
	const groupProgrammerValues = (ownProgrammer?.group_values ?? {}) as Record<
		string,
		Record<string, unknown>
	>;
	const values = useResolvedValues(visualization, server.selectedFixtures);
	const directChoices = useMemo(
		() =>
			directProgrammerChoices(
				server.patch?.fixtures ?? [],
				server.selectedFixtures,
			),
		[server.patch, server.selectedFixtures],
	);
	const attributes = parameterFamilies[family].filter((attribute) =>
		supported.has(attribute),
	);
	return {
		server,
		state,
		ownProgrammer,
		programmerValues,
		groupProgrammerValues,
		...values,
		directChoices,
		encoderSlots: Array.from(
			{ length: 6 },
			(_, index) => attributes[index] ?? null,
		),
		hardwareConnected: Boolean(
			server.bootstrap?.hardware_connected || state.midiProfile,
		),
	};
}

export type ParameterProjection = ReturnType<typeof useParameterProjection>;
