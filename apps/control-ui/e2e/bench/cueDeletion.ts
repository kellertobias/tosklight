import { decodePlaybackSnapshot } from "../../src/api/playbackWire";
import {
	booleanAt,
	enumAt,
	exactRecordAt,
	integerAt,
	numberAt,
	opaqueStringAt,
	positiveIntegerAt,
	stringAt,
} from "../../src/api/playbackWirePrimitives";
import { programmingUuidAt } from "../../src/api/programmingWireProjection";
import { decodeShowObjectBody } from "../../src/api/showObjectBodyWire";
import { HttpShowObjectSnapshotTransport } from "../../src/api/ShowObjectSnapshotTransport";
import type { ShowObject } from "../../src/features/showObjects/contracts";
import { WireValidationError } from "../../src/api/wireValidation";
import type { ApiDriver, Session } from "./api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentHeaders,
	intentRequestId,
	intentSession,
	intentUrl,
} from "./v2IntentHttp";

export type CueDeletionAddress =
	| { type: "pool"; playbackNumber: number }
	| { type: "current_page"; slot: number }
	| { type: "page_slot"; page: number; slot: number };

export interface DeleteCueIntent {
	surface: "api";
	address: CueDeletionAddress;
	cueNumber: number;
}

interface CueDeletionProjection {
	cueListId: string;
	objectId: string;
	objectRevision: number;
	body: ShowObject<"cue_list">["body"];
}

interface CueDeletionOutcomeBase {
	requestId: string;
	correlationId: string;
	replayed: boolean;
	showId: string;
	showRevision: number;
	cueList: CueDeletionProjection;
	deletedCue: { id: string; number: number };
	persistenceWarning: string | null;
}

export type CueDeletionOutcome =
	| (CueDeletionOutcomeBase & {
			status: "changed";
			showEventSequence: number;
	  })
	| (CueDeletionOutcomeBase & {
			status: "no_change";
			showEventSequence: null;
	  });

const ERROR_KINDS = [
	"invalid",
	"unauthorized",
	"forbidden",
	"not_found",
	"conflict",
	"unavailable",
	"internal",
] as const;

type CueDeletionErrorKind = (typeof ERROR_KINDS)[number];

export class CueDeletionActionError extends Error {
	readonly name = "CueDeletionActionError";

	constructor(
		message: string,
		readonly status: number,
		readonly kind: CueDeletionErrorKind,
		readonly currentRevision: number | null,
		readonly currentRelatedRevision: number | null,
		readonly retryable: boolean,
	) {
		super(message);
	}
}

interface ActiveShowAuthority {
	deskId: string;
	showId: string;
	showRevision: number;
	activePage: number;
}

interface ResolvedCueAuthority {
	address: Record<string, string | number>;
	playbackNumber: number;
	cueList: ShowObject<"cue_list">;
	cue: { id: string; number: number };
}

