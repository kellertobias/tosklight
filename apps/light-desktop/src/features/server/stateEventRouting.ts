import type {
	HighlightState,
	RuntimeCapabilityEvent,
	SessionResponse,
} from "../../api/types";
import type { ServerState } from "./useServerState";

export type LoadShowObjects = (
	showId: string | null,
	userId: string | null,
) => Promise<void>;

function installHighlight(
	event: RuntimeCapabilityEvent,
	session: SessionResponse,
	state: ServerState,
) {
	if (event.type !== "highlight_changed") return;
	const { change } = event;
	if (
		change.desk_id !== session.desk.id ||
		change.user_id !== session.user.id ||
		!["selection", "step"].includes(change.state.mode)
	)
		return;
	state.highlightEpoch.current += 1;
	state.setHighlight(change.state as HighlightState);
	if (!state.highlightErrorSticky.current) state.setHighlightError(null);
}

function refreshConfiguration(
	event: RuntimeCapabilityEvent,
	state: ServerState,
) {
	if (event.type !== "server_configuration_changed") return;
	void state.api.desk
		.configuration()
		.then((next) => {
			state.setConfiguration(next.configuration);
			state.setMatter(next.matter);
		})
		.catch(() => undefined);
}

function installHardwareConnection(
	event: RuntimeCapabilityEvent,
	state: ServerState,
) {
	if (event.type !== "hardware_connection_changed") return;
	state.setBootstrap((current) =>
		current
			? { ...current, hardware_connected: event.change.connected }
			: current,
	);
}

function refreshScreens(event: RuntimeCapabilityEvent, state: ServerState) {
	if (
		event.type !== "screens_changed" &&
		!isShowLibraryEvent(event, ["show_opened"])
	)
		return;
	void state.api.playback
		.screens()
		.then(state.setScreens)
		.catch(() => undefined);
}

function refreshBootstrap(
	event: RuntimeCapabilityEvent,
	session: SessionResponse,
	getState: () => ServerState,
	loadShowObjects: LoadShowObjects,
) {
	if (!isShowLibraryEvent(event, ["show_opened", "show_rolled_back"])) return;
	const state = getState();
	const previousShowId = state.bootstrap?.active_show?.id ?? null;
	const requestedEpoch = state.commandLineEpoch.current;
	void state.commandLineWrite.current
		.catch(() => undefined)
		.then(() => state.api.runtime.bootstrap())
		.then(async (next) => {
			const current = getState();
			const nextShowId = next.active_show?.id ?? null;
			const showChanged =
				isShowLibraryEvent(event, ["show_opened", "show_rolled_back"]) ||
				previousShowId !== nextShowId;
			const loadingOperation = showChanged
				? current.beginDeskLoading(
						next.active_show?.name
							? `Loading show ${next.active_show.name}…`
							: "Loading show…",
						"Installing the show engine snapshot and preparing control surfaces",
					)
				: null;
			try {
				current.setBootstrap(next);
				const own = next.active_programmers.find(
					(programmer) => programmer.session_id === session.session_id,
				);
				if (own) {
					if (requestedEpoch === current.commandLineEpoch.current) {
						const command =
							own.command_line?.trim() || current.commandTargetModeRef.current;
						current.setCommandLineState(command);
						current.setCommandLinePristine(
							command === current.commandTargetModeRef.current,
						);
					}
					current.setSelectedFixtures(own.selected ?? []);
				}
				if (showChanged) await loadShowObjects(nextShowId, session.user.id);
			} finally {
				if (loadingOperation != null) {
					getState().finishDeskLoading(loadingOperation);
				}
			}
		})
		.catch(() => undefined);
}

function refreshFixtureLibrary(
	event: RuntimeCapabilityEvent,
	state: ServerState,
) {
	if (event.type !== "fixture_library_changed") return;
	void state.api.fixtures
		.fixtureLibrary()
		.then(state.setFixtureLibrary)
		.catch(() => undefined);
	void state.api.fixtures
		.fixtureProfiles()
		.then(state.setFixtureProfiles)
		.catch(() => undefined);
	void state.api.fixtures
		.fixtureProfileWarnings()
		.then(state.setFixtureProfileWarnings)
		.catch(() => undefined);
}

function refreshShows(event: RuntimeCapabilityEvent, state: ServerState) {
	if (event.type !== "show_library_changed") return;
	void state.api.shows
		.shows()
		.then((shows) => {
			state.setShows(shows);
			if (event.change.kind !== "show_renamed") return;
			state.setBootstrap((current) => {
				if (!current?.active_show) return current;
				const active = shows.find(
					(show) => show.id === current.active_show?.id,
				);
				return active ? { ...current, active_show: active } : current;
			});
		})
		.catch(() => undefined);
}

function refreshMedia(event: RuntimeCapabilityEvent, state: ServerState) {
	if (
		event.type !== "media_changed" &&
		!isShowLibraryEvent(event, ["show_opened"])
	)
		return;
	void state.api.mediaOutput
		.mediaServers()
		.then((next) => state.setMediaServers(next.fixtures))
		.catch(() => undefined);
}

function refreshSelection(
	event: RuntimeCapabilityEvent,
	session: SessionResponse,
	state: ServerState,
) {
	if (!isShowLibraryEvent(event, ["show_opened"])) return;
	void state.api.programming
		.programmingInteractionSnapshot(session.desk.id)
		.then((snapshot) => {
			state.setSelectedFixtures([...snapshot.projection.selection.selected]);
		})
		.catch(() => undefined);
}

export function createStateEventRouter(
	getState: () => ServerState,
	session: SessionResponse,
	loadShowObjects: LoadShowObjects,
) {
	return (event: RuntimeCapabilityEvent) => {
		const state = getState();
		installHighlight(event, session, state);
		refreshConfiguration(event, state);
		installHardwareConnection(event, state);
		refreshScreens(event, state);
		refreshBootstrap(event, session, getState, loadShowObjects);
		refreshFixtureLibrary(event, state);
		refreshShows(event, state);
		refreshMedia(event, state);
		refreshSelection(event, session, state);
	};
}

function isShowLibraryEvent(
	event: RuntimeCapabilityEvent,
	kinds: Array<
		| "show_opened"
		| "show_renamed"
		| "show_rolled_back"
		| "show_uploaded"
		| "show_deleted"
	>,
) {
	return (
		event.type === "show_library_changed" && kinds.includes(event.change.kind)
	);
}
