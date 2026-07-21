import { decodePlaybackSnapshot } from "../../src/api/playbackWire";
import {
	decodeSelectionActionOutcome,
	encodeSelectionActionRequest,
} from "../../src/api/programmingSelectionWire";
import { decodeProgrammingInteractionSnapshot } from "../../src/api/programmingWire";
import { programmingUuidAt } from "../../src/api/programmingWireProjection";
import { WireValidationError } from "../../src/api/wireValidation";
import type {
	SelectionAction,
	SelectionActionOutcome,
	SelectionGestureSource,
	SelectionRule,
} from "../../src/features/programmingInteraction/contracts";
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

export interface ProgrammingSelectionApiIntent {
	surface: "api";
	showId: string;
}

export interface ActiveProgrammingSelectionApiIntent {
	surface: "api";
}

export interface ReplaceProgrammingSelectionIntent
	extends ProgrammingSelectionApiIntent {
	fixtures: readonly string[];
}

export interface GestureProgrammingSelectionIntent
	extends ProgrammingSelectionApiIntent {
	source: SelectionGestureSource;
	remove?: boolean;
}

export interface SelectProgrammingGroupIntent
	extends ProgrammingSelectionApiIntent {
	groupId: string;
	frozen: boolean;
	rule: SelectionRule;
}

export interface ReplaceActiveProgrammingSelectionIntent
	extends ActiveProgrammingSelectionApiIntent {
	fixtures: readonly string[];
}

export interface GestureActiveProgrammingSelectionIntent
	extends ActiveProgrammingSelectionApiIntent {
	source: SelectionGestureSource;
	remove?: boolean;
}

export class ProgrammingSelectionHttpError extends Error {
	readonly name = "ProgrammingSelectionHttpError";

	constructor(
		readonly status: number,
		readonly payload: unknown,
	) {
		super(selectionErrorMessage(status, payload));
	}
}

export function replaceProgrammingSelection(
	api: ApiDriver,
	intent: ReplaceProgrammingSelectionIntent,
	dependencies: IntentHttpDependencies = {},
) {
	for (const [index, fixtureId] of intent.fixtures.entries())
		programmingUuidAt(fixtureId, `$.fixtures[${index}]`);
	return applySelection(api, intent, intent.showId, dependencies, (revision) => ({
		type: "replace",
		fixtures: intent.fixtures,
		expectedRevision: revision,
	}));
}

export function gestureProgrammingSelection(
	api: ApiDriver,
	intent: GestureProgrammingSelectionIntent,
	dependencies: IntentHttpDependencies = {},
) {
	validateGestureSource(intent.source);
	return applySelection(api, intent, intent.showId, dependencies, () => ({
		type: "gesture",
		source: intent.source,
		remove: intent.remove ?? false,
	}));
}

export function selectProgrammingGroup(
	api: ApiDriver,
	intent: SelectProgrammingGroupIntent,
	dependencies: IntentHttpDependencies = {},
) {
	validateGroupId(intent.groupId);
	return applySelection(api, intent, intent.showId, dependencies, (revision) => ({
		type: "select_group",
		groupId: intent.groupId,
		frozen: intent.frozen,
		rule: intent.rule,
		expectedRevision: revision,
	}));
}

export function replaceActiveProgrammingSelection(
	api: ApiDriver,
	intent: ReplaceActiveProgrammingSelectionIntent,
	dependencies: IntentHttpDependencies = {},
) {
	for (const [index, fixtureId] of intent.fixtures.entries())
		programmingUuidAt(fixtureId, `$.fixtures[${index}]`);
	return applySelection(api, intent, null, dependencies, (revision) => ({
		type: "replace",
		fixtures: intent.fixtures,
		expectedRevision: revision,
	}));
}

export function gestureActiveProgrammingSelection(
	api: ApiDriver,
	intent: GestureActiveProgrammingSelectionIntent,
	dependencies: IntentHttpDependencies = {},
) {
	validateGestureSource(intent.source);
	return applySelection(api, intent, null, dependencies, () => ({
		type: "gesture",
		source: intent.source,
		remove: intent.remove ?? false,
	}));
}

