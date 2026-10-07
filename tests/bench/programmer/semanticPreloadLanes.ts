import type { Page } from "@playwright/test";
import { HttpProgrammerCaptureModeTransport } from "../../../apps/light-desktop/src/api/ProgrammerCaptureModeTransport";
import { HttpProgrammerPreloadValuesTransport } from "../../../apps/light-desktop/src/api/ProgrammerPreloadValuesTransport";
import type { ApiDriver } from "../core/api";
import { enterProgrammerPreload, goProgrammerPreload } from "./programmerPreloadLifecycle";

/**
 * Normal and Preload lane helpers for the semantic operator-control specs (POSITION-CONTROLS-006,
 * FOCUS-ZOOM-006 and FOCUS-ZOOM-008).
 *
 * A dialog gesture is written by the lane writer it started on: the Normal lane through
 * `programming_values` / `/api/v2/programmer/values/actions`, the Preload lane through
 * `programmer_preload_values` / `/api/v2/programmer/preload-values/actions`. The recorder below
 * classifies every write the desk sends, over the live-action socket or HTTP, by that lane.
 */

export type ProgrammerLane = "normal" | "preload";

export interface LaneWrite {
	lane: ProgrammerLane;
	/** `component_edits` for an edit, `finish_gesture` for the one Finish of a gesture. */
	kind: "edit" | "finish" | "other";
	attribute: string | null;
	body: string;
}

function classifyLane(text: string): ProgrammerLane | null {
	if (text.includes('"programmer_preload_values"') || text.includes("/programmer/preload-values/actions")) return "preload";
	if (text.includes('"programming_values"') || text.includes("/programmer/values/actions")) return "normal";
	return null;
}

function laneWrite(lane: ProgrammerLane, body: string): LaneWrite {
	const kind = body.includes("component_edits") ? "edit" : body.includes("finish_gesture") ? "finish" : "other";
	const attribute = /"attribute":"([a-z_]+)"/u.exec(body)?.[1] ?? null;
	return { lane, kind, attribute, body };
}

/** Records every Programmer values write the page sends, classified by lane. */
export function recordLaneWrites(page: Page) {
	const writes: LaneWrite[] = [];
	page.on("websocket", (socket) =>
		socket.on("framesent", ({ payload }) => {
			const text = String(payload);
			const lane = classifyLane(text);
			if (lane) writes.push(laneWrite(lane, text));
		}),
	);
	page.on("request", (request) => {
		if (request.method() !== "POST") return;
		const lane = classifyLane(request.url());
		if (lane) writes.push(laneWrite(lane, request.postData() ?? ""));
	});
	return {
		writes,
		edits: (lane: ProgrammerLane, attribute?: string) =>
			writes.filter((write) => write.lane === lane && write.kind === "edit" && (!attribute || write.attribute === attribute)),
		finishes: (lane: ProgrammerLane, attribute?: string) =>
			writes.filter((write) => write.lane === lane && write.kind === "finish" && (!attribute || write.attribute === attribute)),
	};
}

/** Arms Preload capturing Programmer changes, as the operator's first PRELOAD press does. */
export async function enterPreloadCapture(api: ApiDriver, showId: string) {
	await api.request("PUT", "/api/v2/configuration", { preload_programmer_changes: true });
	await enterProgrammerPreload(api, { surface: "api", showId });
	await waitForCapture(api, true);
}

/**
 * Leaves Preload capture (Blind off) without releasing the pending Preload values, so the
 * next write goes to the Normal programmer while the pending Preload stays intact.
 */
export async function leavePreloadCapture(api: ApiDriver, showId: string) {
	const deskId = api.session?.desk.id;
	await api.request("POST", "/api/v2/programmer-capture-mode/actions", { blind: false }, true, undefined, {
		showId,
		deskId,
	});
	await waitForCapture(api, false);
}

/** Preload GO: the pending Preload values become live. */
export async function goPreload(api: ApiDriver, showId: string) {
	await goProgrammerPreload(api, { surface: "api", showId });
}

/**
 * Authors `value` for each fixture into the pending Preload Programmer through the Preload
 * values route, as an earlier Preload edit would have. Requires active Preload capture.
 */
