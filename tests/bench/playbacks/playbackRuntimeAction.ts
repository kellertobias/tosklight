import { ApiRequestError } from "../../../apps/light-desktop/src/api/ApiRequestError";
import { PlaybackApiClient } from "../../../apps/light-desktop/src/api/client/playback";
import type { LiveClientTransport } from "../../../apps/light-desktop/src/api/client/transport";
import type {
	PlaybackActionOutcome,
	PlaybackRuntimeIdentity,
	PlaybackRuntimeSnapshot,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import { programmingUuidAt } from "../../../apps/light-desktop/src/api/programmingWireProjection";
import type { ApiDriver, Session } from "../core/api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentSession,
	intentUrl,
} from "../core/v2IntentHttp";

export interface GoCueListPlaybackIntent {
	surface: "api";
	showId: string;
	playbackAddress: { page: number; position: number };
	cueListId: string;
}

/** Sends one direct Cuelist GO after capturing its exact v2 Playback authority. */
export async function goCueListPlayback(
	api: ApiDriver,
	intent: GoCueListPlaybackIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<PlaybackActionOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const sessionKey = capturedSessionKey(session);
	const client = playbackClient(api, session, intentFetch(dependencies));
	const identities = runtimeIdentities(intent);
	const snapshot = await client.playbackRuntimeSnapshot(
		session.desk.id,
		identities,
	);
	assertCurrentSession(api, session, sessionKey);
	const revision = assertAuthority(snapshot, session, intent);
	const outcome = await client.playbackRuntimeAction(
		intent.showId,
		session.desk.id,
		{
			address: { kind: "cue_list", cue_list_id: intent.cueListId },
			action: { type: "go", pressed: true },
			control_source: { surface_id: "main", control_id: 1 },
		},
	);
	assertCurrentSession(api, session, sessionKey);
	assertOutcome(outcome, session, intent, revision);
	return outcome;
}

function runtimeIdentities(
	intent: GoCueListPlaybackIntent,
): PlaybackRuntimeIdentity[] {
	return [
		{ kind: "cue_list", cue_list_id: intent.cueListId },
		{ kind: "playback", ...intent.playbackAddress },
	];
}

function assertAuthority(
	snapshot: PlaybackRuntimeSnapshot,
	session: Session,
	intent: GoCueListPlaybackIntent,
) {
	assertDeskScope(snapshot, session, intent.showId);
	const revision = snapshot.desk.scope.show_revision;
	for (const projection of snapshot.projections) {
		assertProjectionScope(projection, intent.showId, revision);
		assertRequestedProjection(projection, intent);
	}
	const cueList = snapshot.projections.some(
		(projection) =>
			projection.identity.kind === "cue_list" &&
			sameId(projection.identity.cue_list_id, intent.cueListId) &&
			isCueListProjection(projection, intent.cueListId),
	);
	const playback = snapshot.projections.some(
		(projection) =>
			projection.identity.kind === "playback" &&
			sameAddress(projection.identity, intent.playbackAddress) &&
			isCueListProjection(projection, intent.cueListId),
	);
	if (!cueList || !playback)
		throw new Error(
			"Playback snapshot does not contain the exact Playback/Cuelist authority",
		);
	return revision;
}

function assertRequestedProjection(
	projection: PlaybackRuntimeSnapshot["projections"][number],
	intent: GoCueListPlaybackIntent,
) {
	const requested = projection.identity;
	const matchesCueList =
		requested.kind === "cue_list" &&
		sameId(requested.cue_list_id, intent.cueListId) &&
		isCueListProjection(projection, intent.cueListId);
	const matchesPlayback =
		requested.kind === "playback" &&
		sameAddress(requested, intent.playbackAddress) &&
		isCueListProjection(projection, intent.cueListId);
	if (!matchesCueList && !matchesPlayback)
		throw new Error("Playback snapshot contains unrelated runtime authority");
}

