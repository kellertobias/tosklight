import type { ServerEvent, SessionResponse } from "../../api/types";
import type { ServerState } from "./useServerState";

export type LoadShowObjects = (
	showId: string | null,
	userId: string | null,
) => Promise<void>;

function refreshHighlight(event: ServerEvent, state: ServerState) {
	if (event.kind !== "highlight_changed") return;
	const request = ++state.highlightEpoch.current;
	void state.highlightWrite.current
		.catch(() => undefined)
		.then(() => state.client.highlight())
		.then((next) => {
			if (request !== state.highlightEpoch.current) return;
			state.setHighlight(next);
			if (!state.highlightErrorSticky.current) state.setHighlightError(null);
		})
		.catch(() => undefined);
}

function refreshPlaybackState(event: ServerEvent, state: ServerState) {
	const kinds = [
		"playback_changed",
		"playback_page_changed",
		"show_opened",
		"show_object_changed",
		"preload_stored",
	];
	if (!kinds.includes(event.kind)) return;
	void state.client
		.playbacks()
		.then(state.setPlaybacks)
		.catch(() => undefined);
}

function refreshConfiguration(event: ServerEvent, state: ServerState) {
	const kinds = [
		"server_configuration_changed",
		"speed_group_command",
		"speed_group_action",
	];
	if (!kinds.includes(event.kind)) return;
	void state.client
		.configuration()
		.then((next) => {
			state.setConfiguration(next.configuration);
			state.setMatter(next.matter);
		})
		.catch(() => undefined);
	void state.client
		.playbacks()
		.then(state.setPlaybacks)
		.catch(() => undefined);
}

function refreshScreens(event: ServerEvent, state: ServerState) {
	const kinds = [
		"screen_configuration_changed",
		"screen_page_changed",
		"playback_page_changed",
		"show_opened",
	];
	if (!kinds.includes(event.kind)) return;
	void state.client
		.screens()
		.then(state.setScreens)
		.catch(() => undefined);
}

function refreshBootstrap(
	event: ServerEvent,
	session: SessionResponse,
	state: ServerState,
	loadShowObjects: LoadShowObjects,
) {
	const kinds = [
		"show_opened",
		"show_renamed",
		"show_rolled_back",
		"server_configuration_changed",
		"session_started",
		"session_disconnected",
		"client_removed",
		"programmer_changed",
		"programmer_cleared",
		"hardware_connection_changed",
	];
	if (!kinds.includes(event.kind)) return;
	const requestedEpoch = state.commandLineEpoch.current;
	void state.commandLineWrite.current
		.catch(() => undefined)
		.then(() => state.client.bootstrap())
		.then((next) => {
			state.setBootstrap(next);
			const own = next.active_programmers.find(
				(programmer) => programmer.session_id === session.session_id,
			);
			if (own) {
				if (requestedEpoch === state.commandLineEpoch.current) {
					const command =
						own.command_line?.trim() || state.commandTargetModeRef.current;
					state.setCommandLineState(command);
					state.setCommandLinePristine(
						command === state.commandTargetModeRef.current,
					);
				}
				state.setSelectedFixtures(own.selected ?? []);
			}
			void loadShowObjects(next.active_show?.id ?? null, session.user.id);
		})
		.catch(() => undefined);
}

function refreshPatch(event: ServerEvent, state: ServerState) {
	if (!["show_opened", "show_object_changed"].includes(event.kind)) return;
	void state.client
		.patch()
		.then(state.setPatch)
		.catch(() => undefined);
}

function refreshFixtureLibrary(event: ServerEvent, state: ServerState) {
	if (
		!["fixture_library_changed", "fixture_profile_changed"].includes(event.kind)
	)
		return;
	void state.client
		.fixtureLibrary()
		.then(state.setFixtureLibrary)
		.catch(() => undefined);
	void state.client
		.fixtureProfiles()
		.then(state.setFixtureProfiles)
		.catch(() => undefined);
	void state.client
		.fixtureProfileWarnings()
		.then(state.setFixtureProfileWarnings)
		.catch(() => undefined);
}

function refreshShows(event: ServerEvent, state: ServerState) {
	const kinds = [
		"show_uploaded",
		"show_deleted",
		"show_opened",
		"show_renamed",
		"show_rolled_back",
	];
	if (!kinds.includes(event.kind)) return;
	void state.client
		.shows()
		.then(state.setShows)
		.catch(() => undefined);
}

function refreshMedia(event: ServerEvent, state: ServerState) {
	const kinds = [
		"show_opened",
		"media_thumbnails_refreshed",
		"media_preview_refreshed",
		"media_server_offline",
	];
	if (!kinds.includes(event.kind)) return;
	void state.client
		.mediaServers()
		.then((next) => state.setMediaServers(next.fixtures))
		.catch(() => undefined);
}

function refreshShowObjects(
	event: ServerEvent,
	session: SessionResponse,
	state: ServerState,
	loadShowObjects: LoadShowObjects,
) {
	if (
		!["show_object_changed", "preset_stored", "preload_stored"].includes(
			event.kind,
		)
	)
		return;
	void state.client
		.bootstrap()
		.then((next) =>
			loadShowObjects(next.active_show?.id ?? null, session.user.id),
		)
		.catch(() => undefined);
}

function refreshSelection(
	event: ServerEvent,
	session: SessionResponse,
	state: ServerState,
) {
	if (!["show_opened", "show_object_changed"].includes(event.kind)) return;
	void state.client
		.programmers()
		.then((programmers) => {
			const own = programmers.find(
				(item) => item.session_id === session.session_id,
			);
			if (own) state.setSelectedFixtures(own.selected);
		})
		.catch(() => undefined);
}

export function routeStateEvent(
	event: ServerEvent,
	session: SessionResponse,
	state: ServerState,
	loadShowObjects: LoadShowObjects,
) {
	refreshHighlight(event, state);
	refreshPlaybackState(event, state);
	refreshConfiguration(event, state);
	refreshScreens(event, state);
	refreshBootstrap(event, session, state, loadShowObjects);
	refreshPatch(event, state);
	refreshFixtureLibrary(event, state);
	refreshShows(event, state);
	refreshMedia(event, state);
	refreshShowObjects(event, session, state, loadShowObjects);
	refreshSelection(event, session, state);
}