export async function deleteCue(
	api: ApiDriver,
	intent: DeleteCueIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<CueDeletionOutcome> {
	validateIntent(intent);
	const session = captureSession(intentSession(api));
	const fetch = intentFetch(dependencies);
	const active = await loadActiveShow(api, session, fetch);
	const authority = await resolveAuthority(api, session, fetch, active, intent);
	assertCurrentSession(api, session, "before mutation");
	const requestId = validRequestId(intentRequestId(dependencies));
	const response = await fetch(actionUrl(api, session, active), {
		method: "POST",
		headers: actionHeaders(session, active.showRevision),
		body: JSON.stringify(actionRequest(requestId, intent, authority)),
	});
	assertCurrentSession(api, session, "after response");
	const value = await readJson(response, "Cue deletion");
	assertCurrentSession(api, session, "after response body");
	if (!response.ok) throw decodeActionError(response, value);
	const outcome = decodeOutcome(value, requestId, active, authority);
	verifyEtag(response, outcome.showRevision);
	return outcome;
}

async function loadActiveShow(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
): Promise<ActiveShowAuthority> {
	const path = `/api/v2/desks/${encodeURIComponent(session.desk.id)}/playback-runtime/snapshot`;
	const response = await fetch(intentUrl(api, path), {
		method: "POST",
		headers: { ...intentHeaders(session), "content-type": "application/json" },
		body: JSON.stringify({ identities: [] }),
	});
	const value = await readJson(response, "Playback runtime");
	if (!response.ok)
		throw new Error(`Playback runtime snapshot returned HTTP ${response.status}`);
	const snapshot = decodePlaybackSnapshot(value);
	const deskId = programmingUuidAt(snapshot.desk.desk_id, "$.desk.desk_id");
	if (!sameId(deskId, session.desk.id))
		throw new Error(`Playback snapshot belongs to foreign desk ${deskId}`);
	return {
		deskId,
		showId: programmingUuidAt(snapshot.desk.scope.show_id, "$.desk.scope.show_id"),
		showRevision: snapshot.desk.scope.show_revision,
		activePage: snapshot.desk.active_page,
	};
}

async function resolveAuthority(
	api: ApiDriver,
	session: Session,
	fetch: typeof globalThis.fetch,
	active: ActiveShowAuthority,
	intent: DeleteCueIntent,
): Promise<ResolvedCueAuthority> {
	const snapshots = new HttpShowObjectSnapshotTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		fetch,
	});
	const [playbacks, cueLists, pageSnapshot] = await Promise.all([
		snapshots.collection(active.showId, "playback"),
		snapshots.collection(active.showId, "cue_list"),
		intent.address.type === "pool"
			? Promise.resolve(null)
			: snapshots.collection(active.showId, "playback_page"),
	]);
	for (const revision of [
		playbacks.showRevision,
		cueLists.showRevision,
		pageSnapshot?.showRevision,
	])
		if (revision != null && revision !== active.showRevision)
			throw new Error("Show authority changed while resolving Cue deletion");
	const resolved = resolveAddress(intent.address, active, pageSnapshot?.objects ?? []);
	const playback = unique(
		playbacks.objects,
		(item) => item.body.number === resolved.playbackNumber,
		`Playback ${resolved.playbackNumber}`,
	);
	if (playback.body.target.type !== "cue_list")
		throw new Error(`Playback ${resolved.playbackNumber} does not contain Cues`);
	const cueListId = programmingUuidAt(
		playback.body.target.cue_list_id,
		"$.playback.target.cue_list_id",
	);
	const cueList = unique(
		cueLists.objects,
		(item) => sameId(item.body.id, cueListId),
		`Cuelist ${cueListId}`,
	);
	programmingUuidAt(cueList.body.id, "$.cue_list.body.id");
	const cue = unique(
		cueList.body.cues,
		(item) => item.number === intent.cueNumber,
		`Cue ${intent.cueNumber}`,
	);
	const cueId = programmingUuidAt(cue.id, "$.cue_list.body.cues.id");
	return { ...resolved, cueList, cue: { id: cueId, number: cue.number } };
}

function resolveAddress(
	address: CueDeletionAddress,
	active: ActiveShowAuthority,
	pages: ShowObject<"playback_page">[],
): Pick<ResolvedCueAuthority, "address" | "playbackNumber"> {
	if (address.type === "pool")
		return {
			address: { type: "pool", playback_number: address.playbackNumber },
			playbackNumber: address.playbackNumber,
		};
	const pageNumber = address.type === "current_page" ? active.activePage : address.page;
	const page = unique(
		pages,
		(item) => item.body.number === pageNumber,
		`Playback Page ${pageNumber}`,
	);
	const playbackNumber = page.body.slots[String(address.slot)];
	if (playbackNumber == null)
		throw new Error(`Playback Page ${pageNumber} slot ${address.slot} is not assigned`);
	return {
		address:
			address.type === "current_page"
				? { type: "current_page", expected_page: pageNumber, slot: address.slot }
				: { type: "page_slot", page: pageNumber, slot: address.slot },
		playbackNumber,
	};
}

function actionRequest(
	requestId: string,
	intent: DeleteCueIntent,
	authority: ResolvedCueAuthority,
) {
	return {
		request_id: requestId,
		address: authority.address,
		cue_number: intent.cueNumber,
		authority: {
			playback_number: authority.playbackNumber,
			cue_list_id: authority.cueList.body.id,
			object_id: authority.cueList.id,
			object_revision: authority.cueList.revision,
			cue_id: authority.cue.id,
		},
	};
}

