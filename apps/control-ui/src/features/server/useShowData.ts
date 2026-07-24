import { useCallback } from "react";
import type {
	CueList,
	OutputRoute,
	PatchLayer,
} from "../../api/types";
import {
	deskLayoutScopeKey,
	type StoredDeskLayout,
	type StoredStageLayout,
} from "./contracts";
import type { ServerState } from "./useServerState";

export function useShowObjects(state: ServerState) {
	const {
		api,
		showObjectsStore,
		showObjectsRequest,
		setCueObjects,
		setDeskLayout,
		setDeskLayoutScope,
		setOutputRoutes,
		setPatchLayers,
		setStageLayout,
		setUnresolvedMvrFixtures,
	} = state;
	return useCallback(
		async (showId: string | null, userId: string | null) => {
			const request = ++showObjectsRequest.current;
			showObjectsStore.reset(showId);
			const scope = deskLayoutScopeKey(showId, userId);
			setDeskLayoutScope((loaded) => (loaded === scope ? loaded : null));
			if (!showId) {
				if (request !== showObjectsRequest.current) return;
				setCueObjects([]);
				setOutputRoutes([]);
				setDeskLayout(null);
				setStageLayout(null);
				setUnresolvedMvrFixtures([]);
				setDeskLayoutScope(null);
				return;
			}
			const [
				cues,
				routes,
				layouts,
				stageLayouts,
				layers,
				unresolved,
			] = await Promise.all([
				api.showObjects.objects<CueList>(showId, "cue_list"),
				api.showObjects.objects<OutputRoute>(showId, "route"),
				userId
					? api.showObjects.objects<StoredDeskLayout>(showId, "user_layout")
					: Promise.resolve([]),
				api.showObjects.objects<StoredStageLayout>(showId, "stage_layout"),
				api.showObjects.objects<PatchLayer>(showId, "patch_layer"),
				api.showObjects.objects<Record<string, unknown>>(
					showId,
					"unresolved_mvr_fixture",
				),
			]);
			if (request !== showObjectsRequest.current) return;
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
			api,
			showObjectsStore,
			showObjectsRequest,
			setCueObjects,
			setDeskLayout,
			setDeskLayoutScope,
			setOutputRoutes,
			setPatchLayers,
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
		api,
		setBootstrap,
		setConfiguration,
		setFixtureLibrary,
		setFixtureProfiles,
		setFixtureProfileWarnings,
		setMatter,
		setMediaServers,
		setShows,
	} = state;
	return useCallback(async () => {
		const bootstrap = await api.runtime.bootstrap();
		setBootstrap(bootstrap);
		setShows(await api.shows.shows());
		const configuration = await api.desk.configuration();
		setConfiguration(configuration.configuration);
		setMatter(configuration.matter);
		setFixtureLibrary(await api.fixtures.fixtureLibrary());
		setFixtureProfiles(await api.fixtures.fixtureProfiles().catch(() => []));
		setFixtureProfileWarnings(
			await api.fixtures.fixtureProfileWarnings().catch(() => []),
		);
		if (api.runtime.currentSession)
			setMediaServers((await api.mediaOutput.mediaServers()).fixtures);
		await loadShowObjects(
			bootstrap.active_show?.id ?? null,
			api.runtime.currentSession?.user.id ?? null,
		);
	}, [
		api,
		loadShowObjects,
		setBootstrap,
		setConfiguration,
		setFixtureLibrary,
		setFixtureProfiles,
		setFixtureProfileWarnings,
		setMatter,
		setMediaServers,
		setShows,
	]);
}
