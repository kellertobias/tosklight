import dgram from "node:dgram";
import { expect, type Locator, type Page } from "@playwright/test";
import type {
	PatchFixtureInput,
	PatchFixturesOutcome,
	PsnSnapshot,
	PsnUpdateOutcome,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import { readPatchSnapshot } from "../../support/operator/patch";
import type { ApiDriver } from "../core/api";
import { PsnSender, PsnStream, type PsnTrackerData, type PsnTrackerInfo } from "../protocols/psnSender";

/**
 * Bench helpers for docs/testing/19-tracking-with-posistagenet.md: the desk's PSN receiver
 * configured on a random free port, a PSN stream sent to it, and the Point/mover rig the
 * scenarios bind trackers to.
 */

export type PsnEdit = Partial<{
	enabled: boolean;
	group: string;
	port: number;
	interface: string | null;
	stale_after_millis: number;
	calibration: PsnSnapshot["configuration"]["calibration"];
	bindings: PsnSnapshot["configuration"]["bindings"];
	zones: PsnSnapshot["configuration"]["zones"];
}>;

export function readPsn(api: ApiDriver, showId?: string): Promise<PsnSnapshot> {
	return api.request<PsnSnapshot>("GET", "/api/v2/psn", undefined, true, undefined, showId ? { showId } : undefined);
}

export function updatePsn(api: ApiDriver, edit: PsnEdit, showId?: string): Promise<PsnUpdateOutcome> {
	return api.request<PsnUpdateOutcome>(
		"POST",
		"/api/v2/psn/update",
		{ request_id: crypto.randomUUID(), ...edit },
		true,
		undefined,
		showId ? { showId } : undefined,
	);
}

/** An edit the desk refuses: the error text, or null when it was accepted. */
export async function refusedPsnEdit(api: ApiDriver, edit: PsnEdit): Promise<string | null> {
	return updatePsn(api, edit).then(
		() => null,
		(error: unknown) => String(error),
	);
}

/** A UDP port nobody holds right now, for the desk's PSN receiver. */
export async function freePsnPort(): Promise<number> {
	const socket = dgram.createSocket("udp4");
	await new Promise<void>((resolve, reject) => {
		socket.once("error", reject);
		socket.bind(0, "0.0.0.0", resolve);
	});
	const port = (socket.address() as dgram.AddressInfo).port;
	await new Promise<void>((resolve) => socket.close(() => resolve()));
	return port;
}

/** Hold a UDP port exclusively, the way another program on the desk machine would. */
export async function occupyUdpPort(): Promise<{ port: number; release(): Promise<void> }> {
	const socket = dgram.createSocket({ type: "udp4", reuseAddr: false });
	await new Promise<void>((resolve, reject) => {
		socket.once("error", reject);
		socket.bind(0, "0.0.0.0", resolve);
	});
	return {
		port: (socket.address() as dgram.AddressInfo).port,
		release: () => new Promise<void>((resolve) => socket.close(() => resolve())),
	};
}

/**
 * A sender that keeps `trackers` on the wire at 60 Hz towards the desk's receiver. The bench sends
 * unicast to 127.0.0.1: the receiver binds the wildcard address on its port and also joins the
 * group, so unicast and group traffic arrive on the same socket.
 */
export async function openPsnStream(port: number, host = "127.0.0.1", systemName = "Bench PSN"): Promise<PsnStream> {
	const sender = await PsnSender.open({ host, port, systemName });
	return new PsnStream(sender);
}

export function trackerAt(id: number, position: readonly [number, number, number]): PsnTrackerData {
	return { id, position, validity: 1 };
}

export type { PsnTrackerData, PsnTrackerInfo };

/** A minimal Art-Net ArtDmx datagram: what a lighting node might put on the wrong port. */
export function artNetDmxPacket(universe = 0, slots = new Uint8Array(512)): Buffer {
	const header = Buffer.alloc(18);
	header.write("Art-Net\0", 0, "ascii");
	header.writeUInt16LE(0x5000, 8);
	header.writeUInt16BE(14, 10);
	header.writeUInt8(1, 12);
	header.writeUInt8(0, 13);
	header.writeUInt16LE(universe, 14);
	header.writeUInt16BE(slots.length, 16);
	return Buffer.concat([header, Buffer.from(slots)]);
}

type LibraryProfile = {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	modes: Array<{ id: string; name: string }>;
};

async function libraryMode(api: ApiDriver, manufacturer: string, name: string, mode: string) {
	const library = await api.request<{ profiles: LibraryProfile[] }>("GET", "/api/v2/fixture-library/profiles");
	const profile = library.profiles.find((candidate) => candidate.manufacturer === manufacturer && candidate.name === name);
	const chosen = profile?.modes.find((candidate) => candidate.name === mode);
	if (!profile || !chosen) throw new Error(`the library has no ${manufacturer} ${name} ${mode}`);
	return { profile, mode: chosen };
}

export interface TrackingRig {
	pointId: string;
	moverId: string;
	/** Logical DMX start address of the mover in universe 1. */
	moverAddress: number;
	/** Where the Point was patched, in metres. */
	pointOrigin: [number, number, number];
}

/**
 * Patch one 3D Point (unpatched, it has no DMX) at `pointOrigin` and one Cameo AURO SPOT Z300
 * (20-Channel) at U1.`moverAddress`, hanging 6 m up.
 */
export async function patchTrackingRig(
	api: ApiDriver,
	showId: string,
	{ pointOrigin = [0, 4, 0] as [number, number, number], moverAddress = 101 } = {},
): Promise<TrackingRig> {
	const pointId = crypto.randomUUID();
	const moverId = crypto.randomUUID();
	const point = await libraryMode(api, "ToskLight", "3D Point", "Full 16 bit");
	const mover = await libraryMode(api, "Cameo", "AURO SPOT Z300", "20-Channel");
	const base = {
		virtual_fixture_number: null,
		layer_id: "default",
		direct_control: null,
		rotation: { x: 0, y: 0, z: 0 },
		multipatch: [],
		move_in_black_enabled: false,
		move_in_black_delay_millis: 0,
		highlight_overrides: [],
	} satisfies Partial<PatchFixtureInput>;
	// Optional patch fields (bindings, masters, inversion …) take the desk's defaults.
	const fixtures: Partial<PatchFixtureInput>[] = [
		{
			...base,
			fixture_id: pointId,
			fixture_number: 901,
			name: "Presenter point",
			profile_id: point.profile.id,
			profile_revision: point.profile.revision,
			mode_id: point.mode.id,
			split_patches: [{ split: 1, universe: null, address: null }],
			location: { x: pointOrigin[0] * 1000, y: pointOrigin[1] * 1000, z: pointOrigin[2] * 1000 },
		},
		{
			...base,
			fixture_id: moverId,
			fixture_number: 101,
			name: "Follow spot",
			profile_id: mover.profile.id,
			profile_revision: mover.profile.revision,
			mode_id: mover.mode.id,
			split_patches: [{ split: 1, universe: 1, address: moverAddress }],
			location: { x: 0, y: 0, z: 6000 },
		},
	];
	const snapshot = await readPatchSnapshot(api, showId);
	await api.request<PatchFixturesOutcome>(
		"POST",
		"/api/v2/patch/fixtures",
		{ request_id: crypto.randomUUID(), fixtures, remove_fixture_ids: [] },
		true,
		snapshot.patch_revision,
		{ showId },
	);
	return { pointId, moverId, moverAddress, pointOrigin };
}

type PointPose = {
	fixture_id: string;
	offset_metres: [number, number, number];
	rotation_degrees: [number, number, number];
};

/** The output frame's DMX and Point poses, as `/api/v2/output/dmx` states them to the Stage. */
export async function outputFrame(api: ApiDriver) {
	return api.request<{
		universes: Array<{ universe: number; slots: number[] }>;
		points: PointPose[];
	}>("GET", "/api/v2/output/dmx");
}

/** Where the output frame puts a Point, in show metres (patched origin plus stated offset). */
export async function pointWorldPosition(api: ApiDriver, rig: TrackingRig): Promise<[number, number, number] | null> {
	const frame = await outputFrame(api);
	const pose = frame.points.find((candidate) => candidate.fixture_id === rig.pointId);
	if (!pose) return null;
	return [0, 1, 2].map((axis) => rig.pointOrigin[axis] + pose.offset_metres[axis]) as [number, number, number];
}

/** The mover's 20 DMX slots in the latest output frame. */
export async function moverSlots(api: ApiDriver, rig: TrackingRig): Promise<number[]> {
	const frame = await outputFrame(api);
	const universe = frame.universes.find((candidate) => candidate.universe === 1);
	return (universe?.slots ?? []).slice(rig.moverAddress - 1, rig.moverAddress - 1 + 20);
}

async function programmerRevisions(api: ApiDriver) {
	const values = await api.request<{ projection: { revision: number } }>("GET", "/api/v2/programmer/values/snapshot");
	const capture = await api.request<{ revision?: number; projection?: { revision: number } }>(
		"GET",
		"/api/v2/programmer/capture-mode/snapshot",
	);
	return {
		revision: values.projection.revision,
		captureModeRevision: capture.projection?.revision ?? capture.revision ?? 0,
	};
}

/** One Normal-programmer `apply_intent` of ordered component edits, without fade. */
export async function applyProgrammerEdits(
	api: ApiDriver,
	fixtureIds: string[],
	attribute: string,
	edits: unknown[],
) {
	const { revision, captureModeRevision } = await programmerRevisions(api);
	return api.request("POST", "/api/v2/programmer/values/actions", {
		request_id: crypto.randomUUID(),
		expected_revision: revision,
		expected_capture_mode_revision: captureModeRevision,
		action: {
			type: "apply_intent",
			fixture_ids: fixtureIds,
			group_id: null,
			attribute,
			operation: { type: "component_edits", edits },
			undo_group: null,
			timing: { fade: false, fade_millis: 0, delay_millis: 0 },
		},
	});
}

/** Aim fixtures at a 3D Point: Position Target with the Point as reference, no offset. */
export function aimAtPoint(api: ApiDriver, fixtureIds: string[], pointId: string) {
	return applyProgrammerEdits(api, fixtureIds, "position", [
		{ kind: "target", reference: { kind: "point", point_id: pointId } },
	]);
}

/** Set one fixture attribute in the Normal programmer, e.g. a 3D Point axis "encoder" move. */
export function setFixtureValue(api: ApiDriver, fixtureId: string, attribute: string, normalized: number) {
	return programmerAction(api, {
		type: "set_fixture",
		fixture_id: fixtureId,
		attribute,
		value: { kind: "normalized", value: normalized },
		timing: { fade: false, fade_millis: 0, delay_millis: 0 },
	});
}

/** Release one fixture attribute from the Normal programmer. */
export function releaseFixtureValue(api: ApiDriver, fixtureId: string, attribute: string) {
	return programmerAction(api, { type: "release_fixture", fixture_id: fixtureId, attribute });
}

async function programmerAction(api: ApiDriver, action: Record<string, unknown>) {
	const { revision, captureModeRevision } = await programmerRevisions(api);
	return api.request("POST", "/api/v2/programmer/values/actions", {
		request_id: crypto.randomUUID(),
		expected_revision: revision,
		expected_capture_mode_revision: captureModeRevision,
		action,
	});
}

/** The 3D Point axis offset (metres from its patched position) as the normalized stored value. */
export function pointAxisValue(offsetMetres: number): number {
	const reach = 100;
	return (offsetMetres + reach) / (2 * reach);
}

/** Whether this runtime publishes the semantic Position family (Target aim at a 3D Point). */
export async function semanticPositionPublished(api: ApiDriver, fixtureIds: string[]): Promise<boolean> {
	const pages = await api
		.request<{ semantic: boolean; families: Array<{ family: string }> }>(
			"GET",
			`/api/v2/programming/family-encoder-pages?fixture_ids=${fixtureIds.join(",")}`,
		)
		.catch(() => null);
	return Boolean(pages?.semantic && pages.families.some((group) => group.family === "position"));
}

/** Create a Macro from command-line source. */
export async function createMacro(api: ApiDriver, showId: string, number: number, name: string, source: string) {
	const id = crypto.randomUUID();
	await api.request(
		"POST",
		"/api/v2/macros/actions",
		{
			request_id: crypto.randomUUID(),
			action: { type: "create", definition: { id, number, name, source, presentation: { color: "#3f6f45" } } },
		},
		true,
		undefined,
		{ showId },
	);
	return id;
}

/** How many times each Macro has been started by a tracking zone. */
export async function trackingMacroRuns(api: ApiDriver, showId: string): Promise<Record<string, number>> {
	const runtime = await api.request<{
		active: Array<{ execution_id?: string; macro_id: string; trigger: { type: string } }>;
		recent: Array<{ execution_id?: string; macro_id: string; trigger: { type: string } }>;
	}>("GET", "/api/v2/macros/runtime", undefined, true, undefined, { showId, deskId: api.session?.desk.id });
	const seen = new Map<string, string>();
	for (const [index, execution] of [...runtime.active, ...runtime.recent].entries()) {
		if (execution.trigger.type !== "tracking") continue;
		seen.set(execution.execution_id ?? `${index}`, execution.macro_id);
	}
	const counts: Record<string, number> = {};
	for (const macroId of seen.values()) counts[macroId] = (counts[macroId] ?? 0) + 1;
	return counts;
}

/** Save the show as a portable file and open that file as a new show, as moving desks would. */
export async function reopenFromSavedFile(api: ApiDriver, showId: string): Promise<string> {
	const response = await fetch(`${api.baseUrl}/api/v2/shows/${showId}/download`, {
		headers: { authorization: `Bearer ${api.session?.token}` },
	});
	if (!response.ok) throw new Error(`show download returned ${response.status}`);
	const copy = await api.createShow<{ id: string }>({
		name: `PSN reopened ${crypto.randomUUID()}`,
		data_base64: Buffer.from(await response.arrayBuffer()).toString("base64"),
		overwrite: false,
	});
	await api.openShow(copy.id, { transition: "hold_current" });
	return copy.id;
}

/** The Show Patch window header holding the Fixtures / Media Servers / Tracking tabs and the ⚙. */
export function showPatchHeader(page: Page): Locator {
	return page.locator("header.ui-window-header").filter({ hasText: "Show Patch" }).first();
}

/** Open the desk, then Show Patch on its Tracking tab. Media discovery is answered empty. */
export async function openTrackingTab(page: Page, desk: { open(url: string): Promise<void> }, baseUrl: string) {
	await page.route("**/api/v2/media-servers/discover", (route) =>
		route.fulfill({ json: { discoveryError: null, servers: [] } }),
	);
	await desk.open(baseUrl);
	await page.getByRole("button", { name: /Open show menu/ }).click();
	await page.getByRole("button", { name: "Show Patch", exact: true }).click();
	const header = showPatchHeader(page);
	await header.getByRole("tab", { name: "Tracking", exact: true }).click();
	const tab = page.locator(".psn-setup");
	await expect(tab.getByRole("switch", { name: /Receive PosiStageNet/ })).toBeVisible();
	return tab;
}