export async function seedPreloadValue(api: ApiDriver, showId: string, fixtureIds: readonly string[], attribute: string, value: unknown) {
	const session = api.session;
	if (!session) throw new Error("API session is not initialized");
	const scope = { showId, sessionId: session.session_id };
	const transport = new HttpProgrammerPreloadValuesTransport({
		baseUrl: api.baseUrl,
		sessionToken: session.token,
		authenticatedSessionId: session.session_id,
	});
	const capture = new HttpProgrammerCaptureModeTransport({ baseUrl: api.baseUrl, sessionToken: session.token });
	for (const fixtureId of fixtureIds) {
		const [values, captureMode] = await Promise.all([transport.loadSnapshot(scope), capture.loadSnapshot(scope)]);
		await transport.applyAction(scope, {
			requestId: crypto.randomUUID(),
			expectedPreloadRevision: values.projection.revision,
			expectedCaptureModeRevision: captureMode.projection.revision,
			action: {
				action: "set_fixture",
				fixtureId,
				attribute,
				value: value as never,
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
		});
	}
}

export async function capturesPreload(api: ApiDriver) {
	const snapshot = await api.request<{ projection: { blind: boolean; preload_capture_programmer: boolean } }>(
		"GET",
		"/api/v2/programmer/capture-mode/snapshot",
	);
	return snapshot.projection.blind && snapshot.projection.preload_capture_programmer;
}

async function waitForCapture(api: ApiDriver, preload: boolean) {
	for (let attempt = 0; attempt < 100; attempt += 1) {
		if ((await capturesPreload(api)) === preload) return;
		await new Promise((resolve) => setTimeout(resolve, 20));
	}
	throw new Error(`Programmer capture did not become ${preload ? "Preload" : "Normal"}`);
}

export interface LaneValue {
	fixture_id?: string;
	attribute: string;
	value: unknown;
	fade?: boolean;
	fade_millis?: number | null;
	[key: string]: unknown;
}

/** The pending Preload Programmer entries of `attribute`. */
export async function preloadValues(api: ApiDriver, attribute: string) {
	const snapshot = await api.request<{ projection: { fixture_values?: LaneValue[] } }>(
		"GET",
		"/api/v2/programmer/preload-values/snapshot",
	);
	return (snapshot.projection.fixture_values ?? []).filter((entry) => entry.attribute === attribute);
}

/** The Normal Programmer entries of `attribute`. */
export async function normalValues(api: ApiDriver, attribute: string) {
	const snapshot = await api.request<{ projection: { fixture_values?: LaneValue[] } }>(
		"GET",
		"/api/v2/programmer/values/snapshot",
	);
	return (snapshot.projection.fixture_values ?? []).filter((entry) => entry.attribute === attribute);
}

/** The first `count` logical DMX slots of universe 1: the live output of the rig. */
export async function liveSlots(api: ApiDriver, count: number) {
	const response = await fetch(`${api.baseUrl}/api/v2/output/dmx`);
	if (!response.ok) throw new Error(`GET /api/v2/output/dmx returned ${response.status}`);
	const snapshot = (await response.json()) as { universes: Array<{ universe: number; slots: number[] }> };
	return (snapshot.universes.find((universe) => universe.universe === 1)?.slots ?? []).slice(0, count);
}

/** Ends the open dialog drag the way switching to another application does. */
export async function blurDesk(page: Page) {
	await page.evaluate(() => window.dispatchEvent(new Event("blur")));
}

/** Ends the open dialog drag the way minimising or hiding the desk window does. */
export async function hideDesk(page: Page) {
	await page.evaluate(() => {
		Object.defineProperty(document, "hidden", { configurable: true, get: () => true });
		Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
		document.dispatchEvent(new Event("visibilitychange"));
	});
}

/** Returns to the desk after `hideDesk`. */
export async function showDesk(page: Page) {
	await page.evaluate(() => {
		Object.defineProperty(document, "hidden", { configurable: true, get: () => false });
		Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
		document.dispatchEvent(new Event("visibilitychange"));
		window.dispatchEvent(new Event("focus"));
	});
}