async function applySelection(
	api: ApiDriver,
	intent: ActiveProgrammingSelectionApiIntent,
	expectedShowId: string | null,
	dependencies: IntentHttpDependencies,
	actionAtRevision: (revision: number) => SelectionAction,
): Promise<SelectionActionOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const fetch = intentFetch(dependencies);
	const [playback, programming] = await Promise.all([
		loadPlaybackAuthority(api, session, fetch),
		loadProgrammingAuthority(api, session, fetch),
	]);
	assertAuthority(playback.desk, session, expectedShowId);
	assertCurrentSession(api, session);
	const requestId = intentRequestId(dependencies);
	const action = actionAtRevision(programming.projection.selection.revision);
	const outcome = await postSelection(api, session, requestId, action, fetch);
	assertCurrentSession(api, session);
	return outcome;
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
	return decodePlaybackSnapshot(await successfulJson(response, "Playback runtime"));
}

async function loadProgrammingAuthority(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
) {
	const path = `/api/v2/desks/${encodeURIComponent(session.desk.id)}/programming-interaction/snapshot`;
	const response = await fetch(intentUrl(api, path), {
		headers: intentHeaders(session),
	});
	const value = await successfulJson(response, "Programming interaction");
	return decodeProgrammingInteractionSnapshot(value, session.desk.id);
}

async function postSelection(
	api: ApiDriver,
	session: Session,
	requestId: string,
	action: SelectionAction,
	fetch: typeof globalThis.fetch,
) {
	const path = `/api/v2/desks/${encodeURIComponent(session.desk.id)}/programming-selection/actions`;
	const response = await fetch(intentUrl(api, path), {
		method: "POST",
		headers: { ...intentHeaders(session), "content-type": "application/json" },
		body: JSON.stringify(encodeSelectionActionRequest({ requestId, action })),
	});
	const value = await responseJson(response, "Programming selection action");
	if (!response.ok) throw new ProgrammingSelectionHttpError(response.status, value);
	const outcome = decodeSelectionActionOutcome(value, requestId);
	const expectedAction = acceptedAction(action);
	if (outcome.action !== expectedAction)
		throw new WireValidationError("$.action", expectedAction, outcome.action);
	return outcome;
}

function acceptedAction(action: SelectionAction) {
	switch (action.type) {
		case "replace":
			return "replaced" as const;
		case "gesture":
			return "gesture_applied" as const;
		case "select_group":
			return "group_selected" as const;
		case "apply_rule":
			return "rule_applied" as const;
	}
}

async function successfulJson(response: Response, label: string) {
	const value = await responseJson(response, label);
	if (!response.ok) throw new ProgrammingSelectionHttpError(response.status, value);
	return value;
}

function assertAuthority(
	desk: { desk_id: string; scope: { show_id: string } },
	session: Session,
	showId: string | null,
) {
	if (desk.desk_id !== session.desk.id)
		throw new Error(`Programming selection authority belongs to foreign desk ${desk.desk_id}`);
	if (showId !== null && desk.scope.show_id !== showId)
		throw new Error(`Programming selection authority belongs to foreign Show ${desk.scope.show_id}`);
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
		throw new Error("Programming selection scope changed during mutation");
}

function validateIntent(intent: ActiveProgrammingSelectionApiIntent) {
	if (intent.surface !== "api")
		throw new Error("Programming selection helper supports only the API surface");
	if ("showId" in intent) programmingUuidAt(intent.showId, "$.showId");
}

function validateGestureSource(source: SelectionGestureSource) {
	if (source.type === "fixture")
		programmingUuidAt(source.fixtureId, "$.source.fixtureId");
	else validateGroupId(source.groupId);
}

function validateGroupId(groupId: string) {
	if (
		groupId.trim().length === 0 ||
		groupId.length > 256 ||
		[...groupId].some((character) => /\p{Cc}/u.test(character))
	)
		throw new Error("Group ID must contain 1-256 printable characters");
}

function selectionErrorMessage(status: number, payload: unknown) {
	if (
		typeof payload === "object" &&
		payload !== null &&
		"error" in payload &&
		typeof payload.error === "string"
	)
		return payload.error;
	return `Programming selection request returned HTTP ${status}`;
}
