import type {
	PlaybackPreloadScope,
	PlaybackPreloadScopeSnapshot,
	PlaybackPreloadScopeUpdateOutcome,
} from "../../api/generated/light-wire";
import {
	arrayAt,
	booleanAt,
	boundedPositiveIntegerAt,
	exactRecordAt,
	integerAt,
	recordAt,
	stringAt,
} from "../../api/playbackWirePrimitives";
import { WireValidationError } from "../../api/wireValidation";
import { programmerValuesUuidAt } from "../../api/programmerValuesWireProjection";
export function validatePreloadScopeScope(scope: { showId: string }) {
	programmerValuesUuidAt(scope.showId, "$.scope.showId");
}
function decodeScope(value: unknown): PlaybackPreloadScope {
	const scope = exactRecordAt(value, "$.scope", ["position_ranges", "pages"]);
	const position_ranges = arrayAt(
		scope.position_ranges,
		"$.scope.position_ranges",
	).map((v, i) => {
		const p = `$.scope.position_ranges[${i}]`;
		const r = exactRecordAt(v, p, ["start", "end"]);
		const start = boundedPositiveIntegerAt(r.start, `${p}.start`, 1000),
			end = boundedPositiveIntegerAt(r.end, `${p}.end`, 1000);
		if (start > end) throw new WireValidationError(p, "ascending range", r);
		return { start, end };
	});
	const pages =
		scope.pages === null
			? null
			: arrayAt(scope.pages, "$.scope.pages").map((p, i) =>
					boundedPositiveIntegerAt(p, `$.scope.pages[${i}]`, 99),
				);
	return { position_ranges, pages };
}
function identity(value: unknown, showId: string, extra: string[]) {
	const r = exactRecordAt(value, "$", [
		"show_id",
		"revision",
		"scope",
		...extra,
	]);
	if (programmerValuesUuidAt(r.show_id, "$.show_id") !== showId)
		throw new WireValidationError("$.show_id", showId, r.show_id);
	return r;
}
export function decodePreloadScopeSnapshot(
	value: unknown,
	expected: { showId: string },
): PlaybackPreloadScopeSnapshot {
	validatePreloadScopeScope(expected);
	const r = identity(value, expected.showId, []);
	return {
		show_id: expected.showId,
		revision: integerAt(r.revision, "$.revision"),
		scope: decodeScope(r.scope),
	};
}
export function decodePreloadScopeSaveOutcome(
	value: unknown,
	expected: { showId: string },
	requestId: string,
): PlaybackPreloadScopeUpdateOutcome {
	const r = identity(value, expected.showId, [
		"request_id",
		"replayed",
		"changed",
	]);
	if (r.request_id !== requestId)
		throw new WireValidationError("$.request_id", requestId, r.request_id);
	return {
		request_id: requestId,
		show_id: expected.showId,
		revision: integerAt(r.revision, "$.revision"),
		scope: decodeScope(r.scope),
		replayed: booleanAt(r.replayed, "$.replayed"),
		changed: booleanAt(r.changed, "$.changed"),
	};
}
export function encodePreloadScopeSaveRequest(
	requestId: string,
	expectedRevision: number,
	scope: PlaybackPreloadScope,
) {
	return {
		request_id: stringAt(requestId, "$.request_id"),
		expected_revision: integerAt(expectedRevision, "$.expected_revision"),
		scope: decodeScope(scope),
	};
}
export function decodePreloadScopeEvent(
	value: unknown,
): { showId: string; revision: number } | "gap" | "ready" | "error" {
	const m = recordAt(value, "$");
	if (m.type === "ready" || m.type === "repaired") return "ready";
	if (m.type === "gap" || m.type === "error") return m.type;
	const e = recordAt(m.event, "$.event"),
		p = recordAt(e.payload, "$.event.payload");
	if (p.type !== "playback_preload_scope_changed")
		throw new WireValidationError(
			"$.event.payload.type",
			"playback_preload_scope_changed",
			p.type,
		);
	const c = exactRecordAt(p.change, "$.event.payload.change", [
		"show_id",
		"revision",
	]);
	return {
		showId: programmerValuesUuidAt(c.show_id, "$.change.show_id"),
		revision: integerAt(c.revision, "$.change.revision"),
	};
}
