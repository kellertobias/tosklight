import { useCallback } from "react";
import type {
	CueList,
	OutputRoute,
	PatchLayer,
	StoredGroup,
	StoredPreset,
} from "../../api/types";
import {
	deskLayoutScopeKey,
	type StoredDeskLayout,
	type StoredStageLayout,
} from "./contracts";
import type { ServerState } from "./useServerState";

export function useShowObjects(state: ServerState) {
	const {
		client,
		showObjectsRequest,
		setCueObjects,
		setDeskLayout,
		setDeskLayoutScope,
		setGroups,
		setOutputRoutes,
		setPatchLayers,
		setPresets,
		setStageLayout,
		setUnresolvedMvrFixtures,
	} = state;
	return useCallback(
		async (showId: string | null, userId: string | null) => {
			const request = ++showObjectsRequest.current;
			const scope = deskLayoutScopeKey(showId, userId);
			setDeskLayoutScope((loaded) => (loaded === scope ? loaded : null));
			if (!showId) {
				if (request !== showObjectsRequest.current) return;
				setGroups([]);
				setPresets([]);
				setCueObjects([]);
				setOutputRoutes([]);
				setDeskLayout(null);
				setStageLayout(null);
				setUnresolvedMvrFixtures([]);
				setDeskLayoutScope(null);
				return;
			}
			const [
				groups,
				presets,
				cues,
				routes,
				layouts,
				stageLayouts,
				layers,
				unresolved,
			] = await Promise.all([
				client.objects<StoredGroup>(showId, "group"),
				client.objects<StoredPreset>(showId, "preset"),
				client.objects<CueList>(showId, "cue_list"),
				client.objects<OutputRoute>(showId, "route"),
				userId
					? client.objects<StoredDeskLayout>(showId, "user_layout")
					: Promise.resolve([]),
				client.objects<StoredStageLayout>(showId, "stage_layout"),
				client.objects<PatchLayer>(showId, "patch_layer"),
				client.objects<Record<string, unknown>>(
					showId,
					"unresolved_mvr_fixture",
				),
			]);
			if (request !== showObjectsRequest.current) return;
			setGroups(groups);
			setPresets(presets);
			setCueObjects(cues);
			setOutputRoutes(routes);
			setDeskLayout(layouts.find((item) => item.id === userId) ?? null);
			setDeskLayoutScope(scope);
			setStageLayout(stageLayouts.find((item) => item.id === "main") ?? null);
			setPatchLayers(
				layers.length
					? layers
					: [
							{
								kind: "patch_layer",
								id: "default",
								revision: 0,
								updated_at: "",
								body: { id: "default", name: "Default", order: 0 },
							},
						],
			);
			setUnresolvedMvrFixtures(unresolved);
		},
		[
			client,
			showObjectsRequest,
			setCueObjects,
			setDeskLayout,
			setDeskLayoutScope,
			setGroups,
			setOutputRoutes,
			setPatchLayers,
			setPresets,
			setStageLayout,
			setUnresolvedMvrFixtures,
		],
	);
}

export function useServerRefresh(
	state: ServerState,
	loadShowObjects: ReturnType<typeof useShowObjects>,
) {
	const {
		client,
		setBootstrap,
		setConfiguration,
		setFixtureLibrary,
		setFixtureProfiles,
		setFixtureProfileWarnings,
		setMatter,
		setMediaServers,
		setPatch,
		setPlaybacks,
		setShows,
	} = state;
	return useCallback(async () => {
		const bootstrap = await client.bootstrap();
		setBootstrap(bootstrap);
		setPatch(await client.patch());
		if (client.currentSession) setPlaybacks(await client.playbacks());
		setShows(await client.shows());
		const configuration = await client.configuration();
		setConfiguration(configuration.configuration);
		setMatter(configuration.matter);
		setFixtureLibrary(await client.fixtureLibrary());
		setFixtureProfiles(await client.fixtureProfiles().catch(() => []));
		setFixtureProfileWarnings(
			await client.fixtureProfileWarnings().catch(() => []),
		);
		if (client.currentSession)
			setMediaServers((await client.mediaServers()).fixtures);
		await loadShowObjects(
			bootstrap.active_show?.id ?? null,
			client.currentSession?.user.id ?? null,
		);
	}, [
		client,
		loadShowObjects,
		setBootstrap,
		setConfiguration,
		setFixtureLibrary,
		setFixtureProfiles,
		setFixtureProfileWarnings,
		setMatter,
		setMediaServers,
		setPatch,
		setPlaybacks,
		setShows,
	]);
}