function decodeOutcome(
	value: unknown,
	requestId: string,
	active: ActiveShowAuthority,
	authority: ResolvedCueAuthority,
): CueDeletionOutcome {
	const keys = outcomeKeys(value);
	const tagged = exactRecordAt(value, "$", keys);
	requireFields(tagged, keys, "$");
	const status = enumAt(tagged.status, "$.status", ["changed", "no_change"]);
	const decodedRequestId = opaqueStringAt(tagged.request_id, "$.request_id", 128);
	if (decodedRequestId !== requestId)
		invalid("$.request_id", `request ${requestId}`, decodedRequestId);
	const showId = programmingUuidAt(tagged.show_id, "$.show_id");
	if (!sameId(showId, active.showId)) invalid("$.show_id", active.showId, showId);
	const showRevision = integerAt(tagged.show_revision, "$.show_revision");
	const expectedShowRevision = active.showRevision + (status === "changed" ? 1 : 0);
	if (showRevision !== expectedShowRevision)
		invalid("$.show_revision", String(expectedShowRevision), showRevision);
	const cueList = decodeProjection(tagged.cue_list, authority, status);
	const deletedCue = decodeDeletedCue(tagged.deleted_cue, authority);
	return {
		status,
		requestId,
		correlationId: programmingUuidAt(tagged.correlation_id, "$.correlation_id"),
		replayed: booleanAt(tagged.replayed, "$.replayed"),
		showId,
		showRevision,
		cueList,
		deletedCue,
		showEventSequence:
			status === "changed"
				? positiveIntegerAt(tagged.show_event_sequence, "$.show_event_sequence")
				: null,
		persistenceWarning: nullableString(
			tagged.persistence_warning,
			"$.persistence_warning",
		),
	} as CueDeletionOutcome;
}

function outcomeKeys(value: unknown) {
	const status = enumAt(
		exactRecordAt(value, "$", [
			"status", "request_id", "correlation_id", "replayed", "show_id",
			"show_revision", "cue_list", "deleted_cue", "show_event_sequence",
			"persistence_warning",
		]).status,
		"$.status",
		["changed", "no_change"],
	);
	return status === "changed"
		? ["status", "request_id", "correlation_id", "replayed", "show_id", "show_revision", "cue_list", "deleted_cue", "show_event_sequence", "persistence_warning"]
		: ["status", "request_id", "correlation_id", "replayed", "show_id", "show_revision", "cue_list", "deleted_cue", "persistence_warning"];
}

function decodeProjection(
	value: unknown,
	authority: ResolvedCueAuthority,
	status: CueDeletionOutcome["status"],
): CueDeletionProjection {
	const keys = [
		"cue_list_id", "object_id", "object_revision", "body",
	];
	const projection = exactRecordAt(value, "$.cue_list", keys);
	requireFields(projection, keys, "$.cue_list");
	const cueListId = programmingUuidAt(projection.cue_list_id, "$.cue_list.cue_list_id");
	const objectId = opaqueStringAt(projection.object_id, "$.cue_list.object_id", 256);
	if (!sameId(cueListId, authority.cueList.body.id))
		invalid("$.cue_list.cue_list_id", authority.cueList.body.id, cueListId);
	if (objectId !== authority.cueList.id)
		invalid("$.cue_list.object_id", authority.cueList.id, objectId);
	const objectRevision = integerAt(projection.object_revision, "$.cue_list.object_revision");
	const expectedRevision = authority.cueList.revision + (status === "changed" ? 1 : 0);
	if (objectRevision !== expectedRevision)
		invalid("$.cue_list.object_revision", String(expectedRevision), objectRevision);
	const body = decodeShowObjectBody("cue_list", projection.body, "$.cue_list.body", objectId);
	if (!sameId(body.id, cueListId)) invalid("$.cue_list.body.id", cueListId, body.id);
	const targetPresent = body.cues.some((cue, index) => {
		const cueId = programmingUuidAt(cue.id, `$.cue_list.body.cues[${index}].id`);
		return sameId(cueId, authority.cue.id) || cue.number === authority.cue.number;
	});
	if (targetPresent === (status === "changed"))
		invalid("$.cue_list.body.cues", status === "changed" ? "deleted Cue absent" : "unchanged Cue present", body.cues);
	return { cueListId, objectId, objectRevision, body };
}

function decodeDeletedCue(value: unknown, authority: ResolvedCueAuthority) {
	const keys = ["id", "number"];
	const cue = exactRecordAt(value, "$.deleted_cue", keys);
	requireFields(cue, keys, "$.deleted_cue");
	const id = programmingUuidAt(cue.id, "$.deleted_cue.id");
	const number = numberAt(cue.number, "$.deleted_cue.number");
	if (!sameId(id, authority.cue.id)) invalid("$.deleted_cue.id", authority.cue.id, id);
	if (number !== authority.cue.number)
		invalid("$.deleted_cue.number", String(authority.cue.number), number);
	return { id, number };
}

