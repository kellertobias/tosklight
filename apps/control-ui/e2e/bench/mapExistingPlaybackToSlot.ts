import { HttpPlaybackTopologyTransport } from "../../src/api/PlaybackTopologyTransport";
import { HttpShowObjectSnapshotTransport } from "../../src/api/ShowObjectSnapshotTransport";
import { decodePlaybackSnapshot } from "../../src/api/playbackWire";
import { programmingUuidAt } from "../../src/api/programmingWireProjection";
import type { PlaybackTopologyOutcome } from "../../src/features/playbackTopology/contracts";
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

export interface MapExistingPlaybackToSlotIntent {
	surface: "api";
	showId: string;
	page: number;
	slot: number;
	playbackNumber: number;
}

export async function mapExistingPlaybackToSlot(
	api: ApiDriver,
	intent: MapExistingPlaybackToSlotIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<PlaybackTopologyOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const fetch = intentFetch(dependencies);
	const authority = await loadAuthority(api, session, intent, fetch);
	return topology(api, session, fetch).apply(intent.showId, authority.showRevision, {
		requestId: intentRequestId(dependencies),
		action: {
			type: "map_existing_playback",
			page: intent.page,
			slot: intent.slot,
			playbackNumber: intent.playbackNumber,
			expectedPageRevision: authority.pageRevision,
			expectedPageObjectId: authority.pageObjectId,
			expectedPlaybackRevision: authority.playbackRevision,
			expectedPlaybackObjectId: authority.playbackObjectId,
		},
	});
}

interface MapAuthority {
	showRevision: number;
	pageRevision: number;
	pageObjectId: string | null;
	playbackRevision: number;
	playbackObjectId: string;
}

async function loadAuthority(
	api: ApiDriver,
	session: Session,
	intent: MapExistingPlaybackToSlotIntent,
	fetch: typeof globalThis.fetch,
): Promise<MapAuthority> {
	const snapshots = showObjects(api, session, fetch);
	const [activeShow, page, playback] = await Promise.all([
		loadActiveShow(api, session, fetch),
		snapshots.object(intent.showId, "playback_page", String(intent.page)),
		snapshots.object(
			intent.showId,
			"playback",
			String(intent.playbackNumber),
		),
	]);
	assertActiveShow(activeShow, session, intent.showId);
	assertShowRevision(activeShow.showRevision, page.showRevision, playback.showRevision);
	if (page.object !== null && page.object.body.number !== intent.page)
		throw new Error(`Playback Page ${page.object.id} has number ${page.object.body.number}`);
	if (playback.object === null)
		throw new Error(`Playback ${intent.playbackNumber} does not exist`);
	if (playback.object.body.number !== intent.playbackNumber)
		throw new Error(
			`Playback ${playback.object.id} has number ${playback.object.body.number}`,
		);
	return {
		showRevision: activeShow.showRevision,
		pageRevision: page.object?.revision ?? 0,
		pageObjectId: page.object?.id ?? null,
		playbackRevision: playback.object.revision,
		playbackObjectId: playback.object.id,
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
		throw new Error(`Playback snapshot belongs to foreign desk ${authority.deskId}`);
	if (authority.showId.toLowerCase() !== showId.toLowerCase())
		throw new Error(`Playback snapshot belongs to foreign Show ${authority.showId}`);
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

function validateIntent(intent: MapExistingPlaybackToSlotIntent) {
	if (intent.surface !== "api")
		throw new Error("Playback mapping helper supports only the API surface");
	programmingUuidAt(intent.showId, "$.showId");
	boundedInteger(intent.page, 127, "Page");
	boundedInteger(intent.slot, 127, "slot");
	boundedInteger(intent.playbackNumber, 1_000, "Playback");
}

function boundedInteger(value: number, maximum: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1 || value > maximum)
		throw new Error(`${label} number must be between 1 and ${maximum}`);
}
