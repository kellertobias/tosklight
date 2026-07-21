import type {
	PlaybackAction,
	PlaybackActionRequest,
	PlaybackAddress,
} from "../../api/types";
import type { PlaybackIdentity } from "./contracts";

type CueListRuntimeSource = {
	identity: Extract<PlaybackIdentity, { kind: "playback" | "cue_list" }>;
};

export function cueListReleaseRequest(
	source: CueListRuntimeSource,
): PlaybackActionRequest {
	const address: PlaybackAddress =
		source.identity.kind === "playback"
			? {
					kind: "playback",
					playback_number: source.identity.playback_number,
				}
			: {
					kind: "cue_list",
					cue_list_id: source.identity.cue_list_id,
				};
	return actionRequest(address, { type: "release" });
}

export function groupActionRequest(
	groupId: string,
	action: PlaybackAction,
): PlaybackActionRequest {
	assertGroupId(groupId);
	return actionRequest({ kind: "group", group_id: groupId }, action);
}

function actionRequest(
	address: PlaybackAddress,
	action: PlaybackAction,
): PlaybackActionRequest {
	return {
		request_id: crypto.randomUUID(),
		address,
		action,
		surface: "virtual",
	};
}

export function isPlaybackSafetyRelease(
	action: string,
	pressed: boolean | undefined,
) {
	return (
		pressed === false &&
		(action === "button" || action === "flash" || action === "swap")
	);
}

export function isRetryablePlaybackFailure(reason: unknown) {
	if (reason instanceof TypeError) return true;
	if (!(reason instanceof Error)) return false;
	const failure = reason as Error & { retryable?: unknown; status?: unknown };
	if (typeof failure.retryable === "boolean") return failure.retryable;
	return (
		failure.status === 0 ||
		failure.status === 408 ||
		failure.status === 429 ||
		(typeof failure.status === "number" && failure.status >= 500)
	);
}

export function playbackActionError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

export function assertGroupMaster(value: number) {
	if (!Number.isFinite(value) || value < 0 || value > 1)
		throw new Error("Group master must be between 0 and 1");
}

function assertGroupId(groupId: string) {
	if (
		!groupId.length ||
		new TextEncoder().encode(groupId).length > 256 ||
		/\p{Cc}/u.test(groupId)
	)
		throw new Error("Group ID is invalid");
}

export function assertPlaybackPage(page: number) {
	if (!Number.isSafeInteger(page) || page < 1 || page > 127)
		throw new Error("Playback page must be an integer between 1 and 127");
}

export function assertPlaybackPageOutcome(
	outcome: {
		desk_id: string;
		page: number;
		event_sequence: number | null;
		page_creation_event_sequence: number | null;
	},
	deskId: string,
	page: number,
) {
	if (outcome.desk_id !== deskId || outcome.page !== page)
		throw new Error(
			"Playback page response does not match the active desk request",
		);
	assertOptionalSequence(outcome.event_sequence, "event sequence");
	assertOptionalSequence(
		outcome.page_creation_event_sequence,
		"page creation event sequence",
	);
	if (outcome.page_creation_event_sequence !== null)
		throw new Error("Playback page selection unexpectedly created a Page");
}

function assertOptionalSequence(value: number | null, label: string) {
	if (value != null && (!Number.isSafeInteger(value) || value < 0))
		throw new Error(`Playback page response ${label} is invalid`);
}
