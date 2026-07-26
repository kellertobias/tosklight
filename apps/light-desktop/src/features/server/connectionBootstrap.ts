import type { LightApi } from "../../api/client/api";
import type {
	BootstrapSnapshot,
	DeskUser,
	ProgrammerState,
	SessionResponse,
} from "../../api/types";
import type { FrontendWarmupTask } from "../frontendWarmup/coordinator";
import {
	frontendPerformanceDiagnostics,
	measureFrontendSnapshot,
	serializedModelBytes,
} from "../frontendWarmup/diagnostics";
import {
	mayCreateSession,
	requirePrimarySession,
	type SessionRole,
} from "../session/ownership";
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
	api: LightApi,
	user: DeskUser,
	role: SessionRole,
): Promise<SessionResponse> {
	if (!mayCreateSession(role)) {
		const restored = requirePrimarySession(
			localStorage.getItem("light.primary-session"),
		);
		api.runtime.restoreSession(restored);
		return restored;
	}
	return api.runtime.login(user.name);
}

async function ensureActiveShow(
	state: ServerState,
	bootstrap: BootstrapSnapshot,
	locked: boolean,
) {
	if (bootstrap.active_show || locked) return bootstrap;
	const library = await state.api.shows.shows();
	const show =
		library.find((candidate) => candidate.name === "Default Stage Show") ??
		(await state.api.shows.createShow("Default Stage Show"));
	await state.api.shows.openShow(show.id, "hold_current");
	const next = await state.api.runtime.bootstrap();
	state.setBootstrap(next);
	return next;
}

async function loadForegroundResources(api: LightApi) {
	const [programmers, configuration, fixtureLibrary, commandHistory] =
		await Promise.all([
			api.desk.programmers(),
			api.desk.configuration(),
			api.fixtures.fixtureLibrary(),
			api.desk.commandHistory(),
		]);
	return {
		programmers,
		configuration,
		fixtureLibrary,
		commandHistory,
	};
}

function installForegroundResources(
	state: ServerState,
	resources: Awaited<ReturnType<typeof loadForegroundResources>>,
) {
	state.setConfiguration(resources.configuration.configuration);
	state.setMatter(resources.configuration.matter);
	state.setFixtureLibrary(resources.fixtureLibrary);
	state.setCommandHistory(resources.commandHistory);
}

export function deferredConnectionWarmupTasks(
	state: ServerState,
): FrontendWarmupTask[] {
	return [
		deferredResourceTask(
			"legacy:shows",
			"near-future",
			() => state.api.shows.shows(),
			state.setShows,
		),
		deferredResourceTask(
			"legacy:screens",
			"near-future",
			() => state.api.playback.screens(),
			state.setScreens,
		),
		deferredResourceTask(
			"legacy:media-servers",
			"idle",
			async () => (await state.api.mediaOutput.mediaServers()).fixtures,
			state.setMediaServers,
		),
		deferredResourceTask(
			"legacy:fixture-profiles",
			"idle",
			() => state.api.fixtures.fixtureProfiles().catch(() => []),
			state.setFixtureProfiles,
		),
		deferredResourceTask(
			"legacy:fixture-profile-warnings",
			"idle",
			() => state.api.fixtures.fixtureProfileWarnings().catch(() => []),
			state.setFixtureProfileWarnings,
		),
	];
}

function deferredResourceTask<T>(
	key: string,
	priority: FrontendWarmupTask["priority"],
	load: () => Promise<T>,
	install: (value: T) => void,
): FrontendWarmupTask {
	return {
		key,
		priority,
		run: async (signal) => {
			if (signal.aborted) throw signal.reason;
			const result = await measureFrontendSnapshot(key, load);
			if (signal.aborted) throw signal.reason;
			install(result);
			return { retainedBytes: serializedModelBytes(result) };
		},
	};
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
	role: SessionRole,
) {
	const finishBootstrap = frontendPerformanceDiagnostics.beginPhase(
		"connection-bootstrap",
	);
	const initial = await state.api.runtime.bootstrap();
	if (isCancelled()) {
		finishBootstrap();
		return null;
	}
	state.setBootstrap(initial);
	const user = selectOperator(initial);
	const session = await restoreOrLogin(state.api, user, role);
	const deskLock = await state.api.desk.deskLock();
	localStorage.setItem("light.operator", user.name);
	const bootstrap = await ensureActiveShow(state, initial, deskLock.locked);
	const finishResources =
		frontendPerformanceDiagnostics.beginPhase("initial-resources");
	const resources = await loadForegroundResources(state.api);
	finishResources();
	if (isCancelled()) {
		finishBootstrap();
		return null;
	}
	state.setSession(session);
	state.setConnectionGeneration((current) => current + 1);
	state.deskLockStore.install(deskLock);
	installForegroundResources(state, resources);
	await loadShowObjects(
		bootstrap.active_show_error ? null : (bootstrap.active_show?.id ?? null),
		session.user.id,
	);
	restoreProgrammerState(state, session, resources.programmers);
	finishBootstrap();
	return session;
}
