import type {
	HighlightState,
	RuntimeCapabilityEvent,
	SessionResponse,
} from "../../api/types";
import type { ServerState } from "./useServerState";

export type LoadShowObjects = (showId: string | null) => Promise<void>;

function installHighlight(event: RuntimeCapabilityEvent, state: ServerState) {
	if (event.type !== "highlight_changed") return;
	const { change } = event;
	// One desk, one Highlight. The desk and user this arrived under used to decide whether it was
	// ours; every surface reads the same Highlight now, so only the mode still narrows it.
	if (!["selection", "step"].includes(change.state.mode)) return;
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
				if (showChanged) await loadShowObjects(nextShowId);
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

const MEDIA_FIXTURE_TYPES = new Set(["media_server", "audio_player"]);

/** Whether a patch change adds, edits, or removes a Media Server or Audio Player fixture. */
function patchTouchesMediaServers(
	delta: Extract<
		RuntimeCapabilityEvent,
		{ type: "show_patch_changed" }
	>["delta"],
	known: readonly { fixture_id: string }[],
) {
	const knownIds = new Set(known.map((server) => server.fixture_id));
	if (delta.removed_fixture_ids.some((id) => knownIds.has(id))) return true;
	const mediaProfiles = new Set(
		delta.profile_revisions
			.filter((profile) =>
				MEDIA_FIXTURE_TYPES.has(profile.fixture_type.trim()),
			)
			.map((profile) => `${profile.profile_id}@${profile.profile_revision}`),
	);
	return delta.fixtures.some(
		(fixture) =>
			knownIds.has(fixture.fixture_id) ||
			mediaProfiles.has(`${fixture.profile_id}@${fixture.profile_revision}`),
	);
}

function refreshMedia(event: RuntimeCapabilityEvent, state: ServerState) {
	// A patch change to a Media Server resets its connection state on the desk, so the rows
	// re-read it instead of showing the previous endpoint's status. Other patch edits leave the
	// Media Server list alone.
	const mediaPatchChange =
		event.type === "show_patch_changed" &&
		patchTouchesMediaServers(event.delta, state.mediaServers);
	if (
		event.type !== "media_changed" &&
		!mediaPatchChange &&
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

function installScheduleRuntime(
	event: RuntimeCapabilityEvent,
	state: ServerState,
) {
	if (event.type !== "schedule_runtime_changed") return;
	state.schedulerRuntimeStore.install(event.change);
}

export function createStateEventRouter(
	getState: () => ServerState,
	session: SessionResponse,
	loadShowObjects: LoadShowObjects,
) {
	return (event: RuntimeCapabilityEvent) => {
		const state = getState();
		installHighlight(event, state);
		refreshConfiguration(event, state);
		installHardwareConnection(event, state);
		refreshScreens(event, state);
		refreshBootstrap(event, session, getState, loadShowObjects);
		refreshFixtureLibrary(event, state);
		refreshShows(event, state);
		refreshMedia(event, state);
		refreshSelection(event, session, state);
		installScheduleRuntime(event, state);
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