function decodeActionError(response: Response, value: unknown) {
	const keys = [
		"kind", "error", "current_revision", "current_related_revision", "retryable",
	];
	const error = exactRecordAt(value, "$", keys);
	requireFields(error, keys, "$");
	const kind = enumAt(error.kind, "$.kind", ERROR_KINDS);
	const currentRevision = nullableInteger(error.current_revision, "$.current_revision");
	const currentRelatedRevision = nullableInteger(
		error.current_related_revision,
		"$.current_related_revision",
	);
	const expectedStatus = statusForKind(kind);
	if (response.status !== expectedStatus)
		invalid("$.kind", `${kind} error for HTTP ${expectedStatus}`, response.status);
	verifyErrorEtag(response, currentRevision);
	return new CueDeletionActionError(
		stringAt(error.error, "$.error"),
		response.status,
		kind,
		currentRevision,
		currentRelatedRevision,
		booleanAt(error.retryable, "$.retryable"),
	);
}

function validateIntent(intent: DeleteCueIntent) {
	if (intent.surface !== "api")
		throw new Error("Cue deletion helper supports only the API surface");
	const cueNumber = numberAt(intent.cueNumber, "$.cueNumber");
	if (cueNumber <= 0) invalid("$.cueNumber", "positive finite number", cueNumber);
	if (intent.address.type === "pool")
		bounded(intent.address.playbackNumber, 1_000, "Playback");
	else {
		bounded(intent.address.slot, 127, "slot");
		if (intent.address.type === "page_slot") bounded(intent.address.page, 127, "Page");
	}
}

function actionUrl(api: ApiDriver, session: Session, active: ActiveShowAuthority) {
	return intentUrl(
		api,
		`/api/v2/desks/${encodeURIComponent(session.desk.id)}/shows/${encodeURIComponent(active.showId)}/cues/delete`,
	);
}

function actionHeaders(session: Session, revision: number) {
	return new Headers({
		...intentHeaders(session),
		"content-type": "application/json",
		"if-match": `"${revision}"`,
	});
}

function captureSession(session: Session): Session {
	return { ...session, user: { ...session.user }, desk: { ...session.desk } };
}

function assertCurrentSession(api: ApiDriver, expected: Session, phase: string) {
	const current = api.session;
	if (
		!current || current.session_id !== expected.session_id ||
		current.client_id !== expected.client_id || current.token !== expected.token ||
		!sameId(current.user.id, expected.user.id) || !sameId(current.desk.id, expected.desk.id)
	)
		throw new Error(`Cue deletion session changed ${phase}`);
}

function unique<T>(items: readonly T[], matches: (item: T) => boolean, label: string) {
	const found = items.filter(matches);
	if (found.length !== 1)
		throw new Error(`${label} resolved to ${found.length} stored objects`);
	return found[0];
}

function bounded(value: number, maximum: number, label: string) {
	if (!Number.isSafeInteger(value) || value < 1 || value > maximum)
		throw new Error(`${label} number must be between 1 and ${maximum}`);
}

function validRequestId(value: string) {
	return opaqueStringAt(value, "$.request_id", 128);
}

function sameId(left: string, right: string) {
	return left.toLowerCase() === right.toLowerCase();
}

async function readJson(response: Response, label: string): Promise<unknown> {
	const text = await response.text();
	try {
		return JSON.parse(text);
	} catch {
		throw new WireValidationError("$", `${label} JSON response`, text);
	}
}

function verifyEtag(response: Response, revision: number) {
	const actual = response.headers.get("etag");
	const expected = `"${revision}"`;
	if (actual !== expected) invalid("$.headers.etag", expected, actual);
}

function verifyErrorEtag(response: Response, revision: number | null) {
	if (revision == null) {
		if (response.headers.get("etag") != null)
			invalid("$.headers.etag", "no ETag", response.headers.get("etag"));
		return;
	}
	verifyEtag(response, revision);
}

function nullableInteger(value: unknown, path: string) {
	return value == null ? null : integerAt(value, path);
}

function nullableString(value: unknown, path: string) {
	return value == null ? null : stringAt(value, path);
}

function requireFields(
	value: Record<string, unknown>,
	keys: readonly string[],
	path: string,
) {
	const missing = keys.find((key) => !(key in value));
	if (missing) invalid(`${path}.${missing}`, "declared wire field", undefined);
}

function statusForKind(kind: CueDeletionErrorKind) {
	if (kind === "invalid") return 400;
	if (kind === "unauthorized") return 401;
	if (kind === "forbidden") return 403;
	if (kind === "not_found") return 404;
	if (kind === "conflict") return 409;
	if (kind === "unavailable") return 503;
	return 500;
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
