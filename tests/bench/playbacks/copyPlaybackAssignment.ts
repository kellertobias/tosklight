import { HttpPlaybackTopologyTransport } from "../../../apps/light-desktop/src/api/PlaybackTopologyTransport";
import { HttpShowObjectSnapshotTransport } from "../../../apps/light-desktop/src/api/ShowObjectSnapshotTransport";
import { decodePlaybackSnapshot } from "../../../apps/light-desktop/src/api/playbackWire";
import { programmingUuidAt } from "../../../apps/light-desktop/src/api/programmingWireProjection";
import type {
	PlaybackAddress,
	PlaybackTopologyOutcome,
} from "../../../apps/light-desktop/src/features/playbackTopology/contracts";
import type { PlaybackDefinition } from "../../../apps/light-desktop/src/api/types";
import type { ApiDriver, Session } from "../core/api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentContextHeaders,
	intentRequestId,
	intentSession,
	intentUrl,
	responseJson,
} from "../core/v2IntentHttp";

export interface CopyPlaybackAssignmentIntent {
	surface: "api";
	showId: string;
	sourceAddress: PlaybackAddress;
	targetAddress: PlaybackAddress;
}

export async function copyPlaybackAssignment(
	api: ApiDriver,
	intent: CopyPlaybackAssignmentIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<PlaybackTopologyOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const fetch = intentFetch(dependencies);
	const sessionKey = sessionIdentity(session);
	const authority = await loadAuthority(api, session, intent, fetch);
	assertSession(api, sessionKey);
	const outcome = await topology(api, session, fetch).apply(
		intent.showId,
		authority.showRevision,
		{
			requestId: intentRequestId(dependencies),
			action: {
				type: "assign_playback",
				address: intent.targetAddress,
				definition: authority.definition,
				expectedPageRevision: authority.pageRevision,
				expectedPageObjectId: authority.pageObjectId,
			},
		},
	);
	assertSession(api, sessionKey);
	return outcome;
}

interface MapAuthority {
	showRevision: number;
	pageRevision: number;
	pageObjectId: string | null;
	definition: PlaybackDefinition;
}

async function loadAuthority(
	api: ApiDriver,
	session: Session,
	intent: CopyPlaybackAssignmentIntent,
	fetch: typeof globalThis.fetch,
): Promise<MapAuthority> {
	const snapshots = showObjects(api, session, fetch);
	const activePromise = loadActiveShow(api, session, fetch);
	const sourcePromise = snapshots.object(
		intent.showId,
		"playback_page",
		String(intent.sourceAddress.page),
	);
	const targetPromise =
		intent.targetAddress.page === intent.sourceAddress.page
			? sourcePromise
			: snapshots.object(
					intent.showId,
					"playback_page",
					String(intent.targetAddress.page),
				);
	const [activeShow, source, target] = await Promise.all([
		activePromise,
		sourcePromise,
		targetPromise,
	]);
	assertActiveShow(activeShow, session, intent.showId);
	assertShowRevision(
		activeShow.showRevision,
		source.showRevision,
		target.showRevision,
	);
	for (const [snapshot, address] of [
		[source, intent.sourceAddress],
		[target, intent.targetAddress],
	] as const) {
		if (
			snapshot.object !== null &&
			snapshot.object.body.number !== address.page
		)
			throw new Error(
				`Playback Page ${snapshot.object.id} has number ${snapshot.object.body.number}`,
			);
	}
	const definition =
		source.object?.body.assignments[intent.sourceAddress.position];
	if (!definition)
		throw new Error(
			`Playback ${intent.sourceAddress.page}.${intent.sourceAddress.position} does not exist`,
		);
	return {
		showRevision: activeShow.showRevision,
		pageRevision: target.object?.revision ?? 0,
		pageObjectId: target.object?.id ?? null,
		definition,
	};
}

function showObjects(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpShowObjectSnapshotTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		fetch,
	});
}

function topology(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	return new HttpPlaybackTopologyTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		fetch,
	});
}

async function loadActiveShow(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	const path = "/api/v2/playback-runtime/snapshot";
	const response = await fetch(intentUrl(api, path), {
		method: "POST",
		headers: {
			...intentContextHeaders(session),
			"content-type": "application/json",
		},
		body: JSON.stringify({ identities: [] }),
	});
	const value = await responseJson(response, "Playback runtime");
	if (!response.ok)
		throw new Error(
			`Playback runtime snapshot returned HTTP ${response.status}`,
		);
	const snapshot = decodePlaybackSnapshot(value);
	return {
		deskId: snapshot.desk.desk_id,
		showId: snapshot.desk.scope.show_id,
		showRevision: snapshot.desk.scope.show_revision,
	};
}

function assertActiveShow(
	authority: { deskId: string; showId: string },
	session: Session,
	showId: string,
) {
	if (authority.deskId.toLowerCase() !== session.desk.id.toLowerCase())
		throw new Error(
			`Playback snapshot belongs to foreign desk ${authority.deskId}`,
		);
	if (authority.showId.toLowerCase() !== showId.toLowerCase())
		throw new Error(
			`Playback snapshot belongs to foreign Show ${authority.showId}`,
		);
}

function assertShowRevision(
	activeRevision: number,
	pageRevision: number,
	playbackRevision: number,
) {
	if (pageRevision !== activeRevision || playbackRevision !== activeRevision)
		throw new Error(
			`Show authority changed while reading Page and Playback (${activeRevision}, ${pageRevision}, ${playbackRevision})`,
		);
}

function validateIntent(intent: CopyPlaybackAssignmentIntent) {
	if (intent.surface !== "api")
		throw new Error(
			"Playback assignment copy helper supports only the API surface",
		);
	programmingUuidAt(intent.showId, "$.showId");
	for (const address of [intent.sourceAddress, intent.targetAddress]) {
		boundedInteger(address.page, 99, "Page");
		boundedInteger(address.position, 1_000, "Playback position");
	}
}

function boundedInteger(value: number, maximum: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1 || value > maximum)
		throw new Error(`${label} number must be between 1 and ${maximum}`);
}

function sessionIdentity(session: Session) {
	return JSON.stringify([session.session_id, session.token, session.desk.id]);
}
function assertSession(api: ApiDriver, captured: string) {
	if (!api.session || sessionIdentity(api.session) !== captured)
		throw new Error("Playback copy session changed during mutation");
}
