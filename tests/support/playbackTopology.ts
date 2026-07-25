import type { ApiDriver } from "../../apps/control-ui/e2e/bench/core/api";
import type { PlaybackRuntimeSnapshot } from "../../apps/control-ui/src/api/generated/light-wire";
import { HttpPlaybackTopologyTransport } from "../../apps/control-ui/src/api/PlaybackTopologyTransport";
import { encodePlaybackTopologyRequest } from "../../apps/control-ui/src/api/playbackTopologyWire";
import type {
	PlaybackDefinition,
	PlaybackPage,
} from "../../apps/control-ui/src/api/types";
import type {
	PlaybackTopologyAction,
	PlaybackTopologyOutcome,
} from "../../apps/control-ui/src/features/playbackTopology/contracts";
import { activeShowId, objects, type VersionedObject } from "./catalog";

export interface PlaybackSlotAuthority {
	showId: string;
	showRevision: number;
	page: VersionedObject<PlaybackPage> | null;
	playback: VersionedObject<PlaybackDefinition> | null;
}

export async function playbackSlotAuthority(
	api: ApiDriver,
	page: number,
	slot: number,
): Promise<PlaybackSlotAuthority> {
	const [showId, pages, playbacks, runtime] = await Promise.all([
		activeShowId(api),
		objects<PlaybackPage>(api, "playback_page"),
		objects<PlaybackDefinition>(api, "playback"),
		api.request<PlaybackRuntimeSnapshot>(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{
				identities: [],
			},
		),
	]);
	const pageObject =
		pages.find((candidate) => candidate.body.number === page) ?? null;
	const playbackNumber = pageObject?.body.slots[String(slot)];
	const playback =
		playbacks.find((candidate) => candidate.body.number === playbackNumber) ??
		null;
	if (runtime.desk.scope.show_id !== showId)
		throw new Error(
			"Playback topology authority crossed an active Show change",
		);
	return {
		showId,
		showRevision: runtime.desk.scope.show_revision,
		page: pageObject,
		playback,
	};
}

export async function configurePlaybackSlot<TPlayback extends object>(
	api: ApiDriver,
	page: number,
	slot: number,
	playback: TPlayback,
) {
	const authority = await playbackSlotAuthority(api, page, slot);
	const outcome = await applyTopology(api, authority, {
		type: "configure_slot",
		page,
		slot,
		expectedPageRevision: authority.page?.revision ?? 0,
		expectedPageObjectId: authority.page?.id ?? null,
		expectedPlaybackRevision: authority.playback?.revision ?? 0,
		expectedPlaybackObjectId: authority.playback?.id ?? null,
		playback: playback as unknown as PlaybackDefinition,
	});
	return {
		outcome,
		playback: presentBody<PlaybackDefinition>(outcome, "playback"),
		page: presentBody<PlaybackPage>(outcome, "playback_page"),
	};
}

export async function clearMappedPlaybackSlot(
	api: ApiDriver,
	page: number,
	slot: number,
) {
	const authority = await playbackSlotAuthority(api, page, slot);
	const outcome = await applyTopology(api, authority, {
		type: "clear_mapped_playback",
		page,
		slot,
		expectedPageRevision: authority.page?.revision ?? 0,
		expectedPageObjectId: authority.page?.id ?? null,
		expectedPlaybackRevision: authority.playback?.revision ?? 0,
		expectedPlaybackObjectId: authority.playback?.id ?? null,
	});
	return { outcome };
}

export async function postPlaybackTopologyAction(
	api: ApiDriver,
	showId: string,
	showRevision: number,
	action: PlaybackTopologyAction,
): Promise<Response> {
	if (!api.session) throw new Error("API session is not initialized");
	return fetch(`${api.baseUrl}/api/v2/playback-topology/actions`, {
		method: "POST",
		headers: {
			authorization: `Bearer ${api.session.token}`,
			"content-type": "application/json",
			"if-match": `"${showRevision}"`,
			"x-tosk-show": showId,
		},
		body: JSON.stringify(
			encodePlaybackTopologyRequest({
				requestId: crypto.randomUUID(),
				action,
			}),
		),
	});
}

async function applyTopology(
	api: ApiDriver,
	authority: PlaybackSlotAuthority,
	action: PlaybackTopologyAction,
) {
	if (!api.session) throw new Error("API session is not initialized");
	return new HttpPlaybackTopologyTransport({
		baseUrl: api.baseUrl,
		sessionToken: api.session.token,
	}).apply(authority.showId, authority.showRevision, {
		requestId: crypto.randomUUID(),
		action,
	});
}

function presentBody<T>(
	outcome: PlaybackTopologyOutcome,
	kind: "playback" | "playback_page",
): T {
	const projection = outcome.objects.find(
		(candidate) => candidate.kind === kind && candidate.state === "present",
	);
	if (!projection || projection.state !== "present")
		throw new Error(`Playback topology outcome omitted ${kind}`);
	return projection.body as T;
}
