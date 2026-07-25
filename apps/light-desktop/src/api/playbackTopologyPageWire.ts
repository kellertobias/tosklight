import type {
	PlaybackTopologyAction,
	PlaybackTopologyResolution,
} from "../features/playbackTopology/contracts";
import { normalizePlaybackPageName } from "../features/playbackTopology/pageNames";
import {
	boundedPositiveIntegerAt,
	exactRecordAt,
	integerAt,
	stringAt,
} from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

type PageAction = Extract<
	PlaybackTopologyAction,
	{ type: "create_page" | "rename_page" }
>;

export function encodePlaybackPageAction(action: PageAction) {
	validatePageAuthority(action);
	const shared = {
		type: action.type,
		page: boundedPositiveIntegerAt(action.page, "$.action.page", 127),
		expected_page_revision: integerAt(
			action.expectedPageRevision,
			"$.action.expectedPageRevision",
		),
		expected_page_object_id: action.expectedPageObjectId,
	};
	if (action.type === "create_page") return shared;
	const name = normalizePlaybackPageName(action.name);
	if (name !== action.name)
		invalid(
			"$.action.name",
			"a trimmed Page name of 1-80 characters",
			action.name,
		);
	return { ...shared, name };
}

export function decodePlaybackPageResolution(
	resolution: Record<string, unknown>,
	action: PageAction,
): PlaybackTopologyResolution {
	exactRecordAt(resolution, "$.resolution", ["kind", "page"]);
	const page = boundedPositiveIntegerAt(
		resolution.page,
		"$.resolution.page",
		127,
	);
	if (page !== action.page)
		invalid("$.resolution", "the requested Playback Page", resolution);
	return { kind: "page", page };
}

function validatePageAuthority(action: PageAction) {
	const absent = action.expectedPageObjectId === null;
	if (absent !== (action.expectedPageRevision === 0))
		invalid(
			"$.action.expectedPageRevision",
			"an exact present or absent Page identity",
			action.expectedPageRevision,
		);
	if (action.type === "rename_page" && absent)
		invalid(
			"$.action.expectedPageObjectId",
			"an existing Playback Page identity",
			action.expectedPageObjectId,
		);
	if (!absent)
		stringAt(action.expectedPageObjectId, "$.action.expectedPageObjectId");
}

function invalid(path: string, expected: string, actual: unknown): never {
	throw new WireValidationError(path, expected, actual);
}
