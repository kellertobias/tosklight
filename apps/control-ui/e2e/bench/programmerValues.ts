import { HttpProgrammerCaptureModeTransport } from "../../src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerValuesTransport } from "../../src/api/ProgrammerValuesTransport";
import { decodePlaybackSnapshot } from "../../src/api/playbackWire";
import { programmerValuesUuidAt } from "../../src/api/programmerValuesWireProjection";
import type { AttributeValue } from "../../src/api/types/playback";
import {
	capturesProgrammerWrites,
	type ProgrammerCaptureModeProjection,
} from "../../src/features/programmerCaptureMode/contracts";
import type {
	ProgrammerValuesActionOutcome,
	ProgrammerValuesCommand,
	ProgrammerValuesMutation,
	ProgrammerValueTiming,
} from "../../src/features/programmerValues/contracts";
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

export type { ProgrammerValuesMutation, ProgrammerValueTiming };

export interface ProgrammerValuesApiIntent {
	surface: "api";
	showId: string;
}

export interface SetProgrammerFixtureValueIntent
	extends ProgrammerValuesApiIntent {
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
	timing: ProgrammerValueTiming;
}

export interface ReleaseProgrammerFixtureValueIntent
	extends ProgrammerValuesApiIntent {
	fixtureId: string;
	attribute: string;
}

export interface SetProgrammerGroupValueIntent
	extends ProgrammerValuesApiIntent {
	groupId: string;
	attribute: string;
	value: AttributeValue;
	timing: ProgrammerValueTiming;
}

export interface ReleaseProgrammerGroupValueIntent
	extends ProgrammerValuesApiIntent {
	groupId: string;
	attribute: string;
}

export interface BatchProgrammerValuesIntent extends ProgrammerValuesApiIntent {
	mutations: readonly ProgrammerValuesMutation[];
}

export function setProgrammerFixtureValue(
	api: ApiDriver,
	intent: SetProgrammerFixtureValueIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerValues(
		api,
		intent,
		{
			action: "set_fixture",
			fixtureId: intent.fixtureId,
			attribute: intent.attribute,
			value: intent.value,
			timing: intent.timing,
		},
		dependencies,
	);
}

export function releaseProgrammerFixtureValue(
	api: ApiDriver,
	intent: ReleaseProgrammerFixtureValueIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerValues(
		api,
		intent,
		{
			action: "release_fixture",
			fixtureId: intent.fixtureId,
			attribute: intent.attribute,
		},
		dependencies,
	);
}

export function setProgrammerGroupValue(
	api: ApiDriver,
	intent: SetProgrammerGroupValueIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerValues(
		api,
		intent,
		{
			action: "set_group",
			groupId: intent.groupId,
			attribute: intent.attribute,
			value: intent.value,
			timing: intent.timing,
		},
		dependencies,
	);
}

export function releaseProgrammerGroupValue(
	api: ApiDriver,
	intent: ReleaseProgrammerGroupValueIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerValues(
		api,
		intent,
		{
			action: "release_group",
			groupId: intent.groupId,
			attribute: intent.attribute,
		},
		dependencies,
	);
}

export function batchProgrammerValues(
	api: ApiDriver,
	intent: BatchProgrammerValuesIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerValues(
		api,
		intent,
		{
			action: "batch",
			mutations: intent.mutations,
		},
		dependencies,
	);
}

export function clearProgrammerValues(
	api: ApiDriver,
	intent: ProgrammerValuesApiIntent,
	dependencies: IntentHttpDependencies = {},
) {
	return applyProgrammerValues(api, intent, { action: "clear" }, dependencies);
}

async function applyProgrammerValues(
	api: ApiDriver,
	intent: ProgrammerValuesApiIntent,
	action: ProgrammerValuesCommand,
	dependencies: IntentHttpDependencies,
): Promise<ProgrammerValuesActionOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const fetch = intentFetch(dependencies);
	const scope = { showId: intent.showId, userId: session.user.id };
	const transport = valuesTransport(api, session, fetch);
	const [playback, values, captureMode] = await Promise.all([
		loadPlaybackAuthority(api, session, fetch),
		transport.loadSnapshot(scope),
		captureModeTransport(api, session, fetch).loadSnapshot(scope),
	]);
	assertPlaybackScope(
		playback.desk.desk_id,
		playback.desk.scope.show_id,
		session,
		intent.showId,
	);
	assertNormalCapture(captureMode.projection);
	assertCurrentSession(api, session);
	return transport.applyAction(scope, {
		requestId: intentRequestId(dependencies),
		expectedRevision: values.projection.revision,
		expectedCaptureModeRevision: captureMode.projection.revision,
		action,
	});
}

function valuesTransport(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerValuesTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		fetch,
	});
}

function captureModeTransport(
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

async function loadPlaybackAuthority(
	api: ApiDriver,
	session: Session,
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
		throw new Error(
			`Playback runtime snapshot returned HTTP ${response.status}`,
		);
	return decodePlaybackSnapshot(value);
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

function assertNormalCapture(projection: ProgrammerCaptureModeProjection) {
	if (capturesProgrammerWrites(projection))
		throw new Error(
			"Normal Programmer values are disabled while Preload capture is active",
		);
}

function assertCurrentSession(api: ApiDriver, captured: Session) {
	const current = api.session;
	if (
		!current ||
		current.session_id !== captured.session_id ||
		current.client_id !== captured.client_id ||
		current.token !== captured.token ||
		current.user.id !== captured.user.id ||
		current.desk.id !== captured.desk.id
	)
		throw new Error("Programmer values scope changed before mutation");
}

function validateIntent(intent: ProgrammerValuesApiIntent) {
	if (intent.surface !== "api")
		throw new Error("Programmer values helper supports only the API surface");
	programmerValuesUuidAt(intent.showId, "$.showId");
}
