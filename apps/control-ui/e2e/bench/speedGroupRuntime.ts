import { HttpSpeedGroupRuntimeTransport } from "../../src/api/SpeedGroupRuntimeTransport";
import type {
	SpeedGroupAction,
	SpeedGroupActionOutcome,
	SpeedGroupActionRequest,
} from "../../src/features/speedGroupRuntime/contracts";
import {
	assertAction,
	assertRequestId,
} from "../../src/features/speedGroupRuntime/projectionValue";
import { SpeedGroupTransportError } from "../../src/features/speedGroupRuntime/transport";
import type { ApiDriver, Session } from "./api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentRequestId,
	intentSession,
} from "./v2IntentHttp";

export interface SpeedGroupRuntimeIntent {
	surface: "api";
	action: SpeedGroupAction;
}

/** Applies one semantic Speed Group command against freshly captured desk authority. */
export async function applySpeedGroupRuntimeAction(
	api: ApiDriver,
	intent: SpeedGroupRuntimeIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<SpeedGroupActionOutcome> {
	validateIntent(intent);
	const requestId = intentRequestId(dependencies);
	assertRequestId(requestId);
	const session = intentSession(api);
	const sessionKey = capturedSessionKey(session);
	const scope = { deskId: session.desk.id };
	const transport = new HttpSpeedGroupRuntimeTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedDeskId: session.desk.id,
		fetch: intentFetch(dependencies),
	});
	const snapshot = await transport.loadSnapshot(scope);
	assertCurrentSession(api, session, sessionKey);
	const request: SpeedGroupActionRequest = {
		requestId,
		expectedAuthorityId: snapshot.projection.authorityId,
		expectedRevision: snapshot.projection.revision,
		expectedGroups: snapshot.projection.groups,
		action: intent.action,
	};
	const outcome = await applyWithOneRetry(
		api,
		session,
		sessionKey,
		transport,
		scope,
		request,
	);
	assertCurrentSession(api, session, sessionKey);
	return outcome;
}

async function applyWithOneRetry(
	api: ApiDriver,
	session: Session,
	sessionKey: string,
	transport: HttpSpeedGroupRuntimeTransport,
	scope: { deskId: string },
	request: SpeedGroupActionRequest,
) {
	try {
		return await transport.applyAction(scope, request);
	} catch (reason) {
		if (!isRetryable(reason)) throw reason;
		assertCurrentSession(api, session, sessionKey);
		return transport.applyAction(scope, request);
	}
}

function validateIntent(intent: SpeedGroupRuntimeIntent) {
	if (intent.surface !== "api")
		throw new Error("Speed Group runtime helper supports only the API surface");
	assertAction(intent.action);
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
	if (
		current !== captured ||
		current == null ||
		capturedSessionKey(current) !== capturedKey
	)
		throw new Error("API session changed while applying a Speed Group action");
}

function isRetryable(reason: unknown) {
	return reason instanceof SpeedGroupTransportError && reason.retryable;
}
