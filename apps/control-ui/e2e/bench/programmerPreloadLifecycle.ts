import { HttpProgrammerCaptureModeTransport } from "../../src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadLifecycleTransport } from "../../src/api/ProgrammerPreloadLifecycleTransport";
import { HttpProgrammerPreloadPlaybackQueueTransport } from "../../src/api/ProgrammerPreloadPlaybackQueueTransport";
import { HttpProgrammerPreloadValuesTransport } from "../../src/api/ProgrammerPreloadValuesTransport";
import { decodePlaybackSnapshot } from "../../src/api/playbackWire";
import { decodeProgrammingInteractionSnapshot } from "../../src/api/programmingWire";
import { programmerPreloadValuesUuidAt } from "../../src/api/programmerPreloadValuesWireProjection";
import type {
	ProgrammerPreloadLifecycleAction,
	ProgrammerPreloadLifecycleOutcome,
} from "../../src/features/programmerPreloadLifecycle/contracts";
import type { ApiDriver, Session } from "./api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentHeaders,
	intentRequestId,
	intentSession,
	intentUrl,
	responseJson,
} from "./v2IntentHttp";

export interface ProgrammerPreloadLifecycleIntent {
	surface: "api";
	showId: string;
}

export function enterProgrammerPreload(
	api: ApiDriver,
	intent: ProgrammerPreloadLifecycleIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerPreloadAction(api, intent, { type: "enter" }, dependencies);
}

export function goProgrammerPreload(
	api: ApiDriver,
	intent: ProgrammerPreloadLifecycleIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerPreloadAction(api, intent, { type: "go" }, dependencies);
}

export function clearPendingProgrammerPreload(
	api: ApiDriver,
	intent: ProgrammerPreloadLifecycleIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerPreloadAction(
		api,
		intent,
		{ type: "clear_pending" },
		dependencies,
	);
}

export function releaseProgrammerPreload(
	api: ApiDriver,
	intent: ProgrammerPreloadLifecycleIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerPreloadAction(
		api,
		intent,
		{ type: "release" },
		dependencies,
	);
}

type IntentAction =
	| { type: "enter" }
	| { type: "go" }
	| { type: "clear_pending" }
	| { type: "release" };

async function applyProgrammerPreloadAction(
	api: ApiDriver,
	intent: ProgrammerPreloadLifecycleIntent,
	action: IntentAction,
	dependencies: IntentHttpDependencies,
): Promise<ProgrammerPreloadLifecycleOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const fetch = intentFetch(dependencies);
	const scope = {
		showId: intent.showId,
		userId: session.user.id,
		deskId: session.desk.id,
	};
	const authority = await loadAuthority(api, session, intent.showId, action, fetch);
	return lifecycleTransport(api, session, fetch).applyAction(scope, {
		requestId: intentRequestId(dependencies),
		expectedCaptureModeRevision: authority.captureModeRevision,
		expectedValuesRevision: authority.valuesRevision,
		expectedQueueRevision: authority.queueRevision,
		expectedSelectionRevision: authority.selectionRevision,
		action: lifecycleAction(action, intent.showId, authority),
	});
}

interface LifecycleAuthority {
	captureModeRevision: number;
	valuesRevision: number;
	queueRevision: number;
	selectionRevision: number;
	showRevision: number | null;
	playbackEventSequence: number | null;
}

async function loadAuthority(
	api: ApiDriver,
	session: Session,
	showId: string,
	action: IntentAction,
	fetch: typeof globalThis.fetch,
): Promise<LifecycleAuthority> {
	const scope = { showId, userId: session.user.id };
	const playback =
		action.type === "go"
			? loadPlaybackAuthority(api, session, showId, fetch)
			: Promise.resolve(null);
	const [captureMode, values, queue, interaction, playbackAuthority] =
		await Promise.all([
			captureModes(api, session, fetch).loadSnapshot(scope),
			preloadValues(api, session, fetch).loadSnapshot(scope),
			preloadQueue(api, session, fetch).loadSnapshot(scope),
			loadInteraction(api, session, fetch),
			playback,
		]);
	return {
		captureModeRevision: captureMode.projection.revision,
		valuesRevision: values.projection.revision,
		queueRevision: queue.projection.revision,
		selectionRevision: interaction.projection.selection.revision,
		showRevision: playbackAuthority?.showRevision ?? null,
		playbackEventSequence: playbackAuthority?.eventSequence ?? null,
	};
}

function lifecycleAction(
	action: IntentAction,
	showId: string,
	authority: LifecycleAuthority,
): ProgrammerPreloadLifecycleAction {
	if (action.type !== "go") return action;
	if (
		authority.showRevision === null ||
		authority.playbackEventSequence === null
	)
		throw new Error("Preload GO requires exact Playback authority");
	return {
		type: "go",
		showId,
		expectedShowRevision: authority.showRevision,
		expectedPlaybackEventSequence: authority.playbackEventSequence,
	};
}

function captureModes(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerCaptureModeTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		fetch,
	});
}

function preloadValues(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerPreloadValuesTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedUserId: session.user.id,
		authenticatedDeskId: session.desk.id,
		fetch,
	});
}

function preloadQueue(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerPreloadPlaybackQueueTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedUserId: session.user.id,
		fetch,
	});
}

function lifecycleTransport(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerPreloadLifecycleTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedUserId: session.user.id,
		authenticatedDeskId: session.desk.id,
		fetch,
	});
}

async function loadInteraction(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	const path = `/api/v2/desks/${encodeURIComponent(session.desk.id)}/programming-interaction/snapshot`;
	const response = await fetch(intentUrl(api, path), {
		headers: intentHeaders(session),
	});
	const value = await responseJson(response, "Programming interaction");
	if (!response.ok)
		throw new Error(
			`Programming interaction snapshot returned HTTP ${response.status}`,
		);
	return decodeProgrammingInteractionSnapshot(value, session.desk.id);
}

async function loadPlaybackAuthority(
	api: ApiDriver,
	session: Session,
	showId: string,
	fetch: typeof globalThis.fetch,
) {
	const path = `/api/v2/desks/${encodeURIComponent(session.desk.id)}/playback-runtime/snapshot`;
	const response = await fetch(intentUrl(api, path), {
		method: "POST",
		headers: { ...intentHeaders(session), "content-type": "application/json" },
		body: JSON.stringify({ identities: [] }),
	});
	const value = await responseJson(response, "Playback runtime");
	if (!response.ok)
		throw new Error(`Playback runtime snapshot returned HTTP ${response.status}`);
	const snapshot = decodePlaybackSnapshot(value);
	assertPlaybackScope(snapshot.desk.desk_id, snapshot.desk.scope.show_id, session, showId);
	return {
		showRevision: snapshot.desk.scope.show_revision,
		eventSequence: snapshot.cursor.sequence,
	};
}

function assertPlaybackScope(
	deskId: string,
	showId: string,
	session: Session,
	expectedShowId: string,
) {
	if (deskId !== session.desk.id)
		throw new Error(`Playback snapshot belongs to foreign desk ${deskId}`);
	if (showId !== expectedShowId)
		throw new Error(`Playback snapshot belongs to foreign Show ${showId}`);
}

function validateIntent(intent: ProgrammerPreloadLifecycleIntent) {
	if (intent.surface !== "api")
		throw new Error("Programmer Preload helper supports only the API surface");
	programmerPreloadValuesUuidAt(intent.showId, "$.showId");
}
