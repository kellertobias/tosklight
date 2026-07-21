import { HttpPresetRecallTransport } from "../../src/api/PresetRecallTransport";
import { HttpProgrammerCaptureModeTransport } from "../../src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerValuesTransport } from "../../src/api/ProgrammerValuesTransport";
import { programmerValuesUuidAt } from "../../src/api/programmerValuesWireProjection";
import { decodeProgrammingInteractionSnapshot } from "../../src/api/programmingWire";
import { HttpShowObjectSnapshotTransport } from "../../src/api/ShowObjectSnapshotTransport";
import type {
	PresetRecallOutcome,
	PresetRecallRequest,
} from "../../src/features/presetRecall/contracts";
import {
	normalizePresetFamily,
	PRESET_FAMILIES,
	type PresetFamily,
	presetStorageKey,
} from "../../src/presetFamilies";
import type { ApiDriver } from "./api";
import {
	type IntentHttpDependencies,
	intentFetch,
	intentHeaders,
	intentRequestId,
	intentSession,
	intentUrl,
	responseJson,
} from "./v2IntentHttp";

export interface RecallPresetIntent {
	surface: "api";
	showId: string;
	preset: {
		objectId: string;
		family: PresetFamily;
		number: number;
	};
}

export async function recallPreset(
	api: ApiDriver,
	intent: RecallPresetIntent,
	dependencies: IntentHttpDependencies = {},
): Promise<PresetRecallOutcome> {
	validateIntent(intent);
	const session = intentSession(api);
	const fetch = intentFetch(dependencies);
	const scope = { showId: intent.showId, userId: session.user.id };
	const address = {
		family: intent.preset.family,
		number: intent.preset.number,
	};
	const [presetSnapshot, values, captureMode, interaction] = await Promise.all([
		showObjects(api, session.token, fetch).object(
			intent.showId,
			"preset",
			intent.preset.objectId,
		),
		programmerValues(api, session.token, fetch).loadSnapshot(scope),
		captureModes(api, session.token, fetch).loadSnapshot(scope),
		loadInteraction(api, fetch),
	]);
	const preset = presetSnapshot.object;
	if (preset === null)
		throw new Error(`Preset ${intent.preset.objectId} does not exist`);
	assertPresetAddress(preset.body, address);
	const request: PresetRecallRequest = {
		requestId: intentRequestId(dependencies),
		presetId: preset.id,
		address,
		expectedPresetRevision: preset.revision,
		expectedShowRevision: presetSnapshot.showRevision,
		expectedProgrammerRevision: values.projection.revision,
		expectedCaptureModeRevision: captureMode.projection.revision,
		expectedSelectionRevision: interaction.projection.selection.revision,
		selectedFixtureCount: interaction.projection.selection.selected.length,
	};
	return presetRecall(api, session.token, fetch).recall(
		{
			showId: intent.showId,
			userId: session.user.id,
			deskId: session.desk.id,
		},
		request,
	);
}

function showObjects(
	api: ApiDriver,
	sessionToken: string,
	fetch: typeof globalThis.fetch,
) {
	return new HttpShowObjectSnapshotTransport({
		baseUrl: api.baseUrl,
		sessionToken,
		fetch,
	});
}

function programmerValues(
	api: ApiDriver,
	sessionToken: string,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerValuesTransport({
		baseUrl: api.baseUrl,
		sessionToken,
		fetch,
	});
}

function captureModes(
	api: ApiDriver,
	sessionToken: string,
	fetch: typeof globalThis.fetch,
) {
	return new HttpProgrammerCaptureModeTransport({
		baseUrl: api.baseUrl,
		sessionToken,
		fetch,
	});
}

function presetRecall(
	api: ApiDriver,
	sessionToken: string,
	fetch: typeof globalThis.fetch,
) {
	return new HttpPresetRecallTransport({
		baseUrl: api.baseUrl,
		sessionToken,
		fetch,
	});
}

async function loadInteraction(api: ApiDriver, fetch: typeof globalThis.fetch) {
	const session = intentSession(api);
	const path = `/api/v2/desks/${encodeURIComponent(session.desk.id)}/programming-interaction/snapshot`;
	const response = await fetch(intentUrl(api, path), {
		headers: intentHeaders(session),
	});
	const value = await responseJson(response, "Programming interaction");
	if (!response.ok)
		throw new Error(
			`Programming interaction snapshot returned HTTP ${response.status}`,
		);
	return decodeProgrammingInteractionSnapshot(value, session.desk.id);
}

function validateIntent(intent: RecallPresetIntent) {
	if (intent.surface !== "api")
		throw new Error("Preset recall helper supports only the API surface");
	programmerValuesUuidAt(intent.showId, "$.showId");
	if (!PRESET_FAMILIES.includes(intent.preset.family))
		throw new Error(`Unsupported Preset family ${intent.preset.family}`);
	if (
		!Number.isSafeInteger(intent.preset.number) ||
		intent.preset.number < 1 ||
		intent.preset.number > 4_294_967_295
	)
		throw new Error("Preset number must be a positive 32-bit integer");
	const expectedId = presetStorageKey(intent.preset);
	if (intent.preset.objectId !== expectedId)
		throw new Error(
			`Preset object ${intent.preset.objectId} does not match address ${expectedId}`,
		);
}

function assertPresetAddress(
	body: { family?: PresetFamily | "All"; number: number },
	address: { family: PresetFamily; number: number },
) {
	if (body.number !== address.number)
		throw new Error(
			`Preset body number ${body.number} does not match ${address.number}`,
		);
	const family = normalizePresetFamily(body.family);
	if (family !== address.family)
		throw new Error(
			`Preset body family ${family} does not match ${address.family}`,
		);
}