function assertDeskScope(
	snapshot: PlaybackRuntimeSnapshot,
	session: Session,
	showId: string,
) {
	if (!sameId(snapshot.desk.desk_id, session.desk.id))
		throw new Error(
			`Playback snapshot belongs to foreign desk ${snapshot.desk.desk_id}`,
		);
	if (!sameId(snapshot.desk.scope.show_id, showId))
		throw new Error(
			`Playback snapshot belongs to foreign Show ${snapshot.desk.scope.show_id}`,
		);
}

function assertProjectionScope(
	projection: { scope: { show_id: string; show_revision: number } },
	showId: string,
	revision: number,
) {
	if (!sameId(projection.scope.show_id, showId))
		throw new Error(
			`Playback projection belongs to foreign Show ${projection.scope.show_id}`,
		);
	if (projection.scope.show_revision !== revision)
		throw new Error(
			"Playback authority revision changed during snapshot capture",
		);
}

function isCueListProjection(
	projection: PlaybackRuntimeSnapshot["projections"][number],
	cueListId: string,
) {
	return (
		projection.target === "cue_list" &&
		sameId(projection.cue_list_id, cueListId)
	);
}

function assertOutcome(
	outcome: PlaybackActionOutcome,
	session: Session,
	intent: GoCueListPlaybackIntent,
	revision: number,
) {
	if (
		outcome.requested.kind !== "cue_list" ||
		!sameId(outcome.requested.cue_list_id, intent.cueListId) ||
		outcome.resolved.kind !== "cue_list" ||
		!sameId(outcome.resolved.cue_list_id, intent.cueListId)
	)
		throw new Error("Playback outcome does not match the requested Cuelist");
	if (
		outcome.projection.identity.kind !== "cue_list" ||
		!sameId(outcome.projection.identity.cue_list_id, intent.cueListId) ||
		!isCueListProjection(outcome.projection, intent.cueListId)
	)
		throw new Error(
			"Playback outcome does not contain the direct Cuelist projection",
		);
	assertProjectionScope(outcome.projection, intent.showId, revision);
	if (outcome.desk !== null) {
		if (!sameId(outcome.desk.desk_id, session.desk.id))
			throw new Error(
				`Playback outcome belongs to foreign desk ${outcome.desk.desk_id}`,
			);
		assertProjectionScope(outcome.desk, intent.showId, revision);
	}
}

function playbackClient(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	const request: LiveClientTransport["request"] = async <T>(
		path: string,
		init: RequestInit = {},
	) => {
		const headers = new Headers(init.headers);
		headers.set("authorization", `Bearer ${session.token}`);
		const response = await fetch(intentUrl(api, path), { ...init, headers });
		if (!response.ok) throw await apiRequestError(response);
		return response.json() as Promise<T>;
	};
	const unsupported = () =>
		Promise.reject(new Error("Playback acceptance intents use HTTP only"));
	return new PlaybackApiClient({
		request,
		blob: unsupported,
		absoluteUrl: (path) => intentUrl(api, path),
		currentDeskId: () => session.desk.id,
		sendAction: unsupported,
	});
}

async function apiRequestError(response: Response) {
	const body = await response.text();
	return new ApiRequestError(
		body || `${response.status} ${response.statusText}`,
		response.status,
	);
}

function capturedSessionKey(session: Session) {
	return [
		session.session_id,
		session.client_id,
		session.token,
		session.session_id,
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
		throw new Error("API session changed while applying a Playback action");
}

function validateIntent(intent: GoCueListPlaybackIntent) {
	if (intent.surface !== "api")
		throw new Error("Playback runtime helper supports only the API surface");
	programmingUuidAt(intent.showId, "$.showId");
	programmingUuidAt(intent.cueListId, "$.cueListId");
	for (const [value, maximum, label] of [
		[intent.playbackAddress.page, 99, "Page"],
		[intent.playbackAddress.position, 1000, "position"],
	] as const) {
		if (!Number.isSafeInteger(value) || value < 1 || value > maximum)
			throw new Error(`Playback ${label} must be between 1 and ${maximum}`);
	}
}

function sameId(left: string, right: string) {
	return left.toLowerCase() === right.toLowerCase();
}

function sameAddress(
	left: { page: number; position: number },
	right: { page: number; position: number },
) {
	return left.page === right.page && left.position === right.position;
}
