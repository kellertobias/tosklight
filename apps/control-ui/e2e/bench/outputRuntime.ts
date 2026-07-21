import { HttpOutputRuntimeTransport } from "../../src/api/OutputRuntimeTransport";
import { decodePlaybackSnapshot } from "../../src/api/playbackWire";
import { programmingUuidAt } from "../../src/api/programmingWireProjection";
import type { OutputRuntimeActionOutcome } from "../../src/features/outputRuntime/contracts";
import { assertOutputMutation } from "../../src/features/outputRuntime/projectionValue";
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

export interface SetOutputRuntimeIntent {
	surface: "api";
	showId: string;
	grandMaster?: number;
	blackout?: boolean;
}

/** Applies one Output mutation against freshly captured active-Show authority. */
export async function setOutputRuntime(
	api: ApiDriver,
	intent: SetOutputRuntimeIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<OutputRuntimeActionOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const sessionKey = capturedSessionKey(session);
	const fetch = intentFetch(dependencies);
	const scope = await loadActiveScope(api, session, intent.showId, fetch);
	assertCurrentSession(api, session, sessionKey);
	const transport = outputTransport(api, session, fetch);
	const snapshot = await transport.loadSnapshot(scope);
	assertCurrentSession(api, session, sessionKey);
	return transport.applyAction(scope, {
		requestId: intentRequestId(dependencies),
		expectedShowId: scope.showId,
		expectedRevision: snapshot.projection.revision,
		grandMaster: intent.grandMaster,
		blackout: intent.blackout,
	});
}

async function loadActiveScope(
	api: ApiDriver,
	session: Session,
	expectedShowId: string,
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
	const snapshot = decodePlaybackSnapshot(value);
	assertActiveScope(snapshot, session, expectedShowId);
	return {
		showId: snapshot.desk.scope.show_id,
		deskId: snapshot.desk.desk_id,
	};
}

function assertActiveScope(
	snapshot: ReturnType<typeof decodePlaybackSnapshot>,
	session: Session,
	expectedShowId: string,
) {
	if (snapshot.projections.length !== 0)
		throw new Error("Empty Playback authority request returned projections");
	if (snapshot.desk.desk_id.toLowerCase() !== session.desk.id.toLowerCase())
		throw new Error(
			`Playback snapshot belongs to foreign desk ${snapshot.desk.desk_id}`,
		);
	if (
		snapshot.desk.scope.show_id.toLowerCase() !== expectedShowId.toLowerCase()
	)
		throw new Error(
			`Playback snapshot belongs to foreign Show ${snapshot.desk.scope.show_id}`,
		);
}

function outputTransport(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpOutputRuntimeTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedDeskId: session.desk.id,
		fetch,
	});
}

function capturedSessionKey(session: Session) {
	return [
		session.session_id,
		session.client_id,
		session.token,
		session.user.id,
		session.desk.id,
	].join("|");
}

function assertCurrentSession(
	api: ApiDriver,
	captured: Session,
	capturedKey: string,
) {
	const current = api.session;
	if (current !== captured || capturedSessionKey(current) !== capturedKey)
		throw new Error("API session changed while capturing Output authority");
}

function validateIntent(intent: SetOutputRuntimeIntent) {
	if (intent.surface !== "api")
		throw new Error("Output runtime helper supports only the API surface");
	programmingUuidAt(intent.showId, "$.showId");
	assertOutputMutation(intent.grandMaster, intent.blackout);
}
