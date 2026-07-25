import { useCallback, useRef } from "react";
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
	const inFlight = useRef<{
		scope: string;
		promise: Promise<void>;
	} | null>(null);
	return useCallback(
		(showId: string | null, userId: string | null) => {
			const requestScope = `${showId ?? ""}\u0000${userId ?? ""}`;
			if (inFlight.current?.scope === requestScope)
				return inFlight.current.promise;
			const promise = (async () => {
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
				const [cues, routes, layouts, stageLayouts, layers, unresolved] =
					await Promise.all([
						api.showObjects.objects<CueList>(showId, "cue_list"),
						api.showObjects.objects<OutputRoute>(showId, "route"),
						userId
							? api.showObjects.objects<StoredDeskLayout>(
									showId,
									"user_layout",
								)
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
				setStageLayout(
					stageLayouts.find((item) => item.id === "main") ?? null,
				);
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
			})();
			inFlight.current = { scope: requestScope, promise };
			const clear = () => {
				if (inFlight.current?.promise === promise) inFlight.current = null;
			};
			void promise.then(clear, clear);
			return promise;
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
		const [
			shows,
			configuration,
			fixtureLibrary,
			fixtureProfiles,
			fixtureProfileWarnings,
			mediaServers,
		] = await Promise.all([
			api.shows.shows(),
			api.desk.configuration(),
			api.fixtures.fixtureLibrary(),
			api.fixtures.fixtureProfiles().catch(() => []),
			api.fixtures.fixtureProfileWarnings().catch(() => []),
			api.runtime.currentSession
				? api.mediaOutput.mediaServers()
				: Promise.resolve(null),
		]);
		setShows(shows);
		setConfiguration(configuration.configuration);
		setMatter(configuration.matter);
		setFixtureLibrary(fixtureLibrary);
		setFixtureProfiles(fixtureProfiles);
		setFixtureProfileWarnings(fixtureProfileWarnings);
		if (mediaServers) setMediaServers(mediaServers.fixtures);
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
