import type { LightApiClient } from "../../api/LightApiClient";
import type {
	BootstrapSnapshot,
	DeskUser,
	ProgrammerState,
	SessionResponse,
} from "../../api/types";
import type { LoadShowObjects } from "./stateEventRouting";
import type { ServerState } from "./useServerState";

function selectOperator(bootstrap: BootstrapSnapshot): DeskUser {
	const users = bootstrap.users.filter((user) => user.enabled);
	if (!users.length) throw new Error("No enabled desk user is configured");
	const remembered = localStorage.getItem("light.operator");
	return (
		users.find((user) => user.name === remembered) ??
		users.find((user) => user.name === "Operator") ??
		users[0]
	);
}

async function restoreOrLogin(
	client: LightApiClient,
	user: DeskUser,
): Promise<SessionResponse> {
	const screenWindow = new URLSearchParams(window.location.search).has(
		"screen",
	);
	const restored = screenWindow
		? (JSON.parse(
				localStorage.getItem("light.primary-session") ?? "null",
			) as SessionResponse | null)
		: null;
	if (restored) {
		client.restoreSession(restored);
		return restored;
	}
	return client.login(user.name);
}

async function ensureActiveShow(
	state: ServerState,
	bootstrap: BootstrapSnapshot,
	locked: boolean,
) {
	if (bootstrap.active_show || locked) return bootstrap;
	const library = await state.client.shows();
	const show =
		library.find((candidate) => candidate.name === "Default Stage Show") ??
		(await state.client.createShow("Default Stage Show"));
	await state.client.openShow(show.id, "hold_current");
	const next = await state.client.bootstrap();
	state.setBootstrap(next);
	return next;
}

async function loadInitialResources(client: LightApiClient) {
	const [
		patch,
		playbacks,
		programmers,
		shows,
		configuration,
		media,
		fixtureLibrary,
		fixtureProfiles,
		fixtureProfileWarnings,
		screens,
	] = await Promise.all([
		client.patch(),
		client.playbacks(),
		client.programmers(),
		client.shows(),
		client.configuration(),
		client.mediaServers(),
		client.fixtureLibrary(),
		client.fixtureProfiles().catch(() => []),
		client.fixtureProfileWarnings().catch(() => []),
		client.screens(),
	]);
	return {
		patch,
		playbacks,
		programmers,
		shows,
		configuration,
		media,
		fixtureLibrary,
		fixtureProfiles,
		fixtureProfileWarnings,
		screens,
	};
}

function installInitialResources(
	state: ServerState,
	resources: Awaited<ReturnType<typeof loadInitialResources>>,
) {
	state.setPatch(resources.patch);
	state.setPlaybacks(resources.playbacks);
	state.setShows(resources.shows);
	state.setConfiguration(resources.configuration.configuration);
	state.setMatter(resources.configuration.matter);
	state.setMediaServers(resources.media.fixtures);
	state.setFixtureLibrary(resources.fixtureLibrary);
	state.setFixtureProfiles(resources.fixtureProfiles);
	state.setFixtureProfileWarnings(resources.fixtureProfileWarnings);
	state.setScreens(resources.screens);
}

function restoreProgrammerState(
	state: ServerState,
	session: SessionResponse,
	programmers: ProgrammerState[],
) {
	const own = programmers.find(
		(item) => item.session_id === session.session_id,
	);
	const command =
		own?.command_line?.trim() || state.commandTargetModeRef.current;
	const target =
		command === "GROUP"
			? "GROUP"
			: command === "FIXTURE"
				? "FIXTURE"
				: state.commandTargetModeRef.current;
	state.commandTargetModeRef.current = target;
	state.setCommandTargetMode(target);
	state.setCommandLineState(command);
	state.setCommandLinePristine(command === target);
	state.setSelectedFixtures(own?.selected ?? []);
}

export async function bootstrapConnection(
	state: ServerState,
	loadShowObjects: LoadShowObjects,
	isCancelled: () => boolean,
) {
	const initial = await state.client.bootstrap();
	if (isCancelled()) return null;
	state.setBootstrap(initial);
	const user = selectOperator(initial);
	const session = await restoreOrLogin(state.client, user);
	const deskLock = await state.client.deskLock();
	localStorage.setItem("light.operator", user.name);
	const bootstrap = await ensureActiveShow(state, initial, deskLock.locked);
	const resources = await loadInitialResources(state.client);
	if (isCancelled()) return null;
	state.setSession(session);
	state.setCommandHistory(await state.client.commandHistory());
	state.setDeskLock(deskLock);
	installInitialResources(state, resources);
	await loadShowObjects(
		bootstrap.active_show_error ? null : (bootstrap.active_show?.id ?? null),
		session.user.id,
	);
	restoreProgrammerState(state, session, resources.programmers);
	return session;
}
